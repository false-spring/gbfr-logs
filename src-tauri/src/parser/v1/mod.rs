use std::collections::HashMap;

use protocol::{ActionType, DamageEvent};
use rusqlite::Connection;
use tauri::Window;

use super::constants::CharacterType;

/// Equippable sigil for a character
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
struct PlayerData {
    /// Display name for this player
    display_name: String,
    /// Character name for this playe if it's an NPC, otherwise it is the same as display_name
    character_name: String,
    /// Sigils that this player has equipped
    sigils: Vec<Sigil>,
}

/// Derived stat breakdown of a particular skill
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

/// Derived stat breakdown for a player
struct PlayerState {
    index: u32,
    character_type: CharacterType,
    total_damage: u64,
    dps: f64,
    skill_breakdown: Vec<SkillState>,
}

/// The necessary details of an encounter that can be used to recreate the state at any point in time.
struct Encounter {
    player_data: [PlayerData; 4],
    event_log: Vec<(i64, DamageEvent)>,
}

/// The status of the parser.
#[derive(Default, Debug)]
enum ParserStatus {
    #[default]
    Waiting,
    InProgress,
    Stopped,
}

/// The state of the encounter after processing all damage events (or all known events for now)
/// Used for parsing the encounter into a calculated format that can be consumed by the front-end.
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
}

/// The parser for the encounter.
struct Parser {
    /// Encounter that will be saved into the database, contains all the state needed to reparse
    encounter: Encounter,
    /// Derived state of the encounter, used for parsing the encounter into a calculated format that can be consumed by the front-end
    derived_state: DerivedEncounterState,
    /// Status of the parser
    status: ParserStatus,
    /// The window handle for the parser, used to send messages to the front-end
    window_handle: Option<Window>,
    /// The database connection for the parser, used to save the encounter
    db: Option<Connection>,
}
