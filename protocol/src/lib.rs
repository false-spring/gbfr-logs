use core::fmt;
use std::fmt::{Display, Formatter};

pub use bincode;

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\gbfr-logs";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Actor {
    /// Index of the actor, unique in the party.
    pub index: u32,
    /// Hash ID of the actor.
    pub actor_type: u32,
    /// Index of the actor's parent. If no parent, then it's the same as `index`.
    pub parent_index: u32,
    /// Hash ID of this actor's parent. If no parent, then it's the same as `actor_type`.
    pub parent_actor_type: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub enum ActionType {
    /// Link Attack
    LinkAttack,
    /// Skybound Arts
    SBA,
    /// Supplementary Damage containing the original skill ID that trigged it.
    SupplementaryDamage(u32),
    /// Damage over time, containing the effect type. (Currently, always 0 until we find more info)
    DamageOverTime(u32),
    /// Normal Skill Attack containing the skill ID.
    Normal(u32),
}

impl Display for ActionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ActionType::LinkAttack => write!(f, "Link Attack"),
            ActionType::SBA => write!(f, "Skybound Arts"),
            ActionType::SupplementaryDamage(id) => write!(f, "Supplementary Damage ({})", id),
            ActionType::DamageOverTime(id) => write!(f, "Damage Over Time ({})", id),
            ActionType::Normal(id) => write!(f, "Skill ({})", id),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DamageEvent {
    pub source: Actor,
    pub target: Actor,
    pub damage: i32,
    pub flags: u64,
    pub action_id: ActionType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    OnAreaEnter,
    DamageEvent(DamageEvent),
}
