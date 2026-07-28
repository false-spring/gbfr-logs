use protocol::{ActionType, Actor, SbaCause};

use super::*;

#[test]
fn can_create_parser() {
    let parser = Parser::default();

    assert_eq!(parser.status, ParserStatus::Waiting);
    assert_eq!(parser.start_time(), 1);
}

fn player_damage_event() -> DamageEvent {
    DamageEvent {
        source: Actor {
            index: 0,
            actor_type: 0x26A4_848A, // Pl0000
            parent_actor_type: 0x26A4_848A,
            parent_index: 0,
        },
        target: Actor {
            index: 1,
            actor_type: 0,
            parent_actor_type: 0,
            parent_index: 1,
        },
        damage: 100,
        flags: 0,
        action_id: ActionType::Normal(0),
        attack_rate: None,
        stun_value: None,
        damage_cap: None,
        stun_fill: None,
        target_base_type: None,
        stun_max: None,
    }
}

#[test]
fn a_roster_seeds_slots_so_a_hit_joins_without_any_identity_event() {
    let mut parser = Parser::default();

    parser.on_party_roster_event(protocol::PartyRosterEvent {
        members: vec![protocol::RosterMember {
            party_index: 1,
            character_type: 0x6FDD_6932, // Pl1800, Cagliostro
            display_name: std::ffi::CString::new("Lvl 1 Crab").unwrap(),
            network_user_name: None,
            network_user_id: None,
            is_online: true,
        }],
    });

    let seeded = parser.encounter.player_data[1]
        .as_ref()
        .expect("roster must seed its slot");
    assert_eq!(seeded.display_name, "Lvl 1 Crab");
    assert_eq!(seeded.character_type, CharacterType::Pl1800);
    assert_eq!(seeded.actor_index, protocol::PLAYER_ID_BASE | 1);

    let mut hit = player_damage_event();
    hit.source.index = protocol::PLAYER_ID_BASE | 1;
    hit.source.parent_index = protocol::PLAYER_ID_BASE | 1;
    hit.source.actor_type = 0x6FDD_6932;
    hit.source.parent_actor_type = 0x6FDD_6932;
    parser.on_damage_event(hit);

    let row = parser
        .derived_state
        .party
        .get(&(protocol::PLAYER_ID_BASE | 1))
        .expect("the hit must open a row under the slot id");
    assert_eq!(row.index, parser.encounter.player_data[1].as_ref().unwrap().actor_index);
}

fn sba_update(actor_index: u32, sba_value: f32, sba_added: f32, cause: SbaCause) -> OnUpdateSBAEvent {
    OnUpdateSBAEvent {
        actor_index,
        sba_value,
        sba_added,
        cause,
    }
}

fn breakdown_for(parser: &Parser, actor: u32) -> Vec<(SbaCause, u32, f64)> {
    parser.derived_state.party[&actor]
        .sba_breakdown
        .iter()
        .map(|s| (s.cause, s.ticks, s.total_sba_added))
        .collect()
}

/// Gauge arrives as f32 and is accumulated as f64, so compare at the reported scale, not exactly.
fn assert_gauge(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "gauge {actual} != {expected}"
    );
}

fn assert_row(actual: &(SbaCause, u32, f64), cause: SbaCause, ticks: u32, gauge: f64) {
    assert_eq!(actual.0, cause);
    assert_eq!(actual.1, ticks);
    assert_gauge(actual.2, gauge);
}

#[test]
fn sba_gains_fold_by_cause_and_keep_the_total_consistent() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    let hit = SbaCause::Action(ActionType::Normal(210));
    let block = SbaCause::Action(ActionType::Normal(611));

    parser.on_sba_update(sba_update(0, 10.0, 10.0, hit));
    parser.on_sba_update(sba_update(0, 35.2, 25.2, block));
    parser.on_sba_update(sba_update(0, 40.2, 5.0, hit));
    parser.on_sba_update(sba_update(0, 42.2, 2.0, SbaCause::Remote));

    let rows = breakdown_for(&parser, 0);
    assert_eq!(rows.len(), 3, "expected one row per distinct cause: {rows:?}");
    assert_row(&rows[0], hit, 2, 15.0);
    assert_row(&rows[1], block, 1, 25.2);
    assert_row(&rows[2], SbaCause::Remote, 1, 2.0);

    let player = &parser.derived_state.party[&0];
    assert_gauge(player.total_sba_added, 42.2);
    assert_gauge(player.total_sba_added, rows.iter().map(|r| r.2).sum::<f64>());
    assert_gauge(player.sba, 42.2);
}

fn hit_by(actor: u32, child: u32, action: u32) -> DamageEvent {
    let mut event = player_damage_event();
    event.source.parent_index = actor;
    event.source.index = actor;
    event.source.parent_actor_type = 0x8056_ABCD; // Pl1900 (Id)
    event.source.actor_type = child;
    event.action_id = ActionType::Normal(action);
    event
}

#[test]
fn id_gauge_follows_him_into_and_out_of_dragon_form() {
    const HUMAN: u32 = 0x8056_ABCD; // Pl1900
    const DRAGON: u32 = 0xF575_5C0E; // Pl2000

    let mut parser = Parser::default();
    let lunge = SbaCause::Action(ActionType::Normal(200));

    parser.on_damage_event(hit_by(0, HUMAN, 200));
    parser.on_sba_update(sba_update(0, 10.0, 10.0, lunge));
    parser.on_damage_event(hit_by(0, DRAGON, 200));
    parser.on_sba_update(sba_update(0, 25.0, 15.0, lunge));
    parser.on_damage_event(hit_by(0, HUMAN, 200));
    parser.on_sba_update(sba_update(0, 30.0, 5.0, lunge));

    let rows = &parser.derived_state.party[&0].sba_breakdown;
    assert_eq!(rows.len(), 2, "one row per performing actor: {rows:?}");

    let human = rows
        .iter()
        .find(|r| r.child_character_type.is_none())
        .expect("human-form gauge is filed under the player themselves");
    assert_eq!(human.ticks, 2);
    assert_gauge(human.total_sba_added, 15.0);

    let dragon = rows
        .iter()
        .find(|r| r.child_character_type == Some(CharacterType::Pl2000))
        .expect("dragon-form gauge names the dragon");
    assert_eq!(dragon.ticks, 1);
    assert_gauge(dragon.total_sba_added, 15.0);

    assert_gauge(parser.derived_state.party[&0].total_sba_added, 30.0);
}

#[test]
fn the_dragon_form_split_survives_a_reparse() {
    const DRAGON: u32 = 0xF575_5C0E;

    let mut parser = Parser::default();
    let lunge = SbaCause::Action(ActionType::Normal(200));

    parser.on_damage_event(hit_by(0, DRAGON, 200));
    parser.on_sba_update(sba_update(0, 15.0, 15.0, lunge));

    let live = breakdown_for(&parser, 0);
    parser.reparse();
    assert_eq!(live, breakdown_for(&parser, 0));

    assert_eq!(
        parser.derived_state.party[&0].sba_breakdown[0].child_character_type,
        Some(CharacterType::Pl2000)
    );
}

#[test]
fn a_gain_with_no_matching_hit_still_lands_on_the_player() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());
    parser.on_sba_update(sba_update(0, 10.0, 10.0, SbaCause::Action(ActionType::Normal(999))));

    let rows = &parser.derived_state.party[&0].sba_breakdown;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].child_character_type, None);
    assert_gauge(rows[0].total_sba_added, 10.0);
}

#[test]
fn damage_taken_and_inferred_are_distinct_rows() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    let dodge = ActionType::Normal(610);
    parser.on_sba_update(sba_update(0, 100.0, 100.0, SbaCause::DamageTaken));
    parser.on_sba_update(sba_update(0, 200.0, 100.0, SbaCause::DamageTaken));
    parser.on_sba_update(sba_update(0, 225.0, 25.2, SbaCause::Inferred(dodge)));
    parser.on_sba_update(sba_update(0, 250.0, 25.2, SbaCause::Action(dodge)));

    let rows = breakdown_for(&parser, 0);
    assert_eq!(rows.len(), 3, "expected 3 distinct causes: {rows:?}");
    assert_row(&rows[0], SbaCause::DamageTaken, 2, 200.0);
    assert_row(&rows[1], SbaCause::Inferred(dodge), 1, 25.2);
    assert_row(&rows[2], SbaCause::Action(dodge), 1, 25.2);
    assert_ne!(rows[1].0, rows[2].0);
}

fn remote_gain(actor: u32, value: f32, added: f32) -> OnUpdateSBAEvent {
    sba_update(actor, value, added, SbaCause::Remote)
}

fn equip(parser: &mut Parser, actor: u32, traits: &[(u32, u32)]) {
    equip_as(parser, actor, true, traits)
}

/// `is_online` marks the LOCAL player, which the retro path keys on.
fn equip_as(parser: &mut Parser, actor: u32, is_online: bool, traits: &[(u32, u32)]) {
    parser.on_player_identity_event(protocol::PlayerIdentityEvent {
        actor_index: actor,
        party_index: (actor & 0xFF) as u8,
        character_name: std::ffi::CString::new("Peer").unwrap(),
        display_name: std::ffi::CString::new("peer").unwrap(),
        character_type: 0x26A4_848A,
        is_online,
        sigils: Vec::new(),
        weapon_info: None,
        summon_info: None,
        skill_loadout: Vec::new(),
        over_mastery: Vec::new(),
        master_trait_flags: Vec::new(),
        effective_traits: traits
            .iter()
            .map(|(hash, level)| protocol::EffectiveTrait {
                hash: *hash,
                level: *level,
            })
            .collect(),
        player_stats: None,
        network_user_id: None,
        network_user_name: None,
        master_level: None,
    });
}

fn equip_with_master_trait(parser: &mut Parser, actor: u32, hash: u32) {
    parser.on_player_identity_event(protocol::PlayerIdentityEvent {
        actor_index: actor,
        party_index: (actor & 0xFF) as u8,
        character_name: std::ffi::CString::new("Narmaya").unwrap(),
        display_name: std::ffi::CString::new("narmaya").unwrap(),
        character_type: 0x26A4_848A,
        is_online: false,
        sigils: Vec::new(),
        weapon_info: None,
        summon_info: None,
        skill_loadout: Vec::new(),
        over_mastery: Vec::new(),
        master_trait_flags: vec![protocol::MasterTraitFlag { hash, on: true }],
        effective_traits: Vec::new(),
        player_stats: None,
        network_user_id: None,
        network_user_name: None,
        master_level: None,
    });
}

