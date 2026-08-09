//! Names causes for SBA gauge gains the hook could not classify.

use std::collections::{HashMap, HashSet};

use protocol::{ActionType, Message, SbaCause};

use super::{counter_grant, is_damage_taken, retro, EffectiveTrait, Encounter};

/// The SBA chain contribution: 10% of the 1000 gauge max.
const CHAIN_GRANT: f32 = 100.0;

/// The chain contribution when the granter runs an Alpha sigil trait at Lv. 30+.
const CHAIN_GRANT_ALPHA: f32 = 130.0;

fn is_flat_grant_value(gain: f32) -> bool {
    (gain - CHAIN_GRANT).abs() < 0.01 || (gain - CHAIN_GRANT_ALPHA).abs() < 0.01
}

/// The join windows. `counter_ms` can be this wide because the grant size is
/// checked too.
#[derive(Debug, Clone, Copy)]
pub struct Windows {
    pub move_ms: i64,
    pub counter_ms: i64,
    pub learning_ms: i64,
    /// How far a gain may sit from the move that pays it, when the size names
    /// the move rather than the timing. Nearest hit measured 66-433 ms away.
    pub value_ms: i64,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            move_ms: 64,
            counter_ms: 250,
            learning_ms: 16,
            value_ms: 500,
        }
    }
}

/// Which rule named a gain, in priority order. Only scoring tooling reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Counter,
    Redistribute,
    PartyFill,
    FlatGrant,
    Move,
    MoveByValue,
    RetroCounter,
    DamageTaken,
    DamageTakenByValue,
    CappedGrant,
    SummonCall,
    FlatGrantFallback,
}

pub(super) fn infer_remote_causes(encounter: &Encounter) -> HashMap<usize, SbaCause> {
    infer_remote_causes_tagged(encounter, Windows::default())
        .into_iter()
        .map(|(index, (cause, _))| (index, cause))
        .collect()
}

