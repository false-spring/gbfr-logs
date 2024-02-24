pub use bincode;

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\gbfr-logs";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Actor {
    pub actor_type: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ActionType {
    LinkAttack,
    SBA,
    Normal(u32),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    OnAreaEnter,
    DamageEvent {
        source: Actor,
        target: Actor,
        damage: i32,
        flags: u64,
        action_id: ActionType,
    },
}
