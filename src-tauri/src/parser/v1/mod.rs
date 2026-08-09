use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::Utc;
use protocol::{
    ActionType, AreaEnterEvent, DamageEvent, Message, OnAttemptSBAEvent, OnContinueSBAChainEvent,
    OnDeathEvent, OnPerformSBAEvent, OnUpdateSBAEvent, PlayerIdentityEvent, QuestAbandonEvent,
    QuestCompleteEvent, SbaCause,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Window};

use super::{
    constants::{self, CharacterType, EnemyType},
    v0,
};
use crate::style_catalog;

mod aura;
mod counter_grant;
mod dto;
mod encounter;
mod player_state;
mod retro;
pub mod sba_inference;
mod sba_state;
mod skill_state;
mod stun;
#[cfg(test)]
mod tests;

use dto::Sigil;
pub use dto::{EffectiveTrait, MasterTraitFlag, PlayerData};
use encounter::ParserStatus;
pub use encounter::{DerivedEncounterState, Encounter};
use player_state::{child_character_type_of, PlayerState};
pub use stun::StunReconstructor;

const SAVE_DEBOUNCE_MILLIS: i64 = 1500;

/// One display frame between live `encounter-update` emits.
pub const LIVE_EMIT_INTERVAL_MILLIS: i64 = 16;

#[derive(Debug, Default)]
struct LiveEmitGate {
    last_emit: i64,
    dirty: bool,
}

impl LiveEmitGate {
    fn should_emit(&mut self, now: i64) -> bool {
        if now - self.last_emit >= LIVE_EMIT_INTERVAL_MILLIS {
            self.emitted(now);
            true
        } else {
            self.dirty = true;
            false
        }
    }

    fn emitted(&mut self, now: i64) {
        self.last_emit = now;
        self.dirty = false;
    }

    fn take_pending(&mut self, now: i64) -> bool {
        if self.dirty {
            self.emitted(now);
            true
        } else {
            false
        }
    }
}

/// An SBA attempt costs 200 of the 1000 max, landed or not.
const SBA_ATTEMPT_LEVEL: f64 = 800.0;

fn is_damage_taken(event: &DamageEvent) -> bool {
    event.target.parent_index >= protocol::PLAYER_ID_BASE
}

pub fn attributed_source_index(player_data: &[Option<PlayerData>; 4], event: &DamageEvent) -> u32 {
    let named = event.source.parent_index;
    if named >= protocol::PLAYER_ID_BASE {
        return named;
    }

    let character_type = canonical_character_type(event.source.parent_actor_type);
    if matches!(character_type, CharacterType::Unknown(_)) {
        return named;
    }

    let mut members = player_data
        .iter()
        .flatten()
        .filter(|player| player.character_type == character_type);

    match (members.next(), members.next()) {
        (Some(only), None) => only.actor_index,
        _ => named,
    }
}

fn is_helper_target(event: &DamageEvent) -> bool {
    constants::is_helper_actor(event.target.actor_type)
        || constants::is_helper_actor(event.target.parent_actor_type)
}

pub(super) fn capped_damage(damage: i32, cap: Option<u64>) -> u64 {
    let d = damage.max(0) as u64;
    match cap {
        Some(c) if c > 0 => d.min(c),
        _ => d,
    }
}

const REVIVE_HEAL_TYPE: u32 = 4;

pub struct MiscCharts {
    pub heal_provided: HashMap<u32, Vec<u64>>,
    pub heal_received: HashMap<u32, Vec<u64>>,
    pub damage_taken: HashMap<u32, Vec<u64>>,
    /// Death offsets (ms from encounter start) for the skull markers.
    pub deaths: HashMap<u32, Vec<i64>>,
}

/// Id's dragon form (Pl2000) is the same player as his human form (Pl1900).
fn canonical_character_type(hash: u32) -> CharacterType {
    match CharacterType::from_hash(hash) {
        CharacterType::Pl2000 => CharacterType::Pl1900,
        character_type => character_type,
    }
}

static CHARACTER_NAMES: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();

fn character_name_for(character_type: CharacterType) -> Option<&'static str> {
    if matches!(character_type, CharacterType::Unknown(_)) {
        return None;
    }

    CHARACTER_NAMES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../../lang/en/characters.json"))
                .unwrap_or_default()
        })
        .get(&character_type.to_string())
        .map(String::as_str)
}

pub struct AdjustedDamageInstance<'a> {
    pub event: &'a DamageEvent,
    pub player_data: Option<&'a PlayerData>,
    pub stun_damage: f64,
    pub source_index: u32,
}