pub fn infer_remote_causes_tagged(
    encounter: &Encounter,
    windows: Windows,
) -> HashMap<usize, (SbaCause, Rule)> {
    let Windows {
        move_ms: window_ms,
        counter_ms: counter_window_ms,
        learning_ms: learning_window_ms,
        value_ms: value_window_ms,
    } = windows;
    const GAUGE_MAX: f32 = 1000.0;
    /// A chain contribution lands 4-10 s after the granter's `OnPerformSBA`.
    const CHAIN_AFTER_SBA_MIN_MS: i64 = 4_000;
    const CHAIN_AFTER_SBA_MAX_MS: i64 = 11_000;

    let mut damage: HashMap<u32, Vec<(i64, ActionType)>> = HashMap::new();
    let mut counters: HashMap<u32, Vec<(i64, u32, f32)>> = HashMap::new();
    for (timestamp, event) in encounter.event_log() {
        if let Message::OnPeerCounter(counter) = event {
            let Some(base) = traits_of(encounter, counter.actor_index)
                .and_then(|traits| counter_grant::base_grant(traits, counter.action_id))
            else {
                continue;
            };
            counters.entry(counter.actor_index).or_default().push((
                *timestamp,
                counter.action_id,
                base,
            ));
        }
    }

    let stacked_allies = stacked_gauge_allies(encounter);
    let calibration = counter_grant::Calibration::solve(encounter.event_log().filter_map(
        |(timestamp, event)| {
            let Message::OnUpdateSBA(sba) = event else {
                return None;
            };
            if sba.cause != SbaCause::Remote || sba.sba_added <= 0.0 {
                return None;
            }
            counters
                .get(&sba.actor_index)?
                .iter()
                .find(|(t, _, _)| (t - timestamp).abs() <= learning_window_ms)
                .map(|(_, _, base)| (sba.sba_added, *base))
        },
    ))
    .map(|c| c.with_stacked_allies(stacked_allies));
    let counter_window = match calibration {
        Some(_) => counter_window_ms,
        None => learning_window_ms,
    };

    let mut performed_sba: HashMap<u32, Vec<i64>> = HashMap::new();
    for (timestamp, event) in encounter.event_log() {
        if let Message::OnPerformSBA(sba) = event {
            performed_sba
                .entry(sba.actor_index)
                .or_default()
                .push(*timestamp);
        }
    }

    let mut taken: HashMap<u32, Vec<i64>> = HashMap::new();
    for (timestamp, event) in encounter.event_log() {
        if let Message::DamageEvent(damage_event) = event {
            if is_damage_taken(damage_event) {
                taken
                    .entry(damage_event.target.parent_index)
                    .or_default()
                    .push(*timestamp);
            } else {
                damage
                    .entry(damage_event.source.parent_index)
                    .or_default()
                    .push((*timestamp, damage_event.action_id));
            }
        }
    }

    let redistributed = redistribute_credits(encounter);
    let party_fills = party_fill_credits(encounter);
    let retro = retro::detect(encounter, &damage);
    let mut credited: HashMap<(u32, i64, ActionType), u32> = HashMap::new();
    let mut counter_runs: HashMap<u32, Vec<i64>> = HashMap::new();
    let gauge_table = move_gauge_table(encounter, &damage, learning_window_ms, retro);

    let mut out = HashMap::new();
    for (index, (timestamp, event)) in encounter.event_log().enumerate() {
        let Message::OnUpdateSBA(sba) = event else {
            continue;
        };
        if sba.sba_added <= 0.0 {
            continue;
        }
        match sba.cause {
            SbaCause::Remote => {}
            // ChainGrant covers three mechanics that share one granter.
            SbaCause::ChainGrant if party_fills.contains(&index) => {
                out.insert(index, (SbaCause::InferredPartyFill, Rule::PartyFill));
                continue;
            }
            SbaCause::ChainGrant if redistributed.contains(&index) => {
                out.insert(index, (SbaCause::InferredRedistribute, Rule::Redistribute));
                continue;
            }
            SbaCause::NotClassified if retro.is_some() => {}
            SbaCause::Unknown | SbaCause::HookUnavailable => {}
            _ => continue,
        }

        if let Some(hits) = counters.get(&sba.actor_index) {
            let at_cap = (sba.sba_value - GAUGE_MAX).abs() < 0.01;
            if let Some((_, action, _)) = hits.iter().find(|(t, _, base)| {
                (t - timestamp).abs() <= counter_window
                    && calibration.map_or(true, |c| c.confirms(*base, sba.sba_added, at_cap))
            }) {
                out.insert(
                    index,
                    (
                        SbaCause::Inferred(ActionType::Normal(*action)),
                        Rule::Counter,
                    ),
                );
                continue;
            }
        }

        if redistributed.contains(&index) {
            out.insert(index, (SbaCause::InferredRedistribute, Rule::Redistribute));
            continue;
        }

        if party_fills.contains(&index) {
            out.insert(index, (SbaCause::InferredPartyFill, Rule::PartyFill));
            continue;
        }

        let chained = performed_sba.iter().any(|(actor, times)| {
            *actor != sba.actor_index
                && times.iter().any(|t| {
                    (timestamp - t) >= CHAIN_AFTER_SBA_MIN_MS
                        && (timestamp - t) <= CHAIN_AFTER_SBA_MAX_MS
                })
        });
        let flat_grant_value = is_flat_grant_value(sba.sba_added);
        let at_cap = (sba.sba_value - GAUGE_MAX).abs() < 0.01;

        // The value alone is not enough: a summon call also pays 10% of max,
        // the same 100.00 below Chaos tier.
        if flat_grant_value && chained {
            out.insert(index, (SbaCause::InferredChainGrant, Rule::FlatGrant));
            continue;
        }

        let hits: &[(i64, ActionType)] = damage.get(&sba.actor_index).map_or(&[], Vec::as_slice);
        // Duplicates are kept: the number of landings bounds the fold below.
        let in_window: Vec<(i64, ActionType)> = hits
            .iter()
            .filter(|(t, action)| (t - timestamp).abs() <= window_ms && generates_gauge(action))
            .copied()
            .collect();
        let mut candidates: Vec<ActionType> = in_window.iter().map(|(_, action)| *action).collect();
        candidates.sort();
        candidates.dedup();
        let landed = |action: ActionType| in_window.iter().filter(|(_, a)| *a == action).count();
        let already_credited = |action: ActionType| {
            in_window
                .iter()
                .filter(|(_, a)| *a == action)
                .map(|(t, a)| {
                    credited
                        .get(&(sba.actor_index, *t, *a))
                        .copied()
                        .unwrap_or(0)
                })
                .min()
                .unwrap_or(0)
        };
        let consistent = |action: ActionType| {
            gauge_is_consistent(
                &gauge_table,
                sba.actor_index,
                action,
                sba.sba_added,
                landed(action),
                at_cap,
            )
        };

        match candidates[..] {
            [action] => {
                if consistent(action) != Some(false) {
                    out.insert(index, (SbaCause::Inferred(action), Rule::Move));
                    continue;
                }
            }
            [_, _, ..] => {
                let mut fits: Vec<ActionType> = candidates
                    .iter()
                    .copied()
                    .filter(|a| consistent(*a) == Some(true))
                    .collect();
                fits.sort_by_key(|a| already_credited(*a));
                let chosen = match fits[..] {
                    [action] => Some(action),
                    [first, second, ..] => {
                        (already_credited(first) < already_credited(second)).then_some(first)
                    }
                    [] => None,
                };
                if let Some(action) = chosen {
                    if let Some((t, a)) = in_window.iter().find(|(_, a)| *a == action) {
                        *credited.entry((sba.actor_index, *t, *a)).or_default() += 1;
                    }
                    out.insert(index, (SbaCause::Inferred(action), Rule::Move));
                    continue;
                }
            }
            [] => {}
        }

        if let Some(tier) = retro.and_then(|r| r.tier()) {
            let build = traits_of(encounter, sba.actor_index).unwrap_or(&[]);
            let fits: Vec<u32> = counter_grant::base_grants(build)
                .into_iter()
                .filter(|(_, base)| {
                    tier.confirms(*base, sba.sba_added, false)
                })
                .map(|(action, _)| action)
                .collect();
            let action = match fits[..] {
                [action] => Some(action),
                [_, _, ..] => {
                    let recent = counter_runs
                        .get(&sba.actor_index)
                        .map(|times| {
                            times
                                .iter()
                                .filter(|t| {
                                    (timestamp - *t).abs()
                                        <= counter_grant::Calibration::COUNTER_RUN_MS
                                })
                                .count()
                        })
                        .unwrap_or(0);
                    Some(counter_grant::Calibration::counter_kind(recent))
                }
                [] => None,
            };
            if let Some(action) = action {
                counter_runs
                    .entry(sba.actor_index)
                    .or_default()
                    .push(*timestamp);
                out.insert(
                    index,
                    (
                        SbaCause::Inferred(ActionType::Normal(action)),
                        Rule::RetroCounter,
                    ),
                );
                continue;
            }
        }

        let took_a_hit = taken
            .get(&sba.actor_index)
            .is_some_and(|times| times.iter().any(|t| (t - timestamp).abs() <= window_ms));
        if took_a_hit {
            out.insert(index, (SbaCause::InferredDamageTaken, Rule::DamageTaken));
            continue;
        }

        if let Some(tier) = retro.and_then(|r| r.tier()).or(calibration) {
            let build = traits_of(encounter, sba.actor_index).unwrap_or(&[]);
            if tier.confirms(
                counter_grant::damage_taken_base(build),
                sba.sba_added,
                false,
            ) {
                out.insert(
                    index,
                    (SbaCause::InferredDamageTaken, Rule::DamageTakenByValue),
                );
                continue;
            }
        }

        if at_cap && chained {
            out.insert(index, (SbaCause::InferredChainGrant, Rule::CappedGrant));
            continue;
        }

        if !chained && calibration.is_some_and(|c| c.is_summon_call(sba.sba_added)) && !at_cap {
            out.insert(index, (SbaCause::InferredSummonCall, Rule::SummonCall));
            continue;
        }

        // Name the move by the gauge it pays, over a window too wide for the
        // timing alone. That demands a move that has provably paid exactly this,
        // and the fold is pinned at k = 1.
        if value_window_ms > window_ms {
            let wide: Vec<ActionType> = hits
                .iter()
                .filter(|(t, action)| {
                    (t - timestamp).abs() <= value_window_ms && generates_gauge(action)
                })
                .map(|(_, action)| *action)
                .collect();
            let mut named: Vec<ActionType> = wide
                .iter()
                .copied()
                .filter(|action| {
                    gauge_is_consistent(
                        &gauge_table,
                        sba.actor_index,
                        *action,
                        sba.sba_added,
                        1,
                        at_cap,
                    ) == Some(true)
                })
                .collect();
            named.sort();
            named.dedup();
            if let [action] = named[..] {
                out.insert(index, (SbaCause::Inferred(action), Rule::MoveByValue));
                continue;
            }
        }

        if flat_grant_value {
            out.insert(
                index,
                (SbaCause::InferredChainGrant, Rule::FlatGrantFallback),
            );
        }
    }

    out
}

