//! Whether a `5000`/`5010` hit came from the shared ether gun or from the
//! character's own skill of the same id.
//!
//! Action ID `5000`/`5010` is used by `Pl0400`, `Pl2200`, `Pl2600`, `Pl2800`
//! and `Pl2900`. Disambiguate by bit 24 of the flags qword.

use protocol::ActionType;

/// Ether Round -- the ether gun's basic shot, `skills.default.5000`.
const ETHER_ROUND: u32 = 5000;
/// Charged Shot -- its charged shot, `skills.default.5010`.
const CHARGED_SHOT: u32 = 5010;

/// Set on every ether-gun hit and on no other action; see the module docs.
const BIT_ETHER_GUN: u64 = 1 << 24;

/// True when this hit is the shared ether gun rather than the character's own
/// skill at the same id.
pub(super) fn is_ether_gun(action: ActionType, flags: u64) -> bool {
    if !matches!(
        action,
        ActionType::Normal(ETHER_ROUND) | ActionType::Normal(CHARGED_SHOT)
    ) {
        return false;
    }

    flags & BIT_ETHER_GUN != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real flag words: the gun's two, and three characters' own `5000`.
    const GUN_PLAIN: u64 = 0x0000_1000_0100_0000;
    const GUN_ALT: u64 = 0x0000_1000_0500_0000;
    const HEARTS_ON_FIRE: u64 = 0x0000_0000_0002_0800;
    const DEAD_LANDS: u64 = 0x0000_0000_0002_0A00;
    const PRISION_DE_ARMAS: u64 = 0x0000_0000_0202_0004;

    #[test]
    fn the_guns_own_shots_are_recognised() {
        assert!(is_ether_gun(ActionType::Normal(ETHER_ROUND), GUN_PLAIN));
        assert!(is_ether_gun(ActionType::Normal(ETHER_ROUND), GUN_ALT));
        assert!(is_ether_gun(ActionType::Normal(CHARGED_SHOT), GUN_PLAIN));
        assert!(is_ether_gun(ActionType::Normal(CHARGED_SHOT), GUN_ALT));
    }

    #[test]
    fn a_characters_own_skill_at_the_same_id_is_not_the_gun() {
        assert!(!is_ether_gun(
            ActionType::Normal(ETHER_ROUND),
            HEARTS_ON_FIRE
        ));
        assert!(!is_ether_gun(ActionType::Normal(ETHER_ROUND), DEAD_LANDS));
        assert!(!is_ether_gun(
            ActionType::Normal(ETHER_ROUND),
            PRISION_DE_ARMAS
        ));
        assert!(!is_ether_gun(ActionType::Normal(CHARGED_SHOT), DEAD_LANDS));
    }

    #[test]
    fn the_bit_is_only_ever_read_for_these_two_ids() {
        // If the bit ever shows up elsewhere it must not rename that move.
        for action in [
            ActionType::Normal(0),
            ActionType::Normal(100),
            ActionType::Normal(4999),
            ActionType::Normal(5001),
            ActionType::Normal(5011),
            ActionType::Normal(80000),
            ActionType::Normal(99999),
            ActionType::LinkAttack,
            ActionType::SBA,
            ActionType::SupplementaryDamage(ETHER_ROUND),
            ActionType::DamageOverTime(ETHER_ROUND),
        ] {
            assert!(!is_ether_gun(action, GUN_PLAIN), "{action:?}");
        }
    }

    #[test]
    fn zero_flags_never_look_like_the_gun() {
        assert!(!is_ether_gun(ActionType::Normal(ETHER_ROUND), 0));
        assert!(!is_ether_gun(ActionType::Normal(CHARGED_SHOT), 0));
    }
}