impl<'a> AdjustedDamageInstance<'a> {
    pub fn from_damage_event(event: &'a DamageEvent, player_data: Option<&'a PlayerData>) -> Self {
        let stun_damage = event.stun_value.unwrap_or(0.0) as f64;

        Self {
            event,
            player_data,
            stun_damage,
            source_index: event.source.parent_index,
        }
    }

    pub fn with_reconstructed_stun(
        event: &'a DamageEvent,
        player_data: Option<&'a PlayerData>,
        stun_damage: f64,
    ) -> Self {
        Self {
            event,
            player_data,
            stun_damage,
            source_index: event.source.parent_index,
        }
    }

    /// Files this hit under `source_index` instead of the actor the event names.
    ///
    /// Ensures that the correct player is attributed.
    pub fn adopted_by(mut self, source_index: u32) -> Self {
        self.source_index = source_index;
        self
    }
}

/// The parser for the encounter.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Parser {
    /// Encounter that will be saved into the database, contains all the state needed to reparse
    pub encounter: Encounter,
    /// Derived state of the encounter, used for parsing the encounter into a calculated format that can be consumed by the front-end
    pub derived_state: DerivedEncounterState,
    /// Status of the parser
    status: ParserStatus,

    /// The window handle for the parser, used to send messages to the front-end
    #[serde(skip)]
    app: Option<AppHandle>,

    /// The window handle for the parser, used to send messages to the front-end
    #[serde(skip)]
    window_handle: Option<Window>,

    /// The database connection for the parser, used to save the encounter
    #[serde(skip)]
    db: Option<Connection>,

    #[serde(skip)]
    game_version: Option<String>,

    #[serde(skip)]
    last_save_time: Option<i64>,

    #[serde(skip)]
    stun_recon: StunReconstructor,

    #[serde(skip)]
    action_actors: ActionActors,

    #[serde(skip)]
    live_emit: LiveEmitGate,
}

/// The gauge message carries no actor, so a gain is named by the last damage
/// event with the same action id from that player.
#[derive(Debug, Default)]
struct ActionActors(HashMap<(u32, ActionType), CharacterType>);

impl ActionActors {
    fn remember(&mut self, event: &DamageEvent) {
        self.0.insert(
            (event.source.parent_index, event.action_id),
            child_character_type_of(event),
        );
    }

    fn resolve(
        &self,
        actor_index: u32,
        cause: SbaCause,
        character_type: CharacterType,
    ) -> Option<CharacterType> {
        let action = match cause {
            SbaCause::Action(action) | SbaCause::Inferred(action) => action,
            _ => return None,
        };

        let child = *self.0.get(&(actor_index, action))?;

        (child != character_type).then_some(child)
    }
}

impl Parser {
    pub fn new(
        app: AppHandle,
        window: Window,
        db: Connection,
        game_version: Option<String>,
    ) -> Self {
        Self {
            app: Some(app),
            db: Some(db),
            window_handle: Some(window),
            game_version,
            ..Default::default()
        }
    }

    /// Peeks at the first damage event in the log to get the start time of the encounter.
    pub fn start_time(&self) -> i64 {
        if let Some((timestamp, _)) = self.encounter.raw_event_log.first() {
            *timestamp
        } else {
            1
        }
    }