/// The mechanic grants no gauge for supplementary damage or damage over time.
/// `Normal(u32::MAX)` is the engine's unset action id, shared by several
/// mechanics, so it is not one move and the value table does not hold for it.
fn generates_gauge(action: &ActionType) -> bool {
    !matches!(
        action,
        ActionType::SupplementaryDamage(_)
            | ActionType::DamageOverTime(_)
            | ActionType::Normal(u32::MAX)
    )
}

fn traits_of(encounter: &Encounter, actor_index: u32) -> Option<&[EffectiveTrait]> {
    encounter
        .player_data
        .iter()
        .flatten()
        .find(|player| player.actor_index == actor_index)
        .map(|player| player.effective_traits.as_slice())
        .filter(|traits| !traits.is_empty())
}

fn stacked_gauge_allies(encounter: &Encounter) -> usize {
    encounter
        .player_data
        .iter()
        .flatten()
        .filter(|player| {
            player
                .master_trait_flags()
                .iter()
                .any(|flag| flag.hash == counter_grant::BUTTERFLY_SBA_GAIN && flag.on)
        })
        .count()
}

/// What each move has been seen to grant, learned only from gains whose tight
/// window holds exactly one move. Per-action gain is a constant on the attack
/// param (`+0x340`, beside the action id at `+0x3B8`), scaled by the player's
/// own modifiers.
fn move_gauge_table(
    encounter: &Encounter,
    damage: &HashMap<u32, Vec<(i64, ActionType)>>,
    window_ms: i64,
    retro: Option<retro::Retro>,
) -> HashMap<(u32, ActionType), Vec<f32>> {
    let mut table: HashMap<(u32, ActionType), Vec<f32>> = HashMap::new();
    for (timestamp, event) in encounter.event_log() {
        let Message::OnUpdateSBA(sba) = event else {
            continue;
        };
        let eligible = match sba.cause {
            SbaCause::Remote | SbaCause::Unknown | SbaCause::HookUnavailable => true,
            SbaCause::NotClassified => retro.is_some(),
            _ => false,
        };
        if !eligible || sba.sba_added <= 0.0 || is_flat_grant_value(sba.sba_added) {
            continue;
        }
        let Some(hits) = damage.get(&sba.actor_index) else {
            continue;
        };
        let mut near: Vec<ActionType> = hits
            .iter()
            .filter(|(t, _)| (t - timestamp).abs() <= window_ms)
            .map(|(_, action)| *action)
            .filter(generates_gauge)
            .collect();
        near.sort();
        near.dedup();
        if let [action] = near[..] {
            let seen = table.entry((sba.actor_index, action)).or_default();
            if !seen.iter().any(|v| (v - sba.sba_added).abs() < 0.005) {
                seen.push(sba.sba_added);
            }
        }
    }
    table
}