/// Replays a counter and the gauge it granted, tight enough to calibrate the tier scalar from.
fn counter_at(parser: &mut Parser, ts: i64, actor: u32, action_id: u32, gauge: f32) {
    parser
        .encounter
        .raw_event_log
        .push((ts, Message::OnUpdateSBA(remote_gain(actor, 500.0, gauge))));
    parser.encounter.raw_event_log.push((
        ts + 2,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: actor,
            action_id,
        }),
    ));
}

fn hit_at(parser: &mut Parser, ts: i64, actor: u32, action: ActionType) {
    let mut e = player_damage_event();
    e.source.parent_index = actor;
    e.action_id = action;
    parser.encounter.raw_event_log.push((ts, Message::DamageEvent(e)));
}

#[test]
fn a_peer_gain_is_inferred_from_its_own_damage() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "expected an inferred row: {rows:?}"
    );
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::Action(ActionType::Normal(210))),
        "a deduced cause must never be reported as a read one"
    );
}

#[test]
fn an_ambiguous_peer_gain_stays_remote() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 5_004, 0, ActionType::Normal(310));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(rows.iter().any(|(c, _, _)| *c == SbaCause::Remote), "{rows:?}");
}

/// Teaches the value table that `action` grants `gauge` for `actor`, with
/// pairs inside the tight learning window.
fn teach(parser: &mut Parser, base_ts: i64, actor: u32, action: ActionType, gauge: f32, n: i64) {
    for i in 0..n {
        let at = base_ts + i * 5_000;
        hit_at(parser, at, actor, action);
        parser
            .encounter
            .raw_event_log
            .push((at + 2, Message::OnUpdateSBA(remote_gain(actor, 50.0, gauge))));
    }
}

#[test]
fn a_move_that_never_grants_this_much_is_not_credited_with_it() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    teach(&mut parser, 10_000, peer, ActionType::Normal(130), 18.2, 6);

    hit_at(&mut parser, 900_040, peer, ActionType::Normal(130));
    parser
        .encounter
        .raw_event_log
        .push((900_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 83.52))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let named = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(130)));
    assert_eq!(
        named.map(|r| r.1),
        Some(6),
        "only the six taught gains should carry this move: {rows:?}"
    );
    assert!(
        rows.iter().any(|(c, _, g)| *c == SbaCause::Remote && (*g - 83.52).abs() < 0.01),
        "the odd gain should abstain, not take the move's name: {rows:?}"
    );
}

#[test]
fn a_gain_that_folds_two_hits_of_one_move_is_still_that_move() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    teach(&mut parser, 10_000, peer, ActionType::Normal(2604), 4.2175, 6);

    hit_at(&mut parser, 899_960, peer, ActionType::Normal(2604));
    hit_at(&mut parser, 900_040, peer, ActionType::Normal(2604));
    parser
        .encounter
        .raw_event_log
        .push((900_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 8.435))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let named = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(2604)));
    assert_eq!(named.map(|r| r.1), Some(7), "the folded gain counts too: {rows:?}");
}

#[test]
fn the_gauge_value_chooses_between_two_moves_in_the_window() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    teach(&mut parser, 10_000, peer, ActionType::Normal(210), 7.0, 5);
    teach(&mut parser, 400_000, peer, ActionType::Normal(310), 25.2, 5);

    hit_at(&mut parser, 899_960, peer, ActionType::Normal(210));
    hit_at(&mut parser, 900_040, peer, ActionType::Normal(310));
    parser
        .encounter
        .raw_event_log
        .push((900_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 25.2))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let by_310 = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(310)));
    assert_eq!(
        by_310.map(|r| r.1),
        Some(6),
        "25.2 is 310's amount and not 210's: {rows:?}"
    );
}

#[test]
fn an_unlearned_move_is_still_credited() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    hit_at(&mut parser, 5_000, peer, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(peer, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "an empty table must not reject: {rows:?}"
    );
}

fn peer_sba_before(parser: &mut Parser, at: i64, secs: i64, actor: u32) {
    parser.encounter.raw_event_log.push((
        at - secs * 1_000,
        Message::OnPerformSBA(protocol::OnPerformSBAEvent {
            actor_index: actor,
        }),
    ));
}

