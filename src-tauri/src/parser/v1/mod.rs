use std::{collections::HashMap, io::BufReader};

use anyhow::Result;
use chrono::Utc;
use protocol::{ActionType, AreaEnterEvent, DamageEvent, PlayerLoadEvent, QuestCompleteEvent};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::Window;

use super::{
    constants::{CharacterType, EnemyType},
    v0,
};

/// Equippable sigil for a character
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Sigil {
    /// ID of the first trait in this sigil
    pub first_trait_id: u32,
    /// Level of the first trait in this sigil
    pub first_trait_level: u32,
    /// ID of the second trait in this sigil
    pub second_trait_id: u32,
    /// Level of the second trait in this sigil
    pub second_trait_level: u32,
    /// ID of the sigil
    pub sigil_id: u32,
    /// ID of the character that this sigil is equipped to
    pub equipped_character: u32,
    /// Level of the sigil
    pub sigil_level: u32,
    /// Acquisition count, at what sigil count this sigil was acquired
    pub acquisition_count: u32,
    /// 0 is new sigil and shows a (!), 1 is nothing, 2 is notification was checked and removes the (!)
    pub notification_enum: u32,
}

/// Data for a player in the encounter
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlayerData {
    /// Actor index for this player
    actor_index: u32,
    /// Display name for this player, empty if its an NPC
    display_name: String,
    /// Character name for this player if it's an NPC, otherwise it is the same as display_name
    character_name: String,
    /// Character type for this player
    character_type: CharacterType,
    /// Sigils that this player has equipped
    sigils: Vec<Sigil>,
    /// Whether this player was an online player or not
    is_online: bool,
}

/// Derived stat breakdown of a particular skill
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillState {
    /// Type of action ID that this skill is
    action_type: ActionType,
    /// Child character this skill belongs to (pet, Id's dragonform, etc.)
    child_character_type: CharacterType,
    /// Number of hits this skill has done
    hits: u32,
    /// Minimum damage done by this skill
    min_damage: Option<u64>,
    /// Maximum damage done by this skill
    max_damage: Option<u64>,
    /// Total damage done by this skill
    total_damage: u64,
}

impl SkillState {
    fn new(action_type: ActionType, child_character_type: CharacterType) -> Self {
        Self {
            action_type,
            child_character_type,
            hits: 0,
            min_damage: None,
            max_damage: None,
            total_damage: 0,
        }
    }

    fn update_from_damage_event(&mut self, event: &DamageEvent) {
        self.hits += 1;
        self.total_damage += event.damage as u64;

        if let Some(min_damage) = self.min_damage {
            self.min_damage = Some(min_damage.min(event.damage as u64));
        } else {
            self.min_damage = Some(event.damage as u64);
        }

        if let Some(max_damage) = self.max_damage {
            self.max_damage = Some(max_damage.max(event.damage as u64));
        } else {
            self.max_damage = Some(event.damage as u64);
        }
    }
}

/// Derived stat breakdown for a player
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub index: u32,
    character_type: CharacterType,
    total_damage: u64,
    dps: f64,
    skill_breakdown: Vec<SkillState>,
}

impl PlayerState {
    fn update_dps(&mut self, now: i64, start_time: i64) {
        self.dps = self.total_damage as f64 / ((now - start_time) as f64 / 1000.0);
    }

    fn update_from_damage_event(&mut self, event: &DamageEvent) {
        self.total_damage += event.damage as u64;

        // If the skill is already being tracked, update it.
        for skill in self.skill_breakdown.iter_mut() {
            // Aggregate all supplementary damage events into the same skill instance.
            if matches!(
                skill.action_type,
                protocol::ActionType::SupplementaryDamage(_)
            ) && matches!(
                event.action_id,
                protocol::ActionType::SupplementaryDamage(_)
            ) {
                skill.update_from_damage_event(event);
                return;
            }

            // If the skill is already being tracked, update it.
            if skill.action_type == event.action_id
                && skill.child_character_type == CharacterType::from_hash(event.source.actor_type)
            {
                skill.update_from_damage_event(event);
                return;
            }
        }

        // Otherwise, create a new skill and track it.
        let mut skill = SkillState::new(
            event.action_id,
            CharacterType::from_hash(event.source.actor_type),
        );

        skill.update_from_damage_event(event);
        self.skill_breakdown.push(skill);
    }
}

/// Derived breakdown for an enemy target
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnemyState {
    index: u32,
    target_type: EnemyType,
    raw_target_type: u32,
    total_damage: u64,
}

impl EnemyState {
    fn update_from_damage_event(&mut self, event: &DamageEvent) {
        self.total_damage += event.damage as u64;
    }
}

/// The necessary details of an encounter that can be used to recreate the state at any point in time.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    player_data: [Option<PlayerData>; 4],
    quest_id: Option<u32>,
    quest_timer: Option<u32>,
    #[serde(default)]
    quest_completed: bool,
    pub event_log: Vec<(i64, DamageEvent)>,
}

