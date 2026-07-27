use std::{collections::HashMap, io::BufReader};

use anyhow::Result;
use chrono::Utc;
use protocol::{DamageEvent, HealEvent, Message};
use serde::{Deserialize, Serialize};

use super::{
    canonical_character_type,
    player_state::{classify_heal, HealBreakdown},
    capped_damage, AdjustedDamageInstance, CharacterType, EnemyType, PlayerData, PlayerState,
};

/// Derived breakdown for an enemy target
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EnemyState {
    index: u32,
    target_type: EnemyType,
    pub(super) raw_target_type: u32,
    base_target_type: EnemyType,
    total_damage: u64,
}

impl EnemyState {
    fn update_from_damage_event(&mut self, damage_instance: &AdjustedDamageInstance) {
        self.total_damage += damage_instance.event.damage as u64;
    }
}

/// The necessary details of an encounter that can be used to recreate the state at any point in time.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    pub player_data: [Option<PlayerData>; 4],
    pub quest_id: Option<u32>,
    pub quest_timer: Option<u32>,
    #[serde(default)]
    pub quest_completed: bool,
    #[serde(default)]
    pub quest_abandoned: bool,

    /// DEPRECATED: Use `self.event_log()` instead.
    pub event_log: Vec<(i64, DamageEvent)>,

    #[serde(default)]
    pub raw_event_log: Vec<(i64, Message)>,
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

    pub fn builds_from_blob(blob: &[u8]) -> Result<[Option<PlayerData>; 4]> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Builds {
            player_data: [Option<PlayerData>; 4],
        }

        let decompressed = zstd::decode_all(blob)?;
        let builds: Builds = cbor4ii::serde::from_slice(&decompressed)?;
        Ok(builds.player_data)
    }

    /// For older logs that don't have the event log, we need to repopulate it.
    pub fn repopulate_event_log(&mut self) {
        if !self.raw_event_log.is_empty() {
            return;
        }

        for (timestamp, event) in self.event_log.iter() {
            self.raw_event_log
                .push((*timestamp, Message::DamageEvent(event.clone())));
        }
    }

    pub(super) fn reset_player_data(&mut self) {
        self.player_data[0..=3].clone_from_slice(&[None, None, None, None]);
    }

    pub(super) fn reset_quest(&mut self) {
        self.quest_id = None;
        self.quest_timer = None;
    }

    pub(super) fn push_event(&mut self, timestamp: i64, event: protocol::Message) {
        self.raw_event_log.push((timestamp, event));
    }

    /// Hits on player-side helpers are dropped on the way out. The live path
    /// already refuses to record them, so this is for the logs written before
    /// it, which still hold the duplicates.
    pub fn event_log(&self) -> impl Iterator<Item = &(i64, Message)> {
        self.raw_event_log.iter().filter(|(_, event)| match event {
            Message::DamageEvent(event) => !super::is_helper_target(event),
            _ => true,
        })
    }
}

/// The status of the parser.
#[derive(Debug, Serialize, Deserialize, Default, PartialEq, PartialOrd, Clone, Copy)]
pub(super) enum ParserStatus {
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
    pub(super) start_time: i64,
    /// Timestamp of the last damage event (or the last known damage event if the encounter is still in progress)
    pub(super) end_time: i64,
    /// The total damage done in the encounter
    pub(super) total_damage: u64,
    /// The total DPS done in the encounter
    dps: f64,
    /// The total stun value done in the encounter
    total_stun_value: f64,
    /// The total stun value per second done in the encounter
    stun_per_second: f64,
    /// Status of the parser
    pub(super) status: ParserStatus,
    /// Derived party stats
    pub party: HashMap<u32, PlayerState>,
    /// Derived target stats, damage done to each target.
    pub(super) targets: HashMap<u32, EnemyState>,
}

impl Default for DerivedEncounterState {
    fn default() -> Self {
        Self {
            start_time: 0,
            end_time: 0,
            total_damage: 0,
            dps: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            status: ParserStatus::Waiting,
            party: HashMap::new(),
            targets: HashMap::new(),
        }
    }
}

/// The live `encounter-update` / `on-area-enter` payload, borrowed from
/// `DerivedEncounterState`. `targets` is left out: it grows for the whole
/// encounter and the live meter does not read it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveEncounterUpdate<'a> {
    start_time: i64,
    end_time: i64,
    total_damage: u64,
    dps: f64,
    total_stun_value: f64,
    stun_per_second: f64,
    status: ParserStatus,
    party: &'a HashMap<u32, PlayerState>,
}