fn gauge_is_consistent(
    table: &HashMap<(u32, ActionType), Vec<f32>>,
    actor: u32,
    action: ActionType,
    gain: f32,
    landed: usize,
    at_cap: bool,
) -> Option<bool> {
    /// The game quantises grants to 0.01.
    const EPSILON: f32 = 0.02;

    let known = table.get(&(actor, action))?;
    if known.is_empty() || landed == 0 {
        return None;
    }
    Some(known.iter().any(|base| {
        (1..=landed).any(|k| {
            let folded = base * k as f32;
            if at_cap {
                folded >= gain - EPSILON
            } else {
                (folded - gain).abs() < EPSILON
            }
        })
    }))
}

/// Names a party-wide gauge fill — every member topped to the cap in one burst
/// — and returns the log index of each gain that made one up.
///
/// Shares the `ChainGrant` granter with the chain contribution and the
/// redistribute credit, and is told apart by shape: a chain contribution cannot
/// move a bar by more than 130.00, and a redistribute MOVES gauge so its caster
/// ends short. Both clauses are needed — each alone matches something innocent.
///
/// Fails closed on an incomplete roster: a dead member takes no fill, so the
/// whole-roster test fails and the burst goes unnamed.
fn party_fill_credits(encounter: &Encounter) -> HashSet<usize> {
    const GAUGE_MAX: f32 = 1000.0;
    const MAX_CHAIN_GRANT: f32 = 130.0;
    const EPSILON: f32 = 0.02;
    const AT_CAP: f32 = 0.01;
    /// Half-width around the fill's signature gain, not a forward window from
    /// an arbitrary first row: the rows are spread by cross-client jitter, so a
    /// forward window reads the same fill differently depending on where it
    /// starts. Covers both observed fills (60 ms and 283 ms) with margin.
    const BURST_MS: i64 = 500;

    let roster: HashSet<u32> = encounter
        .player_data
        .iter()
        .flatten()
        .map(|player| player.actor_index)
        .collect();
    if roster.is_empty() {
        return HashSet::new();
    }

    let mut rows: Vec<(usize, i64, u32, f32, f32)> = Vec::new();
    for (index, (timestamp, event)) in encounter.event_log().enumerate() {
        if let Message::OnUpdateSBA(sba) = event {
            rows.push((
                index,
                *timestamp,
                sba.actor_index,
                sba.sba_value,
                sba.sba_added,
            ));
        }
    }

    let at_cap = |level: f32| (level - GAUGE_MAX).abs() < AT_CAP;

    let mut out = HashSet::new();
    // Anchored on the gain no chain contribution could have paid: the one row a
    // fill always produces, so the same burst is found from any direction.
    for (_, anchor_at, _, value, added) in rows.iter().copied() {
        if !at_cap(value) || added <= MAX_CHAIN_GRANT + EPSILON {
            continue;
        }

        let burst: Vec<&(usize, i64, u32, f32, f32)> = rows
            .iter()
            .filter(|(_, t, _, level, _)| (t - anchor_at).abs() <= BURST_MS && at_cap(*level))
            .collect();

        // Each actor's last level at or before the burst ends, not a row from
        // everyone: a member already at the cap has nothing to sync and emits
        // no row. Reading the last level also excludes one who spent the bar
        // inside the window.
        let filled: HashSet<u32> = roster
            .iter()
            .copied()
            .filter(|actor| {
                rows.iter()
                    .rev()
                    .find(|(_, t, a, _, _)| a == actor && *t <= anchor_at + BURST_MS)
                    .is_some_and(|(_, _, _, level, _)| at_cap(*level))
            })
            .collect();
        if !roster.is_subset(&filled) {
            continue;
        }

        out.extend(
            burst
                .iter()
                .filter(|(_, _, _, _, added)| *added > 0.0)
                .map(|(index, _, _, _, _)| *index),
        );
    }

    out
}