impl Encounter {
    /// Compresses this encounter data into a binary blob.
    pub fn to_blob(&self) -> Result<Vec<u8>> {
        let blob = cbor4ii::serde::to_vec(Vec::new(), &self)?;
        let mut reader = BufReader::new(blob.as_slice());
        let compressed_blob = zstd::encode_all(&mut reader, 3)?;
        Ok(compressed_blob)
    }

    /// Deserializes a binary blob into encounter instance.
    pub fn from_blob(blob: &[u8]) -> Result<Self> {
        let decompressed = zstd::decode_all(blob)?;
        Ok(cbor4ii::serde::from_slice(&decompressed)?)
    }

    /// Processes a damage event and adds it to the event log.
    pub fn process_damage_event(&mut self, timestamp: i64, event: &DamageEvent) {
        self.event_log.push((timestamp, event.clone()));
    }

    fn reset_player_data(&mut self) {
        self.player_data[1..=3].clone_from_slice(&[None, None, None]);
    }

    fn reset_quest(&mut self) {
        self.quest_id = None;
        self.quest_timer = None;
    }
}

/// The status of the parser.
#[derive(Debug, Serialize, Deserialize, Default, PartialEq, PartialOrd, Clone, Copy)]
enum ParserStatus {
    #[default]
    Waiting,
    InProgress,
    Stopped,
}

/// The state of the encounter after processing all damage events (or all known events for now)
/// Used for parsing the encounter into a calculated format that can be consumed by the front-end.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedEncounterState {
    /// Timestamp of the first damage event
    start_time: i64,
    /// Timestamp of the last damage event (or the last known damage event if the encounter is still in progress)
    end_time: i64,
    /// The total damage done in the encounter
    total_damage: u64,
    /// The total DPS done in the encounter
    dps: f64,
    /// Status of the parser
    status: ParserStatus,
    /// Derived party stats
    pub party: HashMap<u32, PlayerState>,
    /// Derived target stats, damage done to each target.
    targets: HashMap<u32, EnemyState>,
}

impl Default for DerivedEncounterState {
    fn default() -> Self {
        Self {
            start_time: 0,
            end_time: 0,
            total_damage: 0,
            dps: 0.0,
            status: ParserStatus::Waiting,
            party: HashMap::new(),
            targets: HashMap::new(),
        }
    }
}

impl DerivedEncounterState {
    pub fn duration(&self) -> i64 {
        (self.end_time - self.start_time).max(1)
    }

    fn utc_start_time(&self) -> Result<chrono::DateTime<Utc>> {
        chrono::DateTime::from_timestamp_millis(self.start_time)
            .ok_or(anyhow::anyhow!("Failed to convert start time to DateTime"))
    }

    fn start(&mut self, now: i64) {
        self.start_time = now;
        self.end_time = now;
    }

    /// Gets the primary target of the encounter (the target that had the most damage done to it)
    fn get_primary_target(&self) -> Option<&EnemyState> {
        self.targets
            .values()
            .max_by_key(|target| target.total_damage)
    }

    fn process_damage_event(&mut self, now: i64, event: &DamageEvent) {
        self.end_time = now;
        self.total_damage += event.damage as u64;
        self.dps = self.total_damage as f64 / ((self.duration()) as f64 / 1000.0);

        // Add actor to party if not already present.
        let source_player = self
            .party
            .entry(event.source.parent_index)
            .or_insert(PlayerState {
                index: event.source.parent_index,
                character_type: CharacterType::from_hash(event.source.parent_actor_type),
                total_damage: 0,
                dps: 0.0,
                skill_breakdown: Vec::new(),
            });

        // Update player stats from damage event.
        source_player.update_from_damage_event(event);

        // Update target stats from damage event.
        let target = self
            .targets
            .entry(event.target.parent_index)
            .or_insert(EnemyState {
                index: event.target.parent_index,
                target_type: EnemyType::from_hash(event.target.parent_actor_type),
                raw_target_type: event.target.parent_actor_type,
                total_damage: 0,
            });

        target.update_from_damage_event(event);

        // Update everyone's DPS
        for player in self.party.values_mut() {
            player.update_dps(now, self.start_time);
        }
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
    window_handle: Option<Window>,

    /// The database connection for the parser, used to save the encounter
    #[serde(skip)]
    db: Option<Connection>,
}

impl Parser {
    pub fn new(window: Window, db: Connection) -> Self {
        Self {
            db: Some(db),
            window_handle: Some(window),
            ..Default::default()
        }
    }

