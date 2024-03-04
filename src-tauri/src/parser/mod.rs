use std::{collections::HashMap, fs::File, io::BufReader};

use anyhow::Result;
use chrono::prelude::*;
use protocol::DamageEvent;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::Window;

use self::constants::{CharacterType, EnemyType};

pub mod constants;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillState {
    action_type: protocol::ActionType,
    child_character_type: CharacterType,
    hits: u32,
    min_damage: Option<u64>,
    max_damage: Option<u64>,
    total_damage: u64,
}

impl SkillState {
    pub fn new(action_type: protocol::ActionType, child_character_type: CharacterType) -> Self {
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
        self.min_damage = self.min_damage.map_or(Some(event.damage as u64), |min| {
            Some(min.min(event.damage as u64))
        });
        self.max_damage = self.max_damage.map_or(Some(event.damage as u64), |max| {
            Some(max.max(event.damage as u64))
        });
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub index: u32,
    character_type: CharacterType,
    total_damage: u64,
    dps: f64,
    last_damage_time: i64,
    skills: Vec<SkillState>,
}

impl PlayerState {
    fn reset(&mut self) {
        self.total_damage = 0;
        self.dps = 0.0;
        self.last_damage_time = 0;
        self.skills.clear();
    }

    fn update_from_damage_event(&mut self, event: &DamageEvent, start_time: i64, now: i64) {
        self.total_damage += event.damage as u64;
        self.last_damage_time = now;
        self.dps = self.total_damage as f64 / ((now - start_time) as f64 / 1000.0);

        // If the skill is already being tracked, update it.
        for skill in self.skills.iter_mut() {
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
        self.skills.push(skill);
    }
}

type PlayerIndex = u32;

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub enum EncounterStatus {
    #[default]
    Waiting,
    InProgress,
    Stopped,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterState {
    pub start_time: i64,
    pub end_time: i64,
    pub total_damage: u64,
    pub dps: f64,
    pub party: HashMap<PlayerIndex, PlayerState>,
    pub status: EncounterStatus,
}

impl EncounterState {
    fn reset_stats(&mut self) {
        self.total_damage = 0;
        self.dps = 0.0;
        self.start_time = 0;
        self.end_time = 0;
        self.status = EncounterStatus::Waiting;
        self.party.clear();
    }

    fn has_damage(&self) -> bool {
        self.total_damage > 0
    }

    fn process_damage_event(&mut self, now: i64, event: &DamageEvent) {
        self.total_damage += event.damage as u64;
        self.dps = self.total_damage as f64 / ((now - self.start_time) as f64 / 1000.0);

        // Add actor to party if not already present.
        let source_player = self
            .party
            .entry(event.source.parent_index)
            .or_insert(PlayerState {
                index: event.source.parent_index,
                character_type: CharacterType::from_hash(event.source.parent_actor_type),
                total_damage: 0,
                dps: 0.0,
                last_damage_time: now,
                skills: Vec::new(),
            });

        // Update player stats from damage event.
        source_player.update_from_damage_event(&event, self.start_time, now);
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parser {
    /// The current state of the encounter.
    pub encounter_state: EncounterState,

    /// A log of all damage events that have occurred during the encounter.
    pub damage_event_log: Vec<(i64, DamageEvent)>,

    #[serde(skip)]
    window_handle: Option<Window>,

    #[serde(skip)]
    db: Option<Connection>,
}

impl Parser {
    pub fn new(window_handle: Option<Window>, db: Connection) -> Self {
        Self {
            window_handle,
            db: Some(db),
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.encounter_state.reset_stats();
        self.damage_event_log.clear();
    }

    pub fn on_area_enter_event(&mut self) {
        // If there was damage, then save this encounter as a new log.
        if self.encounter_state.status == EncounterStatus::InProgress {
            // If the encounter was is in progress, then stop it as we've left the instance.
            self.encounter_state.status = EncounterStatus::Stopped;

            if self.encounter_state.has_damage() {
                match self.save_parse_to_db() {
                    Ok(file_name) => {
                        if let Some(window) = &self.window_handle {
                            let _ = window.emit("encounter-saved", file_name);
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
            // Otherwise, we're waiting for the encounter to start.
            self.encounter_state.status = EncounterStatus::Waiting;
        }

        if let Some(window) = &self.window_handle {
            let _ = window.emit("on-area-enter", &self.encounter_state);
        }
    }

    // Re-analyzes the encounter with the given targets.
    pub fn reparse(&mut self, targets: &Vec<EnemyType>) {
        self.encounter_state.total_damage = 0;
        self.encounter_state.dps = 0.0;

        for player in self.encounter_state.party.values_mut() {
            player.reset();
        }

        let event_log = self.damage_event_log.iter_mut();

        for (time, event) in event_log {
            // If the target list is empty, then we're not filtering by target.
            // Otherwise, we only process damage events that match the target list.
            let target_type = EnemyType::from_hash(event.target.parent_actor_type);

            if targets.is_empty() || targets.contains(&target_type) {
                self.encounter_state.process_damage_event(*time, &event);
            }
        }
    }

    // Called when a damage event is received from the game.
    pub fn on_damage_event(&mut self, event: DamageEvent) {
        let now = Utc::now().timestamp_millis();

        if Self::should_ignore_damage_event(&event) {
            return;
        }

        // If this is the first damage event, set the start time.
        if self.encounter_state.status == EncounterStatus::Stopped
            || self.encounter_state.status == EncounterStatus::Waiting
        {
            self.reset();
            self.encounter_state.start_time = now;
            self.encounter_state.status = EncounterStatus::InProgress;
        }

        self.damage_event_log.push((now, event.clone()));
        self.encounter_state.end_time = now;

        self.encounter_state.process_damage_event(now, &event);

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-update", &self.encounter_state);
        }
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

    fn save_parse_to_db(&mut self) -> Result<()> {
        let duration_in_millis = self.encounter_state.end_time - self.encounter_state.start_time;
        let start_datetime =
            chrono::DateTime::from_timestamp_millis(self.encounter_state.start_time)
                .ok_or(anyhow::anyhow!("Failed to convert start time to DateTime"))?;

        let mut party_members = self
            .encounter_state
            .party
            .clone()
            .into_values()
            .collect::<Vec<PlayerState>>();

        party_members.sort_by(|a, b| a.index.partial_cmp(&b.index).unwrap());

        let name = party_members
            .into_iter()
            .map(|p| p.character_type.to_string())
            .collect::<Vec<String>>()
            .join(", ");

        let serialized_self = protocol::bincode::serialize(&self)?;

        if let Some(conn) = &mut self.db {
            conn.execute(
                "INSERT INTO logs (name, time, duration, data) VALUES (?, ?, ?, ?)",
                params![
                    name,
                    start_datetime.timestamp_millis(),
                    duration_in_millis,
                    &serialized_self,
                ],
            )?;
        }

        Ok(())
    }

    pub fn load_parse_log_from_file(file_name: &str) -> Result<Self, anyhow::Error> {
        let file = File::open(file_name)?;
        let reader = BufReader::new(file);
        let parsed_log: Self = protocol::bincode::deserialize_from(reader)?;

        Ok(parsed_log)
    }
}