#[test]
fn a_flat_grant_needs_the_chain_that_owes_it() {
    let peer = protocol::PLAYER_ID_BASE;
    let other = peer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    peer_sba_before(&mut parser, 100_000, 8, other);
    parser
        .encounter
        .raw_event_log
        .push((100_000, Message::OnUpdateSBA(remote_gain(peer, 400.0, 100.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "{rows:?}"
    );
}

fn learn_move(parser: &mut Parser, peer: u32, action: ActionType, grant: f32, times: i64) {
    let mut level = 0.0;
    for n in 0..times {
        let at = 20_000 + n * 1_000;
        hit_at(parser, at, peer, action);
        level += grant;
        parser
            .encounter
            .raw_event_log
            .push((at + 2, Message::OnUpdateSBA(remote_gain(peer, level, grant))));
    }
}

/// At tier scale 1.0 a summon call is also 100.00, so only the chain can name this grant.
#[test]
fn a_chain_grant_just_under_five_seconds_is_not_credited_to_the_move_beneath_it() {
    const POWER_STRIKE: ActionType = ActionType::Normal(120);
    let peer = protocol::PLAYER_ID_BASE;
    let other = peer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    equip(&mut parser, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);
    counter_at(&mut parser, 5_000, peer, counter_grant::PERFECT_DODGE, 40.0);
    learn_move(&mut parser, peer, POWER_STRIKE, 3.57, 3);

    let at = 100_000;
    parser.encounter.raw_event_log.push((
        at - 4_843,
        Message::OnPerformSBA(protocol::OnPerformSBAEvent {
            actor_index: other,
        }),
    ));
    hit_at(&mut parser, at, peer, POWER_STRIKE);
    parser
        .encounter
        .raw_event_log
        .push((at, Message::OnUpdateSBA(remote_gain(peer, 664.17, 100.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let chain = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::InferredChainGrant);
    assert_eq!(chain.map(|r| r.1), Some(1), "{rows:?}");
    assert_gauge(chain.unwrap().2, 100.0);
    let strike = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(POWER_STRIKE))
        .expect("the learned hits are still the move's: {rows:?}");
    assert_gauge(strike.2, 3.57 * 3.0);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredSummonCall),
        "a grant with a chain behind it is not a summon call: {rows:?}"
    );
}

#[test]
fn the_shortest_sba_in_the_game_still_owes_a_chain_grant() {
    let peer = protocol::PLAYER_ID_BASE;
    let zeta = peer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    equip(&mut parser, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);
    counter_at(&mut parser, 5_000, peer, counter_grant::PERFECT_DODGE, 40.0);

    let at = 100_000;
    parser.encounter.raw_event_log.push((
        at - 4_377,
        Message::OnPerformSBA(protocol::OnPerformSBAEvent { actor_index: zeta }),
    ));
    parser
        .encounter
        .raw_event_log
        .push((at, Message::OnUpdateSBA(remote_gain(peer, 500.0, 100.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "{rows:?}"
    );
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredSummonCall),
        "{rows:?}"
    );
}

#[test]
fn a_flat_grant_is_never_learned_as_a_moves_own_output() {
    const POWER_STRIKE: ActionType = ActionType::Normal(120);
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    learn_move(&mut parser, peer, POWER_STRIKE, 3.57, 3);

    let at = 100_000;
    hit_at(&mut parser, at, peer, POWER_STRIKE);
    parser
        .encounter
        .raw_event_log
        .push((at, Message::OnUpdateSBA(remote_gain(peer, 664.17, 100.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let strike = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(POWER_STRIKE))
        .expect("the learned hits are still the move's: {rows:?}");
    assert_eq!(strike.1, 3, "the flat grant is not the move's: {rows:?}");
    assert_gauge(strike.2, 3.57 * 3.0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "{rows:?}"
    );
}

#[test]
fn a_flat_grant_clipped_by_the_cap_is_recovered_by_its_chain() {
    let peer = protocol::PLAYER_ID_BASE;
    let other = peer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    peer_sba_before(&mut parser, 100_000, 9, other);
    parser
        .encounter
        .raw_event_log
        .push((100_000, Message::OnUpdateSBA(remote_gain(peer, 1000.0, 46.489))));
    parser
        .encounter
        .raw_event_log
        .push((300_000, Message::OnUpdateSBA(remote_gain(peer, 1000.0, 7.75))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let chain = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::InferredChainGrant);
    assert_eq!(chain.map(|r| r.1), Some(1), "{rows:?}");
    assert!(
        rows.iter().any(|(c, _, g)| *c == SbaCause::Remote && (*g - 7.75).abs() < 0.01),
        "a cap landing with no chain behind it is still not evidence: {rows:?}"
    );
}

/// A summon call grants 10% of max; below Chaos that collides with the chain
/// contribution, so the rule turns on the absence of a chain.
#[test]
fn a_flat_grant_with_no_chain_behind_it_is_a_summon_call() {
    let peer = protocol::PLAYER_ID_BASE;
    let other = peer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip(&mut parser, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);
    counter_at(&mut parser, 10_000, peer, counter_grant::PERFECT_DODGE, 28.0);

    parser
        .encounter
        .raw_event_log
        .push((100_000, Message::OnUpdateSBA(remote_gain(peer, 380.03, 70.0))));
    peer_sba_before(&mut parser, 300_000, 8, other);
    parser
        .encounter
        .raw_event_log
        .push((300_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 100.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let summon = rows.iter().find(|(c, _, _)| *c == SbaCause::InferredSummonCall);
    assert_eq!(summon.map(|r| r.1), Some(1), "{rows:?}");
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "the chained one is still a chain grant: {rows:?}"
    );
}

#[test]
fn an_uncalibrated_encounter_names_no_summon_calls() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    parser
        .encounter
        .raw_event_log
        .push((100_000, Message::OnUpdateSBA(remote_gain(peer, 380.03, 70.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredSummonCall),
        "{rows:?}"
    );
}

/// A gauge row from a log written before the attribution hook existed.
fn unread_gain(actor: u32, value: f32, added: f32) -> OnUpdateSBAEvent {
    sba_update(actor, value, added, SbaCause::NotClassified)
}

fn pre_attribution_party(parser: &mut Parser, local: u32, peer: u32, traits: &[(u32, u32)]) {
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip_as(parser, local, true, &[]);
    equip_as(parser, peer, false, traits);
}

#[test]
fn an_unread_log_still_names_a_peers_move() {
    let local = protocol::PLAYER_ID_BASE;
    let peer = local + 1;
    let mut parser = Parser::default();
    pre_attribution_party(&mut parser, local, peer, &[]);

    hit_at(&mut parser, 5_000, peer, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(unread_gain(peer, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn an_unread_log_names_the_local_player_too() {
    let local = protocol::PLAYER_ID_BASE;
    let peer = local + 1;
    let mut parser = Parser::default();
    pre_attribution_party(&mut parser, local, peer, &[]);

    hit_at(&mut parser, 5_000, local, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(unread_gain(local, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, local);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn a_log_the_hook_touched_is_not_treated_as_unread() {
    let local = protocol::PLAYER_ID_BASE;
    let peer = local + 1;
    let mut parser = Parser::default();
    pre_attribution_party(&mut parser, local, peer, &[]);

    hit_at(&mut parser, 5_000, peer, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(unread_gain(peer, 50.0, 7.0))));
    parser
        .encounter
        .raw_event_log
        .push((9_000, Message::OnUpdateSBA(remote_gain(peer, 60.0, 10.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::NotClassified),
        "a mixed log must not be retro-attributed: {rows:?}"
    );
}

#[test]
fn an_offline_unread_log_is_attributed_too() {
    let a = protocol::PLAYER_ID_BASE;
    let b = a + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = b;
    parser.on_damage_event(dealt);
    equip_as(&mut parser, a, true, &[]);
    equip_as(&mut parser, b, true, &[]);

    hit_at(&mut parser, 5_000, b, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(unread_gain(b, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, b);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn an_unread_log_recognises_a_counter_by_its_grant() {
    let local = protocol::PLAYER_ID_BASE;
    let peer = local + 1;
    let mut parser = Parser::default();
    pre_attribution_party(&mut parser, local, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    for (i, at) in [40_000i64, 80_000, 120_000].iter().enumerate() {
        parser.encounter.raw_event_log.push((
            *at,
            Message::OnUpdateSBA(unread_gain(peer, 100.0 + i as f32 * 28.0, 28.0)),
        ));
    }

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let counter = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610)));
    assert_eq!(counter.map(|r| r.1), Some(3), "{rows:?}");
}

#[test]
fn an_unread_log_recognises_gauge_from_being_hit() {
    let local = protocol::PLAYER_ID_BASE;
    let peer = local + 1;
    let mut parser = Parser::default();
    pre_attribution_party(&mut parser, local, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    parser
        .encounter
        .raw_event_log
        .push((40_000, Message::OnUpdateSBA(unread_gain(peer, 28.0, 28.0))));
    for at in [80_000i64, 120_000] {
        parser
            .encounter
            .raw_event_log
            .push((at, Message::OnUpdateSBA(unread_gain(peer, 40.0, 7.0))));
    }

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let taken = rows.iter().find(|(c, _, _)| *c == SbaCause::InferredDamageTaken);
    assert_eq!(taken.map(|r| r.1), Some(2), "{rows:?}");
}

#[test]
fn a_player_who_dealt_no_damage_still_gets_a_gauge_row() {
    let dealer = protocol::PLAYER_ID_BASE;
    let support = dealer + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = dealer;
    parser.on_damage_event(dealt);
    equip_as(&mut parser, support, false, &[]);

    parser.encounter.raw_event_log.push((
        5_000,
        Message::OnUpdateSBA(sba_update(
            support,
            50.0,
            7.0,
            SbaCause::Action(ActionType::Normal(210)),
        )),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, support);
    assert_gauge(rows.iter().map(|r| r.2).sum(), 7.0);
}

#[test]
fn reparse_replays_the_events_that_spend_gauge() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(player, 990.0, 10.0))));
    parser.encounter.raw_event_log.push((
        6_000,
        Message::OnPerformSBA(protocol::OnPerformSBAEvent {
            actor_index: player,
        }),
    ));

    parser.reparse();

    assert_eq!(parser.derived_state.party[&player].sba, 0.0);
}

#[test]
fn a_row_the_hook_could_not_name_is_still_inferred() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    hit_at(&mut parser, 5_000, player, ActionType::Normal(210));
    parser.encounter.raw_event_log.push((
        5_002,
        Message::OnUpdateSBA(sba_update(player, 50.0, 7.0, SbaCause::Unknown)),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

/// The butterfly node's 10% boost is conditional on a butterfly count the log
/// does not record, so both the boosted and unboosted grant are accepted.
#[test]
fn a_partys_narmaya_node_admits_the_boosted_counter_grant() {
    let narmaya = protocol::PLAYER_ID_BASE;
    let peer = narmaya + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip_as(&mut parser, peer, false, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);
    equip_with_master_trait(&mut parser, narmaya, counter_grant::BUTTERFLY_SBA_GAIN);

    counter_at(&mut parser, 10_000, peer, counter_grant::PERFECT_DODGE, 28.0);
    parser
        .encounter
        .raw_event_log
        .push((30_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 30.8))));
    parser.encounter.raw_event_log.push((
        30_200,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: peer,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let counter = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610)));
    assert_eq!(
        counter.map(|r| r.1),
        Some(2),
        "the boosted grant is the same counter: {rows:?}"
    );
}

#[test]
fn a_peers_damage_taken_is_named_by_value_on_a_hooked_log() {
    let peer = protocol::PLAYER_ID_BASE + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip(&mut parser, peer, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    counter_at(&mut parser, 10_000, peer, counter_grant::PERFECT_DODGE, 28.0);
    parser.encounter.raw_event_log.push((
        30_000,
        Message::OnUpdateSBA(remote_gain(peer, 500.0, 7.0)),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let taken = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::InferredDamageTaken);
    assert_eq!(taken.map(|r| r.1), Some(1), "{rows:?}");
}

#[test]
fn without_the_node_the_boosted_value_is_not_a_counter() {
    let peer = protocol::PLAYER_ID_BASE + 1;
    let mut parser = Parser::default();
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip_as(&mut parser, peer, false, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    counter_at(&mut parser, 10_000, peer, counter_grant::PERFECT_DODGE, 28.0);
    parser
        .encounter
        .raw_event_log
        .push((30_000, Message::OnUpdateSBA(remote_gain(peer, 500.0, 30.8))));
    parser.encounter.raw_event_log.push((
        30_200,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: peer,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    let counter = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610)));
    assert_eq!(counter.map(|r| r.1), Some(1), "{rows:?}");
}

#[test]
fn the_builds_only_decode_matches_the_full_one() {
    let mut parser = Parser::default();
    let peer = protocol::PLAYER_ID_BASE;
    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);
    equip_with_master_trait(&mut parser, peer, counter_grant::BUTTERFLY_SBA_GAIN);
    for i in 0..500 {
        parser
            .encounter
            .raw_event_log
            .push((i, Message::OnUpdateSBA(remote_gain(peer, 50.0, 1.0))));
    }

    let blob = parser.encounter.to_blob().expect("blob");
    let full = Encounter::from_blob(&blob).expect("full decode");
    let builds = Encounter::builds_from_blob(&blob).expect("builds decode");

    for slot in 0..4 {
        let flags = |p: &PlayerData| {
            p.master_trait_flags()
                .iter()
                .map(|f| (f.hash, f.on))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            full.player_data[slot].as_ref().map(flags),
            builds[slot].as_ref().map(flags),
            "slot {slot}"
        );
        assert_eq!(
            full.player_data[slot].as_ref().map(|p| p.actor_index),
            builds[slot].as_ref().map(|p| p.actor_index),
        );
    }
}

#[test]
fn a_peer_counter_matches_a_gauge_sync_that_arrived_first() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);
    equip(&mut parser, player, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(player, 50.0, 28.0))));
    parser.encounter.raw_event_log.push((
        5_002,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: player,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610))),
        "a counter arriving after its gauge must still match: {rows:?}"
    );
}

#[test]
fn a_counter_from_a_player_without_the_trait_claims_nothing() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);
    equip(&mut parser, player, &[(counter_grant::PRECISE_WRATH, 20)]);

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(player, 50.0, 7.28))));
    parser.encounter.raw_event_log.push((
        5_002,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: player,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610))),
        "a dodge without Nimble Onslaught cannot have granted gauge: {rows:?}"
    );
}

#[test]
fn a_counter_is_judged_by_its_grant_not_its_distance() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);
    equip(&mut parser, player, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    counter_at(&mut parser, 10_000, player, counter_grant::PERFECT_DODGE, 28.0);
    counter_at(&mut parser, 20_000, player, counter_grant::PERFECT_DODGE, 28.0);

    // Right size, 200 ms out.
    parser
        .encounter
        .raw_event_log
        .push((30_000, Message::OnUpdateSBA(remote_gain(player, 500.0, 28.0))));
    parser.encounter.raw_event_log.push((
        30_200,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: player,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    // Wrong size, 2 ms out.
    parser
        .encounter
        .raw_event_log
        .push((40_000, Message::OnUpdateSBA(remote_gain(player, 600.0, 7.28))));
    parser.encounter.raw_event_log.push((
        40_002,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: player,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    let counter = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610)));
    assert_eq!(
        counter.map(|r| r.1),
        Some(3),
        "the two calibrators and the distant right-sized one: {rows:?}"
    );
    assert!(
        rows.iter().any(|(c, _, g)| *c == SbaCause::Remote && (*g - 7.28).abs() < 0.01),
        "the wrong-sized gain must stay unnamed: {rows:?}"
    );
}

#[test]
fn an_uncalibrated_counter_join_stays_tight() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);
    equip(&mut parser, player, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    parser
        .encounter
        .raw_event_log
        .push((30_000, Message::OnUpdateSBA(remote_gain(player, 500.0, 28.0))));
    parser.encounter.raw_event_log.push((
        30_200,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: player,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610))),
        "an unchecked join must not reach 200ms: {rows:?}"
    );
}

#[test]
fn a_peer_counter_is_keyed_to_its_own_actor() {
    let player = protocol::PLAYER_ID_BASE;
    let other = protocol::PLAYER_ID_BASE + 1;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);
    equip(&mut parser, other, &[(counter_grant::NIMBLE_ONSLAUGHT, 20)]);

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(player, 50.0, 28.0))));
    parser.encounter.raw_event_log.push((
        5_010,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: other,
            action_id: counter_grant::PERFECT_DODGE,
        }),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(rows.iter().any(|(c, _, _)| *c == SbaCause::Remote), "{rows:?}");
}

#[test]
fn a_peer_gain_is_inferred_from_damage_they_took() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    let mut incoming = player_damage_event();
    incoming.source.parent_index = 9_999;
    incoming.target.parent_index = player;
    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::DamageEvent(incoming)));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(player, 50.0, 7.91))));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredDamageTaken),
        "{rows:?}"
    );
}

#[test]
fn a_move_match_outranks_damage_taken() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    hit_at(&mut parser, 5_000, player, ActionType::Normal(210));
    let mut incoming = player_damage_event();
    incoming.source.parent_index = 9_999;
    incoming.target.parent_index = player;
    parser
        .encounter
        .raw_event_log
        .push((5_001, Message::DamageEvent(incoming)));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(player, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, player);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "the move match should win: {rows:?}"
    );
}