    /// Peeks at the first damage event in the log to get the start time of the encounter.
    pub fn start_time(&self) -> i64 {
        if let Some((timestamp, _)) = self.encounter.event_log.first() {
            *timestamp
        } else {
            1
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
        let encounter = Encounter::from_blob(blob)?;
        Ok(Self::from_encounter(encounter))
    }

    /// Reparses derived state from the current encounter.
    pub fn reparse(&mut self) {
        self.derived_state = Default::default();
        self.derived_state.start(self.start_time());

        for (timestamp, event) in self.encounter.event_log.iter() {
            self.derived_state.process_damage_event(*timestamp, event);
        }
    }

    // Re-analyzes the encounter with the given targets.
    pub fn reparse_with_options(&mut self, targets: &[EnemyType]) {
        self.derived_state = Default::default();
        self.derived_state.start(self.start_time());

        for (timestamp, event) in self.encounter.event_log.iter() {
            // If the target list is empty, then we're not filtering by target.
            // Otherwise, we only process damage events that match the target list.
            let target_type = EnemyType::from_hash(event.target.parent_actor_type);

            if targets.is_empty() || targets.contains(&target_type) {
                self.derived_state.process_damage_event(*timestamp, event);
            }
        }
    }

    /// Handles the event when an area is entered.
    /// If the current encounter was in progress, then stop it as we've left the instance.
    /// If there was damage in that stopped instance, then save it as a new log.
    /// Otherwise, we're waiting for the encounter to start.
    pub fn on_area_enter_event(&mut self, event: AreaEnterEvent) {
        self.encounter.quest_id = Some(event.last_known_quest_id);

        if self.status == ParserStatus::InProgress {
            self.update_status(ParserStatus::Stopped);

            if self.has_damage() {
                match self.save_encounter_to_db() {
                    Ok(_) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved", "");
                        }
                    }
                    Err(e) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved-error", e.to_string());
                        }
                    }
                }
            }
        } else {
            self.update_status(ParserStatus::Waiting);
        }

        self.encounter.quest_completed = false;
        self.encounter.reset_player_data();

        if let Some(window) = &self.window_handle {
            let _ = window.emit("on-area-enter", &self.derived_state);
        }
    }

    pub fn on_quest_complete_event(&mut self, event: QuestCompleteEvent) {
        // @TODO(false): Check to see if we need any logic to run on quest completion.
        self.encounter.quest_id = Some(event.quest_id);
        self.encounter.quest_timer = Some(event.elapsed_time_in_secs);
        self.encounter.quest_completed = true;
    }

    // Called when a damage event is received from the game.
    pub fn on_damage_event(&mut self, event: DamageEvent) {
        let now = Utc::now().timestamp_millis();

        if Self::should_ignore_damage_event(&event) {
            return;
        }

        // If this is the first damage event, set the start time.
        if self.status == ParserStatus::Stopped || self.status == ParserStatus::Waiting {
            self.reset();
            self.derived_state.start(now);
            self.update_status(ParserStatus::InProgress);
        }

        self.encounter.process_damage_event(now, &event);
        self.derived_state.process_damage_event(now, &event);

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-update", &self.derived_state);
        }
    }

    pub fn on_player_load_event(&mut self, event: PlayerLoadEvent) {
        let character_type = CharacterType::from_hash(event.character_type);

        // Ignore Id's transformation.
        if character_type == CharacterType::Pl2000 {
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
            character_name: event.character_name.to_string_lossy().to_string(),
            is_online: event.is_online,
            character_type,
            sigils,
        };

        // Insert into encounter player data array, using actor_index.
        if !player_data.is_online && event.party_index == 0 {
            self.encounter.player_data[0] = Some(player_data.clone());
        } else {
            for i in 1..=3 {
                if let Some(player) = &self.encounter.player_data[i] {
                    // If this is the same player, update it.
                    if player.actor_index == player_data.actor_index {
                        self.encounter.player_data[i] = Some(player_data.clone());
                        break;
                    }

                    // If the actor index we're trying to insert is lower than the current slot's actor index,
                    // then we need to shift the rest of the array to the right.
                    if player_data.actor_index < player.actor_index {
                        self.encounter.player_data[i..].rotate_right(1);
                        self.encounter.player_data[i] = Some(player_data.clone());
                        break;
                    }
                } else {
                    self.encounter.player_data[i] = Some(player_data.clone());
                    break;
                }
            }
        }

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-party-update", &self.encounter.player_data);
        }
    }

    fn reset(&mut self) {
        self.encounter.event_log.clear();
        self.encounter.event_log.shrink_to_fit();
        self.derived_state = Default::default();
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

        if event.damage <= 0 {
            return true;
        }

        // Eugen's Grenade should be ignored.
        if event.target.actor_type == 0x022a350f {
            return true;
        }

        // @TODO(false): Sometimes monsters can damage themselves, we should track those.
        // For now, I'm ignoring them from the damage calculation.
        if matches!(character_type, CharacterType::Unknown(_)) {
            return true;
        }

        false
    }

    fn save_encounter_to_db(&mut self) -> Result<()> {
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
                        quest_elapsed_time
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
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
                ],
            )?;
        }

        Ok(())
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
