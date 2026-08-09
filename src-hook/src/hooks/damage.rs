use std::collections::{HashMap, HashSet};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use protocol::{ActionType, Actor, DamageEvent, Message};
use retour::static_detour;

use crate::{event, hooks::ffi::DamageInstance, process::Process};

use super::gbfr_hash::gbfr_hash;
use super::status;
use super::{
    actor_idx, actor_type_id, get_source_parent, is_local_sim_only_class, probe_actor,
    resolve_source_identity, safe_read, source_is_local_for_dedup,
};

// Object uid: `(category << 16) | class_number` in hex, so 0x0103_7701 -> "We7701".
// Named variants share one class type-id, so the uid is their only identity.
const TARGET_INSTANCE_UID_OFFSET: usize = 0x1FC;

const TARGET_INSTANCE_UID_UNSET: u32 = 0xFFFF_FFFF;

static ACTOR_TYPE_CACHE: OnceLock<Mutex<HashMap<u32, u32>>> = OnceLock::new();

/// Summon uids hold the So-number in the high byte; page 0xFF00 is a shared camera/helper dummy.
fn specific_type_id_from_uid(uid: u32) -> Option<u32> {
    let (prefix, mask) = match uid >> 16 {
        0x0103 => ("We", 0xFFFF),
        0x0002 => ("Em", 0xFFFF),
        0x000F => ("Ba", 0xFFFF),
        0x000E => ("Bh", 0xFFFF),
        0x010A => ("Np", 0xFFFF),
        0x010D if (uid & 0xFF00) != 0xFF00 => ("So", 0xFF00),
        _ => return None,
    };
    Some(gbfr_hash(&format!("{}{:04x}", prefix, uid & mask)))
}

/// Reads the uid from the spawn descriptor's getter vfunc. Preferred over
/// +0x1FC, which some class inits (Em1500 Golem) re-stamp with their base uid.
fn descriptor_uid(instance: *const usize) -> Option<u32> {
    let base = instance as *const u8;
    let holder = safe_read::<*const u8>(base.wrapping_add(0x28) as *const *const u8)?;
    if holder.is_null() {
        return None;
    }
    let desc = safe_read::<*const u8>(holder as *const *const u8)?;
    if desc.is_null() {
        return None;
    }
    let vt = safe_read::<*const u8>(desc as *const *const u8)?;
    if vt.is_null() {
        return None;
    }
    let getter = safe_read::<*const u8>(vt.wrapping_add(0x18) as *const *const u8)?;
    if getter.is_null() {
        return None;
    }
    let body = safe_read::<[u8; 4]>(getter as *const [u8; 4])?;
    if body[0] != 0x8B || body[1] != 0x41 || body[3] != 0xC3 {
        return None;
    }
    safe_read::<u32>(desc.wrapping_add(body[2] as usize) as *const u32)
}

fn instance_uid(instance: *const usize) -> Option<u32> {
    descriptor_uid(instance)
        .or_else(|| {
            safe_read::<u32>(
                (instance as *const u8).wrapping_add(TARGET_INSTANCE_UID_OFFSET) as *const u32,
            )
        })
        .filter(|&uid| uid != TARGET_INSTANCE_UID_UNSET)
}

fn resolve_specific_actor_type(instance: *const usize, vfunc_hash: u32) -> u32 {
    let Some(uid) = instance_uid(instance) else {
        return vfunc_hash;
    };
    resolve_uid_actor_type(uid, vfunc_hash)
}

fn resolve_uid_actor_type(uid: u32, vfunc_hash: u32) -> u32 {
    let cache = ACTOR_TYPE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some(&resolved) = cache.get(&uid) {
        return resolved;
    }

    let decoded = specific_type_id_from_uid(uid).unwrap_or(vfunc_hash);
    let resolved = if decoded != vfunc_hash && decoded != 0 {
        #[cfg(feature = "console")]
        println!(
            "actor variant re-type: vfunc={:#010x} uid={:#010x} resolved={:#010x}",
            vfunc_hash, uid, decoded
        );
        decoded
    } else {
        vfunc_hash
    };

    cache.insert(uid, resolved);
    resolved
}

/// Uid category "Bh": destructible scenery, never an enemy. "Ba" (0x000F)
/// mixes real minibosses with props, so it is not flagged.
const DESTRUCTIBLE_UID_CATEGORY: u32 = 0x000E;

fn resolve_target_type(instance: *const usize, vfunc_hash: u32) -> (u32, Option<u32>, bool) {
    let uid = instance_uid(instance);
    let is_destructible = matches!(uid, Some(uid) if (uid >> 16) == DESTRUCTIBLE_UID_CATEGORY);
    let resolved = match uid {
        Some(uid) => resolve_uid_actor_type(uid, vfunc_hash),
        None => vfunc_hash,
    };
    let flavored = uid
        .and_then(super::subname::resolve_flavor_type_id)
        .filter(|&hash| hash != resolved);
    let (type_id, base_fallback) = match flavored {
        Some(hash) => (hash, Some(resolved)),
        None => (resolved, (resolved != vfunc_hash).then_some(vfunc_hash)),
    };
    (type_id, base_fallback, is_destructible)
}

// The perfect-guard counter's internal shot id (0x263/611). Its shot pipeline
// never writes `action_id`, so flags bit 38 marks it instead.
pub(crate) const PERFECT_BLOCK_COUNTER_ACTION_ID: u32 = 0x263;
const PERFECT_BLOCK_COUNTER_FLAG_BIT: u64 = 1 << 38;

pub(crate) fn classify_action(flags: u64, action_id: u32) -> ActionType {
    if ((1 << 7 | 1 << 50) & flags) != 0 {
        ActionType::LinkAttack
    } else if ((1 << 13 | 1 << 14) & flags) != 0 {
        ActionType::SBA
    } else if ((1 << 15) & flags) != 0 {
        ActionType::SupplementaryDamage(action_id)
    } else {
        ActionType::Normal(action_id)
    }
}