#[test]
fn a_supplementary_only_window_names_nothing() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::SupplementaryDamage(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(rows.iter().any(|(c, _, _)| *c == SbaCause::Remote), "{rows:?}");
    assert!(
        !rows.iter().any(|(c, _, _)| matches!(
            c,
            SbaCause::Inferred(ActionType::SupplementaryDamage(_))
        )),
        "supplementary damage must never be named as the cause: {rows:?}"
    );
}

#[test]
fn supplementary_hits_do_not_make_a_peer_gain_ambiguous() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 5_003, 0, ActionType::SupplementaryDamage(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn a_dot_only_window_names_nothing() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::DamageOverTime(0));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(rows.iter().any(|(c, _, _)| *c == SbaCause::Remote), "{rows:?}");
    assert!(
        !rows.iter().any(|(c, _, _)| matches!(
            c,
            SbaCause::Inferred(ActionType::DamageOverTime(_))
        )),
        "damage over time must never be named as the cause: {rows:?}"
    );
}

#[test]
fn dot_ticks_do_not_make_a_peer_gain_ambiguous() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 5_003, 0, ActionType::DamageOverTime(0));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn the_unset_action_id_is_never_named() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(u32::MAX));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(rows.iter().any(|(c, _, _)| *c == SbaCause::Remote), "{rows:?}");
    assert!(
        !rows
            .iter()
            .any(|(c, _, _)| matches!(c, SbaCause::Inferred(ActionType::Normal(u32::MAX)))),
        "the unset action id must never be named as the cause: {rows:?}"
    );
}

#[test]
fn the_unset_action_id_does_not_make_a_peer_gain_ambiguous() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 5_003, 0, ActionType::Normal(u32::MAX));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "{rows:?}"
    );
}

#[test]
fn a_gain_outside_the_window_is_named_by_the_gauge_it_pays() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    // Teaches the table what Normal(210) pays, from a tight enough join.
    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 3.57))));

    // 200 ms is far outside the 64 ms the correlation decides at.
    hit_at(&mut parser, 20_000, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((20_200, Message::OnUpdateSBA(remote_gain(0, 60.0, 3.57))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    let named = rows
        .iter()
        .find(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210)));
    assert!(named.is_some(), "{rows:?}");
    assert_eq!(named.unwrap().1, 2, "both gains should be named: {rows:?}");
}

#[test]
fn the_value_rule_abstains_when_two_moves_pay_the_same() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 3.57))));
    hit_at(&mut parser, 10_000, 0, ActionType::Normal(310));
    parser
        .encounter
        .raw_event_log
        .push((10_002, Message::OnUpdateSBA(remote_gain(0, 55.0, 3.57))));

    hit_at(&mut parser, 20_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 20_100, 0, ActionType::Normal(310));
    parser
        .encounter
        .raw_event_log
        .push((20_200, Message::OnUpdateSBA(remote_gain(0, 60.0, 3.57))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Remote),
        "two moves pay 3.57 here, so the third gain is undecidable: {rows:?}"
    );
}

#[test]
fn the_value_rule_does_not_fold_hits() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 50.0, 3.57))));

    hit_at(&mut parser, 20_000, 0, ActionType::Normal(210));
    hit_at(&mut parser, 20_100, 0, ActionType::Normal(210));
    parser
        .encounter
        .raw_event_log
        .push((20_200, Message::OnUpdateSBA(remote_gain(0, 60.0, 7.14))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Remote),
        "two hits of a 3.57 move do not license naming a 7.14 gain: {rows:?}"
    );
}

#[test]
fn a_peer_chain_grant_is_recognised_by_value() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(0, 400.0, 100.0))));
    parser
        .encounter
        .raw_event_log
        .push((60_000, Message::OnUpdateSBA(remote_gain(0, 700.0, 130.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    let chain = rows.iter().find(|(c, _, _)| *c == SbaCause::InferredChainGrant);
    assert!(chain.is_some(), "{rows:?}");
    assert_eq!(chain.unwrap().1, 2, "10% and 13% are the same mechanic");
}

#[test]
fn a_gain_that_merely_tops_off_the_bar_is_not_a_chain_grant() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(212));
    parser
        .encounter
        .raw_event_log
        .push((5_002, Message::OnUpdateSBA(remote_gain(0, 1000.0, 7.75))));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "landing on the cap is not evidence of a grant: {rows:?}"
    );
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(212))),
        "the move that filled the bar should still be named: {rows:?}"
    );
}

#[test]
fn the_counter_window_is_tighter_than_the_move_window() {
    let peer = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.parent_index = peer;
    parser.on_damage_event(dealt);

    hit_at(&mut parser, 5_040, peer, ActionType::Normal(210));
    parser.encounter.raw_event_log.push((
        5_040,
        Message::OnPeerCounter(protocol::PeerCounterEvent {
            actor_index: peer,
            action_id: 610,
        }),
    ));
    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(peer, 50.0, 7.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, peer);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(610))),
        "a counter 40ms away must not claim this gauge: {rows:?}"
    );
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Inferred(ActionType::Normal(210))),
        "a move 40ms away is well inside the move window: {rows:?}"
    );
}

#[test]
fn a_redistribute_payout_is_named_from_the_casters_loss() {
    let caster = protocol::PLAYER_ID_BASE;
    let (a, b) = (caster + 1, caster + 2);
    let mut parser = Parser::default();
    for actor in [caster, a, b] {
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(caster, 308.49, 8.49))));
    parser
        .encounter
        .raw_event_log
        .push((6_000, Message::OnUpdateSBA(remote_gain(caster, 8.49, 0.0))));
    parser
        .encounter
        .raw_event_log
        .push((7_400, Message::OnUpdateSBA(remote_gain(a, 289.11, 130.0))));
    parser
        .encounter
        .raw_event_log
        .push((7_450, Message::OnUpdateSBA(remote_gain(b, 530.55, 130.0))));

    parser.reparse();

    for actor in [a, b] {
        let rows = breakdown_for(&parser, actor);
        assert!(
            rows.iter().any(|(c, _, _)| *c == SbaCause::InferredRedistribute),
            "recipient {actor} should be credited to the caster: {rows:?}"
        );
    }
}

#[test]
fn a_partial_redistribute_is_named_from_the_share_it_implies() {
    let caster = protocol::PLAYER_ID_BASE;
    let ally = caster + 1;
    let mut parser = Parser::default();
    for actor in [caster, ally] {
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(caster, 151.06, 1.82))));
    parser
        .encounter
        .raw_event_log
        .push((6_000, Message::OnUpdateSBA(remote_gain(caster, 3.64, 3.64))));
    parser
        .encounter
        .raw_event_log
        .push((6_260, Message::OnUpdateSBA(remote_gain(ally, 197.32, 80.35))));

    parser.reparse();

    let rows = breakdown_for(&parser, ally);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredRedistribute),
        "80.35 is 151.06/3 + 30 and nothing else: {rows:?}"
    );
}

/// The redistribute rides the same granter as the chain contribution, so the
/// hook reads it as `ChainGrant`. That read cause the join may correct.
#[test]
fn a_redistribute_credit_read_as_a_chain_grant_is_corrected() {
    let caster = protocol::PLAYER_ID_BASE;
    let me = caster + 1;
    let other = caster + 2;
    let mut parser = Parser::default();
    for actor in [caster, me, other] {
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(caster, 478.56, 10.0))));
    parser
        .encounter
        .raw_event_log
        .push((6_000, Message::OnUpdateSBA(remote_gain(caster, 178.56, 0.0))));
    parser.encounter.raw_event_log.push((
        7_400,
        Message::OnUpdateSBA(sba_update(me, 912.70, 130.0, SbaCause::ChainGrant)),
    ));
    parser
        .encounter
        .raw_event_log
        .push((7_430, Message::OnUpdateSBA(remote_gain(other, 290.39, 130.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, me);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredRedistribute),
        "a conflated read cause should be split once the caster is visible: {rows:?}"
    );
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::ChainGrant),
        "and not left under both names: {rows:?}"
    );
}

#[test]
fn a_lone_chain_grant_value_is_not_claimed_by_the_redistribute_join() {
    let caster = protocol::PLAYER_ID_BASE;
    let ally = caster + 1;
    let mut parser = Parser::default();
    for actor in [caster, ally] {
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(caster, 478.56, 10.0))));
    parser
        .encounter
        .raw_event_log
        .push((6_000, Message::OnUpdateSBA(remote_gain(caster, 178.56, 0.0))));
    parser
        .encounter
        .raw_event_log
        .push((7_400, Message::OnUpdateSBA(remote_gain(ally, 912.70, 130.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, ally);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredChainGrant),
        "one recipient at a chain-grant value stays a chain grant: {rows:?}"
    );
}

#[test]
fn spending_a_bar_on_an_sba_is_not_a_redistribute() {
    let caster = protocol::PLAYER_ID_BASE;
    let ally = caster + 1;
    let mut parser = Parser::default();
    for actor in [caster, ally] {
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser
        .encounter
        .raw_event_log
        .push((5_000, Message::OnUpdateSBA(remote_gain(caster, 1000.0, 5.0))));
    parser
        .encounter
        .raw_event_log
        .push((6_000, Message::OnUpdateSBA(remote_gain(caster, 0.91, 0.91))));
    parser
        .encounter
        .raw_event_log
        .push((7_400, Message::OnUpdateSBA(remote_gain(ally, 400.0, 363.33))));

    parser.reparse();

    let rows = breakdown_for(&parser, ally);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredRedistribute),
        "a 1000-point spend is not a 30% redistribute: {rows:?}"
    );
}

fn seat_party(parser: &mut Parser) -> [u32; 4] {
    let base = protocol::PLAYER_ID_BASE;
    let party = [base, base + 1, base + 2, base + 3];
    for actor in party {
        equip_as(parser, actor, actor == base, &[]);
        let mut dealt = player_damage_event();
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }
    party
}

/// Seofon's Seven Star's Brilliance: every bar to the cap inside one burst.
#[test]
fn a_party_wide_fill_is_named_across_the_whole_roster() {
    let mut parser = Parser::default();
    let party = seat_party(&mut parser);

    for (index, gain) in [756.54, 598.57, 747.67, 769.35].into_iter().enumerate() {
        parser.encounter.raw_event_log.push((
            60_000 + index as i64 * 20,
            Message::OnUpdateSBA(remote_gain(party[index], 1000.0, gain)),
        ));
    }

    parser.reparse();

    for actor in party {
        let rows = breakdown_for(&parser, actor);
        assert!(
            rows.iter().any(|(c, _, _)| *c == SbaCause::InferredPartyFill),
            "every member of a filled party is credited to the fill: {rows:?}"
        );
    }
}

