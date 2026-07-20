/// `attack_rate` is the multiplier the damage core consumes, not +0xD8 or +0xDC.
#[derive(Debug)]
#[repr(C)]
pub struct DamageInstance {
    padding_00: [u8; 0xD4],   // 0x000 - 0x0D4
    pub damage: i32,          // 0x0D4
    padding_d8: [u8; 0x08],   // 0x0D8 - 0x0E0
    pub attack_rate: f32,     // 0x0E0
    padding_e4: [u8; 0x04],   // 0x0E4 - 0x0E8
    pub flags: u64,           // 0x0E8
    padding_f0: [u8; 0x04],   // 0x0F0 - 0x0F4
    /// Per-hit stun in gauge units, pre-bonus.
    pub stun: f32,            // 0x0F4
    padding_f8: [u8; 0x74],   // 0x0F8 - 0x16C
    pub action_id: u32,       // 0x16C
    padding_170: [u8; 0x14C], // 0x170 - 0x2BC
    pub damage_cap: i32,      // 0x2BC
}

/// Based at the quest manager singleton itself, not at a1+0x1D8 as pre-2.0.
#[derive(Debug)]
#[repr(C)]
pub struct QuestState {
    padding_00: [u8; 0xAC8],  // 0x000 - 0xAC8
    pub elapsed_time: u32,    // 0xAC8 — seconds; the final clear time, frozen at quest end
    padding_acc: [u8; 0x10],  // 0xACC - 0xADC
    // 0 while playing, 1 the instant the quest ends, 0 again on quest load.
    pub freeze_flag: u8,      // 0xADC
    padding_add: [u8; 0x2EB], // 0xADD - 0xDC8
    pub quest_id: u32,        // 0xDC8
}

/// One of the 13 sigil slots at record+0x5E60 (the array grew 12 -> 13 in ER).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SigilEntry {
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

impl From<&SigilEntry> for protocol::Sigil {
    fn from(sigil: &SigilEntry) -> Self {
        protocol::Sigil {
            first_trait_id: sigil.first_trait_id,
            first_trait_level: sigil.first_trait_level,
            second_trait_id: sigil.second_trait_id,
            second_trait_level: sigil.second_trait_level,
            sigil_id: sigil.sigil_id,
            equipped_character: sigil.equipped_character,
            sigil_level: sigil.sigil_level,
            acquisition_count: sigil.acquisition_count,
            notification_enum: sigil.notification_enum,
        }
    }
}

/// Equipped-summon slot, four per record; populated for remote players too.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SummonSlot {
    pub id: u32, // 0x00 — summon id hash; 0x887AE0B0 = empty slot
    unk_04: u32, // 0x04 (0-init)
    pub trait_id: u32, // 0x08 — aura trait id hash granted by this summon
    pub equip_bonus_id: u32, // 0x0C — 0x887AE0B0 = none; names via `summonbonuses`
    pub trait_level: i32, // 0x10 — aura level as shown on the card ("T.Lvl"; -1 init)
    pub equip_bonus_level: i32, // 0x14 — 0-based index into the bonus's ladder (-1 init)
    unk_18: u32, // 0x18 (0-init)
}

/// Five weapon pairs at record+0xF4, three wrightstone pairs at record+0x70.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TraitPair {
    pub trait_id: u32,
    pub level: u32,
}

/// Active Over Mastery slot, four of them at record+0x58B8 + n*0x10.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct OverMasterySlot {
    pub param_hash: u32, // 0x00 — limit_bonus_param hash; 0x887AE0B0 or 0 = empty
    pub rank_bit: u32, // 0x04 — single set bit: 1 << (star_rank - 1); 0 = empty
    pub param_type: u32, // 0x08 — param type code (0..3 / 100+ family)
    pub value: f32, // 0x0C — raw per-rank value
}

/// One row of the 400-slot mastery array at record+0x138. The first three u32s
/// classify it: skillboard flag, limit_bonus catalog entry, or empty fill.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MasteryNodeRow {
    pub hash: u32,        // 0x00
    pub flag: u32,        // 0x04
    pub marker: u32,      // 0x08
    pub rest: [u8; 0x2C], // 0x0C - 0x38 (unused by the meter)
}

#[cfg(test)]
mod tests {
    use super::DamageInstance;
    use std::mem::offset_of;

    #[test]
    fn damage_instance_field_offsets() {
        assert_eq!(offset_of!(DamageInstance, damage), 0xD4);
        assert_eq!(offset_of!(DamageInstance, attack_rate), 0xE0);
        assert_eq!(offset_of!(DamageInstance, flags), 0xE8);
        assert_eq!(offset_of!(DamageInstance, stun), 0xF4);
        assert_eq!(offset_of!(DamageInstance, action_id), 0x16C);
        assert_eq!(offset_of!(DamageInstance, damage_cap), 0x2BC);
    }
}