// The break-gauge fields at tgt+0xB24..0xBB0, read as one guarded block.
const STUN_BLOCK_BASE: usize = 0xB24;
const STUN_BLOCK_LEN: usize = 0x8C;
const STUN_OFF_BREAK_TIMER: usize = 0xB24 - STUN_BLOCK_BASE;
const STUN_OFF_RECOVER_TIMER: usize = 0xB30 - STUN_BLOCK_BASE;
const STUN_OFF_IMMUNE_GAUGE: usize = 0xB58 - STUN_BLOCK_BASE;
const STUN_OFF_ENABLED: usize = 0xB59 - STUN_BLOCK_BASE;
const STUN_OFF_CURRENT: usize = 0xB90 - STUN_BLOCK_BASE;
const STUN_OFF_MAX: usize = 0xB94 - STUN_BLOCK_BASE;
const STUN_OFF_TAKEN_BONUS: usize = 0xBAC - STUN_BLOCK_BASE;
/// Target Overdrive/Break state on the damage-event struct (`a2`), not on the
/// enemy instance; the game reads these bytes back to gate the Assassin bonuses.
const TARGET_IN_OVERDRIVE_OFFSET: usize = 0x162;
const TARGET_IN_BREAK_OFFSET: usize = 0x163;

/// The same state on the enemy, as a mode enum. Preferred over the two ctx
/// bytes above, which can come back clear on this hook's path.
const TARGET_MODE_OFFSET: usize = 0x1A60;
const TARGET_MODE_OVERDRIVE: [i32; 2] = [1, 4];
const TARGET_MODE_BREAK: [i32; 2] = [2, 5];

const STUN_IMMUNE_CLASS_OFFSET: usize = 0x14E0;
const STUN_IMMUNE_HARD_OFFSET: usize = 0x16B8;
const STUN_IMMUNE_FLAG_OFFSET: usize = 0x1863;

type ProcessDamageEventFunc =
    unsafe extern "system" fn(*const usize, *const usize, *const usize, u8) -> usize;

type ProcessDotEventFunc = unsafe extern "system" fn(*const usize, i32, *const usize) -> usize;

static_detour! {
    static ProcessDamageEvent: unsafe extern "system" fn(*const usize, *const usize, *const usize, u8) -> usize;
    static ProcessDotEvent: unsafe extern "system" fn(*const usize, i32, *const usize) -> usize;
}

static ENEMY_DEAD_SEEN: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();

fn enemy_dead_seen() -> &'static Mutex<HashSet<u32>> {
    ENEMY_DEAD_SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

const ENEMY_HP_OFFSET: usize = 0x160;

fn note_enemy_death(tx: &event::Tx, target_index: u32, actor_type: u32, target: *const u8) {
    let Some(hp) = safe_read::<u64>(target.wrapping_add(ENEMY_HP_OFFSET) as *const u64) else {
        return;
    };
    if hp != 0 {
        return;
    }

    let first = match enemy_dead_seen().lock() {
        Ok(mut seen) => seen.insert(target_index),
        Err(_) => false,
    };
    if !first {
        return;
    }

    let _ = tx.send(Message::OnEnemyDeath(protocol::EnemyDeathEvent {
        target_index,
        actor_type,
    }));
}

static ENEMY_MODE_SEEN: OnceLock<Mutex<HashMap<u32, (bool, bool)>>> = OnceLock::new();
static SBA_WINDOW_SEEN: AtomicU8 = AtomicU8::new(0);

fn enemy_mode_seen() -> &'static Mutex<HashMap<u32, (bool, bool)>> {
    ENEMY_MODE_SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn clear_state_transitions() {
    if let Ok(mut seen) = enemy_mode_seen().lock() {
        seen.clear();
    }
    if let Ok(mut dead) = enemy_dead_seen().lock() {
        dead.clear();
    }
    SBA_WINDOW_SEEN.store(0, Ordering::Relaxed);
}

fn note_state_transitions(
    tx: &event::Tx,
    target_index: u32,
    in_overdrive: Option<bool>,
    in_break: Option<bool>,
    sba_active: Option<bool>,
) {
    if let (Some(in_overdrive), Some(in_break)) = (in_overdrive, in_break) {
        let changed = match enemy_mode_seen().lock() {
            Ok(mut seen) => seen.insert(target_index, (in_overdrive, in_break))
                != Some((in_overdrive, in_break)),
            Err(_) => false,
        };
        if changed {
            let _ = tx.send(Message::OnEnemyModeChange(protocol::EnemyModeChangeEvent {
                target_index,
                in_overdrive,
                in_break,
            }));
        }
    }

    if let Some(active) = sba_active {
        let encoded = if active { 2 } else { 1 };
        if SBA_WINDOW_SEEN.swap(encoded, Ordering::Relaxed) != encoded {
            let _ = tx.send(Message::OnSbaWindowChange(protocol::SbaWindowChangeEvent {
                active,
            }));
        }
    }
}

/// Whether this hit's damage should be discarded as re-simulation while its
/// stun is kept. `original_value == 0` means the game rejected the hit, so the
/// player never saw a damage number for it.
///
/// Local-sim-only classes are exempt. Ferry's Umlauf satellite returns 0 for
/// hits that really landed, and this rule would delete the skill outright.
fn should_zero_rejected_damage(
    a4: u8,
    original_value: usize,
    damage: i32,
    source_type_id: u32,
) -> bool {
    a4 == 0
        && original_value == 0
        && damage > 0
        && !is_local_sim_only_class(source_type_id)
}

/// Learned from calls whose source did resolve; attributes replicated records
/// that arrive with a zeroed source handle.
#[derive(Clone, Copy)]
struct ActionOwner {
    parent_idx: u32,
    parent_type: u32,
    source_type: u32,
    ambiguous: bool,
}

static ACTION_OWNERS: OnceLock<Mutex<HashMap<u32, ActionOwner>>> = OnceLock::new();

fn lock_action_owners() -> std::sync::MutexGuard<'static, HashMap<u32, ActionOwner>> {
    match ACTION_OWNERS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn note_action_owner(action_id: u32, parent_idx: u32, parent_type: u32, source_type: u32) {
    if parent_idx < protocol::PLAYER_ID_BASE {
        return;
    }
    lock_action_owners()
        .entry(action_id)
        .and_modify(|owner| {
            if owner.parent_idx != parent_idx {
                owner.ambiguous = true;
            }
        })
        .or_insert(ActionOwner {
            parent_idx,
            parent_type,
            source_type,
            ambiguous: false,
        });
}

fn owner_for_action(action_id: u32) -> Option<ActionOwner> {
    lock_action_owners()
        .get(&action_id)
        .copied()
        .filter(|o| !o.ambiguous)
}

pub(crate) fn clear_action_owners() {
    lock_action_owners().clear();
}

static REPLICATED_SOURCES: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();

