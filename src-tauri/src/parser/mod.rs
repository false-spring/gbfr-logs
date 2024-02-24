use std::collections::HashMap;

use chrono::Utc;
use protocol::DamageEvent;
use serde::{Deserialize, Serialize};

use self::constants::CharacterType;

mod constants;

#[derive(Debug, Serialize, Deserialize)]
struct PlayerState {
    index: u32,
    character_type: CharacterType,
    total_damage: u64,
    dps: f64,
    last_damage_time: u64,
}

type PlayerIndex = u32;
type DamageLogEvent = (u64, u64);

#[derive(Debug, Serialize, Deserialize)]
struct EncounterState {
    /// Total damage dealt by the party in this encounter
    total_damage: u64,
    /// The current DPS of the party in this encounter
    dps: f64,
    /// The start time of the encounter on first damage event (in milliseconds since epoch)
    start_time: u64,
    /// The end time of the encounter on last damage event (in milliseconds since epoch)
    end_time: u64,
    /// The players in the party,
    party: HashMap<PlayerIndex, PlayerState>,

    #[serde(skip)]
    damage_event_log: HashMap<PlayerIndex, Vec<DamageLogEvent>>,
}

impl EncounterState {
    pub fn new() -> Self {
        Self {
            total_damage: 0,
            dps: 0.0,
            start_time: 0,
            end_time: 0,
            party: HashMap::new(),
            damage_event_log: HashMap::new(),
        }
    }

    pub fn on_damage_event(&mut self, event: DamageEvent) {
        // @TODO(false): Do heals come through as negative damage?
        if event.damage <= 0 {
            return;
        }

        let now = Utc::now().timestamp_millis() as u64;

        if self.start_time == 0 {
            self.start_time = now;
        }

        self.end_time = now;
        self.total_damage += event.damage as u64;
        self.dps = self.total_damage as f64 / ((now - self.start_time) as f64 / 1000.0);

        // Add actor to party if not already present.
        let source_player = self.party.entry(event.source.index).or_insert(PlayerState {
            index: event.source.index,
            character_type: CharacterType::from(event.source.actor_type),
            total_damage: 0,
            dps: 0.0,
            last_damage_time: now,
        });

        source_player.total_damage += event.damage as u64;
        source_player.last_damage_time = now;
        source_player.dps =
            source_player.total_damage as f64 / ((now - self.start_time) as f64 / 1000.0);
    }
}
