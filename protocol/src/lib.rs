/*!
This library crate provides the event protocol that is emitted by the "hook"
injected into the game process and consumed by the GBFR Logs parser.

Keep in mind that the serialization protocol is not defined here, only the
serializable message types.

The protocol between the hook and the parser is a simple named pipe, where the
messages are encoded as "bincode" serialized bytes. This means that the hook and
the parser must be compiled together to ensure that the serialization format is
the same.

The parser saves these messages in a different serialization format that provides
forward-compatibility so that old logs can still be read by newer versions of the
parser.

Because of this, any changes to the protocol must be done carefully to ensure that
the parser can still read old logs. This is done by adding new fields to the existing
message types, or adding new message types that are ignored by the parser.

New fields carry `#[serde(default)]` to keep the ability to open older parses.
*/

use core::fmt;
use std::{
    ffi::CString,
    fmt::{Display, Formatter},
};

pub use bincode;

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\gbfr-logs";

pub const PLAYER_ID_BASE: u32 = 0x8000_0000;

pub const fn party_slot_from_actor_index(actor_index: u32) -> Option<u8> {
    if actor_index >= PLAYER_ID_BASE {
        Some((actor_index - PLAYER_ID_BASE) as u8)
    } else {
        None
    }
}

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
    pub attack_rate: Option<f32>,
    pub stun_value: Option<f32>,
    pub damage_cap: Option<i32>,

    /// Stun fill of target at this hit, 0..1
    #[serde(default)]
    pub stun_fill: Option<f32>,

    /// Target's base class ID. Set when the `target.parent_actor_type` differs which can happen
    /// whenever the hit target is actually the child of the actual main actor.
    #[serde(default)]
    pub target_base_type: Option<u32>,

    /// Target's stun max capacity in stun units at hit time.
    #[serde(default)]
    pub stun_max: Option<f32>,
}

/// For debugging (damage events from remote players that should
/// get suppressed because damage is 0).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SuppressedDuplicateDamageEvent {
    pub source: Actor,
    pub damage: i32,
    pub action_id: ActionType,
    pub is_replicated: u8,
    pub source_is_local: Option<bool>,
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
    /// Fourth trait ID
    pub trait_4_id: u32,
    /// Fourth trait level
    pub trait_4_level: u32,
    /// Fifth trait ID
    pub trait_5_id: u32,
    /// Fifth trait level
    pub trait_5_level: u32,
    /// Wrightstone used on the weapon
    pub wrightstone_id: u32,
    /// Current weapon level
    pub weapon_level: u32,
    /// Transcendence level
    pub transcendence_level: u32,
    /// Awakening level
    pub awakening_level_er: u32,
    /// Weapon's HP Stats (before plus marks)
    pub weapon_hp: u32,
    /// Weapon's Attack Stats (before plus marks)
    pub weapon_attack: u32,
    /// First wrightstone trait ID
    pub wrightstone_trait_1_id: u32,
    /// First wrightstone trait level
    pub wrightstone_trait_1_level: u32,
    /// Second wrightstone trait ID
    pub wrightstone_trait_2_id: u32,
    /// Second wrightstone trait level
    pub wrightstone_trait_2_level: u32,
    /// Third wrightstone trait ID
    pub wrightstone_trait_3_id: u32,
    /// Third wrightstone trait level
    pub wrightstone_trait_3_level: u32,
}

/// Overmastery, also known as `limit_bonus`.
/// Pre-2.0 uses `Overmastery`, 2.0+ uses `OverMasteryLine`.
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
pub struct SummonSlot {
    /// Summon ID hash
    pub id: u32,
    /// Trait ID hash
    pub trait_id: u32,
    /// Aura trait level as shown on the summon card ("T.Lvl")
    /// -1 means uninitialized.
    pub trait_level: i32,
    /// Equip-bonus ID hash
    pub equip_bonus_id: u32,
    /// The level used here is a 0-based index lookup for the equip bonus value.
    /// -1 means uninitialized.
    pub equip_bonus_level: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SummonInfo {
    pub summons: Vec<SummonSlot>,
}

/// ER Over Mastery bonus.
/// The displayed number is `value` for percent params, `value` x 10 for Stun
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OverMasteryLine {
    /// limit_bonus_param hash (MED_EFF_*)
    /// 0x887AE0B0 / 0 = empty slot.
    pub id: u32,
    /// Star rank 1..10
    /// 0 = empty.
    pub rank: u32,
    /// Raw per-rank value
    pub value: f32,
}

/// One Master Trait (`skillboard`) flag row.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MasterTraitFlag {
    pub hash: u32,
    pub on: bool,
}