    fn send_live_payload(&self) {
        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-update", &self.derived_state.live_payload());
        }
    }

    fn emit_live_update(&mut self) {
        self.send_live_payload();
        self.live_emit.emitted(Utc::now().timestamp_millis());
    }

    fn emit_live_update_throttled(&mut self, now: i64) {
        if self.live_emit.should_emit(now) {
            self.send_live_payload();
        }
    }

    pub fn flush_live_update(&mut self) {
        if self.live_emit.take_pending(Utc::now().timestamp_millis()) {
            self.send_live_payload();
        }
    }

    /// Reparses derived state from a given encounter.
    pub fn from_encounter(encounter: Encounter) -> Self {
        let mut parser = Self {
            encounter,
            ..Default::default()
        };

        parser.reparse();
        parser
    }

    pub fn from_encounter_blob(blob: &[u8]) -> Result<Self> {
        let mut encounter = Encounter::from_blob(blob)?;

        // Repopulate the event log if it's empty.
        encounter.repopulate_event_log();

        Ok(Self::from_encounter(encounter))
    }

    /// Reparses derived state from the current encounter.
    pub fn reparse(&mut self) {
        self.derived_state = Default::default();
        self.derived_state.start(self.start_time());

        let mut stun_recon = StunReconstructor::default();

        for (timestamp, event) in self.encounter.event_log() {
            match event {
                Message::DamageEvent(event) if is_damage_taken(event) => {
                    let cap = self.max_hp_for(event.target.parent_index);
                    self.derived_state.process_damage_taken(event, cap);
                }
                Message::DamageEvent(event) => {
                    let source_index =
                        attributed_source_index(&self.encounter.player_data, event);
                    let player_data = self
                        .encounter
                        .player_data
                        .iter()
                        .flatten()
                        .find(|player| player.actor_index == source_index);

                    let stun_damage = stun_recon.counted_stun(event);
                    let damage_instance = AdjustedDamageInstance::with_reconstructed_stun(
                        event,
                        player_data,
                        stun_damage,
                    )
                    .adopted_by(source_index);

                    self.derived_state
                        .process_damage_event(*timestamp, &damage_instance);
                }
                // Status events are not folded here. `pair_status_intervals` in
                // `commands.rs` derives them from this same log at read time.
                Message::OnHeal(event) => {
                    self.derived_state.process_heal_event(event);
                }
                _ => {}
            }
        }

        self.reparse_sba_gains();
    }

    fn reparse_sba_gains(&mut self) {
        let inferred = sba_inference::infer_remote_causes(&self.encounter);
        let mut actors = ActionActors::default();

        for (index, (_, event)) in self.encounter.event_log().enumerate() {
            match event {
                Message::DamageEvent(event) if !is_damage_taken(event) => {
                    actors.remember(event);
                }
                Message::OnUpdateSBA(sba) => {
                    let character_type = self.character_type_of(sba.actor_index);
                    let cause = inferred.get(&index).copied().unwrap_or(sba.cause);
                    let child = actors.resolve(sba.actor_index, cause, character_type);
                    let player = self
                        .derived_state
                        .party_row(sba.actor_index, character_type);
                    player.set_sba(sba.sba_value as f64);
                    player.add_sba_gain(cause, child, sba.sba_added as f64);
                }
                Message::OnPerformSBA(sba) => {
                    if let Some(player) = self.derived_state.party.get_mut(&sba.actor_index) {
                        player.set_sba(0.0);
                    }
                }
                Message::OnAttemptSBA(sba) => {
                    if let Some(player) = self.derived_state.party.get_mut(&sba.actor_index) {
                        player.set_sba(SBA_ATTEMPT_LEVEL);
                    }
                }
                _ => {}
            }
        }
    }

    fn character_type_of(&self, actor_index: u32) -> CharacterType {
        self.encounter
            .player_data
            .iter()
            .flatten()
            .find(|player| player.actor_index == actor_index)
            .map(|player| player.character_type)
            .unwrap_or(CharacterType::Unknown(0))
    }

    /// Gauge is left out: it belongs to the player, not to who they hit, so
    /// there is no per-enemy slice of it to take.
    pub fn derived_state_for_enemy(&self, enemy_index: u32) -> DerivedEncounterState {
        let start_time = self.start_time();
        let mut state = DerivedEncounterState::default();
        state.start(start_time);

        let mut stun_recon = StunReconstructor::default();

        for (timestamp, event) in self.encounter.event_log() {
            let Message::DamageEvent(event) = event else {
                continue;
            };
            if event.target.parent_index != enemy_index {
                continue;
            }

            let source_index = attributed_source_index(&self.encounter.player_data, event);
            let player_data = self
                .encounter
                .player_data
                .iter()
                .flatten()
                .find(|player| player.actor_index == source_index);
            let stun_damage = stun_recon.counted_stun(event);
            let damage_instance =
                AdjustedDamageInstance::with_reconstructed_stun(event, player_data, stun_damage)
                    .adopted_by(source_index);
            state.process_damage_event(*timestamp, &damage_instance);
        }

        state
    }

    pub fn generate_sba_chart(&self, interval: i64) -> HashMap<u32, Vec<f32>> {
        let start_time = self.start_time();
        let duration = self.derived_state.duration();

        let mut chart_values: HashMap<u32, Vec<f32>> = HashMap::new();

        for player in self.derived_state.party.values() {
            chart_values.insert(player.index, vec![0.0; (duration / interval) as usize + 1]);
        }

        let mut last_event_timestamp = start_time;

        // SBA syncs keep arriving after the last hit; clamp them into the last slice.
        let last_slot = (duration / interval) as usize;

        for (timestamp, event) in self.encounter.event_log() {
            let last_index = (((last_event_timestamp - start_time) / interval) as usize).min(last_slot);
            let index = (((timestamp - start_time) / interval) as usize).min(last_slot);

            // Carry over the previous values to the current timeslice.
            if last_index != index && last_index > 0 {
                for (_, entries) in chart_values.iter_mut() {
                    let previous_value = entries[last_index];

                    for i in last_index..=index {
                        if i > 0 && i < entries.len() {
                            entries[i] = previous_value;
                        }
                    }
                }
            }

            if let Some((actor_index, sba_value)) = match event {
                Message::OnUpdateSBA(sba_update_event) => {
                    Some((sba_update_event.actor_index, sba_update_event.sba_value))
                }
                Message::OnAttemptSBA(sba_attempt_event) => {
                    Some((sba_attempt_event.actor_index, 800.0))
                }
                Message::OnPerformSBA(sba_perform_event) => {
                    Some((sba_perform_event.actor_index, 0.0))
                }
                Message::OnContinueSBAChain(sba_continue_event) => {
                    Some((sba_continue_event.actor_index, 0.0))
                }
                _ => None,
            } {
                if let Some(entries) = chart_values.get_mut(&actor_index) {
                    entries[index] = sba_value;
                }
            }

            last_event_timestamp = *timestamp;
        }

        chart_values
    }

    fn max_hp_for(&self, actor_index: u32) -> Option<u64> {
        self.encounter
            .player_data
            .iter()
            .flatten()
            .find(|p| p.actor_index == actor_index)
            .and_then(|p| p.player_stats.as_ref())
            .map(|s| s.total_hp as u64)
            .filter(|&hp| hp > 0)
    }

    pub fn generate_misc_charts(&self, interval: i64) -> MiscCharts {
        let start_time = self.start_time();
        let duration = self.derived_state.duration();
        let len = (duration / interval) as usize + 1;

        let players: Vec<u32> = self.derived_state.party.keys().copied().collect();
        let seed = || -> HashMap<u32, Vec<u64>> {
            players.iter().map(|&p| (p, vec![0u64; len])).collect()
        };
        let mut provided = seed();
        let mut received = seed();
        let mut taken = seed();
        let mut deaths: HashMap<u32, Vec<i64>> = HashMap::new();

        let mut run_provided: HashMap<u32, u64> = players.iter().map(|&p| (p, 0)).collect();
        let mut run_received: HashMap<u32, u64> = players.iter().map(|&p| (p, 0)).collect();
        let mut run_taken: HashMap<u32, u64> = players.iter().map(|&p| (p, 0)).collect();

        let bucket = |ts: i64| -> usize { (((ts - start_time).max(0) / interval) as usize).min(len - 1) };

        fn carry(series: &mut HashMap<u32, Vec<u64>>, run: &HashMap<u32, u64>, from: usize, to: usize) {
            for (p, entries) in series.iter_mut() {
                let v = run[p];
                for entry in entries.iter_mut().take(to + 1).skip(from + 1) {
                    *entry = v;
                }
            }
        }

        // A hit on a downed body re-emits a death, so drop deaths while a player is down.
        let mut down: HashSet<u32> = HashSet::new();

        let mut last_idx = 0usize;
        for (timestamp, event) in self.encounter.event_log() {
            let idx = bucket(*timestamp);
            if idx > last_idx {
                carry(&mut provided, &run_provided, last_idx, idx);
                carry(&mut received, &run_received, last_idx, idx);
                carry(&mut taken, &run_taken, last_idx, idx);
                last_idx = idx;
            }

            match event {
                Message::DamageEvent(e) => {
                    down.remove(&e.source.parent_index);
                }
                Message::OnHeal(e) if e.heal_type == REVIVE_HEAL_TYPE => {
                    down.remove(&e.target);
                }
                _ => {}
            }

            match event {
                Message::OnHeal(e) if e.amount > 0 => {
                    if let Some(src) = e.source {
                        if let Some(r) = run_provided.get_mut(&src) {
                            *r += e.amount as u64;
                            if let Some(a) = provided.get_mut(&src) {
                                a[idx] = *r;
                            }
                        }
                    }
                    if let Some(r) = run_received.get_mut(&e.target) {
                        *r += e.amount as u64;
                        if let Some(a) = received.get_mut(&e.target) {
                            a[idx] = *r;
                        }
                    }
                }
                Message::DamageEvent(e) if is_damage_taken(e) && e.damage > 0 => {
                    let t = e.target.parent_index;
                    let hit = capped_damage(e.damage, self.max_hp_for(t));
                    if let Some(r) = run_taken.get_mut(&t) {
                        *r += hit;
                        if let Some(a) = taken.get_mut(&t) {
                            a[idx] = a[idx].max(*r);
                        }
                    }
                }
                Message::OnDeathEvent(e) => {
                    if !down.insert(e.actor_index) {
                        continue;
                    }

                    deaths.entry(e.actor_index).or_default().push(*timestamp - start_time);
                    if let Some(r) = run_taken.get_mut(&e.actor_index) {
                        *r = 0;
                    }
                }
                _ => {}
            }
        }

        carry(&mut provided, &run_provided, last_idx, len - 1);
        carry(&mut received, &run_received, last_idx, len - 1);
        carry(&mut taken, &run_taken, last_idx, len - 1);

        MiscCharts {
            heal_provided: provided,
            heal_received: received,
            damage_taken: taken,
            deaths,
        }
    }

    /// Handles the event when an area is entered.
    /// If the current encounter was in progress, then stop it as we've left the instance.
    /// If there was damage in that stopped instance, then save it as a new log.
    /// Otherwise, we're waiting for the encounter to start.
    pub fn on_area_enter_event(&mut self, event: AreaEnterEvent) {
        // 0 = no quest known; keep what the encounter already has.
        if event.last_known_quest_id != 0 {
            self.encounter.quest_id = Some(event.last_known_quest_id);
        }

        if self.status == ParserStatus::InProgress {
            self.update_status(ParserStatus::Stopped);

            if self.has_damage() {
                match self.save_encounter_to_db() {
                    Ok(id) => {
                        if let Some(app) = &self.app {
                            let _ = app.emit_all("encounter-saved", id);
                        }
                    }
                    Err(e) => {
                        if let Some(app) = &self.app {
                            let _ = app.emit_all("encounter-saved-error", e.to_string());
                        }
                    }
                }
            }
        } else {
            self.update_status(ParserStatus::Waiting);
        }

        self.encounter.quest_completed = false;
        self.encounter.quest_abandoned = false;
        self.encounter.reset_player_data();

        if let Some(window) = &self.window_handle {
            let _ = window.emit("on-area-enter", &self.derived_state.live_payload());
        }
    }

    pub fn on_quest_complete_event(&mut self, event: QuestCompleteEvent) {
        self.encounter.quest_id = Some(event.quest_id);
        // 0 = no authoritative timer; in Conflux the game's timer counts only the current area.
        if event.elapsed_time_in_secs != 0 {
            self.encounter.quest_timer = Some(event.elapsed_time_in_secs);
        }
        self.encounter.quest_completed = true;
        self.encounter.quest_abandoned = false;

        if self.status == ParserStatus::InProgress {
            self.update_status(ParserStatus::Stopped);

            if self.has_damage() {
                match self.save_encounter_to_db() {
                    Ok(id) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved", id);
                        }
                    }
                    Err(e) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved-error", e.to_string());
                        }
                    }
                }
            }

            self.emit_live_update();
        }

        self.encounter.reset_player_data();
    }

    pub fn on_quest_abandon_event(&mut self, event: QuestAbandonEvent) {
        if event.quest_id != 0 {
            self.encounter.quest_id = Some(event.quest_id);
        }
        if event.elapsed_time_in_secs != 0 {
            self.encounter.quest_timer = Some(event.elapsed_time_in_secs);
        }
        self.encounter.quest_completed = false;
        self.encounter.quest_abandoned = true;

        if self.status == ParserStatus::InProgress {
            self.update_status(ParserStatus::Stopped);

            if self.has_damage() {
                match self.save_encounter_to_db() {
                    Ok(id) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved", id);
                        }
                    }
                    Err(e) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved-error", e.to_string());
                        }
                    }
                }
            }

            self.emit_live_update();
        }

        self.encounter.reset_player_data();
    }

    // Called when a damage event is received from the game.
    pub fn on_damage_event(&mut self, event: DamageEvent) {
        let now = Utc::now().timestamp_millis();

        if is_damage_taken(&event) {
            if self.status == ParserStatus::InProgress {
                self.encounter
                    .push_event(now, Message::DamageEvent(event.clone()));
                let cap = self.max_hp_for(event.target.parent_index);
                self.derived_state.process_damage_taken(&event, cap);
            }
            return;
        }

        if Self::should_ignore_damage_event(&event) {
            return;
        }

        // A stray hit right after the kill blow must not start a phantom encounter.
        if matches!(self.status, ParserStatus::Stopped | ParserStatus::Waiting) {
            if let Some(last_save) = self.last_save_time {
                if now - last_save < SAVE_DEBOUNCE_MILLIS {
                    return;
                }
            }
        }

        // If this is the first damage event, set the start time.
        if self.status == ParserStatus::Stopped || self.status == ParserStatus::Waiting {
            self.reset();
            self.encounter.quest_completed = false;
            self.encounter.quest_abandoned = false;
            self.derived_state.start(now);
            self.update_status(ParserStatus::InProgress);
        }

        self.encounter
            .push_event(now, Message::DamageEvent(event.clone()));

        self.action_actors.remember(&event);

        let source_index = attributed_source_index(&self.encounter.player_data, &event);
        let player_data = self
            .encounter
            .player_data
            .iter()
            .flatten()
            .find(|player| player.actor_index == source_index);

        let stun_damage = self.stun_recon.counted_stun(&event);
        let damage_instance =
            AdjustedDamageInstance::with_reconstructed_stun(&event, player_data, stun_damage)
                .adopted_by(source_index);

        self.derived_state
            .process_damage_event(now, &damage_instance);

        self.emit_live_update_throttled(now);
    }

    pub fn on_player_identity_event(&mut self, event: PlayerIdentityEvent) {
        let character_type = canonical_character_type(event.character_type);

        if event.party_index > 3 {
            return;
        }

        let sigils = event
            .sigils
            .into_iter()
            .map(|sigil| Sigil {
                first_trait_id: sigil.first_trait_id,
                first_trait_level: sigil.first_trait_level,
                second_trait_id: sigil.second_trait_id,
                second_trait_level: sigil.second_trait_level,
                sigil_id: sigil.sigil_id,
                equipped_character: sigil.equipped_character,
                sigil_level: sigil.sigil_level,
                acquisition_count: sigil.acquisition_count,
                notification_enum: sigil.notification_enum,
            })
            .collect();

        let player_data = PlayerData {
            actor_index: event.actor_index,
            display_name: event.display_name.to_string_lossy().to_string(),
            character_name: character_name_for(character_type)
                .map(str::to_string)
                .unwrap_or_else(|| event.character_name.to_string_lossy().to_string()),
            is_online: event.is_online,
            character_type,
            sigils,
            weapon_info: event.weapon_info.map(Into::into),
            overmastery_info: None,
            summon_info: event.summon_info.map(Into::into),
            skill_loadout: event.skill_loadout,
            over_mastery: event.over_mastery.into_iter().map(Into::into).collect(),
            master_trait_flags: event
                .master_trait_flags
                .into_iter()
                .map(Into::into)
                .collect(),
            effective_traits: event.effective_traits.into_iter().map(Into::into).collect(),
            player_stats: event.player_stats.map(Into::into),
            network_user_id: event.network_user_id,
            network_user_name: event.network_user_name,
            master_level: event.master_level,
        };

        self.encounter.player_data[event.party_index as usize] = Some(player_data);

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-party-update", &self.encounter.player_data);
        }
    }

    pub fn on_party_roster_event(&mut self, event: protocol::PartyRosterEvent) {
        let in_progress = self.status == ParserStatus::InProgress && self.has_damage();

        if in_progress {
            for member in event.members {
                let slot = member.party_index as usize;
                if slot > 3 {
                    continue;
                }
                if let Some(existing) = self.encounter.player_data[slot].as_mut() {
                    if existing.network_user_id.is_none() {
                        existing.network_user_id = member.network_user_id;
                    }
                    if existing.network_user_name.is_none() {
                        existing.network_user_name = member.network_user_name;
                    }
                }
            }
        } else {
            self.reset();
            self.encounter.reset_player_data();
            self.update_status(ParserStatus::Waiting);

            for member in &event.members {
                let slot = member.party_index as usize;
                if slot > 3 {
                    continue;
                }
                let character_type = canonical_character_type(member.character_type);
                self.encounter.player_data[slot] = Some(PlayerData {
                    actor_index: protocol::PLAYER_ID_BASE | u32::from(member.party_index),
                    display_name: member.display_name.to_string_lossy().to_string(),
                    character_name: character_name_for(character_type)
                        .map(str::to_string)
                        .unwrap_or_default(),
                    character_type,
                    is_online: member.is_online,
                    sigils: Vec::new(),
                    weapon_info: None,
                    overmastery_info: None,
                    summon_info: None,
                    skill_loadout: Vec::new(),
                    over_mastery: Vec::new(),
                    master_trait_flags: Vec::new(),
                    effective_traits: Vec::new(),
                    player_stats: None,
                    network_user_id: member.network_user_id.clone(),
                    network_user_name: member.network_user_name.clone(),
                    master_level: None,
                });
            }
        }

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-party-update", &self.encounter.player_data);
        }
        self.emit_live_update();
    }

    /// Handles setting the SBA gauge value for a player
    pub fn on_sba_update(&mut self, event: OnUpdateSBAEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnUpdateSBA(event.clone()),
            );
        }

        let player_index = event.actor_index;
        if let Some(player) = self.derived_state.party.get_mut(&player_index) {
            let child = self
                .action_actors
                .resolve(player_index, event.cause, player.character_type);
            player.set_sba(event.sba_value as f64);
            player.add_sba_gain(event.cause, child, event.sba_added as f64);
        }

        self.emit_live_update_throttled(Utc::now().timestamp_millis());
    }

    pub fn on_peer_counter(&mut self, event: protocol::PeerCounterEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnPeerCounter(event),
            );
        }
    }

    pub fn on_link_time_start(&mut self, event: protocol::LinkTimeStartEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnLinkTimeStart(event),
            );
        }
    }

    pub fn on_link_time_end(&mut self, event: protocol::LinkTimeEndEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter
                .push_event(Utc::now().timestamp_millis(), Message::OnLinkTimeEnd(event));
        }
    }

    /// A status effect was applied to, or refreshed on, an actor. Log-only:
    /// `pair_status_intervals` in `commands.rs` derives everything from the log.
    pub fn on_status_applied(&mut self, event: protocol::StatusAppliedEvent) {
        if self.status != ParserStatus::InProgress {
            return;
        }

        let now = Utc::now().timestamp_millis();
        self.encounter
            .push_event(now, Message::OnStatusApplied(event));
    }

    /// HP was restored to an actor. The heal is credited to its source.
    pub fn on_heal_event(&mut self, event: protocol::HealEvent) {
        if self.status != ParserStatus::InProgress {
            return;
        }

        let now = Utc::now().timestamp_millis();
        self.derived_state.process_heal_event(&event);
        self.encounter.push_event(now, Message::OnHeal(event));
    }

    /// A status effect left an actor. A removal is not necessarily the end of
    /// the status: `stacks` is the depth REMAINING after it, and what that
    /// means for an uptime interval is `pair_status_intervals`'s job.
    pub fn on_status_removed(&mut self, event: protocol::StatusRemovedEvent) {
        if self.status != ParserStatus::InProgress {
            return;
        }

        let now = Utc::now().timestamp_millis();
        self.encounter
            .push_event(now, Message::OnStatusRemoved(event));
    }

    /// A status's stack depth moved while the actor kept holding it. The apply
    /// event fires before the game writes the stack level, so it reports the
    /// depth from before the grant; this corrects it, when it fires at all.
    pub fn on_status_stacks_changed(&mut self, event: protocol::StatusStacksChangedEvent) {
        if self.status != ParserStatus::InProgress {
            return;
        }

        let now = Utc::now().timestamp_millis();
        self.encounter
            .push_event(now, Message::OnStatusStacksChanged(event));
    }

    pub fn on_link_attack_chance(&mut self, event: protocol::LinkAttackChanceEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnLinkAttackChance(event),
            );
        }
    }

    pub fn on_conflux_area_clear(&mut self, event: protocol::ConfluxAreaClearEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnConfluxAreaClear(event),
            );
        }
    }

    pub fn on_conflux_advance(&mut self, event: protocol::ConfluxAdvanceEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnConfluxAdvance(event),
            );
        }
    }

    pub fn on_conflux_boss_clear(&mut self, event: protocol::ConfluxBossClearEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnConfluxBossClear(event),
            );
        }
    }

    pub fn on_enemy_mode_change(&mut self, event: protocol::EnemyModeChangeEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnEnemyModeChange(event),
            );
        }
    }

    pub fn on_sba_window_change(&mut self, event: protocol::SbaWindowChangeEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnSbaWindowChange(event),
            );
        }
    }

    pub fn on_enemy_death(&mut self, event: protocol::EnemyDeathEvent) {
        if self.status == ParserStatus::InProgress {
            self.encounter.push_event(
                Utc::now().timestamp_millis(),
                Message::OnEnemyDeath(event),
            );
        }
    }

    pub fn on_sba_attempt(&mut self, event: OnAttemptSBAEvent) {
        self.encounter.push_event(
            Utc::now().timestamp_millis(),
            Message::OnAttemptSBA(event.clone()),
        );

        let player_index = event.actor_index;
        if let Some(player) = self.derived_state.party.get_mut(&player_index) {
            player.set_sba(SBA_ATTEMPT_LEVEL);
        }

        self.emit_live_update();
    }

    pub fn on_sba_perform(&mut self, event: OnPerformSBAEvent) {
        self.encounter.push_event(
            Utc::now().timestamp_millis(),
            Message::OnPerformSBA(event.clone()),
        );

        let player_index = event.actor_index;
        if let Some(player) = self.derived_state.party.get_mut(&player_index) {
            player.set_sba(0.0);
        }

        self.emit_live_update();
    }

    /// @TODO(false): Note that this event only fires for the local player.
    pub fn on_continue_sba_chain(&mut self, event: OnContinueSBAChainEvent) {
        self.encounter.push_event(
            Utc::now().timestamp_millis(),
            Message::OnContinueSBAChain(event.clone()),
        );

        let player_index = event.actor_index;
        if let Some(player) = self.derived_state.party.get_mut(&player_index) {
            player.set_sba(0.0);
        }

        self.emit_live_update();
    }

    pub fn on_death_event(&mut self, event: OnDeathEvent) {
        self.encounter.push_event(
            Utc::now().timestamp_millis(),
            Message::OnDeathEvent(event.clone()),
        );
    }

    fn reset(&mut self) {
        self.encounter.raw_event_log.clear();
        self.encounter.raw_event_log.shrink_to_fit();
        self.derived_state = Default::default();
        self.stun_recon = StunReconstructor::default();
        self.action_actors = ActionActors::default();
    }

    fn update_status(&mut self, new_status: ParserStatus) {
        self.status = new_status;
        self.derived_state.status = new_status;
    }

    fn has_damage(&self) -> bool {
        self.derived_state.total_damage > 0
    }

    // Checks if the damage event should be ignored for the purposes of parsing.
    fn should_ignore_damage_event(event: &DamageEvent) -> bool {
        let character_type = CharacterType::from_hash(event.source.parent_actor_type);

        if event.damage <= 0 && event.stun_value.unwrap_or(0.0) <= 0.0 {
            return true;
        }

        // A hit on a player's own helper entity, like Eugen's grenade, is a
        // duplicate of the one that landed on the real enemy.
        if is_helper_target(event) {
            return true;
        }

        // If the parent actor type is unknown (not tied to a player character), then ignore it.
        // This usually happens if the damage instance is tied to an enemy/monster.
        if matches!(character_type, CharacterType::Unknown(_)) {
            return true;
        }

        false
    }

    fn save_encounter_to_db(&mut self) -> Result<Option<i64>> {
        let duration_in_millis = self.derived_state.duration();
        let start_datetime = self.derived_state.utc_start_time()?;

        let primary_target = self
            .derived_state
            .get_primary_target()
            .map(|target| target.raw_target_type);

        // Sir Barrold should never save quest ID, as it could be stale.
        if primary_target == Some(0xA379AC65) {
            self.encounter.quest_id = None;
            self.encounter.quest_timer = None;
        }

        let encounter_data = self.encounter.to_blob()?;

        let p1 = self.encounter.player_data[0].as_ref();
        let p2 = self.encounter.player_data[1].as_ref();
        let p3 = self.encounter.player_data[2].as_ref();
        let p4 = self.encounter.player_data[3].as_ref();

        if let Some(conn) = &mut self.db {
            conn.execute(
                r#"INSERT INTO logs (
                        name,
                        time,
                        duration,
                        data,
                        version,
                        primary_target,
                        p1_name,
                        p1_type,
                        p2_name,
                        p2_type,
                        p3_name,
                        p3_type,
                        p4_name,
                        p4_type,
                        quest_id,
                        quest_elapsed_time,
                        quest_completed,
                        game_version,
                        p1_network_id,
                        p2_network_id,
                        p3_network_id,
                        p4_network_id,
                        p1_styles,
                        p2_styles,
                        p3_styles,
                        p4_styles,
                        app_version
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                params![
                    "",
                    start_datetime.timestamp_millis(),
                    duration_in_millis,
                    &encounter_data,
                    1,
                    primary_target,
                    p1.map(|p| p.display_name.as_str()),
                    p1.map(|p| p.character_type.to_string()),
                    p2.map(|p| p.display_name.as_str()),
                    p2.map(|p| p.character_type.to_string()),
                    p3.map(|p| p.display_name.as_str()),
                    p3.map(|p| p.character_type.to_string()),
                    p4.map(|p| p.display_name.as_str()),
                    p4.map(|p| p.character_type.to_string()),
                    self.encounter.quest_id,
                    self.encounter.quest_timer,
                    self.encounter.quest_completed,
                    self.game_version.as_deref(),
                    p1.and_then(|p| p.network_user_id.as_deref()),
                    p2.and_then(|p| p.network_user_id.as_deref()),
                    p3.and_then(|p| p.network_user_id.as_deref()),
                    p4.and_then(|p| p.network_user_id.as_deref()),
                    // Completed-styles bitmasks; 0 = computed none, NULL = not yet backfilled.
                    p1.map_or(0, |p| style_catalog::completed_styles(&p.master_trait_flags)),
                    p2.map_or(0, |p| style_catalog::completed_styles(&p.master_trait_flags)),
                    p3.map_or(0, |p| style_catalog::completed_styles(&p.master_trait_flags)),
                    p4.map_or(0, |p| style_catalog::completed_styles(&p.master_trait_flags)),
                    self.app
                        .as_ref()
                        .map(|app| app.package_info().version.to_string()),
                ],
            )?;

            let id = conn.last_insert_rowid();
            self.last_save_time = Some(Utc::now().timestamp_millis());

            return Ok(Some(id));
        }

        Ok(None)
    }
}

/// Converts a v0 parser into a v1 parser, but does not reparse the encounter.
impl From<v0::Parser> for Parser {
    fn from(parser: v0::Parser) -> Self {
        let encounter = Encounter {
            event_log: parser.damage_event_log,
            ..Default::default()
        };

        Self {
            encounter,
            status: ParserStatus::Stopped,
            ..Default::default()
        }
    }
}

