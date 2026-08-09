use serde::{Deserialize, Serialize};

use super::CharacterType;

/// Weapon equipped by a character
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct WeaponInfo {
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
    #[serde(default)]
    pub trait_4_id: u32,
    /// Fourth trait level
    #[serde(default)]
    pub trait_4_level: u32,
    /// Fifth trait ID
    #[serde(default)]
    pub trait_5_id: u32,
    /// Fifth trait level
    #[serde(default)]
    pub trait_5_level: u32,
    /// Wrightstone used on the weapon
    pub wrightstone_id: u32,
    /// Current weapon level
    pub weapon_level: u32,
    /// Transcendence level (Endless Ragnarok; 0 = none)
    #[serde(default)]
    pub transcendence_level: u32,
    /// Awakening level (Endless Ragnarok; 0 = none)
    #[serde(default)]
    pub awakening_level_er: u32,
    /// Weapon's HP Stats (before plus marks)
    pub weapon_hp: u32,
    /// Weapon's Attack Stats (before plus marks)
    pub weapon_attack: u32,
    /// First wrightstone trait ID
    #[serde(default)]
    pub wrightstone_trait_1_id: u32,
    /// First wrightstone trait level
    #[serde(default)]
    pub wrightstone_trait_1_level: u32,
    /// Second wrightstone trait ID
    #[serde(default)]
    pub wrightstone_trait_2_id: u32,
    /// Second wrightstone trait level
    #[serde(default)]
    pub wrightstone_trait_2_level: u32,
    /// Third wrightstone trait ID
    #[serde(default)]
    pub wrightstone_trait_3_id: u32,
    /// Third wrightstone trait level
    #[serde(default)]
    pub wrightstone_trait_3_level: u32,
}

impl From<protocol::WeaponInfo> for WeaponInfo {
    fn from(info: protocol::WeaponInfo) -> Self {
        Self {
            weapon_id: info.weapon_id,
            star_level: info.star_level,
            plus_marks: info.plus_marks,
            awakening_level: info.awakening_level,
            trait_1_id: info.trait_1_id,
            trait_1_level: info.trait_1_level,
            trait_2_id: info.trait_2_id,
            trait_2_level: info.trait_2_level,
            trait_3_id: info.trait_3_id,
            trait_3_level: info.trait_3_level,
            trait_4_id: info.trait_4_id,
            trait_4_level: info.trait_4_level,
            trait_5_id: info.trait_5_id,
            trait_5_level: info.trait_5_level,
            wrightstone_id: info.wrightstone_id,
            weapon_level: info.weapon_level,
            transcendence_level: info.transcendence_level,
            awakening_level_er: info.awakening_level_er,
            weapon_hp: info.weapon_hp,
            weapon_attack: info.weapon_attack,
            wrightstone_trait_1_id: info.wrightstone_trait_1_id,
            wrightstone_trait_1_level: info.wrightstone_trait_1_level,
            wrightstone_trait_2_id: info.wrightstone_trait_2_id,
            wrightstone_trait_2_level: info.wrightstone_trait_2_level,
            wrightstone_trait_3_id: info.wrightstone_trait_3_id,
            wrightstone_trait_3_level: info.wrightstone_trait_3_level,
        }
    }
}

/// Overmastery, also known as `limit_bonus`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Overmastery {
    /// Overmastery ID
    pub id: u32,
    /// Flags
    pub flags: u32,
    /// Value
    pub value: f32,
}

