use std::{collections::HashMap, io::BufReader};

use anyhow::Result;
use chrono::Utc;
use protocol::{ActionType, DamageEvent};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::Window;

use super::constants::{CharacterType, EnemyType};

/// Equippable sigil for a character
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
struct PlayerData {
    /// Display name for this player
    display_name: String,
    /// Character name for this playe if it's an NPC, otherwise it is the same as display_name
    character_name: String,
    /// Sigils that this player has equipped
    sigils: Vec<Sigil>,
}

/// Derived stat breakdown of a particular skill
#[derive(Debug, Serialize, Deserialize)]
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
struct PlayerState {
    index: u32,
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
            event.action_id.clone(),
            CharacterType::from_hash(event.source.actor_type),
        );

        skill.update_from_damage_event(event);
        self.skill_breakdown.push(skill);
    }
}

/// Derived breakdown for an enemy target
#[derive(Debug, Serialize, Deserialize)]
struct EnemyState {
    index: u32,
    target_type: EnemyType,
    total_damage: u64,
}

impl EnemyState {
    fn update_from_damage_event(&mut self, event: &DamageEvent) {
        self.total_damage += event.damage as u64;
    }
}

/// The necessary details of an encounter that can be used to recreate the state at any point in time.
#[derive(Debug, Serialize, Deserialize)]
struct Encounter {
    player_data: [Option<PlayerData>; 4],
    event_log: Vec<(i64, DamageEvent)>,
}

impl Default for Encounter {
    fn default() -> Self {
        Self {
            player_data: [None, None, None, None],
            event_log: vec![],
        }
    }
}

impl Encounter {
    /// Compresses this encounter data into a binary blob.
    pub fn to_blob(&self) -> Result<Vec<u8>> {
        let blob = protocol::bincode::serialize(self)?;
        let mut reader = BufReader::new(blob.as_slice());
        let compressed_blob = zstd::encode_all(&mut reader, 3)?;
        Ok(compressed_blob)
    }

    /// Deserializes a binary blob into encounter instance.
    pub fn from_blob(blob: &[u8]) -> Result<Self> {
        let decompressed = zstd::decode_all(blob)?;
        Ok(protocol::bincode::deserialize(&decompressed)?)
    }

    /// Processes a damage event and adds it to the event log.
    pub fn process_damage_event(&mut self, timestamp: i64, event: &DamageEvent) {
        self.event_log.push((timestamp, event.clone()));
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
struct DerivedEncounterState {
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
    party: HashMap<u32, PlayerState>,
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
    fn duration(&self) -> i64 {
        (self.end_time - self.start_time).min(1)
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
        source_player.update_from_damage_event(&event);

        // Update target stats from damage event.
        let target = self
            .targets
            .entry(event.target.parent_index)
            .or_insert(EnemyState {
                index: event.target.parent_index,
                target_type: EnemyType::from_hash(event.target.parent_actor_type),
                total_damage: 0,
            });

        target.update_from_damage_event(&event);

        // Update everyone's DPS
        for player in self.party.values_mut() {
            player.update_dps(now, self.start_time);
        }
    }
}

/// The parser for the encounter.
#[derive(Debug, Serialize, Deserialize, Default)]
struct Parser {
    /// Encounter that will be saved into the database, contains all the state needed to reparse
    encounter: Encounter,
    /// Derived state of the encounter, used for parsing the encounter into a calculated format that can be consumed by the front-end
    derived_state: DerivedEncounterState,
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

    /// Reparses derived state from a given encounter.
    pub fn from_encounter(encounter: Encounter) -> Self {
        let mut parser = Self {
            encounter,
            ..Default::default()
        };

        parser.reparse();
        parser
    }

    /// Reparses derived state from the current encounter.
    pub fn reparse(&mut self) {
        todo!()
    }

    /// Handles the event when an area is entered.
    /// If the current encounter was in progress, then stop it as we've left the instance.
    /// If there was damage in that stopped instance, then save it as a new log.
    /// Otherwise, we're waiting for the encounter to start.
    pub fn on_area_enter_event(&mut self) {
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

        if let Some(window) = &self.window_handle {
            let _ = window.emit("on-area-enter", &self.derived_state);
        }
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

    fn reset(&mut self) {
        self.encounter = Default::default();
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

        // @TODO(false): Do heals come through as negative damage?
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

        // @TODO(false): Generate good display name
        let log_name = "Log";

        let encounter_data = self.encounter.to_blob()?;

        let primary_target = self
            .derived_state
            .get_primary_target()
            .map(|target| target.index);

        if let Some(conn) = &mut self.db {
            conn.execute(
                "INSERT INTO logs (name, time, duration, data, version, primary_target) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    log_name,
                    start_datetime.timestamp_millis(),
                    duration_in_millis,
                    &encounter_data,
                    1,
                    primary_target
                ],
            )?;
        }

        Ok(())
    }
}