fn replicated_sources() -> &'static Mutex<HashSet<u32>> {
    REPLICATED_SOURCES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn lock_replicated_sources() -> std::sync::MutexGuard<'static, HashSet<u32>> {
    match replicated_sources().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplicationStatus {
    delivers: bool,
    newly_marked: bool,
}

fn note_and_check_replicated(source_idx: u32, a4: u8) -> ReplicationStatus {
    let mut sources = lock_replicated_sources();
    let newly_marked = a4 != 0 && sources.insert(source_idx);
    ReplicationStatus {
        delivers: sources.contains(&source_idx),
        newly_marked,
    }
}

static UNLEARNED_KEEPS: OnceLock<Mutex<HashMap<u32, (u32, bool)>>> = OnceLock::new();

const UNLEARNED_KEEP_WARN_AT: u32 = 200;

fn note_unlearned_keep(source_idx: u32, source_type_id: u32) {
    let counts = UNLEARNED_KEEPS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut counts = match counts.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let entry = counts.entry(source_idx).or_insert((0, false));
    entry.0 += 1;
    if entry.0 > UNLEARNED_KEEP_WARN_AT && !entry.1 {
        entry.1 = true;
        log::warn!(
            "[dedup] source {source_idx} ({source_type_id:#010x}) has had {} local-sim hits kept \
             without ever delivering a replicated one — if this player's damage reads high, this \
             is the cause",
            entry.0
        );
    }
}

const UNLEARNED_HOLD: Duration = Duration::from_millis(50);

struct PendingFrame {
    held_at: Instant,
    source_type_id: u32,
    action_id: ActionType,
    event: Message,
}

/// Frames parked because their source is not classified yet. Sending one before
/// the source's first replicated call could count it twice.
static PENDING_UNLEARNED: OnceLock<Mutex<HashMap<u32, Vec<PendingFrame>>>> = OnceLock::new();

static PENDING_UNLEARNED_COUNT: AtomicUsize = AtomicUsize::new(0);

static PENDING_TX: OnceLock<event::Tx> = OnceLock::new();

