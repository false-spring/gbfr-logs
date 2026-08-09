//! SBA attribution for logs written before the attribution hook existed.

use std::collections::HashMap;

use protocol::{ActionType, Message, SbaCause};

use super::{counter_grant::Calibration, Encounter};

#[derive(Debug, Clone, Copy)]
pub(super) struct Retro {
    tier: Option<Calibration>,
}

impl Retro {
    pub(super) fn tier(&self) -> Option<Calibration> {
        self.tier
    }
}

/// Returns `None` for any modern log: the hook classified it, so there is nothing to recover.
pub(super) fn detect(
    encounter: &Encounter,
    damage: &HashMap<u32, Vec<(i64, ActionType)>>,
) -> Option<Retro> {
    let unclassified = encounter.event_log().all(|(_, event)| match event {
        Message::OnUpdateSBA(sba) => sba.cause == SbaCause::NotClassified,
        _ => true,
    });
    if !unclassified {
        return None;
    }

    let stacked = encounter
        .player_data
        .iter()
        .flatten()
        .filter(|player| {
            player
                .master_trait_flags()
                .iter()
                .any(|flag| flag.hash == super::counter_grant::BUTTERFLY_SBA_GAIN && flag.on)
        })
        .count();

    Some(Retro {
        tier: choose_tier(encounter, damage, stacked),
    })
}

/// Picks the quest-tier scalar (`AddRateChaos`: 0.7 at Chaos and above, 1.0
/// below, not on the wire) by trying both and seeing which explains more
/// otherwise-unexplained gauge. Ties and shutouts yield `None`.
fn choose_tier(
    encounter: &Encounter,
    damage: &HashMap<u32, Vec<(i64, ActionType)>>,
    stacked: usize,
) -> Option<Calibration> {
    const WINDOW_MS: i64 = 64;

    let traits: HashMap<u32, &[super::EffectiveTrait]> = encounter
        .player_data
        .iter()
        .flatten()
        .map(|player| (player.actor_index, player.effective_traits.as_slice()))
        .collect();

    let mut best: Option<(f32, Calibration)> = None;
    for tier in Calibration::CANDIDATES {
        let calibration = Calibration::assumed(tier).with_stacked_allies(stacked);
        let mut explained = 0.0f32;
        for (timestamp, event) in encounter.event_log() {
            let Message::OnUpdateSBA(sba) = event else {
                continue;
            };
            if sba.sba_added <= 0.0 {
                continue;
            }
            let has_move = damage
                .get(&sba.actor_index)
                .is_some_and(|hits| hits.iter().any(|(t, _)| (t - timestamp).abs() <= WINDOW_MS));
            if has_move {
                continue;
            }
            let Some(build) = traits.get(&sba.actor_index) else {
                continue;
            };
            let counter = super::counter_grant::base_grants(build)
                .into_iter()
                .any(|(_, base)| calibration.confirms(base, sba.sba_added, false));
            let taken = calibration.confirms(
                super::counter_grant::damage_taken_base(build),
                sba.sba_added,
                false,
            );
            if counter || taken {
                explained += sba.sba_added;
            }
        }
        if explained > 0.0 && best.map_or(true, |(most, _)| explained > most) {
            best = Some((explained, calibration));
        }
    }
    best.map(|(_, calibration)| calibration)
}
