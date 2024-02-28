use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

use chrono::Utc;
use protocol::DamageEvent;
use serde::{Deserialize, Serialize};
use tauri::Window;

use self::constants::CharacterType;

mod constants;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    index: u32,
    character_type: CharacterType,
    total_damage: u64,
    dps: f64,
    last_damage_time: i64,
}

type PlayerIndex = u32;

#[derive(Default, Debug, Serialize, Deserialize)]
pub enum EncounterStatus {
    #[default]
    Waiting,
    InProgress,
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
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parser {
    /// Whether the parser is currently live and listening for damage events.
    is_live: bool,
    /// The current state of the encounter.
    encounter_state: EncounterState,
    /// A log of all damage events that have occurred during the encounter.
    damage_event_log: Vec<DamageEvent>,

    #[serde(skip)]
    window_handle: Option<Window>,
}

impl Parser {
    pub fn new(window_handle: Option<Window>) -> Self {
        Self {
            is_live: true,
            window_handle,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        // If there was damage, then save this encounter as a new log.
        if self.encounter_state.has_damage() {
            match self.save_parse_log_to_file() {
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

        self.encounter_state.reset_stats();
        self.damage_event_log.clear();

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-reset", &self.encounter_state);
        }
    }

    pub fn on_area_enter_event(&mut self) {
        self.reset();

        if let Some(window) = &self.window_handle {
            let _ = window.emit("on-area-enter", "");
        }
    }

    pub fn on_damage_event(&mut self, event: DamageEvent) {
        self.damage_event_log.push(event.clone());

        let character_type = CharacterType::from(event.source.parent_actor_type);

        // @TODO(false): Sometimes monsters can damage themselves, we should track those.
        // For now, I'm ignoring them from the damage calculation.
        if matches!(character_type, CharacterType::Unknown(_)) {
            return;
        }

        // @TODO(false): Do heals come through as negative damage?
        if event.damage <= 0 {
            return;
        }

        let now = Utc::now().timestamp_millis();

        // If this is the first damage event, set the start time.
        if self.encounter_state.start_time == 0 {
            self.encounter_state.start_time = now;
            self.encounter_state.status = EncounterStatus::InProgress;
        }

        self.encounter_state.end_time = now;
        self.encounter_state.total_damage += event.damage as u64;
        self.encounter_state.dps = self.encounter_state.total_damage as f64
            / ((now - self.encounter_state.start_time) as f64 / 1000.0);

        // Add actor to party if not already present.
        let source_player = self
            .encounter_state
            .party
            .entry(event.source.parent_index)
            .or_insert(PlayerState {
                index: event.source.parent_index,
                character_type: CharacterType::from(event.source.parent_actor_type),
                total_damage: 0,
                dps: 0.0,
                last_damage_time: now,
            });

        source_player.total_damage += event.damage as u64;
        source_player.last_damage_time = now;
        source_player.dps = source_player.total_damage as f64
            / ((now - self.encounter_state.start_time) as f64 / 1000.0);

        if let Some(window) = &self.window_handle {
            let _ = window.emit("encounter-update", &self.encounter_state);
        }
    }

    fn save_parse_log_to_file(&self) -> Result<String, anyhow::Error> {
        let start_datetime =
            chrono::DateTime::from_timestamp_millis(self.encounter_state.start_time)
                .ok_or(anyhow::anyhow!("Failed to convert start time to DateTime"))?;

        let mut folder_path = PathBuf::new();
        folder_path.push("logs");
        folder_path.push(start_datetime.format("%Y%m%d").to_string());
        std::fs::create_dir_all(folder_path.as_path())?;
        let file_name = format!("encounter-{}.gbfr", start_datetime.format("%Y%m%d-%H%M%S"));
        folder_path.push(file_name.clone());

        let file = File::create(folder_path)?;
        let writer = BufWriter::new(file);
        protocol::bincode::serialize_into(writer, &self)?;

        Ok(file_name.to_string())
    }

    pub fn load_parse_log_from_file(file_name: &str) -> Result<Self, anyhow::Error> {
        let file = File::open(file_name)?;
        let reader = BufReader::new(file);
        let parsed_log: Self = protocol::bincode::deserialize_from(reader)?;

        Ok(parsed_log)
    }
}
