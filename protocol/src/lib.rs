pub use bincode;

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\gbfr-logs";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Actor {
    pub index: u32,
    pub actor_type: u32,
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