/// Charlotta's redistribute, the one mechanic that moves gauge between
/// players: she spends `min(300, held)` and every other ally gets
/// `spent / 3 + 30`, ~1.4 s later, arriving as a plain `ChainGrant`.
fn redistribute_credits(encounter: &Encounter) -> HashSet<usize> {
    const GAUGE_MAX: f32 = 1000.0;
    const MAX_SPEND: f32 = 300.0;
    const FLAT_BONUS: f32 = 30.0;
    const RECIPIENTS: f32 = 3.0;
    const EPSILON: f32 = 0.02;
    /// The caster's own drop only surfaces at her next gain.
    const LEAD_MIN_MS: i64 = -400;
    const LEAD_MAX_MS: i64 = 2200;

    let mut rows: Vec<(usize, i64, u32, f32, f32)> = Vec::new();
    let mut casts: Vec<(i64, u32, f32)> = Vec::new();
    let mut last_level: HashMap<u32, f32> = HashMap::new();

    for (index, (timestamp, event)) in encounter.event_log().enumerate() {
        let Message::OnUpdateSBA(sba) = event else {
            continue;
        };
        rows.push((
            index,
            *timestamp,
            sba.actor_index,
            sba.sba_value,
            sba.sba_added,
        ));

        let before = sba.sba_value - sba.sba_added;
        if let Some(previous) = last_level.get(&sba.actor_index) {
            let spent = previous - before;
            // A cast spends the full 300 or drains the bar; nothing else
            // matches (an SBA use costs the whole 1000, a failed attempt 200).
            let drained = before <= 0.005;
            if spent > 0.5
                && spent <= MAX_SPEND + EPSILON
                && ((spent - MAX_SPEND).abs() < EPSILON || drained)
            {
                casts.push((*timestamp, sba.actor_index, spent));
            }
        }
        last_level.insert(sba.actor_index, sba.sba_value);
    }

    let mut out = HashSet::new();
    for (cast_at, caster, spent) in casts {
        let share = spent / RECIPIENTS + FLAT_BONUS;
        let in_window = |t: i64| (t - cast_at) >= LEAD_MIN_MS && (t - cast_at) <= LEAD_MAX_MS;

        let credits: Vec<(usize, u32)> = rows
            .iter()
            .filter(|(_, t, actor, _, added)| {
                in_window(*t) && *actor != caster && (added - share).abs() < EPSILON
            })
            .map(|(index, _, actor, _, _)| (*index, *actor))
            .collect();
        if credits.is_empty() {
            continue;
        }

        // A share of 100 or 130 reads as a chain grant, so demand a second
        // recipient (an ally already at the cap counts, via a zero-gain row).
        let collides = (share - 100.0).abs() < EPSILON || (share - 130.0).abs() < EPSILON;
        if collides {
            let mut recipients: HashSet<u32> =
                credits.iter().map(|(_, actor)| *actor).collect();
            recipients.extend(rows.iter().filter_map(|(_, t, actor, value, added)| {
                (in_window(*t)
                    && *actor != caster
                    && *added == 0.0
                    && (value - GAUGE_MAX).abs() < 0.01)
                    .then_some(*actor)
            }));
            if recipients.len() < 2 {
                continue;
            }
        }

        out.extend(credits.into_iter().map(|(index, _)| index));
    }

    out
}