/// Consolidated trait level for "Effective Traits" tab
/// `level` is the game's 0..99 level after all sources
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EffectiveTrait {
    pub hash: u32,
    pub level: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerStats {
    pub level: u32,
    pub total_hp: u32,
    pub total_attack: u32,
    pub stun_power: f32,
    pub critical_rate: f32,
    pub total_power: u32,

    /// Stores 3 damage caps.
    ///
    /// 1. Normal ATK
    /// 2. Skill
    /// 3. SBA
    ///
    /// Includes everything from Masteries, Master Trait Rank bonus, OMs, Weapons traits, Summons,
    /// Weapon Collection, some sigils (Fatebreaker), the +20% cap per basic sigil master trait.
    ///
    /// However, this does NOT include the "DMG Cap" trait and conditional cap sigils like
    /// Celestial Lumen or most Mastery Trait nodes themselves.
    pub dmg_cap_channels: [f32; 3],
}

/// Pre-2.0 player-load payload.
/// Deprecated, replaced by `PlayerIdentityEvent` in 2.0+.
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
pub struct PlayerIdentityEvent {
    pub sigils: Vec<Sigil>,
    pub character_name: CString,
    pub display_name: CString,
    pub character_type: u32,
    pub party_index: u8,
    pub actor_index: u32,
    pub is_online: bool,
    pub weapon_info: Option<WeaponInfo>,
    pub summon_info: Option<SummonInfo>,
    pub over_mastery: Vec<OverMasteryLine>,
    pub skill_loadout: Vec<u32>,
    pub player_stats: Option<PlayerStats>,
    pub master_trait_flags: Vec<MasterTraitFlag>,
    pub effective_traits: Vec<EffectiveTrait>,

    /// Cygames PlayFab entity id
    #[serde(default)]
    pub network_user_id: Option<String>,

    /// Platform account name (the current Steam persona / crossplay id)
    #[serde(default)]
    pub network_user_name: Option<String>,

    /// 0 to 55, accounts for Master Break
    #[serde(default)]
    pub master_level: Option<u32>,
}

pub const MASTER_LEVEL_DISPLAY_CAP: u32 = 50;
pub const MASTER_LEVEL_MAX: u32 = 55;

pub fn split_master_level(master_level: u32) -> (u32, u32) {
    (
        master_level.min(MASTER_LEVEL_DISPLAY_CAP),
        master_level.saturating_sub(MASTER_LEVEL_DISPLAY_CAP),
    )
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RosterMember {
    pub party_index: u8,
    pub character_type: u32,
    pub display_name: CString,
    /// Platform account name
    /// - `None` if not yet known.
    pub network_user_name: Option<String>,
    pub network_user_id: Option<String>,
    pub is_online: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartyRosterEvent {
    pub members: Vec<RosterMember>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AreaEnterEvent {
    /// Quest ID, last known.
    /// - Could be stale if no other quest was ran while changing areas.
    /// - `0` if no quest.
    pub last_known_quest_id: u32,
    /// Elapsed time in seconds, the in-game quest timer.
    /// Could be stale if no other quest was ran while changing areas.
    pub last_known_elapsed_time_in_secs: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuestCompleteEvent {
    pub quest_id: u32,
    pub elapsed_time_in_secs: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuestAbandonEvent {
    pub quest_id: u32,
    pub elapsed_time_in_secs: u32,
}

/// The reason why a player's SBA gauge moved. Emitted and used for visualization.
/// SBA sources are inferred and not 100% accurate, especially due to a lack of information when
/// replicating across networks.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SbaCause {
    /// Legacy logs
    #[default]
    NotClassified,

    /// Credited to an action, read directly from the causing call.
    /// `Normal(611)` / `Normal(610)`.
    Action(ActionType),

    /// Gauge built by taking a hit (1%/0.7% in chaos).
    DamageTaken,
    /// Gauge built by taking a hit (for online party members).
    InferredDamageTaken,

    /// The 10%/13% granted to party members after someone else SBAs.
    ChainGrant,
    /// Inferred chain grant, for online party members.
    InferredChainGrant,
    /// Gauge from calling a summon: 10% of max, or 7% at Chaos tier and above.
    InferredSummonCall,

    /// Gauge given by another player via a redistribute skill.
    InferredRedistribute,

    /// When the whole party fills to 100% (Seven Star's Brilliance).
    InferredPartyFill,

    /// A remote party member's perfect-guard/perfect-dodge counter.
    Inferred(ActionType),

    /// Generic remote gauge sync event with no cause.
    Remote,

    /// Fallback when we cannot infer any of the other causes.
    Unknown,

    /// For when we can't attribute the cause due to the hook not being available.
    HookUnavailable,
}

/// A network peer's perfect-guard / perfect-dodge. `action_id` is the
/// synthesized id: 611 for a perfect guard, 610 for a perfect dodge.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerCounterEvent {
    pub actor_index: u32,
    pub action_id: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnUpdateSBAEvent {
    pub actor_index: u32,
    pub sba_value: f32,
    pub sba_added: f32,
    #[serde(default)]
    pub cause: SbaCause,
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
pub struct OnDeathEvent {
    pub actor_index: u32,
    pub death_counter: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkTimeStartEvent {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkTimeEndEvent {
    pub reason: u32,
}

/// Fires when an area in Conflux survey is cleared.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfluxAreaClearEvent {
    /// ID of the area that just ended (the `0x8xxxxx` family).
    pub content_id: u32,
    /// Position of that area in the survey (1-based)
    pub area: u32,
    pub cycle: u32,
    pub area_count: u32,
    pub cycle_count: u32,
    /// Area-specific quest timer, resets per area
    pub area_elapsed_time_in_secs: u32,
}

/// Fires when a Conflux survey advances to its next area.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfluxAdvanceEvent {
    /// ID of the area being moved into
    pub content_id: u32,
    /// Position of that area in the survey (1-based)
    pub area: u32,
    pub cycle: u32,
    pub area_count: u32,
    pub cycle_count: u32,
}

/// Fires when an enemy's HP reaches zero.
/// Tracked per target because a destroyed part has its own HP and its own death.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnemyDeathEvent {
    pub target_index: u32,
    pub actor_type: u32,
}

/// Fires when an enemy's overdrive / break state changes.
/// Currently, we don't hook this into the actual state change, it's inferred based on damage events.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnemyModeChangeEvent {
    /// The enemy this is about (`target.parent_index`).
    pub target_index: u32,
    pub in_overdrive: bool,
    pub in_break: bool,
}

/// Fires when the SBA chain-burst window opens or closes.
/// Currently, this isn't hooked into actual changes, this event is inferred.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SbaWindowChangeEvent {
    pub active: bool,
}

/// Fires when a boss is defeated inside a Conflux survey.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfluxBossClearEvent {
    /// ID of the area whose boss just died.
    pub content_id: u32,
    /// Position of that area in the survey (1-based)
    pub area: u32,
    pub cycle: u32,
    pub area_count: u32,
    pub cycle_count: u32,
}

/// Fires when a link attack became available.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkAttackChanceEvent {}

/// Fires when a status effect gets applied to an actor (or gets refreshed).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusAppliedEvent {
    pub actor_index: u32,
    pub status_id: u32,
    pub is_refresh: bool,

    /// The total duration this application granted, in seconds.
    #[serde(default)]
    pub full_duration_secs: Option<f32>,
    pub remaining_secs: f32,

    /// `None` for environmental and self-applied statuses.
    #[serde(default)]
    pub applier_index: Option<u32>,

    /// The game's source object key, deconstructed.
    #[serde(default)]
    pub source_ids: Option<[u32; 3]>,

    #[serde(default)]
    pub stacks: Option<u32>,

    /// Whether the values are a FRACTION of a stat (0.20 = a 20% debuff)
    /// rather than an absolute number (DoT amount, Shield HP).
    #[serde(default)]
    pub value_is_fraction: Option<bool>,

    /// Summed multiplier of every applied status on the actor,
    /// (two DEF DOWNs at 0.10 and 0.20 act as 0.30). `None` if not numeric
    #[serde(default)]
    pub value_total: Option<f32>,

    /// This status object's own multiplier.
    #[serde(default)]
    pub value: Option<f32>,

    /// True for a status with no expiry at all (like auras)
    /// The game stores a 9999.0 in both duration fields for these.
    #[serde(default)]
    pub is_permanent: Option<bool>,
}

/// Fires when a status effect is removed from an actor.
/// Either through natural expiry, dispel, death, respawn, teardown.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusRemovedEvent {
    pub actor_index: u32,
    pub status_id: u32,

    #[serde(default)]
    pub stacks: Option<u32>,
    #[serde(default)]
    pub stacks_before: Option<u32>,
    #[serde(default)]
    pub value_total: Option<f32>,
    #[serde(default)]
    pub value_is_fraction: Option<bool>,
    #[serde(default)]
    pub applier_index: Option<u32>,
    #[serde(default)]
    pub source_ids: Option<[u32; 3]>,

    /// This status's own multiplier at the moment it is removed.
    #[serde(default)]
    pub value: Option<f32>,
}