#[test]
fn a_chain_grant_that_tops_every_bar_off_is_not_a_fill() {
    let mut parser = Parser::default();
    let party = seat_party(&mut parser);

    for (index, actor) in party.into_iter().enumerate() {
        parser.encounter.raw_event_log.push((
            60_000 + index as i64 * 20,
            Message::OnUpdateSBA(remote_gain(actor, 1000.0, 100.0)),
        ));
    }

    parser.reparse();

    for actor in party {
        let rows = breakdown_for(&parser, actor);
        assert!(
            !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredPartyFill),
            "100.00 apiece is a chain contribution, not a fill: {rows:?}"
        );
    }
}

#[test]
fn one_player_reaching_the_cap_alone_is_not_a_fill() {
    let mut parser = Parser::default();
    let party = seat_party(&mut parser);

    parser
        .encounter
        .raw_event_log
        .push((60_000, Message::OnUpdateSBA(remote_gain(party[0], 1000.0, 400.0))));

    parser.reparse();

    let rows = breakdown_for(&parser, party[0]);
    assert!(
        !rows.iter().any(|(c, _, _)| *c == SbaCause::InferredPartyFill),
        "one bar is not a party-wide anything: {rows:?}"
    );
}

/// A member already at the cap has nothing to sync and emits no row at all.
#[test]
fn a_member_already_at_the_cap_still_counts_towards_the_roster() {
    let mut parser = Parser::default();
    let party = seat_party(&mut parser);

    parser
        .encounter
        .raw_event_log
        .push((10_000, Message::OnUpdateSBA(remote_gain(party[3], 1000.0, 50.0))));
    for (index, actor) in party[..3].iter().copied().enumerate() {
        parser.encounter.raw_event_log.push((
            60_000 + index as i64 * 20,
            Message::OnUpdateSBA(remote_gain(actor, 1000.0, 700.0)),
        ));
    }

    parser.reparse();

    for actor in party[..3].iter().copied() {
        let rows = breakdown_for(&parser, actor);
        assert!(
            rows.iter().any(|(c, _, _)| *c == SbaCause::InferredPartyFill),
            "a silent member at the cap must not veto the fill: {rows:?}"
        );
    }
}

#[test]
fn a_fill_read_as_a_chain_grant_is_corrected() {
    let mut parser = Parser::default();
    let party = seat_party(&mut parser);

    parser.encounter.raw_event_log.push((
        60_000,
        Message::OnUpdateSBA(sba_update(party[0], 1000.0, 756.54, SbaCause::ChainGrant)),
    ));
    for (index, actor) in party[1..].iter().copied().enumerate() {
        parser.encounter.raw_event_log.push((
            60_020 + index as i64 * 20,
            Message::OnUpdateSBA(remote_gain(actor, 1000.0, 700.0)),
        ));
    }

    parser.reparse();

    let rows = breakdown_for(&parser, party[0]);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::InferredPartyFill),
        "756.54 is far past any chain contribution: {rows:?}"
    );
}

#[test]
fn inference_never_overwrites_a_read_cause() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    hit_at(&mut parser, 5_000, 0, ActionType::Normal(210));
    parser.encounter.raw_event_log.push((
        5_002,
        Message::OnUpdateSBA(sba_update(0, 50.0, 25.2, SbaCause::Action(ActionType::Normal(611)))),
    ));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert!(
        rows.iter().any(|(c, _, _)| *c == SbaCause::Action(ActionType::Normal(611))),
        "{rows:?}"
    );
}

#[test]
fn damage_taken_is_recorded_without_touching_damage_dealt() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.index = player;
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    let dealt_before = parser.derived_state.total_damage;
    assert!(dealt_before > 0, "setup: the player should have dealt damage");

    let mut incoming = player_damage_event();
    incoming.source.parent_index = 9_999; // an enemy
    incoming.source.parent_actor_type = 0xDEAD_BEEF;
    incoming.target.parent_index = player;
    incoming.damage = 4_242;
    parser.on_damage_event(incoming);

    assert_eq!(
        parser.derived_state.total_damage, dealt_before,
        "damage taken leaked into the dealt total"
    );
    assert_eq!(parser.derived_state.party[&player].total_damage_taken, 4_242);
    assert_eq!(
        parser.derived_state.party[&player].total_damage, dealt_before,
        "damage taken leaked into the recipient's dealt total"
    );
    assert!(
        !parser.derived_state.party.contains_key(&9_999),
        "an attacker must not get a party row"
    );
}

#[test]
fn healing_done_is_credited_to_the_source_without_touching_damage() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.index = player;
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    let dealt_before = parser.derived_state.total_damage;
    assert!(dealt_before > 0, "setup: the player should have dealt damage");

    parser.on_heal_event(protocol::HealEvent {
        source: Some(player),
        target: player,
        amount: 3_500,
        heal_type: 2,
        channel: 0xFFFF_FFFF,
        via_apply: false,
    });

    assert_eq!(parser.derived_state.party[&player].heal_done, 3_500);
    assert_eq!(
        parser.derived_state.total_damage, dealt_before,
        "healing leaked into the dealt total"
    );
    assert_eq!(
        parser.derived_state.party[&player].total_damage, dealt_before,
        "healing leaked into the healer's dealt total"
    );
    assert_eq!(
        parser.derived_state.party[&player].total_damage_taken, 0,
        "healing leaked into damage taken"
    );
}

#[test]
fn a_source_less_or_empty_heal_is_dropped() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.index = player;
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    parser.on_heal_event(protocol::HealEvent {
        source: None,
        target: player,
        amount: 1_000,
        heal_type: 2,
        channel: 0xFFFF_FFFF,
        via_apply: false,
    });
    parser.on_heal_event(protocol::HealEvent {
        source: Some(player),
        target: player,
        amount: 0,
        heal_type: 2,
        channel: 0xFFFF_FFFF,
        via_apply: false,
    });

    assert_eq!(parser.derived_state.party[&player].heal_done, 0);
    assert_eq!(parser.derived_state.party[&player].heal_received, 1_000);
}

#[test]
fn heals_are_classified_into_source_buckets() {
    use crate::parser::v1::player_state::{classify_heal, HealCategory};

    let heal = |heal_type: u32, channel: u32, via_apply: bool| protocol::HealEvent {
        source: Some(protocol::PLAYER_ID_BASE),
        target: protocol::PLAYER_ID_BASE,
        amount: 100,
        heal_type,
        channel,
        via_apply,
    };

    // Revive is kind 4 regardless of caller.
    assert_eq!(classify_heal(&heal(4, 0xFFFF_FFFF, true)), HealCategory::Revive);
    // The generic PlayerHealHpAction channel is a skill.
    assert_eq!(classify_heal(&heal(0, 0x0001_3880, true)), HealCategory::Skill);
    // A character-unique heal: apply primitive, kind 0, no channel.
    assert_eq!(classify_heal(&heal(0, 0xFFFF_FFFF, true)), HealCategory::Skill);
    // Drain and potion are both kind 2 via the ledger flush -> one bucket.
    assert_eq!(classify_heal(&heal(2, 0xFFFF_FFFF, false)), HealCategory::SelfRecovery);
    // Regen is kind 1 via the ledger flush.
    assert_eq!(classify_heal(&heal(1, 0xFFFF_FFFF, false)), HealCategory::Regen);
    // Environment / quest / system: kind 0 through the ledger flush.
    assert_eq!(classify_heal(&heal(0, 0xFFFF_FFFF, false)), HealCategory::Other);
}

#[test]
fn heal_breakdown_buckets_provided_and_received() {
    let healer = protocol::PLAYER_ID_BASE;
    let ally = protocol::PLAYER_ID_BASE | 1;
    let mut parser = Parser::default();

    for actor in [healer, ally] {
        let mut dealt = player_damage_event();
        dealt.source.index = actor;
        dealt.source.parent_index = actor;
        parser.on_damage_event(dealt);
    }

    parser.on_heal_event(protocol::HealEvent {
        source: Some(healer),
        target: ally,
        amount: 700,
        heal_type: 0,
        channel: 0x0001_3880,
        via_apply: true,
    });
    parser.on_heal_event(protocol::HealEvent {
        source: Some(healer),
        target: healer,
        amount: 300,
        heal_type: 2,
        channel: 0xFFFF_FFFF,
        via_apply: false,
    });

    let h = &parser.derived_state.party[&healer];
    assert_eq!(h.heal_done, 1_000);
    assert_eq!(h.heal_provided_by_type.skill, 700);
    assert_eq!(h.heal_provided_by_type.self_recovery, 300);
    assert_eq!(h.heal_received_by_type.self_recovery, 300);

    let a = &parser.derived_state.party[&ally];
    assert_eq!(a.heal_received, 700);
    assert_eq!(a.heal_received_by_type.skill, 700);
}

/// The game reports the attack's full value, not the HP it removed.
#[test]
fn a_single_overkill_hit_is_capped_to_max_hp() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    let mut dealt = player_damage_event();
    dealt.source.index = player;
    dealt.source.parent_index = player;
    parser.on_damage_event(dealt);

    parser.on_player_identity_event(protocol::PlayerIdentityEvent {
        actor_index: player,
        party_index: 0,
        character_name: std::ffi::CString::new("").unwrap(),
        display_name: std::ffi::CString::new("").unwrap(),
        character_type: 0,
        is_online: false,
        sigils: Vec::new(),
        weapon_info: None,
        summon_info: None,
        skill_loadout: Vec::new(),
        over_mastery: Vec::new(),
        master_trait_flags: Vec::new(),
        effective_traits: Vec::new(),
        player_stats: Some(protocol::PlayerStats {
            level: 1,
            total_hp: 40_000,
            total_attack: 0,
            stun_power: 0.0,
            critical_rate: 0.0,
            total_power: 0,
            dmg_cap_channels: [0.0; 3],
        }),
        network_user_id: None,
        network_user_name: None,
        master_level: None,
    });

    let mut overkill = player_damage_event();
    overkill.source.parent_index = 9_999; // an enemy
    overkill.target.parent_index = player;
    overkill.damage = 4_700_000;
    parser.on_damage_event(overkill);

    assert_eq!(parser.derived_state.party[&player].total_damage_taken, 40_000);
}