impl DerivedEncounterState {
    pub(super) fn live_payload(&self) -> LiveEncounterUpdate<'_> {
        LiveEncounterUpdate {
            start_time: self.start_time,
            end_time: self.end_time,
            total_damage: self.total_damage,
            dps: self.dps,
            total_stun_value: self.total_stun_value,
            stun_per_second: self.stun_per_second,
            status: self.status,
            party: &self.party,
        }
    }

    pub fn duration(&self) -> i64 {
        (self.end_time - self.start_time).max(1)
    }

    pub(super) fn utc_start_time(&self) -> Result<chrono::DateTime<Utc>> {
        chrono::DateTime::from_timestamp_millis(self.start_time)
            .ok_or(anyhow::anyhow!("Failed to convert start time to DateTime"))
    }

    pub(super) fn start(&mut self, now: i64) {
        self.start_time = now;
        self.end_time = now;
    }

    /// Gets the primary target of the encounter (the target that had the most damage done to it)
    pub(super) fn get_primary_target(&self) -> Option<&EnemyState> {
        self.targets
            .values()
            .max_by_key(|target| target.total_damage)
    }

    pub(super) fn process_damage_taken(&mut self, event: &DamageEvent, cap: Option<u64>) {
        if event.damage <= 0 {
            return;
        }
        if let Some(player) = self.party.get_mut(&event.target.parent_index) {
            player.total_damage_taken += capped_damage(event.damage, cap);
        }
    }

    pub(super) fn process_heal_event(&mut self, event: &HealEvent) {
        if event.amount <= 0 {
            return;
        }
        let amount = event.amount as u64;
        let category = classify_heal(event);
        if let Some(source) = event.source {
            if let Some(player) = self.party.get_mut(&source) {
                player.heal_done += amount;
                player.heal_provided_by_type.add(category, amount);
            }
        }
        if let Some(player) = self.party.get_mut(&event.target) {
            player.heal_received += amount;
            player.heal_received_by_type.add(category, amount);
        }
    }

    /// The party row for an actor, opened on first sight. A player who dealt no
    /// damage still gets one.
    pub(super) fn party_row(&mut self, actor_index: u32, character_type: CharacterType) -> &mut PlayerState {
        self.party.entry(actor_index).or_insert(PlayerState {
            index: actor_index,
            character_type,
            total_damage: 0,
            dps: 0.0,
            sba: 0.0,
            stun_per_second: 0.0,
            total_stun_value: 0.0,
            skill_breakdown: Vec::new(),
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: Vec::new(),
            total_sba_added: 0.0,
        })
    }

    pub(super) fn process_damage_event(&mut self, now: i64, damage_instance: &AdjustedDamageInstance) {
        self.end_time = now;
        self.total_damage += damage_instance.event.damage as u64;
        self.dps = self.total_damage as f64 / ((self.duration()) as f64 / 1000.0);

        // Update stun value
        self.total_stun_value += damage_instance.stun_damage;
        self.stun_per_second = self.total_stun_value / ((self.duration()) as f64 / 1000.0);

        // Add actor to party if not already present.
        let source_player = self
            .party
            .entry(damage_instance.event.source.parent_index)
            .or_insert(PlayerState {
                index: damage_instance.event.source.parent_index,
                character_type: canonical_character_type(
                    damage_instance.event.source.parent_actor_type,
                ),
                total_damage: 0,
                dps: 0.0,
                sba: 0.0,
                stun_per_second: 0.0,
                total_stun_value: 0.0,
                skill_breakdown: Vec::new(),
                last_known_pet_skill: None,
                last_non_sentinel_action: None,
                total_damage_taken: 0,
                heal_done: 0,
                heal_received: 0,
                heal_provided_by_type: HealBreakdown::default(),
                heal_received_by_type: HealBreakdown::default(),
                sba_breakdown: Vec::new(),
                total_sba_added: 0.0,
            });

        // Update player stats from damage event.
        source_player.update_from_damage_event(damage_instance);

        // Update target stats from damage event.
        let target = self
            .targets
            .entry(damage_instance.event.target.parent_index)
            .or_insert(EnemyState {
                index: damage_instance.event.target.parent_index,
                target_type: EnemyType::from_hash(damage_instance.event.target.parent_actor_type),
                raw_target_type: damage_instance.event.target.parent_actor_type,
                base_target_type: EnemyType::from_hash(
                    damage_instance
                        .event
                        .target_base_type
                        .unwrap_or(damage_instance.event.target.parent_actor_type),
                ),
                total_damage: 0,
            });

        target.update_from_damage_event(damage_instance);

        // Update everyone's DPS
        for player in self.party.values_mut() {
            player.update_dps(now, self.start_time);
        }
    }
}
