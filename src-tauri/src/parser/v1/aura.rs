//! Which Conflux aura produced a sentinel `99999` damage hit. Every Conflux
//! buff without an action id of its own reports 99999; the flags qword tells
//! them apart:
//!
//! | `0x1080100000060000` | Reflection of a Fallen Fort                 |
//! | `0x1080100002020000` | Toxic Blast                                 |
//! | `0x1080100002040000` | Mystery Box                                 |
//! | `0x1000100002040000` | Luster of Darkness, or an Ice and Fire tail |

use protocol::ActionType;
use serde::{Deserialize, Serialize};

pub(super) const CONFLUX_AURA_SENTINEL: u32 = 99999;

pub(super) const ICE_AND_FIRE: u32 = 100008;

const BIT_CLASS_A: u64 = 1 << 44;
const BIT_CLASS_B: u64 = 1 << 60;
const BIT_PIPELINE_EXEMPT: u64 = 1 << 55;
const BIT_REFLECTION: u64 = 1 << 18;
const BIT_TOXIC: u64 = 1 << 25;

/// Variant names serialize verbatim and `AURA_SOURCE_KEYS` in
/// `src/utils/i18n.ts` keys off them. Add a variant there too.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuraSource {
    #[default]
    None,
    ToxicBlast,
    LusterOfDarkness,
    ReflectionOfAFallenFort,
    MysteryBox,
    IceAndFireFollowUp,
}

pub(super) fn aura_source_of(
    action: ActionType,
    flags: u64,
    last_action: Option<ActionType>,
) -> AuraSource {
    if action != ActionType::Normal(CONFLUX_AURA_SENTINEL) {
        return AuraSource::None;
    }

    if flags & BIT_CLASS_A == 0 || flags & BIT_CLASS_B == 0 {
        return AuraSource::None;
    }

    match (
        flags & BIT_REFLECTION != 0,
        flags & BIT_TOXIC != 0,
        flags & BIT_PIPELINE_EXEMPT != 0,
    ) {
        (true, false, _) => AuraSource::ReflectionOfAFallenFort,
        (false, true, _) => AuraSource::ToxicBlast,
        (true, true, true) => AuraSource::MysteryBox,
        (true, true, false) => {
            if last_action == Some(ActionType::Normal(ICE_AND_FIRE)) {
                AuraSource::IceAndFireFollowUp
            } else {
                AuraSource::LusterOfDarkness
            }
        }
        (false, false, _) => AuraSource::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOXIC_BLAST: u64 = 0x1080100002020000;
    const REFLECTION: u64 = 0x1080100000060000;
    const MYSTERY_BOX: u64 = 0x1080100002040000;
    const LUSTER: u64 = 0x1000100002040000;
    const ORDINARY: u64 = 0x0000000002020000;

    const SENTINEL: ActionType = ActionType::Normal(CONFLUX_AURA_SENTINEL);

    #[test]
    fn names_each_captured_aura() {
        assert_eq!(
            aura_source_of(SENTINEL, TOXIC_BLAST, None),
            AuraSource::ToxicBlast
        );
        assert_eq!(
            aura_source_of(SENTINEL, REFLECTION, None),
            AuraSource::ReflectionOfAFallenFort
        );
        assert_eq!(
            aura_source_of(SENTINEL, MYSTERY_BOX, None),
            AuraSource::MysteryBox
        );
        assert_eq!(
            aura_source_of(SENTINEL, LUSTER, None),
            AuraSource::LusterOfDarkness
        );
    }

    #[test]
    fn an_ice_and_fire_tail_is_not_luster() {
        assert_eq!(
            aura_source_of(SENTINEL, LUSTER, Some(ActionType::Normal(ICE_AND_FIRE))),
            AuraSource::IceAndFireFollowUp
        );

        for prior in [
            ActionType::Normal(100007),
            ActionType::Normal(110),
            ActionType::LinkAttack,
            ActionType::SBA,
            ActionType::SupplementaryDamage(150),
            ActionType::DamageOverTime(0),
        ] {
            assert_eq!(
                aura_source_of(SENTINEL, LUSTER, Some(prior)),
                AuraSource::LusterOfDarkness,
                "prior action {prior:?} should not suppress Luster"
            );
        }
    }

    #[test]
    fn the_gate_applies_only_to_lusters_flag_word() {
        let prior = Some(ActionType::Normal(ICE_AND_FIRE));

        assert_eq!(
            aura_source_of(SENTINEL, MYSTERY_BOX, prior),
            AuraSource::MysteryBox
        );
        assert_eq!(
            aura_source_of(SENTINEL, REFLECTION, prior),
            AuraSource::ReflectionOfAFallenFort
        );
        assert_eq!(
            aura_source_of(SENTINEL, TOXIC_BLAST, prior),
            AuraSource::ToxicBlast
        );
    }

    #[test]
    fn ignores_every_action_but_the_sentinel() {
        for action in [
            ActionType::Normal(100009),
            ActionType::Normal(100011),
            ActionType::Normal(ICE_AND_FIRE),
            ActionType::Normal(0),
            ActionType::LinkAttack,
            ActionType::SBA,
            ActionType::SupplementaryDamage(CONFLUX_AURA_SENTINEL),
            ActionType::DamageOverTime(0),
        ] {
            assert_eq!(aura_source_of(action, TOXIC_BLAST, None), AuraSource::None);
        }
    }

    #[test]
    fn leaves_unrecognised_flags_unnamed() {
        assert_eq!(aura_source_of(SENTINEL, ORDINARY, None), AuraSource::None);
        assert_eq!(
            aura_source_of(SENTINEL, BIT_CLASS_A | BIT_CLASS_B, None),
            AuraSource::None
        );
        assert_eq!(
            aura_source_of(
                SENTINEL,
                BIT_CLASS_A | BIT_CLASS_B | BIT_PIPELINE_EXEMPT,
                None
            ),
            AuraSource::None
        );
    }

    #[test]
    fn supplementary_halves_are_not_classified() {
        let supp = ActionType::SupplementaryDamage(CONFLUX_AURA_SENTINEL);

        assert_eq!(
            aura_source_of(supp, TOXIC_BLAST | (1 << 15), None),
            AuraSource::None
        );
    }
}