#[test]
fn misc_charts_accumulate_and_reset_damage_taken_at_death() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    fn taken_at(parser: &mut Parser, ts: i64, player: u32, amount: i32) {
        let mut e = player_damage_event();
        e.source.parent_index = 9_999; // an enemy
        e.target.parent_index = player;
        e.damage = amount;
        parser.encounter.raw_event_log.push((ts, Message::DamageEvent(e)));
    }

    // Only damage dealt advances end_time, so hits at the ends fix the span.
    hit_at(&mut parser, 0, player, ActionType::Normal(0));
    taken_at(&mut parser, 1_000, player, 300);
    taken_at(&mut parser, 2_000, player, 200);
    parser.encounter.raw_event_log.push((
        2_500,
        Message::OnHeal(protocol::HealEvent {
            source: Some(player),
            target: player,
            amount: 1_000,
            heal_type: 2,
            channel: 0xFFFF_FFFF,
            via_apply: false,
        }),
    ));
    parser.encounter.raw_event_log.push((
        3_000,
        Message::OnDeathEvent(protocol::OnDeathEvent {
            actor_index: player,
            death_counter: 1,
        }),
    ));
    taken_at(&mut parser, 4_000, player, 150); // a fresh life
    hit_at(&mut parser, 4_000, player, ActionType::Normal(0));

    parser.reparse();

    assert_eq!(parser.derived_state.party[&player].total_damage_taken, 650);
    assert_eq!(parser.derived_state.party[&player].heal_received, 1_000);

    let charts = parser.generate_misc_charts(1_000);

    let recv = &charts.heal_received[&player];
    assert_eq!(*recv.last().unwrap(), 1_000);
    assert!(recv.windows(2).all(|w| w[1] >= w[0]), "received must never decrease");

    // Ends at the post-death life's 150, not the lifetime 650; the death's own
    // bucket keeps the first life's peak of 500 and the reset shows from the next.
    let dmg = &charts.damage_taken[&player];
    assert_eq!(*dmg.last().unwrap(), 150);
    assert_eq!(dmg.iter().max().copied().unwrap(), 500, "first life's peak survives the reset");
    assert_eq!(dmg[3], 500);
    assert_eq!(dmg[4], 150);

    assert_eq!(charts.deaths[&player].len(), 1);
}

#[test]
fn a_hit_in_the_death_bucket_does_not_erase_the_reset() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    fn taken_at(parser: &mut Parser, ts: i64, player: u32, amount: i32) {
        let mut e = player_damage_event();
        e.source.parent_index = 9_999; // an enemy
        e.target.parent_index = player;
        e.damage = amount;
        parser.encounter.raw_event_log.push((ts, Message::DamageEvent(e)));
    }

    hit_at(&mut parser, 0, player, ActionType::Normal(0));
    taken_at(&mut parser, 1_000, player, 400);
    parser.encounter.raw_event_log.push((
        2_000,
        Message::OnDeathEvent(protocol::OnDeathEvent {
            actor_index: player,
            death_counter: 1,
        }),
    ));
    // The killing blow, logged after the death it caused.
    taken_at(&mut parser, 2_000, player, 50);
    hit_at(&mut parser, 3_000, player, ActionType::Normal(0));

    parser.reparse();
    let dmg = &parser.generate_misc_charts(1_000).damage_taken[&player];

    assert_eq!(dmg[2], 400, "the death's bucket holds the peak, not the new life");
    assert_eq!(dmg[3], 50);
}

/// The hook re-emits a death when a hit lands on a body that is still down.
#[test]
fn a_death_event_for_a_player_still_down_is_not_a_second_death() {
    let player = protocol::PLAYER_ID_BASE;

    fn taken_at(parser: &mut Parser, ts: i64, player: u32, amount: i32) {
        let mut e = player_damage_event();
        e.source.parent_index = 9_999; // an enemy
        e.target.parent_index = player;
        e.damage = amount;
        parser.encounter.raw_event_log.push((ts, Message::DamageEvent(e)));
    }
    fn died_at(parser: &mut Parser, ts: i64, player: u32, counter: u32) {
        parser.encounter.raw_event_log.push((
            ts,
            Message::OnDeathEvent(protocol::OnDeathEvent {
                actor_index: player,
                death_counter: counter,
            }),
        ));
    }

    let mut parser = Parser::default();
    hit_at(&mut parser, 0, player, ActionType::Normal(0));
    taken_at(&mut parser, 1_000, player, 300);
    died_at(&mut parser, 2_000, player, 1);
    taken_at(&mut parser, 3_000, player, 200);
    died_at(&mut parser, 4_000, player, 2);
    hit_at(&mut parser, 5_000, player, ActionType::Normal(0));
    died_at(&mut parser, 6_000, player, 3);
    hit_at(&mut parser, 7_000, player, ActionType::Normal(0));

    parser.reparse();
    let charts = parser.generate_misc_charts(1_000);

    assert_eq!(
        charts.deaths[&player],
        vec![2_000, 6_000],
        "the event fired while down is not a death"
    );
    assert_eq!(charts.damage_taken[&player][3], 200);
}

#[test]
fn a_revive_between_two_death_events_makes_them_two_deaths() {
    let player = protocol::PLAYER_ID_BASE;
    let mut parser = Parser::default();

    hit_at(&mut parser, 0, player, ActionType::Normal(0));
    parser.encounter.raw_event_log.push((
        1_000,
        Message::OnDeathEvent(protocol::OnDeathEvent {
            actor_index: player,
            death_counter: 1,
        }),
    ));
    parser.encounter.raw_event_log.push((
        2_000,
        Message::OnHeal(protocol::HealEvent {
            source: Some(player),
            target: player,
            amount: 5_000,
            heal_type: 4, // revive
            channel: 0xFFFF_FFFF,
            via_apply: true,
        }),
    ));
    parser.encounter.raw_event_log.push((
        3_000,
        Message::OnDeathEvent(protocol::OnDeathEvent {
            actor_index: player,
            death_counter: 2,
        }),
    ));
    hit_at(&mut parser, 4_000, player, ActionType::Normal(0));

    parser.reparse();

    assert_eq!(parser.generate_misc_charts(1_000).deaths[&player], vec![1_000, 3_000]);
}

#[test]
fn a_zero_gain_creates_no_row_and_bumps_no_tick() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    parser.on_sba_update(sba_update(0, 50.0, 0.0, SbaCause::Unknown));

    assert!(breakdown_for(&parser, 0).is_empty());
    assert_eq!(parser.derived_state.party[&0].total_sba_added, 0.0);
    assert_eq!(parser.derived_state.party[&0].sba, 50.0);
}

#[test]
fn reparse_recovers_gains_that_precede_the_players_first_hit() {
    let mut parser = Parser::default();
    let cause = SbaCause::Action(ActionType::Normal(611));

    parser.encounter.raw_event_log.push((
        1_000,
        Message::OnUpdateSBA(sba_update(0, 25.2, 25.2, cause)),
    ));
    parser.encounter.raw_event_log.push((
        2_000,
        Message::OnUpdateSBA(sba_update(0, 50.4, 25.2, cause)),
    ));
    parser
        .encounter
        .raw_event_log
        .push((3_000, Message::DamageEvent(player_damage_event())));

    parser.reparse();

    let rows = breakdown_for(&parser, 0);
    assert_eq!(rows.len(), 1, "gains before the first hit were lost: {rows:?}");
    assert_row(&rows[0], cause, 2, 50.4);
}

#[test]
fn per_enemy_state_carries_no_sba_breakdown() {
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());
    parser.on_sba_update(sba_update(0, 25.2, 25.2, SbaCause::Remote));

    let per_enemy = parser.derived_state_for_enemy(1);

    assert!(per_enemy
        .party
        .values()
        .all(|p| p.sba_breakdown.is_empty() && p.total_sba_added == 0.0));
}

#[test]
fn stray_hit_right_after_a_save_is_dropped_not_a_new_encounter() {
    let mut parser = Parser::default();
    parser.status = ParserStatus::Stopped;
    parser.last_save_time = Some(Utc::now().timestamp_millis());

    parser.on_damage_event(player_damage_event());

    assert_eq!(parser.status, ParserStatus::Stopped);
    assert!(!parser.has_damage());
}

#[test]
fn hit_after_the_debounce_window_starts_a_new_encounter() {
    let mut parser = Parser::default();
    parser.status = ParserStatus::Stopped;
    parser.last_save_time =
        Some(Utc::now().timestamp_millis() - SAVE_DEBOUNCE_MILLIS - 100);

    parser.on_damage_event(player_damage_event());

    assert_eq!(parser.status, ParserStatus::InProgress);
    assert!(parser.has_damage());
}

#[test]
fn start_time_depends_on_first_event() {
    let mut parser = Parser::default();

    parser.encounter.raw_event_log.push((
        1_000,
        Message::DamageEvent(DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            damage: 0,
            flags: 0,
            action_id: ActionType::Normal(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        }),
    ));

    assert_eq!(parser.start_time(), 1_000);
}

#[test]
fn duration_calculated_from_start_to_current_event() {
    let mut parser = Parser::default();

    parser.encounter.raw_event_log.push((
        1_000,
        Message::DamageEvent(DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            damage: 0,
            flags: 0,
            action_id: ActionType::Normal(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        }),
    ));

    parser.encounter.raw_event_log.push((
        5_000,
        Message::DamageEvent(DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            damage: 0,
            flags: 0,
            action_id: ActionType::Normal(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        }),
    ));

    parser.reparse();

    assert_eq!(parser.derived_state.start_time, 1_000);
    assert_eq!(parser.derived_state.end_time, 5_000);
    assert_eq!(parser.derived_state.duration(), 4_000);
}

#[test]
fn an_event_after_the_last_hit_does_not_overrun_the_sba_chart() {
    let mut parser = Parser::default();

    let hit = |ts: i64| {
        (
            ts,
            Message::DamageEvent(DamageEvent {
                source: Actor { index: 0, actor_type: 0, parent_actor_type: 0, parent_index: 0 },
                target: Actor { index: 0, actor_type: 0, parent_actor_type: 0, parent_index: 0 },
                damage: 100,
                flags: 0,
                action_id: ActionType::Normal(0),
                attack_rate: None,
                stun_value: None,
                damage_cap: None,
                stun_fill: None,
                target_base_type: None,
                stun_max: None,
            }),
        )
    };

    parser.encounter.raw_event_log.push(hit(1_000));
    parser.encounter.raw_event_log.push(hit(211_815));
    parser
        .encounter
        .raw_event_log
        .push((217_000, status_removed(SLOT_0, 0x04)));

    parser.reparse();

    assert_eq!(parser.derived_state.duration(), 210_815);
}

/// Statuses are torn down ~5.5 s after the last hit; only damage dealt extends the encounter.
#[test]
fn a_status_after_the_last_hit_does_not_extend_the_encounter() {
    let mut parser = Parser::default();

    parser.encounter.raw_event_log.push((
        1_000,
        Message::DamageEvent(DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            damage: 100,
            flags: 0,
            action_id: ActionType::Normal(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        }),
    ));

    parser.encounter.raw_event_log.push((
        14_237,
        Message::DamageEvent(DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            damage: 100,
            flags: 0,
            action_id: ActionType::Normal(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        }),
    ));

    parser
        .encounter
        .raw_event_log
        .push((19_761, status_removed(SLOT_0, 0x04)));

    parser.reparse();

    assert_eq!(parser.derived_state.end_time, 14_237, "the last HIT, not the last event");
    assert_eq!(parser.derived_state.duration(), 13_237);
}