impl From<protocol::Overmastery> for Overmastery {
    fn from(info: protocol::Overmastery) -> Self {
        Self {
            id: info.id,
            flags: info.flags,
            value: info.value,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OvermasteryInfo {
    pub overmasteries: Vec<Overmastery>,
}

impl From<protocol::OvermasteryInfo> for OvermasteryInfo {
    fn from(info: protocol::OvermasteryInfo) -> Self {
        Self {
            overmasteries: info
                .overmasteries
                .into_iter()
                .map(Overmastery::from)
                .collect(),
        }
    }
}

/// One equipped summon (Endless Ragnarok).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SummonSlot {
    /// Summon ID hash (0x887AE0B0 = empty slot)
    pub id: u32,
    /// Aura trait ID granted by this summon
    pub trait_id: u32,
    /// Aura trait level as shown on the summon card ("T.Lvl"; -1 uninitialized)
    pub trait_level: i32,
    /// Equip-bonus id hash (0x887AE0B0 = none)
    #[serde(default)]
    pub equip_bonus_id: u32,
    /// Equip-bonus level: 0-based index into the bonus's value ladder (-1 uninit)
    #[serde(default)]
    pub equip_bonus_level: i32,
}

impl From<protocol::SummonSlot> for SummonSlot {
    fn from(info: protocol::SummonSlot) -> Self {
        Self {
            id: info.id,
            trait_id: info.trait_id,
            trait_level: info.trait_level,
            equip_bonus_id: info.equip_bonus_id,
            equip_bonus_level: info.equip_bonus_level,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SummonInfo {
    pub summons: Vec<SummonSlot>,
}

impl From<protocol::SummonInfo> for SummonInfo {
    fn from(info: protocol::SummonInfo) -> Self {
        Self {
            summons: info.summons.into_iter().map(SummonSlot::from).collect(),
        }
    }
}

/// One active Over Mastery bonus (Endless Ragnarok).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OverMasteryLine {
    /// limit_bonus_param hash (0x887AE0B0 / 0 = empty slot)
    pub id: u32,
    /// Star rank 1..10; 0 = empty
    pub rank: u32,
    /// Raw per-rank value
    pub value: f32,
}

impl From<protocol::OverMasteryLine> for OverMasteryLine {
    fn from(line: protocol::OverMasteryLine) -> Self {
        Self {
            id: line.id,
            rank: line.rank,
            value: line.value,
        }
    }
}

/// One skillboard flag row (Endless Ragnarok): an SBE_* effect hash and
/// whether it is allocated.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MasterTraitFlag {
    pub hash: u32,
    pub on: bool,
}

impl From<protocol::MasterTraitFlag> for MasterTraitFlag {
    fn from(flag: protocol::MasterTraitFlag) -> Self {
        Self {
            hash: flag.hash,
            on: flag.on,
        }
    }
}

/// One post-cap effective trait (Endless Ragnarok): the game's 0..99 level
/// for a trait hash.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveTrait {
    pub hash: u32,
    pub level: u32,
}

impl From<protocol::EffectiveTrait> for EffectiveTrait {
    fn from(t: protocol::EffectiveTrait) -> Self {
        Self {
            hash: t.hash,
            level: t.level,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub level: u32,
    pub total_hp: u32,
    pub total_attack: u32,
    pub stun_power: f32,
    pub critical_rate: f32,
    pub total_power: u32,
    /// Raw DMG-cap accumulator channels (rec+0x28/+0x30/+0x34).
    #[serde(default)]
    pub dmg_cap_channels: [f32; 3],
}

impl From<protocol::PlayerStats> for PlayerStats {
    fn from(stats: protocol::PlayerStats) -> Self {
        Self {
            level: stats.level,
            total_hp: stats.total_hp,
            total_attack: stats.total_attack,
            stun_power: stats.stun_power,
            critical_rate: stats.critical_rate,
            total_power: stats.total_power,
            dmg_cap_channels: stats.dmg_cap_channels,
        }
    }
}

/// Equippable sigil for a character
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct Sigil {
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
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerData {
    /// Actor index for this player
    pub(super) actor_index: u32,
    /// Display name for this player, empty if its an NPC
    pub(super) display_name: String,
    /// Character name for this player if it's an NPC, otherwise it is the same as display_name
    pub(super) character_name: String,
    /// Character type for this player
    pub(super) character_type: CharacterType,
    /// Sigils that this player has equipped
    pub(super) sigils: Vec<Sigil>,
    /// Whether this player was an online player or not
    pub(super) is_online: bool,
    /// Weapon info for this player
    pub(super) weapon_info: Option<WeaponInfo>,
    /// Overmastery info for this player (pre-ER logs only)
    pub(super) overmastery_info: Option<OvermasteryInfo>,
    /// Equipped summons for this player (Endless Ragnarok logs only)
    #[serde(default)]
    pub(super) summon_info: Option<SummonInfo>,
    /// Equipped skill ability-ids (Endless Ragnarok logs only; 0 = empty slot)
    #[serde(default)]
    pub(super) skill_loadout: Vec<u32>,
    /// Active Over Mastery bonuses (Endless Ragnarok logs only)
    #[serde(default)]
    pub(super) over_mastery: Vec<OverMasteryLine>,
    /// Skillboard flag band (Endless Ragnarok logs only)
    #[serde(default)]
    pub(super) master_trait_flags: Vec<MasterTraitFlag>,
    /// Consolidated post-cap effective traits (Endless Ragnarok logs only)
    #[serde(default)]
    pub(super) effective_traits: Vec<EffectiveTrait>,
    /// Player stats for this player
    pub(super) player_stats: Option<PlayerStats>,
    /// Persistent cross-platform account id (PlayFab entity id); `None` for
    /// local/offline play.
    #[serde(default)]
    pub(super) network_user_id: Option<String>,
    /// Account handle, distinct from `display_name`.
    #[serde(default)]
    pub(super) network_user_name: Option<String>,
    /// Master level 0..=55 (Endless Ragnarok logs only): the UI draws
    /// `min(v, 50)` as Master Lvl and `max(0, v - 50)` as Mastery Break stars.
    #[serde(default)]
    pub(super) master_level: Option<u32>,
}

impl PlayerData {
    pub fn master_trait_flags(&self) -> &[MasterTraitFlag] {
        &self.master_trait_flags
    }
}
