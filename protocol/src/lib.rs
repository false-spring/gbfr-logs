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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ActionType {
    LinkAttack,
    SBA,
    Normal(u32),
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