#[test]
fn id_dragon_damage_merges_into_id_row() {
    const PL1900_HASH: u32 = 0x8056ABCD;
    const PL2000_HASH: u32 = 0xF5755C0E;
    const ID_ACTOR_INDEX: u32 = 7;

    let mut parser = Parser::default();

    let hit = |actor_type: u32, damage: i32| DamageEvent {
        source: Actor {
            index: ID_ACTOR_INDEX,
            actor_type,
            parent_actor_type: actor_type,
            parent_index: ID_ACTOR_INDEX,
        },
        target: Actor {
            index: 100,
            actor_type: 1,
            parent_actor_type: 1,
            parent_index: 100,
        },
        damage,
        flags: 0,
        action_id: ActionType::Normal(1),
        attack_rate: None,
        stun_value: None,
        damage_cap: None,
        stun_fill: None,
        target_base_type: None,
        stun_max: None,
    };

    parser.on_damage_event(hit(PL1900_HASH, 100));
    parser.on_damage_event(hit(PL2000_HASH, 50));

    parser.on_player_identity_event(protocol::PlayerIdentityEvent {
        actor_index: ID_ACTOR_INDEX,
        party_index: 1,
        character_name: std::ffi::CString::new("Id").unwrap(),
        display_name: std::ffi::CString::new("Cleista").unwrap(),
        character_type: PL2000_HASH,
        is_online: true,
        sigils: Vec::new(),
        weapon_info: None,
        summon_info: None,
        skill_loadout: Vec::new(),
        over_mastery: Vec::new(),
        master_trait_flags: Vec::new(),
        effective_traits: Vec::new(),
        player_stats: None,
        network_user_id: None,
        network_user_name: None,
        master_level: None,
    });

    assert_eq!(parser.derived_state.party.len(), 1);
    let row = &parser.derived_state.party[&ID_ACTOR_INDEX];
    assert_eq!(row.total_damage, 150);
    assert_eq!(row.character_type, CharacterType::Pl1900);

    let placed = parser.encounter.player_data[1]
        .as_ref()
        .expect("identity event should populate the party slot");
    assert_eq!(placed.actor_index, ID_ACTOR_INDEX);
    assert_eq!(placed.character_type, CharacterType::Pl1900);
    assert_eq!(placed.display_name, "Cleista");
}

#[test]
fn identity_event_upserts_build_data() {
    const EMPTY_HASH: u32 = 0x887AE0B0;

    let mut parser = Parser::default();

    let sigil = |sigil_id: u32, level: u32| protocol::Sigil {
        first_trait_id: 0x1111,
        first_trait_level: level,
        second_trait_id: EMPTY_HASH,
        second_trait_level: 0,
        sigil_id,
        equipped_character: 0xABCD,
        sigil_level: level,
        acquisition_count: 0,
        notification_enum: 0,
    };

    let event = |sigil_level: u32, equip_bonus_level: i32| protocol::PlayerIdentityEvent {
        actor_index: 3,
        party_index: 2,
        character_name: std::ffi::CString::new("Eustace").unwrap(),
        display_name: std::ffi::CString::new("Cleista").unwrap(),
        character_type: 0,
        is_online: true,
        sigils: (0..13).map(|i| sigil(0x2000 + i, sigil_level)).collect(),
        weapon_info: Some(protocol::WeaponInfo {
            weapon_id: 0xDED16FCF,
            star_level: 0,
            plus_marks: 0,
            awakening_level: 0,
            trait_1_id: 0x1E1CECCE,
            trait_1_level: 32,
            trait_2_id: 0x2222,
            trait_2_level: 10,
            trait_3_id: 0x3333,
            trait_3_level: 5,
            trait_4_id: 0x4444,
            trait_4_level: 1,
            trait_5_id: EMPTY_HASH,
            trait_5_level: 0,
            wrightstone_id: 0,
            weapon_level: 81,
            transcendence_level: 6,
            awakening_level_er: 10,
            weapon_hp: 0,
            weapon_attack: 0,
            wrightstone_trait_1_id: EMPTY_HASH,
            wrightstone_trait_1_level: 0,
            wrightstone_trait_2_id: EMPTY_HASH,
            wrightstone_trait_2_level: 0,
            wrightstone_trait_3_id: EMPTY_HASH,
            wrightstone_trait_3_level: 0,
        }),
        summon_info: Some(protocol::SummonInfo {
            summons: vec![protocol::SummonSlot {
                id: 0x0F986ED9,
                trait_id: 0x4C588C27, // "War Elemental"
                trait_level: 15,
                equip_bonus_id: 0x54B09A37, // "Chain Burst Damage Up"
                equip_bonus_level,
            }],
        }),
        skill_loadout: vec![0xAA6E61E5, 0xE1410C7B, 0x9ABFC62C, 0xA5391991],
        over_mastery: vec![
            protocol::OverMasteryLine {
                id: 0x45C65767,
                rank: 4,
                value: 4.0,
            },
            protocol::OverMasteryLine {
                id: 0x68B39018,
                rank: 10,
                value: 20.0,
            },
            protocol::OverMasteryLine {
                id: 0x43B7581D,
                rank: 10,
                value: 20.0,
            },
            protocol::OverMasteryLine {
                id: 0x6CB38EF3,
                rank: 8,
                value: 1.2,
            },
        ],
        master_trait_flags: vec![
            protocol::MasterTraitFlag {
                hash: 0x3A583A9C, // SBE_PL0000_SP000 style card
                on: true,
            },
            protocol::MasterTraitFlag {
                hash: 0xFF561659, // cost-50 gem node
                on: false,
            },
        ],
        effective_traits: vec![
            protocol::EffectiveTrait {
                hash: 0xCEB700EE, // Stun Power
                level: 48,
            },
            protocol::EffectiveTrait {
                hash: 0xDC584F60, // DMG Cap
                level: 76,
            },
            protocol::EffectiveTrait {
                hash: 0x4C588C27, // War Elemental
                level: 15,
            },
        ],
        player_stats: Some(protocol::PlayerStats {
            level: 12,
            total_hp: 115643,
            total_attack: 77046,
            stun_power: 25.5,
            critical_rate: 21.0,
            total_power: 53470,
            dmg_cap_channels: [1.5, 2.5, 3.5],
        }),
        network_user_id: None,
        network_user_name: None,
        master_level: None,
    };

    parser.on_player_identity_event(event(10, 4));
    parser.on_player_identity_event(event(11, 5));

    let placed = parser.encounter.player_data[2]
        .as_ref()
        .expect("identity event should populate the party slot");
    assert_eq!(placed.sigils.len(), 13);
    assert_eq!(placed.sigils[0].sigil_level, 11);
    let weapon = placed.weapon_info.as_ref().expect("weapon info");
    assert_eq!(weapon.weapon_id, 0xDED16FCF);
    assert_eq!(weapon.weapon_level, 81);
    assert_eq!(weapon.transcendence_level, 6);
    assert_eq!(weapon.awakening_level_er, 10);
    assert_eq!(weapon.trait_1_level, 32);
    assert_eq!(weapon.trait_4_id, 0x4444);
    let summons = placed.summon_info.as_ref().expect("summon info");
    assert_eq!(summons.summons.len(), 1);
    assert_eq!(summons.summons[0].trait_id, 0x4C588C27);
    assert_eq!(summons.summons[0].trait_level, 15);
    assert_eq!(summons.summons[0].equip_bonus_id, 0x54B09A37);
    // The second event's equip-bonus level wins.
    assert_eq!(summons.summons[0].equip_bonus_level, 5);
    assert_eq!(placed.skill_loadout, vec![0xAA6E61E5, 0xE1410C7B, 0x9ABFC62C, 0xA5391991]);
    assert!(placed.overmastery_info.is_none());
    assert_eq!(placed.over_mastery.len(), 4);
    assert_eq!(placed.over_mastery[0].id, 0x45C65767);
    assert_eq!(placed.over_mastery[0].rank, 4);
    assert_eq!(placed.over_mastery[0].value, 4.0);
    assert_eq!(placed.over_mastery[1].rank, 10);
    assert_eq!(placed.over_mastery[1].value, 20.0);
    assert_eq!(placed.over_mastery[2].id, 0x43B7581D);
    assert_eq!(placed.over_mastery[3].id, 0x6CB38EF3);
    assert_eq!(placed.over_mastery[3].rank, 8);
    assert_eq!(placed.over_mastery[3].value, 1.2);
    assert_eq!(placed.master_trait_flags.len(), 2);
    assert_eq!(placed.master_trait_flags[0].hash, 0x3A583A9C);
    assert!(placed.master_trait_flags[0].on);
    assert!(!placed.master_trait_flags[1].on);
    assert_eq!(placed.effective_traits.len(), 3);
    assert_eq!(placed.effective_traits[0].hash, 0xCEB700EE);
    assert_eq!(placed.effective_traits[0].level, 48);
    assert_eq!(placed.effective_traits[1].hash, 0xDC584F60);
    assert_eq!(placed.effective_traits[1].level, 76);
    assert_eq!(placed.effective_traits[2].hash, 0x4C588C27);
    assert_eq!(placed.effective_traits[2].level, 15);
    let stats = placed.player_stats.as_ref().expect("player stats");
    assert_eq!(stats.level, 12);
    assert_eq!(stats.total_hp, 115643);
    assert_eq!(stats.total_attack, 77046);
    assert_eq!(stats.stun_power, 25.5);
    assert_eq!(stats.critical_rate, 21.0);
    assert_eq!(stats.total_power, 53470);
    assert_eq!(stats.dmg_cap_channels, [1.5, 2.5, 3.5]);

    assert!(parser.encounter.player_data[0].is_none());
    assert!(parser.encounter.player_data[1].is_none());
    assert!(parser.encounter.player_data[3].is_none());
}


/// One hit on enemy `tgt`; `fill` is the gauge fraction AFTER the hit, `stun_max: None` models an old log.
fn stun_hit(tgt: u32, raw: f32, fill: Option<f32>, max: Option<f32>) -> DamageEvent {
    DamageEvent {
        source: Actor { index: 0, actor_type: 0, parent_actor_type: 0, parent_index: 0 },
        target: Actor { index: tgt, actor_type: 0, parent_actor_type: 0, parent_index: tgt },
        damage: 1,
        flags: 0,
        action_id: ActionType::Normal(0),
        attack_rate: None,
        stun_value: Some(raw),
        damage_cap: None,
        stun_fill: fill,
        target_base_type: None,
        stun_max: max,
    }
}

