//! Gauge a peer's perfect guard or perfect dodge grants them, as a closed
//! form: `grant = pct(trait, level) * 10 * tier * (1 + uplift_pct(level) / 100)`.

use super::EffectiveTrait;

/// Precise Wrath, `SKILL_109_00`.
pub(super) const PRECISE_WRATH: u32 = 0x7EDD_69D0;
/// Nimble Onslaught, `SKILL_106_00`.
pub(super) const NIMBLE_ONSLAUGHT: u32 = 0xD2C8_E10A;
/// Uplift, `SKILL_072_00`.
pub(super) const UPLIFT: u32 = 0xB5FF_9FD3;

pub(super) const PERFECT_GUARD: u32 = 611;
pub(super) const PERFECT_DODGE: u32 = 610;

/// Percent of max gauge per counter, by trait level; both traits share the curve.
const COUNTER_PCT: [f32; 30] = [
    0.5, 0.8, 1.1, 1.4, 1.5, 1.8, 2.0, 2.1, 2.3, 2.5, 2.7, 2.9, 3.1, 3.3, 3.5, 3.6, 3.7, 3.8, 3.9,
    4.0, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 5.0,
];

/// Uplift's percentage bonus by level.
const UPLIFT_PCT: [f32; 45] = [
    5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0,
    13.5, 14.0, 14.5, 15.0, 15.5, 16.0, 16.5, 17.0, 17.5, 18.0, 18.5, 19.0, 19.5, 20.0, 20.5, 21.0,
    21.5, 22.0, 22.5, 23.0, 23.5, 24.0, 24.5, 26.0, 27.0, 28.0, 29.0, 30.0,
];

const DAMAGE_HIT_ADD_POINT: f32 = 10.0;

fn level_of(traits: &[EffectiveTrait], hash: u32) -> Option<u32> {
    traits.iter().find(|t| t.hash == hash).map(|t| t.level)
}

fn build_modifier(traits: &[EffectiveTrait]) -> f32 {
    let uplift = level_of(traits, UPLIFT).map_or(0.0, |l| at_level(&UPLIFT_PCT, l));
    1.0 + uplift / 100.0
}

/// Gauge from taking a hit is flat, whatever the damage was.
pub(super) fn damage_taken_base(traits: &[EffectiveTrait]) -> f32 {
    DAMAGE_HIT_ADD_POINT * build_modifier(traits)
}

pub(super) fn base_grants(traits: &[EffectiveTrait]) -> Vec<(u32, f32)> {
    [PERFECT_GUARD, PERFECT_DODGE]
        .into_iter()
        .filter_map(|action| base_grant(traits, action).map(|grant| (action, grant)))
        .collect()
}

fn at_level(curve: &[f32], level: u32) -> f32 {
    let index = (level.max(1) as usize - 1).min(curve.len() - 1);
    curve[index]
}

pub(super) fn base_grant(traits: &[EffectiveTrait], action_id: u32) -> Option<f32> {
    let gate = match action_id {
        PERFECT_GUARD => PRECISE_WRATH,
        PERFECT_DODGE => NIMBLE_ONSLAUGHT,
        _ => return None,
    };
    let pct = at_level(&COUNTER_PCT, level_of(traits, gate)?);
    let uplift = level_of(traits, UPLIFT).map_or(0.0, |l| at_level(&UPLIFT_PCT, l));
    Some(pct * 10.0 * (1.0 + uplift / 100.0))
}

/// Narmaya's `SBE_PL1400_CM133` node: "at butterfly count 6, SBA gauge gain
/// +10%". Despite the text it scales the whole party's gauge, not just hers.
pub(super) const BUTTERFLY_SBA_GAIN: u32 = 0x73CF_906D;

const BUTTERFLY_STEP: f32 = 0.1;

/// The quest-tier scalar, solved as the modal observed/predicted counter ratio.
#[derive(Debug, Clone, Copy)]
pub(super) struct Calibration {
    scale: f32,
    stacked: u8,
}

impl Calibration {
    const VOTE_QUANTUM: f32 = 1000.0;
    const EPSILON: f32 = 0.02;
    /// `AddRateChaos`: 0.7 at Chaos tier and above, 1.0 below.
    pub(super) const CANDIDATES: [f32; 2] = [0.7, 1.0];

    pub(super) fn assumed(tier: f32) -> Self {
        Self {
            scale: tier,
            stacked: 0,
        }
    }

    pub(super) fn with_stacked_allies(self, stacked: usize) -> Self {
        Self {
            stacked: stacked.min(u8::MAX as usize) as u8,
            ..self
        }
    }

