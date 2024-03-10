use std::collections::HashMap;

use anyhow::Result;
use protocol::DamageEvent;
use serde::{Deserialize, Serialize};

use crate::parser::constants::CharacterType;

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

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parser {
    /// The current state of the encounter.
    pub encounter_state: EncounterState,

    /// A log of all damage events that have occurred during the encounter.
    pub damage_event_log: Vec<(i64, DamageEvent)>,
}

impl Parser {
    /// Deserializes a binary blob into a Parser instance.
    /// The blob can be optionally compressed with zstd.
    pub fn from_blob(blob: &[u8]) -> Result<Self> {
        let is_zstd = blob.len() > 4 && blob[0..4] == [0x28, 0xb5, 0x2f, 0xfd];

        if is_zstd {
            let decompressed = zstd::decode_all(blob)?;
            Ok(protocol::bincode::deserialize(&decompressed)?)
        } else {
            Ok(protocol::bincode::deserialize(blob)?)
        }
    }
}