/// Fires when a status effect's stack changes. (Mirror Image charge spent)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusStacksChangedEvent {
    pub actor_index: u32,
    pub status_id: u32,
    pub stacks: u32,
    #[serde(default)]
    pub value_total: Option<f32>,
    #[serde(default)]
    pub value_is_fraction: Option<bool>,
    #[serde(default)]
    pub applier_index: Option<u32>,
    #[serde(default)]
    pub source_ids: Option<[u32; 3]>,

    /// This status's multiplier at its new stack.
    #[serde(default)]
    pub value: Option<f32>,
}

/// Fires when HP is restored to an actor.
/// Either through a heal skill, potion, HP drain, regen, etc.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HealEvent {
    /// The actor that caused the heal (same ID space as `DamageEvent::source`)
    /// `None` = environmental or source-less.
    pub source: Option<u32>,

    /// The actor whose HP went up (same ID space as `DamageEvent::source`)
    pub target: u32,

    pub amount: i32,

    /// Type of heal:
    /// - 0 = skill/character heal or environment,
    /// - 1 = regen/auto-heal tick
    /// - 2 = drain + potion
    /// - 3 = null-source system heal
    /// - 4 = revive/res-potion.
    pub heal_type: u32,

    /// Ability channel:
    /// - `0x13880` means this is a `PlayerHealHpAction` heal skill.
    /// - `0xFFFFFFFF` is used for other sources.
    /// - `0` default for older logs.
    #[serde(default)]
    pub channel: u32,

    /// Which code path this heal came from:
    /// - `true` = the "apply primitive" (skill / character heal / revive)
    /// - `false` = the per-target update flush (drain / potion / regen / system).
    #[serde(default)]
    pub via_apply: bool,
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
    OnDeathEvent(OnDeathEvent),
    PlayerIdentityEvent(PlayerIdentityEvent),
    OnQuestAbandon(QuestAbandonEvent),
    PartyRoster(PartyRosterEvent),

    /// Debug-Mode-only.
    SuppressedDuplicateDamage(SuppressedDuplicateDamageEvent),

    OnPeerCounter(PeerCounterEvent),
    OnLinkTimeStart(LinkTimeStartEvent),
    OnLinkTimeEnd(LinkTimeEndEvent),
    OnLinkAttackChance(LinkAttackChanceEvent),

    OnConfluxAreaClear(ConfluxAreaClearEvent),
    OnConfluxAdvance(ConfluxAdvanceEvent),
    OnConfluxBossClear(ConfluxBossClearEvent),
    OnEnemyModeChange(EnemyModeChangeEvent),
    OnSbaWindowChange(SbaWindowChangeEvent),
    OnEnemyDeath(EnemyDeathEvent),
    OnStatusApplied(StatusAppliedEvent),
    OnStatusRemoved(StatusRemovedEvent),
    OnStatusStacksChanged(StatusStacksChangedEvent),
    OnHeal(HealEvent),
}