    fn factors(&self) -> impl Iterator<Item = f32> + '_ {
        let levels = 0..=u32::from(self.stacked);
        levels.clone().flat_map(move |from| {
            let levels = levels.clone();
            levels.map(move |to| {
                (1.0 + BUTTERFLY_STEP * to as f32) / (1.0 + BUTTERFLY_STEP * from as f32)
            })
        })
    }

    /// A lone counter reads as a dodge; several within five seconds as blocks.
    pub(super) fn counter_kind(recent_counters: usize) -> u32 {
        if recent_counters > 0 {
            PERFECT_GUARD
        } else {
            PERFECT_DODGE
        }
    }

    pub(super) const COUNTER_RUN_MS: i64 = 5_000;

    pub(super) fn solve(samples: impl Iterator<Item = (f32, f32)>) -> Option<Self> {
        let mut votes: Vec<(i64, u32)> = Vec::new();
        for (observed, base) in samples {
            if base <= 0.0 {
                continue;
            }
            let key = (observed / base * Self::VOTE_QUANTUM).round() as i64;
            match votes.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => votes.push((key, 1)),
            }
        }
        votes
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(key, _)| Self {
                scale: key as f32 / Self::VOTE_QUANTUM,
                stacked: 0,
            })
    }

    /// A summon call grants 10% of max scaled by the tier. Below Chaos that
    /// equals the chain contribution, so the caller must rule a chain out.
    pub(super) fn is_summon_call(&self, observed: f32) -> bool {
        const SUMMON_CALL_BASE: f32 = 100.0;

        self.factors()
            .any(|f| (SUMMON_CALL_BASE * self.scale * f - observed).abs() < Self::EPSILON)
    }

    pub(super) fn confirms(&self, base: f32, observed: f32, at_cap: bool) -> bool {
        self.factors().any(|factor| {
            let expected = base * self.scale * factor;
            if at_cap {
                observed <= expected + Self::EPSILON
            } else {
                (expected - observed).abs() < Self::EPSILON
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traits(pairs: &[(u32, u32)]) -> Vec<EffectiveTrait> {
        pairs
            .iter()
            .map(|(hash, level)| EffectiveTrait {
                hash: *hash,
                level: *level,
            })
            .collect()
    }

    #[test]
    fn the_closed_form_reproduces_the_paired_capture() {
        let beatrix = traits(&[(PRECISE_WRATH, 16)]);
        assert!((base_grant(&beatrix, PERFECT_GUARD).unwrap() * 0.7 - 25.20).abs() < 0.001);

        let charlotta = traits(&[(NIMBLE_ONSLAUGHT, 16), (UPLIFT, 48)]);
        assert!((base_grant(&charlotta, PERFECT_DODGE).unwrap() * 0.7 - 32.76).abs() < 0.001);

        let sandalphon = traits(&[(NIMBLE_ONSLAUGHT, 29), (UPLIFT, 32)]);
        assert!((base_grant(&sandalphon, PERFECT_DODGE).unwrap() * 0.7 - 41.3315).abs() < 0.001);
    }

    #[test]
    fn a_counter_without_its_trait_grants_nothing() {
        let no_traits = traits(&[]);
        assert!(base_grant(&no_traits, PERFECT_GUARD).is_none());
        assert!(base_grant(&no_traits, PERFECT_DODGE).is_none());

        let guard_only = traits(&[(PRECISE_WRATH, 20)]);
        assert!(base_grant(&guard_only, PERFECT_GUARD).is_some());
        assert!(base_grant(&guard_only, PERFECT_DODGE).is_none());
    }

    #[test]
    fn a_level_past_the_end_of_the_curve_clamps() {
        let capped = traits(&[(PRECISE_WRATH, 99), (UPLIFT, 99)]);
        let maxed = traits(&[(PRECISE_WRATH, 30), (UPLIFT, 45)]);
        assert_eq!(base_grant(&capped, PERFECT_GUARD), base_grant(&maxed, PERFECT_GUARD));
    }

    #[test]
    fn the_tier_scalar_is_the_modal_ratio() {
        let samples = [(25.2, 36.0), (25.2, 36.0), (25.2, 36.0), (18.0, 36.0)];
        let calibration = Calibration::solve(samples.into_iter()).unwrap();
        assert!(calibration.confirms(36.0, 25.2, false));
        assert!(!calibration.confirms(36.0, 18.0, false));
    }

    #[test]
    fn a_grant_clipped_by_the_cap_may_come_up_short() {
        let calibration = Calibration::solve(std::iter::once((25.2, 36.0))).unwrap();
        assert!(calibration.confirms(36.0, 21.5294, true));
        assert!(calibration.confirms(36.0, 3.5793, true));
        assert!(!calibration.confirms(36.0, 3.5793, false));
        assert!(!calibration.confirms(36.0, 30.0, true));
    }

    #[test]
    fn nothing_to_learn_from_yields_no_calibration() {
        assert!(Calibration::solve(std::iter::empty()).is_none());
    }

    #[test]
    fn a_lone_counter_reads_as_a_dodge_and_a_run_as_blocks() {
        assert_eq!(Calibration::counter_kind(0), PERFECT_DODGE);
        assert_eq!(Calibration::counter_kind(1), PERFECT_GUARD);
        assert_eq!(Calibration::counter_kind(4), PERFECT_GUARD);
    }

    #[test]
    fn the_take_hit_gain_follows_the_same_build_modifier() {
        assert!((damage_taken_base(&traits(&[])) * 0.7 - 7.0).abs() < 0.001);
        assert!((damage_taken_base(&traits(&[(UPLIFT, 48)])) * 0.7 - 9.1).abs() < 0.001);
    }

    #[test]
    fn base_grants_lists_every_counter_the_build_pays_for() {
        assert_eq!(base_grants(&traits(&[])).len(), 0);
        assert_eq!(base_grants(&traits(&[(PRECISE_WRATH, 20)])).len(), 1);
        let both = base_grants(&traits(&[(PRECISE_WRATH, 20), (NIMBLE_ONSLAUGHT, 20)]));
        assert_eq!(both.len(), 2);
        assert!((both[0].1 - both[1].1).abs() < 0.001, "same level, same grant");
    }
}
