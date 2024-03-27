use core::fmt;
use std::{
    ffi::CString,
    fmt::{Display, Formatter},
};

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
pub struct Sigil {
    pub first_trait_id: u32,
    pub first_trait_level: u32,
    pub second_trait_id: u32,
    pub second_trait_level: u32,
    pub sigil_id: u32,
    pub equipped_character: u32,
    pub sigil_level: u32,
    pub acquisition_count: u32,
    pub notification_enum: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponInfo {
    /// Weapon ID Hash
    pub weapon_id: u32,
    /// How many uncap stars the weapon has
    pub star_level: u32,
    /// Number of plus marks on the weapon
    pub plus_marks: u32,
    /// Weapon's awakening level
    pub awakening_level: u32,
    /// First trait ID
    pub trait_1_id: u32,
    /// First trait level
    pub trait_1_level: u32,
    /// Second trait ID
    pub trait_2_id: u32,
    /// Second trait level
    pub trait_2_level: u32,
    /// Third trait ID
    pub trait_3_id: u32,
    /// Third trait level
    pub trait_3_level: u32,
    /// Wrightstone used on the weapon
    pub wrightstone_id: u32,
    /// Current weapon level
    pub weapon_level: u32,
    /// Weapon's HP Stats (before plus marks)
    pub weapon_hp: u32,
    /// Weapon's Attack Stats (before plus marks)
    pub weapon_attack: u32,
}

/// Overmastery, also known as `limit_bonus`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Overmastery {
    /// Overmastery ID
    pub id: u32,
    /// Flags
    pub flags: u32,
    /// Value
    pub value: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OvermasteryInfo {
    pub overmasteries: Vec<Overmastery>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerStats {
    pub level: u32,
    pub total_hp: u32,
    pub total_attack: u32,
    pub stun_power: f32,
    pub critical_rate: f32,
    pub total_power: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerLoadEvent {
    pub sigils: Vec<Sigil>,
    pub character_name: CString,
    pub display_name: CString,
    pub character_type: u32,
    pub party_index: u8,
    pub actor_index: u32,
    pub is_online: bool,
    pub weapon_info: WeaponInfo,
    pub overmastery_info: OvermasteryInfo,
    pub player_stats: PlayerStats,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AreaEnterEvent {
    /// Quest ID, last known. Could be stale if no other quest was ran while changing areas. 0 if no quest.
    pub last_known_quest_id: u32,
    /// Elapsed time in seconds, the in-game quest timer. Could be stale if no other quest was ran while changing areas.
    pub last_known_elapsed_time_in_secs: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuestCompleteEvent {
    pub quest_id: u32,
    pub elapsed_time_in_secs: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnUpdateSBAEvent {
    pub actor_index: u32,
    pub sba_value: f32,
    pub sba_added: f32,
}

/// Whenever SBA is attempted, but not necessarily hit.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnAttemptSBAEvent {
    pub actor_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnPerformSBAEvent {
    pub actor_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnContinueSBAChainEvent {
    pub actor_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    OnAreaEnter(AreaEnterEvent),
    OnQuestComplete(QuestCompleteEvent),
    DamageEvent(DamageEvent),
    OnUpdateSBA(OnUpdateSBAEvent),
    OnAttemptSBA(OnAttemptSBAEvent),
    OnPerformSBA(OnPerformSBAEvent),
    OnContinueSBAChain(OnContinueSBAChainEvent),
    PlayerLoadEvent(PlayerLoadEvent),
}