fn lock_pending_unlearned() -> std::sync::MutexGuard<'static, HashMap<u32, Vec<PendingFrame>>>
{
    let map = PENDING_UNLEARNED.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn hold_unlearned_frame(
    source_idx: u32,
    source_type_id: u32,
    action_id: ActionType,
    event: Message,
) {
    lock_pending_unlearned()
        .entry(source_idx)
        .or_default()
        .push(PendingFrame {
            held_at: Instant::now(),
            source_type_id,
            action_id,
            event,
        });
    PENDING_UNLEARNED_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Per action, not per source: a whole-buffer discard would delete moves that
/// never replicate at all.
fn discard_pending_unlearned(source_idx: u32, source_type_id: u32, action_id: ActionType) {
    let mut dropped = 0usize;
    {
        let mut pending = lock_pending_unlearned();
        if let Some(frames) = pending.get_mut(&source_idx) {
            let before = frames.len();
            frames.retain(|f| f.action_id != action_id);
            dropped = before - frames.len();
            if frames.is_empty() {
                pending.remove(&source_idx);
            }
        }
    }
    if dropped > 0 {
        PENDING_UNLEARNED_COUNT.fetch_sub(dropped, Ordering::Relaxed);
        log::debug!(
            "[dedup] source {source_idx} ({source_type_id:#010x}) delivered a replicated record \
             for {action_id:?}; discarded {dropped} parked local-sim frame(s) of that action"
        );
    }
}

fn release_expired_unlearned(tx: &event::Tx) {
    release_expired_at(tx, Instant::now(), UNLEARNED_HOLD)
}

fn release_expired_at(tx: &event::Tx, now: Instant, hold: Duration) {
    if PENDING_UNLEARNED_COUNT.load(Ordering::Relaxed) == 0 {
        return;
    }
    let mut released = Vec::new();
    {
        let mut pending = lock_pending_unlearned();
        pending.retain(|_, frames| {
            // Ages decrease with arrival order, so the expired frames are a prefix.
            let split = frames.partition_point(|f| now.duration_since(f.held_at) >= hold);
            released.extend(frames.drain(..split));
            !frames.is_empty()
        });
    }
    emit_released(tx, released);
}

fn flush_pending_unlearned() {
    if PENDING_UNLEARNED_COUNT.load(Ordering::Relaxed) == 0 {
        return;
    }
    let released: Vec<PendingFrame> = lock_pending_unlearned()
        .drain()
        .flat_map(|(_, frames)| frames)
        .collect();
    match PENDING_TX.get() {
        Some(tx) => emit_released(tx, released),
        // No channel registered (unit tests): keep the counter honest.
        None => {
            PENDING_UNLEARNED_COUNT.fetch_sub(released.len(), Ordering::Relaxed);
        }
    }
}

fn emit_released(tx: &event::Tx, released: Vec<PendingFrame>) {
    if released.is_empty() {
        return;
    }
    PENDING_UNLEARNED_COUNT.fetch_sub(released.len(), Ordering::Relaxed);
    for frame in released {
        if let Message::DamageEvent(ref damage) = frame.event {
            note_unlearned_keep(damage.source.index, frame.source_type_id);
        }
        let _ = tx.send(frame.event);
    }
}

pub(crate) fn clear_replicated_sources() {
    // Flush before the classification these frames are keyed by is dropped.
    flush_pending_unlearned();
    lock_replicated_sources().clear();
    if let Some(counts) = UNLEARNED_KEEPS.get() {
        match counts.lock() {
            Ok(mut counts) => counts.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }
}

/// A remote source's local-sim calls are suppressed only once it has delivered
/// a replicated one. A session property, not a class property.
fn is_duplicate_frame(
    source_is_local: Option<bool>,
    a4: u8,
    source_type_id: u32,
    source_delivers_replicated: bool,
    action_type: ActionType,
) -> bool {
    match source_is_local {
        Some(true) => a4 != 0,
        Some(false) => {
            a4 == 0
                && source_delivers_replicated
                && !is_local_sim_only_class(source_type_id)
                && !is_never_replicated_action(action_type)
        }
        None => false,
    }
}

/// The perfect-block counter is sourced from the blocking player's own class,
/// so the exemption is per action; exempting the class would exempt its whole kit.
fn is_never_replicated_action(action_type: ActionType) -> bool {
    matches!(
        action_type,
        ActionType::Normal(PERFECT_BLOCK_COUNTER_ACTION_ID)
    )
}

#[derive(Clone)]
pub struct OnProcessDamageHook {
    tx: event::Tx,
}

const PROCESS_DAMAGE_EVENT_SIG: &str = "e8 $ { ' } 66 83 bc 24 ? ? ? ? ?";

impl OnProcessDamageHook {
    /// Emits a replicated record whose source handle arrived zeroed, attributing
    /// it to the party member the local-sim stream showed owns this action id.
    fn recover_source_less_hit(&self, a2: *const usize, target: usize, a4: u8) {
        if a4 == 0 {
            return;
        }

        // Something in the argument block was already unexpected, so assume
        // nothing about a2: a panic unwinding out of a detour crashes the game.
        let Some(instance) = NonNull::new(a2 as *mut DamageInstance) else {
            return;
        };
        let instance = unsafe { instance.as_ref() };
        let damage = instance.damage;
        if damage <= 0 {
            return;
        }

        let Some(owner) = owner_for_action(instance.action_id) else {
            return;
        };

        let flags = instance.flags;
        let action_type: ActionType = if ((1 << 7 | 1 << 50) & flags) != 0 {
            ActionType::LinkAttack
        } else if ((1 << 13 | 1 << 14) & flags) != 0 {
            ActionType::SBA
        } else if ((1 << 15) & flags) != 0 {
            ActionType::SupplementaryDamage(instance.action_id)
        } else {
            ActionType::Normal(instance.action_id)
        };

        let target_ptr = target as *const usize;
        let target_vfunc_type_id = actor_type_id(target_ptr);
        let (target_type_id, target_base_type, target_is_destructible) =
            resolve_target_type(target_ptr, target_vfunc_type_id);
        if target_is_destructible {
            return;
        }
        let target_idx = actor_idx(target_ptr);

        let _ = self.tx.send(Message::DamageEvent(DamageEvent {
            source: Actor {
                index: owner.parent_idx,
                actor_type: owner.source_type,
                parent_index: owner.parent_idx,
                parent_actor_type: owner.parent_type,
            },
            target: Actor {
                index: target_idx,
                actor_type: target_type_id,
                parent_index: target_idx,
                parent_actor_type: target_type_id,
            },
            damage,
            flags,
            action_id: action_type,
            attack_rate: Some(instance.attack_rate),
            damage_cap: Some(instance.damage_cap),
            stun_value: None,
            stun_fill: None,
            target_base_type,
            stun_max: None,
            // Replicated records only reach here (the `a4 == 0` bail above), and
            // the clamp block never runs on that path, so there is nothing to
            // report -- the same reason `damage_cap` arrives as a sentinel.
            hit_calc: None,
        }));
    }

    pub fn new(tx: event::Tx) -> Self {
        OnProcessDamageHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        let _ = PENDING_TX.set(self.tx.clone());

        if let Ok(process_dmg_evt) = process.search_address(PROCESS_DAMAGE_EVENT_SIG) {
            #[cfg(feature = "console")]
            println!("Found process dmg event");

            unsafe {
                let func: ProcessDamageEventFunc = std::mem::transmute(process_dmg_evt);

                ProcessDamageEvent
                    .initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;

                ProcessDamageEvent.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find process_dmg_evt"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize, a3: *const usize, a4: u8) -> usize {
        // Target is the instance of the actor being damaged.
        // For example: Instance of the Em2700 class.
        let target_specified_instance_ptr: usize = unsafe { *(*a1.byte_add(0x08) as *const usize) };

        // Stun comes from the target's break-gauge delta across the call, or, on
        // a client where the gauge is host-authoritative, the per-hit stun.
        let tgt = target_specified_instance_ptr as *const u8;
        let per_hit_stun_pre =
            safe_read::<f32>(unsafe { std::ptr::addr_of!((*(a2 as *const DamageInstance)).stun) });

        let stun_block = safe_read::<[u8; STUN_BLOCK_LEN]>(
            tgt.wrapping_add(STUN_BLOCK_BASE) as *const [u8; STUN_BLOCK_LEN],
        );
        let block_f32 = |off: usize, default: f32| match stun_block {
            Some(b) => f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]),
            None => default,
        };
        let block_u8_set = |off: usize| matches!(stun_block, Some(b) if b[off] != 0);

        let stun_enabled = block_u8_set(STUN_OFF_ENABLED);
        let stun_break_timer = block_f32(STUN_OFF_BREAK_TIMER, f32::MAX);
        let stun_recover_timer = block_f32(STUN_OFF_RECOVER_TIMER, f32::MAX);
        let stun_taken_bonus = block_f32(STUN_OFF_TAKEN_BONUS, 0.0);
        let stun_max = block_f32(STUN_OFF_MAX, 0.0);
        let stun_current_before = block_f32(STUN_OFF_CURRENT, 0.0);
        let stun_immune_gauge = block_u8_set(STUN_OFF_IMMUNE_GAUGE);
        // Read before the call: post-call, the hit that causes a break would
        // count as having landed during one.
        let target_mode = safe_read::<i32>(tgt.wrapping_add(TARGET_MODE_OFFSET) as *const i32);

        let ctx = a2 as *const u8;
        let ctx_in_overdrive_pre =
            safe_read::<u8>(ctx.wrapping_add(TARGET_IN_OVERDRIVE_OFFSET)).map(|b| b != 0);
        let ctx_in_break_pre =
            safe_read::<u8>(ctx.wrapping_add(TARGET_IN_BREAK_OFFSET)).map(|b| b != 0);

        let stun_immune_hard =
            safe_read::<u8>(tgt.wrapping_add(STUN_IMMUNE_HARD_OFFSET)).unwrap_or(0) != 0;
        let stun_immune_flag =
            safe_read::<u8>(tgt.wrapping_add(STUN_IMMUNE_FLAG_OFFSET)).unwrap_or(0) != 0;
        let stun_immune_class =
            safe_read::<u8>(tgt.wrapping_add(STUN_IMMUNE_CLASS_OFFSET)).unwrap_or(0) != 0;

        let original_value = unsafe { ProcessDamageEvent.call(a1, a2, a3, a4) };

        // This points to the first Entity instance in the 'a2' entity list.
        let source_entity_ptr = unsafe { (a2.byte_add(0x18) as *const *const usize).read() };

        // @TODO(false): For some reason, online + Ferry's Umlauf skill pet can return a null pointer here.
        // Possible data race with online?

        // On the network path the deserializer resolves the source into
        // `{idx, ptr, gen}` at a2+0x10/0x18/0x20, `ptr` being this field, so a
        // record can arrive with no source at all. `recover_source_less_hit`
        // attributes it from its action id instead of dropping it.
        if source_entity_ptr.is_null() {
            self.recover_source_less_hit(a2, target_specified_instance_ptr, a4);
            return original_value;
        }

        // entity->m_pSpecifiedInstance, offset 0x70 from entity pointer.
        // Returns the specific class instance of the source entity. (e.g. Instance of Pl1200 / Pl0700Ghost)
        let source_specified_instance_ptr: usize = unsafe { *(source_entity_ptr.byte_add(super::ENTITY_SPECIFIED_INSTANCE_OFFSET)) };
        let source = source_specified_instance_ptr as *const usize;

        // Get the source actor's type ID.
        let source_type_id = actor_type_id(source);
        let source_probe = probe_actor(source);
        let source_idx = source_probe.id;

        // If the source_type is any of the following, then we need to get their parent entity.
        let (source_parent_type_id, source_parent_idx) =
            get_source_parent(source_type_id, source).unwrap_or((source_type_id, source_idx));

        let Some(damage_instance) = NonNull::new(a2 as *mut DamageInstance) else {
            return original_value;
        };
        let damage_instance = unsafe { damage_instance.as_ref() };
        let damage: i32 = damage_instance.damage;
        let flags: u64 = damage_instance.flags;

        // Recorded before any gate: the calls dropped below carry the
        // attribution a source-less record lacks.
        note_action_owner(
            damage_instance.action_id,
            source_parent_idx,
            source_parent_type_id,
            source_type_id,
        );

        let action_type: ActionType = classify_action(flags, damage_instance.action_id);

        // A u32::MAX ("no skill id") hit carrying the counter's flag bit and no
        // summon-owner indirection is the perfect-block counter.
        let action_type = if matches!(action_type, ActionType::Normal(id) if id == u32::MAX)
            && (flags & PERFECT_BLOCK_COUNTER_FLAG_BIT) != 0
            && source_type_id == source_parent_type_id
        {
            ActionType::Normal(PERFECT_BLOCK_COUNTER_ACTION_ID)
        } else {
            action_type
        };

        // a4: 0 = this client's local simulation, 1 = a network-replicated record.
        let replication = note_and_check_replicated(source_idx, a4);
        let delivers_replicated = replication.delivers;

        if a4 != 0 && PENDING_UNLEARNED_COUNT.load(Ordering::Relaxed) != 0 {
            discard_pending_unlearned(source_idx, source_type_id, action_type);
        }

        release_expired_unlearned(&self.tx);

        let source_is_local = source_is_local_for_dedup(source_type_id, source);

        // The one case this rule can over-count: park the frame instead.
        let park_until_classified = source_is_local == Some(false)
            && a4 == 0
            && !delivers_replicated
            && !is_local_sim_only_class(source_type_id);

        if is_duplicate_frame(
            source_is_local,
            a4,
            source_type_id,
            delivers_replicated,
            action_type,
        ) {
            let _ = self.tx.send(Message::SuppressedDuplicateDamage(
                protocol::SuppressedDuplicateDamageEvent {
                    source: Actor {
                        index: source_idx,
                        actor_type: source_type_id,
                        parent_index: source_parent_idx,
                        parent_actor_type: source_parent_type_id,
                    },
                    damage,
                    action_id: action_type,
                    a4,
                    source_is_local,
                },
            ));
            return original_value;
        }

        let stun_current_after = safe_read::<f32>(tgt.wrapping_add(0xB90) as *const f32)
            .unwrap_or(stun_current_before);
        let delta_stun = (stun_current_after - stun_current_before).max(0.0);

        // The player-source path writes the per-hit stun after the original
        // runs; the deserializer writes it before, so pre-call is a safe fallback.
        let per_hit_stun_post =
            safe_read::<f32>(unsafe { std::ptr::addr_of!((*(a2 as *const DamageInstance)).stun) });
        let per_hit_stun = match per_hit_stun_post {
            Some(post) if post > 0.0 => Some(post),
            _ => per_hit_stun_pre,
        };

        // The ctx bytes get a second chance post-call; whoever fills them may
        // not have run before the pre-call read.
        let ctx_in_overdrive_post =
            safe_read::<u8>(ctx.wrapping_add(TARGET_IN_OVERDRIVE_OFFSET)).map(|b| b != 0);
        let ctx_in_break_post =
            safe_read::<u8>(ctx.wrapping_add(TARGET_IN_BREAK_OFFSET)).map(|b| b != 0);
        let ctx_in_overdrive = match ctx_in_overdrive_post {
            Some(true) => Some(true),
            _ => ctx_in_overdrive_pre,
        };
        let ctx_in_break = match ctx_in_break_post {
            Some(true) => Some(true),
            _ => ctx_in_break_pre,
        };

        let target_in_overdrive = match target_mode {
            Some(mode) => Some(TARGET_MODE_OVERDRIVE.contains(&mode)),
            None => ctx_in_overdrive,
        };
        let target_in_break = match target_mode {
            Some(mode) => Some(TARGET_MODE_BREAK.contains(&mode)),
            None => ctx_in_break,
        };

        let sba_window_active = super::sba::chain_burst_active();

        const STUN_AVAILABLE_EPSILON: f32 = 1.2e-7;
        let stun_available = stun_enabled
            && stun_break_timer < STUN_AVAILABLE_EPSILON
            && stun_recover_timer < STUN_AVAILABLE_EPSILON
            && !stun_immune_hard
            && !stun_immune_flag
            && !stun_immune_class
            && !stun_immune_gauge;
        // Not clamped here; the saturation cap lives in the parser's StunReconstructor.
        let per_hit_formula = match per_hit_stun {
            Some(per_hit) if stun_available && per_hit > 0.0 => {
                per_hit * (1.0 + stun_taken_bonus)
            }
            _ => 0.0,
        };

        // Both values describe the same hit, so this is a choice, not a sum.
        let added_stun_value = if delta_stun > 0.0 {
            delta_stun
        } else {
            per_hit_formula
        };

        let stun_fill = if stun_enabled && stun_max > 0.0 {
            Some((stun_current_after / stun_max).clamp(0.0, 1.0))
        } else {
            None
        };

        // Zero-damage, stun-only hits are real (the Referee summon's stun
        // attack, the perfect-guard counter-shot), so they are not bailed on
        // here. The `original_value == 0` term stays even though it eats real
        // Umlauf satellite hits: dropping it let 11x as many calls through, plus
        // a phantom `Et0007` environment source.
        if added_stun_value <= 0.0 && (original_value == 0 || damage <= 0) {
            return original_value;
        }

        // Withheld, not dropped: the stun on these events is still real.
        let damage = if should_zero_rejected_damage(a4, original_value, damage, source_type_id) {
            0
        } else {
            damage
        };

        if let Some(identity) =
            resolve_source_identity(source_probe.slot, source_parent_idx, source_type_id)
        {
            let _ = self.tx.send(Message::PlayerIdentityEvent(identity));
        }

        let source_actor_type: u32 = resolve_specific_actor_type(source, source_type_id);

        let target_vfunc_type_id: u32 =
            actor_type_id(target_specified_instance_ptr as *const usize);
        let (target_type_id, target_base_type, target_is_destructible) =
            resolve_target_type(target_specified_instance_ptr as *const usize, target_vfunc_type_id);

        if target_is_destructible {
            return original_value;
        }

        let target_idx = actor_idx(target_specified_instance_ptr as *const usize);

        // Past the duplicate-frame gate deliberately: a re-simulated frame
        // gets no vote on when a window opened.
        note_state_transitions(
            &self.tx,
            target_idx,
            target_in_overdrive,
            target_in_break,
            sba_window_active,
        );
        note_enemy_death(&self.tx, target_idx, target_type_id, tgt);
        // A shield rewrites its remaining points with no status event of any
        // kind; damage is the only signal it moved.
        status::note_damage_to_actor(&self.tx, target_idx);

        let event = Message::DamageEvent(DamageEvent {
            source: Actor {
                index: source_idx,
                actor_type: source_actor_type,
                parent_index: source_parent_idx,
                parent_actor_type: source_parent_type_id,
            },
            target: Actor {
                index: target_idx,
                actor_type: target_type_id,
                parent_index: target_idx,
                parent_actor_type: target_type_id,
            },
            damage,
            flags,
            action_id: action_type,
            attack_rate: Some(damage_instance.attack_rate),
            damage_cap: Some(damage_instance.damage_cap),
            stun_value: if matches!(action_type, ActionType::SupplementaryDamage(_)) {
                None
            } else {
                Some(added_stun_value)
            },
            stun_fill,
            target_base_type,
            stun_max: if stun_enabled && stun_max > 0.0 {
                Some(stun_max)
            } else {
                None
            },
            // Only on the local-simulation path. The replicated arm skips the
            // whole clamp block and the instance it hands us is a stack
            // temporary the deserializer only partly fills, so every field in
            // here would be leftovers -- see protocol::HitCalc. Read after the
            // original call (above) because `pre_cap_damage` is written by that
            // clamp block; the other four are inputs it never touches.
            hit_calc: (a4 == 0).then(|| protocol::HitCalc {
                cap_rate: damage_instance.cap_rate,
                class_flags: damage_instance.class_flags,
                reference_damage: damage_instance.reference_damage,
                pre_cap_damage: damage_instance.pre_cap_damage,
                damage_floor: damage_instance.damage_floor,
            }),
        });

        if park_until_classified {
            hold_unlearned_frame(source_idx, source_type_id, action_type, event);
        } else {
            let _ = self.tx.send(event);
        }

        original_value
    }
}

#[derive(Clone)]
pub struct OnProcessDotHook {
    tx: event::Tx,
}

impl OnProcessDotHook {
    pub fn new(tx: event::Tx) -> Self {
        OnProcessDotHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(process_dot_evt) = process.search_address_start(
            "56 57 55 53 48 83 ec 58 4c 89 c7 89 d3 48 89 ce e8 ? ? ? ? 89 c5 48 8d 8e 50 01 00 00",
        ) {
            #[cfg(feature = "console")]
            println!("Found process dot event");

            unsafe {
                let func: ProcessDotEventFunc = std::mem::transmute(process_dot_evt);
                ProcessDotEvent.initialize(func, move |target, damage, source_handle| {
                    cloned_self.run(target, damage, source_handle)
                })?;
                ProcessDotEvent.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find process_dot_evt"));
        }

        Ok(())
    }

    // `damage` is the on-screen tick number. `source_handle` is the entity
    // handle `{u32 idx, CEntityInfo* at +0x08, tag at +0x10}`.
    fn run(&self, target: *const usize, damage: i32, source_handle: *const usize) -> usize {
        let original_value = unsafe { ProcessDotEvent.call(target, damage, source_handle) };

        if damage <= 0 || source_handle.is_null() || target.is_null() {
            return original_value;
        }

        let Some(source_info) =
            safe_read((source_handle as *const u8).wrapping_add(0x08) as *const *const usize)
        else {
            return original_value;
        };
        if source_info.is_null() {
            return original_value;
        }
        let Some(source) =
            safe_read((source_info as *const u8).wrapping_add(super::ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const *const usize)
        else {
            return original_value;
        };
        if source.is_null() {
            return original_value;
        }

        let source_probe = probe_actor(source);
        let source_idx = source_probe.id;
        let source_type_id = actor_type_id(source);

        let target_idx = actor_idx(target);
        let target_vfunc_type_id = actor_type_id(target);
        let (target_type_id, target_base_type, target_is_destructible) =
            resolve_target_type(target, target_vfunc_type_id);

        if target_is_destructible {
            return original_value;
        }

        let (source_parent_type_id, source_parent_idx) =
            get_source_parent(source_type_id, source).unwrap_or((source_type_id, source_idx));

        if let Some(identity) =
            resolve_source_identity(source_probe.slot, source_parent_idx, source_type_id)
        {
            let _ = self.tx.send(Message::PlayerIdentityEvent(identity));
        }

        let source_actor_type = resolve_specific_actor_type(source, source_type_id);

        let event = Message::DamageEvent(DamageEvent {
            source: Actor {
                index: source_idx,
                actor_type: source_actor_type,
                parent_index: source_parent_idx,
                parent_actor_type: source_parent_type_id,
            },
            target: Actor {
                index: target_idx,
                actor_type: target_type_id,
                parent_index: target_idx,
                parent_actor_type: target_type_id,
            },
            damage,
            flags: 0,
            action_id: ActionType::DamageOverTime(0),
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type,
            stun_max: None,
            // A DoT tick has no DamageInstance at all -- this hook is handed the
            // target and a bare damage number.
            hit_calc: None,
        });

        let _ = self.tx.send(event);

        original_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bh_category_uid_is_flagged_destructible() {
        let bh_uid = (0x000E_u32 << 16) | 0x0500;
        assert_eq!((bh_uid >> 16), DESTRUCTIBLE_UID_CATEGORY);
        assert_eq!(specific_type_id_from_uid(bh_uid), Some(0x3c90_44d0));
    }

    #[test]
    fn ba_category_uid_is_not_flagged_destructible() {
        let ba_uid = (0x000F_u32 << 16) | 0x0500;
        assert_ne!((ba_uid >> 16), DESTRUCTIBLE_UID_CATEGORY);
    }

    const WP2290: u32 = 0x5B1AB457; // Seofon's avatar sword
    const GHOST_SAT: u32 = 0x8364C8BC; // Ferry's Umlauf satellite
    const WP1890: u32 = 0xC9F45042; // Cagliostro's sled
    const LOCAL_SIM_ONLY: [u32; 3] = [WP2290, GHOST_SAT, WP1890];

    const PL2000: u32 = 0xF5755C0E; // Id's dragonform: parent-resolved, but replicated
    const SO0000: u32 = 0xD2E5407A; // a summon: parent-resolved, replicated
    const PL0700_GHOST: u32 = 0x2AF678E8; // Ferry's main pet: root-derived, yet replicated

    const ORDINARY: ActionType = ActionType::Normal(210);
    const PBLOCK: ActionType = ActionType::Normal(PERFECT_BLOCK_COUNTER_ACTION_ID);

    #[test]
    fn classify_action_decodes_every_flag_arm() {
        const LINK_A: u64 = 1 << 7;
        const LINK_B: u64 = 1 << 50;
        const SBA_A: u64 = 1 << 13;
        const SBA_B: u64 = 1 << 14;
        const SUPP: u64 = 1 << 15;

        assert_eq!(classify_action(0, 210), ActionType::Normal(210));
        assert_eq!(classify_action(SUPP, 210), ActionType::SupplementaryDamage(210));
        for bit in [SBA_A, SBA_B, SBA_A | SBA_B] {
            assert_eq!(classify_action(bit, 210), ActionType::SBA);
        }
        for bit in [LINK_A, LINK_B, LINK_A | LINK_B] {
            assert_eq!(classify_action(bit, 210), ActionType::LinkAttack);
        }

        assert_eq!(classify_action(LINK_A | SBA_A | SUPP, 7), ActionType::LinkAttack);
        assert_eq!(classify_action(SBA_A | SUPP, 7), ActionType::SBA);

        assert_eq!(classify_action(1 << 38, 611), ActionType::Normal(611));
    }

    #[test]
    fn locally_owned_source_counts_only_its_local_sim_call() {
        for delivers in [false, true] {
            assert!(!is_duplicate_frame(
                Some(true),
                0,
                PL2000,
                delivers,
                ORDINARY
            ));
            assert!(is_duplicate_frame(
                Some(true),
                1,
                PL2000,
                delivers,
                ORDINARY
            ));
        }
    }

    #[test]
    fn remote_source_is_gated_only_once_it_has_delivered_a_replicated_call() {
        for class in [PL2000, SO0000, PL0700_GHOST] {
            assert!(
                is_duplicate_frame(Some(false), 0, class, true, ORDINARY),
                "{class:#010x} must stay gated once it delivers replicated calls"
            );
            assert!(!is_duplicate_frame(Some(false), 1, class, true, ORDINARY));
        }
    }

    #[test]
    fn remote_source_that_never_replicates_keeps_its_local_sim_calls() {
        for class in [PL2000, SO0000, PL0700_GHOST] {
            assert!(
                !is_duplicate_frame(Some(false), 0, class, false, ORDINARY),
                "{class:#010x} lost its only call path"
            );
        }
    }

    #[test]
    fn remote_local_sim_only_class_is_kept_even_if_something_marks_it() {
        for class in LOCAL_SIM_ONLY {
            for delivers in [false, true] {
                assert!(
                    !is_duplicate_frame(Some(false), 0, class, delivers, ORDINARY),
                    "{class:#010x} lost its only call path"
                );
                assert!(!is_duplicate_frame(
                    Some(false),
                    1,
                    class,
                    delivers,
                    ORDINARY
                ));
            }
        }
    }

    #[test]
    fn remote_perfect_block_counter_survives_a_replicating_source() {
        for class in [PL2000, SO0000, PL0700_GHOST] {
            for delivers in [false, true] {
                assert!(
                    !is_duplicate_frame(Some(false), 0, class, delivers, PBLOCK),
                    "{class:#010x} lost the perfect-block counter"
                );
            }
        }
    }

    #[test]
    fn perfect_block_exemption_does_not_leak_to_the_rest_of_the_kit() {
        for action in [
            ORDINARY,
            ActionType::Normal(1100),
            ActionType::SupplementaryDamage(PERFECT_BLOCK_COUNTER_ACTION_ID),
            ActionType::LinkAttack,
            ActionType::SBA,
        ] {
            assert!(
                is_duplicate_frame(Some(false), 0, PL2000, true, action),
                "{action:?} must stay gated"
            );
        }
    }

    /// Tests touching the process-global maps take this first.
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        match TEST_GUARD.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn pending_event(source_idx: u32, damage: i32, action_id: ActionType) -> Message {
        let actor = Actor {
            index: source_idx,
            actor_type: PL2000,
            parent_index: source_idx,
            parent_actor_type: PL2000,
        };
        Message::DamageEvent(DamageEvent {
            source: actor.clone(),
            target: actor,
            damage,
            flags: 0,
            action_id,
            attack_rate: None,
            damage_cap: None,
            stun_value: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        })
    }

    fn pending_len(source_idx: u32) -> usize {
        lock_pending_unlearned()
            .get(&source_idx)
            .map(|f| f.len())
            .unwrap_or(0)
    }

    #[test]
    fn only_the_first_replicated_call_is_newly_marked() {
        let _guard = exclusive();
        clear_replicated_sources();
        assert_eq!(
            note_and_check_replicated(5150, 0),
            ReplicationStatus { delivers: false, newly_marked: false }
        );
        assert_eq!(
            note_and_check_replicated(5150, 1),
            ReplicationStatus { delivers: true, newly_marked: true }
        );
        assert_eq!(
            note_and_check_replicated(5150, 1),
            ReplicationStatus { delivers: true, newly_marked: false }
        );
        clear_replicated_sources();
    }

    #[test]
    fn first_replicated_call_discards_that_sources_parked_frames() {
        let _guard = exclusive();
        const SRC: u32 = 0x5AF3_0001;
        const SBA: ActionType = ActionType::Normal(3000);
        for _ in 0..4 {
            hold_unlearned_frame(SRC, PL2000, SBA, pending_event(SRC, 971_730, SBA));
        }
        assert_eq!(pending_len(SRC), 4);

        discard_pending_unlearned(SRC, PL2000, SBA);
        assert_eq!(pending_len(SRC), 0, "parked re-simulation must not survive");
    }

    #[test]
    fn a_sources_discard_does_not_touch_another_sources_parked_frames() {
        let _guard = exclusive();
        const A: u32 = 0x5AF3_0002;
        const B: u32 = 0x5AF3_0003;
        const ACT: ActionType = ActionType::Normal(3000);
        hold_unlearned_frame(A, PL2000, ACT, pending_event(A, 1, ACT));
        hold_unlearned_frame(B, PL2000, ACT, pending_event(B, 2, ACT));

        discard_pending_unlearned(A, PL2000, ACT);
        assert_eq!(pending_len(A), 0);
        assert_eq!(pending_len(B), 1, "discard must be keyed per source");

        discard_pending_unlearned(B, PL2000, ACT);
    }

    #[test]
    fn rejected_local_sim_damage_is_zeroed_but_umlauf_is_exempt() {
        const TWEYEN: u32 = 0xDA5A_8E25;
        assert!(should_zero_rejected_damage(0, 0, 44_000, TWEYEN));

        for class in LOCAL_SIM_ONLY {
            assert!(
                !should_zero_rejected_damage(0, 0, 44_000, class),
                "{class:#010x} would lose real hits to the rejected-damage rule"
            );
        }
    }

    #[test]
    fn the_rejected_damage_rule_never_touches_an_accepted_or_replicated_hit() {
        const TWEYEN: u32 = 0xDA5A_8E25;
        // pde's success return is `(a2_low32 & 0xFFFFFF00) | 1`, not a clean bool.
        assert!(!should_zero_rejected_damage(0, 0x1234_5601, 44_000, TWEYEN));
        assert!(!should_zero_rejected_damage(1, 0, 44_000, TWEYEN));
        assert!(!should_zero_rejected_damage(0, 0, 0, TWEYEN));
        assert!(!should_zero_rejected_damage(0, 0, -5, TWEYEN));
    }

    #[test]
    fn the_hold_window_stays_within_its_ceiling() {
        assert!(
            UNLEARNED_HOLD <= Duration::from_millis(50),
            "hold window widened past its ceiling: {UNLEARNED_HOLD:?}"
        );
        assert!(
            UNLEARNED_HOLD >= Duration::from_millis(34),
            "hold window no longer covers the measured worst case: {UNLEARNED_HOLD:?}"
        );
    }

    #[test]
    fn a_replicated_record_only_retires_parked_frames_of_its_own_action() {
        let _guard = exclusive();
        const SRC: u32 = 0x5AF3_0006;
        const MOVE_A: ActionType = ActionType::Normal(3000);
        const MOVE_B: ActionType = ActionType::Normal(1000);
        for _ in 0..3 {
            hold_unlearned_frame(SRC, PL2000, MOVE_A, pending_event(SRC, 971_730, MOVE_A));
        }
        hold_unlearned_frame(SRC, PL2000, MOVE_B, pending_event(SRC, 64_631, MOVE_B));
        assert_eq!(pending_len(SRC), 4);

        discard_pending_unlearned(SRC, PL2000, MOVE_A);
        assert_eq!(
            pending_len(SRC),
            1,
            "only move A's frames are proven duplicates; B must survive"
        );

        discard_pending_unlearned(SRC, PL2000, ActionType::SupplementaryDamage(1000));
        assert_eq!(pending_len(SRC), 1, "variant must be part of the match");

        discard_pending_unlearned(SRC, PL2000, MOVE_B);
        assert_eq!(pending_len(SRC), 0);
    }

    #[test]
    fn a_source_that_never_replicates_gets_its_frames_released_not_dropped() {
        let _guard = exclusive();
        const SRC: u32 = 0x5AF3_0004;
        let (tx, mut rx) = tokio::sync::broadcast::channel(16);

        hold_unlearned_frame(SRC, PL2000, ActionType::Normal(3000), pending_event(SRC, 4242, ActionType::Normal(3000)));
        release_expired_at(&tx, Instant::now(), UNLEARNED_HOLD);
        assert_eq!(pending_len(SRC), 1);

        release_expired_at(&tx, Instant::now(), Duration::ZERO);
        assert_eq!(pending_len(SRC), 0, "expired frames must leave the buffer");

        let released: Vec<i32> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|m| match m {
                Message::DamageEvent(d) if d.source.index == SRC => Some(d.damage),
                _ => None,
            })
            .collect();
        assert_eq!(released, vec![4242], "a never-replicating source loses nothing");
    }

    #[test]
    fn encounter_boundary_releases_everything_still_parked() {
        let _guard = exclusive();
        const SRC: u32 = 0x5AF3_0005;
        hold_unlearned_frame(SRC, PL2000, ActionType::Normal(3000), pending_event(SRC, 7, ActionType::Normal(3000)));
        assert_eq!(pending_len(SRC), 1);

        clear_replicated_sources();
        assert_eq!(pending_len(SRC), 0, "boundary must drain the hold");
    }

    #[test]
    fn replicated_source_set_records_clears_and_does_not_leak_between_ids() {
        let _guard = exclusive();
        clear_replicated_sources();
        assert!(!note_and_check_replicated(4242, 0).delivers);
        assert!(note_and_check_replicated(4242, 1).delivers);
        assert!(note_and_check_replicated(4242, 0).delivers);
        assert!(!note_and_check_replicated(4243, 0).delivers);
        clear_replicated_sources();
        assert!(!note_and_check_replicated(4242, 0).delivers);
    }

    #[test]
    fn local_sim_exemption_does_not_leak_to_replicated_classes() {
        for class in LOCAL_SIM_ONLY {
            assert!(is_local_sim_only_class(class), "{class:#010x} missing");
        }
        for class in [PL2000, SO0000, PL0700_GHOST] {
            assert!(
                !is_local_sim_only_class(class),
                "{class:#010x} is replicated and must not be exempted"
            );
        }
    }

    #[test]
    fn ownerless_source_is_never_suppressed() {
        for delivers in [false, true] {
            assert!(!is_duplicate_frame(None, 0, PL2000, delivers, ORDINARY));
            assert!(!is_duplicate_frame(None, 1, PL2000, delivers, ORDINARY));
        }
    }
}
