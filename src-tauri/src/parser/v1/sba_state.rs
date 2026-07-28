use protocol::SbaCause;
use serde::{Deserialize, Serialize};

use crate::parser::constants::CharacterType;

/// One row of a player's SBA gauge generation, keyed by cause and child character type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SbaSourceState {
    pub cause: SbaCause,
    /// Child character that performed the move (pet, Id's dragonform, etc.), when known.
    pub child_character_type: Option<CharacterType>,
    /// Gauge-update calls folded into this row, not a hit count.
    pub ticks: u32,
    /// Same 0..1000 scale as `PlayerState::sba`.
    pub total_sba_added: f64,
}