#[test]
fn old_logs_without_stun_max_pass_raw_stun_through() {
    let mut r = StunReconstructor::default();
    assert_eq!(r.counted_stun(&stun_hit(1, 500.0, Some(0.99), None)), 500.0);
    assert_eq!(r.counted_stun(&stun_hit(1, 500.0, Some(0.99), None)), 500.0);
}

#[test]
fn below_saturation_the_raw_stun_is_credited_in_full() {
    let mut r = StunReconstructor::default();
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(0.10), Some(1000.0))), 100.0);
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(0.20), Some(1000.0))), 100.0);
}

#[test]
fn a_hit_that_reaches_the_cap_is_clamped_to_the_remaining_capacity() {
    let mut r = StunReconstructor::default();
    for _ in 0..9 {
        assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(0.5), Some(1000.0))), 100.0);
    }
    assert_eq!(r.counted_stun(&stun_hit(1, 50.0, Some(0.95), Some(1000.0))), 50.0);
    // Gauge at 950/1000: the 200-stun hit is capped to the last 50.
    assert_eq!(r.counted_stun(&stun_hit(1, 200.0, Some(0.98), Some(1000.0))), 50.0);
}

#[test]
fn a_break_resets_the_gauge_so_later_hits_count_again() {
    let mut r = StunReconstructor::default();
    assert_eq!(r.counted_stun(&stun_hit(1, 1000.0, Some(1.0), Some(1000.0))), 1000.0);
    // A further hit while saturated counts nothing.
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(1.0), Some(1000.0))), 0.0);
    // The breaking hit counts against the pre-break gauge (full, so nothing), then the gauge resets.
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(0.0), Some(1000.0))), 0.0);
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(0.10), Some(1000.0))), 100.0);
}

#[test]
fn gauges_are_tracked_independently_per_enemy() {
    let mut r = StunReconstructor::default();
    assert_eq!(r.counted_stun(&stun_hit(1, 1000.0, Some(1.0), Some(1000.0))), 1000.0);
    assert_eq!(r.counted_stun(&stun_hit(1, 100.0, Some(1.0), Some(1000.0))), 0.0);
    assert_eq!(r.counted_stun(&stun_hit(2, 100.0, Some(0.10), Some(1000.0))), 100.0);
}

#[test]
fn stun_against_an_unbreakable_gauge_counts_for_nothing() {
    let mut r = StunReconstructor::default();
    // The ER sentinel: capacity 1e9, so nothing is credited.
    for _ in 0..42 {
        assert_eq!(r.counted_stun(&stun_hit(1, 150.24, Some(4.2e-7), Some(1e9))), 0.0);
    }
    assert_eq!(r.counted_stun(&stun_hit(2, 150.0, Some(0.10), Some(12_000.0))), 150.0);
}

#[test]
fn an_unbreakable_hit_records_no_gauge_state() {
    let mut r = StunReconstructor::default();
    assert_eq!(r.counted_stun(&stun_hit(1, 900.0, Some(4.2e-7), Some(1e9))), 0.0);
    assert_eq!(r.counted_stun(&stun_hit(1, 900.0, Some(0.10), Some(1000.0))), 900.0);
}

#[test]
fn host_and_client_converge_on_the_breaking_hit() {
    let seq = [
        (300.0f32, 0.30f32),
        (300.0, 0.60),
        (300.0, 0.90),
        (300.0, 0.00), // breaking hit: raw 300, gauge was 900 -> only 100 counts
    ];
    let mut total = 0.0;
    let mut r = StunReconstructor::default();
    for (raw, fill) in seq {
        total += r.counted_stun(&stun_hit(1, raw, Some(fill), Some(1000.0)));
    }
    // 300 + 300 + 300 (=900, all fit) + 100 (capped) = 1000 = one full gauge.
    assert_eq!(total, 1000.0);
}

// Only the log-shape helper lives here. `pair_status_intervals` in
// `commands.rs` owns the uptime rules and their tests. The
// encounter-boundary tests above need a status event only as a non-damage
// event arriving after the last hit.

/// A removal that takes the last of its kind away.
fn status_removed(actor_index: u32, status_id: u32) -> Message {
    Message::OnStatusRemoved(protocol::StatusRemovedEvent {
        actor_index,
        status_id,
        stacks: Some(0),
        stacks_before: None,
        value_total: None,
        value_is_fraction: None,
        applier_index: None,
        source_ids: None,
        value: None,
    })
}

const SLOT_0: u32 = protocol::PLAYER_ID_BASE;

#[test]
fn the_live_payload_omits_the_growing_maps() {
    // `targets` grows for the life of an encounter and the meter does not
    // render it, so the per-hit emit must not carry it.
    let mut parser = Parser::default();
    parser.on_damage_event(player_damage_event());

    let full = serde_json::to_value(&parser.derived_state).unwrap();
    let slim = serde_json::to_value(parser.derived_state.live_payload()).unwrap();

    // Guard the contrast, so a rename cannot hollow out the assertion.
    assert!(full.get("targets").is_some());

    assert!(slim.get("targets").is_none());

    for key in ["startTime", "endTime", "totalDamage", "dps", "status", "party"] {
        assert!(slim.get(key).is_some(), "live payload lost {key}");
    }
    assert_eq!(slim["totalDamage"], 100);
    assert!(slim["party"].get("0").is_some());
}

#[test]
fn the_emit_gate_coalesces_a_burst_to_one_emit_per_interval() {
    let mut gate = LiveEmitGate::default();

    assert!(gate.should_emit(10_000));
    assert!(!gate.should_emit(10_001));
    assert!(!gate.should_emit(10_000 + LIVE_EMIT_INTERVAL_MILLIS - 1));
    assert!(gate.should_emit(10_000 + LIVE_EMIT_INTERVAL_MILLIS));
}

#[test]
fn a_suppressed_update_is_flushed_exactly_once() {
    let mut gate = LiveEmitGate::default();

    assert!(gate.should_emit(10_000));
    assert!(!gate.should_emit(10_001)); // held

    assert!(gate.take_pending(10_200));
    assert!(!gate.take_pending(10_400));
}

#[test]
fn an_unconditional_emit_drops_the_pending_update() {
    let mut gate = LiveEmitGate::default();

    assert!(gate.should_emit(10_000));
    assert!(!gate.should_emit(10_001)); // held
    gate.emitted(10_050); // e.g. quest complete emitted unconditionally

    assert!(!gate.take_pending(10_300));
}

#[test]
fn a_flush_restarts_the_throttle_window() {
    let mut gate = LiveEmitGate::default();

    let flushed_at = 10_000 + LIVE_EMIT_INTERVAL_MILLIS / 2;
    assert!(gate.should_emit(10_000));
    assert!(!gate.should_emit(10_001)); // held
    assert!(gate.take_pending(flushed_at));

    // The window is measured from the flush, not from the original emit.
    assert!(!gate.should_emit(flushed_at + LIVE_EMIT_INTERVAL_MILLIS - 1));
    assert!(gate.should_emit(flushed_at + LIVE_EMIT_INTERVAL_MILLIS));
}

/// Eustace's Flamek Thunder: one hit on the enemy, then the identical damage
/// on the Heaven Comes Down mark 33 ms later, under the same action id.
mod helper_targets {
    use super::*;

    /// Sir Barrold, the training-room dummy.
    const BARROLD: u32 = 0xA379_AC65;
    /// `Pl2700MarkingTarget`, the mark Heaven Comes Down leaves on the field.
    const MARK: u32 = 0xA90F_5847;
    /// Flamek Thunder Lv3.
    const FLAMEK_THUNDER: ActionType = ActionType::Normal(1602);
    const HIT: i32 = 3_904_545;

    fn flamek_thunder(target_index: u32, target_type: u32) -> DamageEvent {
        let mut event = player_damage_event();
        event.source.actor_type = 0x9141_8145; // Pl2700, Eustace
        event.source.parent_actor_type = 0x9141_8145;
        event.target = Actor {
            index: target_index,
            actor_type: target_type,
            parent_actor_type: target_type,
            parent_index: target_index,
        };
        event.action_id = FLAMEK_THUNDER;
        event.damage = HIT;
        event
    }

    fn assert_counted_once(parser: &Parser) {
        assert_eq!(parser.derived_state.total_damage, HIT as u64);
        assert_eq!(parser.derived_state.party[&0].total_damage, HIT as u64);

        let skill = parser.derived_state.party[&0]
            .skill_breakdown
            .iter()
            .find(|s| s.action_type == FLAMEK_THUNDER)
            .expect("the move must still have a row");
        assert_eq!(skill.hits, 1);
        assert_eq!(skill.total_damage, HIT as u64);

        // The mark must not appear beside the enemy it was stuck to.
        assert_eq!(parser.derived_state.targets.len(), 1);
        assert_eq!(
            parser.derived_state.get_primary_target().unwrap().raw_target_type,
            BARROLD
        );
    }

    #[test]
    fn a_hit_on_the_mark_is_not_a_second_hit_on_the_enemy() {
        let mut parser = Parser::default();

        parser.on_damage_event(flamek_thunder(1, BARROLD));
        parser.on_damage_event(flamek_thunder(2, MARK));

        assert_counted_once(&parser);
        // Never recorded, so it cannot come back on a reparse.
        assert_eq!(parser.encounter.raw_event_log.len(), 1);
    }

    #[test]
    fn a_log_saved_with_the_duplicate_still_reparses_to_one_hit() {
        let mut parser = Parser::default();
        parser.encounter.raw_event_log = vec![
            (1_000, Message::DamageEvent(flamek_thunder(1, BARROLD))),
            (1_033, Message::DamageEvent(flamek_thunder(2, MARK))),
        ];

        parser.reparse();

        assert_counted_once(&parser);
        // The capture itself is left intact.
        assert_eq!(parser.encounter.raw_event_log.len(), 2);
    }

    #[test]
    fn the_per_enemy_view_ignores_the_mark_too() {
        let mut parser = Parser::default();
        parser.encounter.raw_event_log = vec![
            (1_000, Message::DamageEvent(flamek_thunder(1, BARROLD))),
            (1_033, Message::DamageEvent(flamek_thunder(2, MARK))),
        ];

        assert_eq!(parser.derived_state_for_enemy(1).total_damage, HIT as u64);
        assert_eq!(parser.derived_state_for_enemy(2).total_damage, 0);
    }

    #[test]
    fn two_real_enemies_taking_the_same_hit_both_count() {
        let mut parser = Parser::default();

        parser.on_damage_event(flamek_thunder(1, BARROLD));
        parser.on_damage_event(flamek_thunder(2, BARROLD));

        assert_eq!(parser.derived_state.total_damage, 2 * HIT as u64);
        assert_eq!(parser.derived_state.targets.len(), 2);
    }
}
