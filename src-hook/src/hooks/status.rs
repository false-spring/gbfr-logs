//! Status-effect (buff / debuff / ailment) capture, the `ExStatus` lifecycle.

use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
        OnceLock,
    },
    time::Instant,
};

use anyhow::{anyhow, Result};
use protocol::{Message, StatusAppliedEvent, StatusRemovedEvent, StatusStacksChangedEvent};
use retour::{static_detour, RawDetour};

use crate::{event, process::Process};

use super::{
    actor_type_id, get_source_parent, probe_actor, safe_read, ENTITY_SPECIFIED_INSTANCE_OFFSET,
};

const ON_STATUS_APPLY_SIG: &str = "48 8d 94 24 a0 00 00 00 4c 89 f1 49 89 d8 41 89 f9 e8 $ { ' }";

const ON_STATUS_APPLY_SIG_BACKUP: &str =
    "c5 fa 11 74 24 20 48 8d 55 20 4c 89 f1 49 89 d8 41 89 f9 e8 $ { ' }";

const ON_STATUS_DESTROY_SIG: &str =
    "56 57 48 83 ec 68 48 89 d6 48 85 d2 0f 84 ? ? ? ? 48 89 cf 45 84 c0 74 ?";

const ON_STATUS_CLEAR_ALL_SIG: &str = "41 56 56 57 53 48 83 ec 38 4c 8b 71 18 4c 3b 71 20";

const ON_STATUS_STACKS_SIG: &str =
    "41 57 41 56 41 54 56 57 55 53 48 81 ec 90 00 00 00 44 89 cb 45 89 c4 89 d5 \
     48 89 cf 44 8b bc 24 f0 00 00 00 8b 51 50";

const COMPONENT_VECTOR_BEGIN_OFFSET: usize = 0x18;
const COMPONENT_VECTOR_END_OFFSET: usize = 0x20;

/// The `CEntityInfo*` in the target's handle triple `{u32 idx, CEntityInfo*, u32 gen}`.
const STATUS_TARGET_INFO_OFFSET: usize = 0x18;
const STATUS_APPLIER_INFO_OFFSET: usize = 0x30;

const STATUS_KIND_ID_OFFSET: usize = 0x50;

/// The three `u32`s the game compares to decide whether an application
/// refreshes an existing status or inserts a second one.
const STATUS_SOURCE_IDS_OFFSETS: [usize; 3] = [0x44, 0x4C, 0x54];

const STATUS_REMAINING_SECS_OFFSET: usize = 0x80;

const STATUS_FULL_DURATION_SECS_OFFSET: usize = 0x7C;

/// Set when the caller asked for a negative duration; both duration fields
/// then hold the sentinel `9999.0`.
const STATUS_PERMANENT_OFFSET: usize = 0x79;
/// Stack level (`i32`); only meaningful when `is_stackable(kind)`.
const STATUS_STACK_LEVEL_OFFSET: usize = 0x58;

/// Max stack level; on `StatusBarrier` these bytes are remaining shield points.
const STATUS_MAX_LEVEL_OFFSET: usize = 0xB0;

/// The kinds the game's own `isStackable(kind)` considers stackable; kinds
/// outside `9..=0x94` are non-stackable by its range check.
const STACKABLE_KINDS: [u32; 68] = [
    0x09, 0x14, 0x19, 0x25, 0x36, 0x37, 0x39, 0x3A, 0x3B, 0x3C, 0x40, 0x41, 0x42, 0x43, 0x44,
    0x45, 0x47, 0x48, 0x4A, 0x4B, 0x4E, 0x4F, 0x50, 0x52, 0x55, 0x56, 0x57, 0x58, 0x5A, 0x5B,
    0x5C, 0x5D, 0x5E, 0x60, 0x66, 0x68, 0x69, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x72, 0x73,
    0x74, 0x75, 0x76, 0x7B, 0x7C, 0x7D, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x87, 0x88,
    0x89, 0x8A, 0x8B, 0x90, 0x91, 0x92, 0x93, 0x94,
];

const VALUE_VFUNC_SLOT: usize = 3;

/// Decodes where a status keeps its magnitude from its `getValue` leaf:
///
/// ```text
/// C5 FA 10 81 <disp32> C3    vmovss    xmm0, [rcx+disp32] ; ret
/// C5 FA 2A 81 <disp32> C3    vcvtsi2ss xmm0, xmm0, [rcx+disp32] ; ret
/// ```
fn decode_value_getter(code: [u8; 9]) -> Option<(usize, bool)> {
    let is_i32 = match &code[..4] {
        [0xC5, 0xFA, 0x10, 0x81] => false,
        [0xC5, 0xFA, 0x2A, 0x81] => true,
        _ => return None,
    };
    if code[8] != 0xC3 {
        return None;
    }
    let disp = u32::from_le_bytes([code[4], code[5], code[6], code[7]]);
    if disp > 0x4000 {
        return None;
    }
    Some((disp as usize, is_i32))
}

fn value_offset_of(status: *const u8) -> Option<(usize, bool)> {
    let vtable = safe_read(status as *const *const u8)?;
    if vtable.is_null() {
        return None;
    }
    let getter = safe_read(
        vtable.wrapping_add(VALUE_VFUNC_SLOT * std::mem::size_of::<usize>()) as *const *const u8,
    )?;
    if getter.is_null() {
        return None;
    }
    // Read, never called: calling game code from inside a detour risks a crash.
    let code = safe_read(getter as *const [u8; 9])?;
    decode_value_getter(code)
}

const STATUS_BASE_SIZE: usize = 0xB0;

/// Whether the magnitude is a fraction of a stat rather than a count. A stat
/// multiplier carries a second vtable right before its value; a count does not.
fn value_is_fractional(status: *const u8, offset: usize, is_i32: bool) -> bool {
    if is_i32 {
        return false;
    }

    if offset >= STATUS_BASE_SIZE + std::mem::size_of::<usize>() {
        return true;
    }

    // At the base boundary the two meanings coexist: Strength is a multiplier
    // and Burn is damage per tick, both at +0xB0.

    if offset < std::mem::size_of::<usize>() {
        return false;
    }
    let iface = match safe_read(status.wrapping_add(offset - std::mem::size_of::<usize>()) as *const usize) {
        Some(v) => v,
        None => return false,
    };
    const PLAUSIBLE_VTABLE_MIN: usize = 0x1_0000;
    iface >= PLAUSIBLE_VTABLE_MIN && iface % std::mem::align_of::<usize>() == 0
}

struct ObjectFacts {
    applier_index: Option<u32>,
    source_ids: Option<[u32; 3]>,
    value: Option<f32>,
    value_is_fraction: Option<bool>,
}

fn object_facts(status: *const u8) -> ObjectFacts {
    let mut source_ids = [0u32; 3];
    let mut ids_ok = true;
    for (slot, offset) in STATUS_SOURCE_IDS_OFFSETS.iter().enumerate() {
        match safe_read(status.wrapping_add(*offset) as *const u32) {
            Some(v) => source_ids[slot] = v,
            None => ids_ok = false,
        }
    }

    let (value, value_is_fraction) = match value_offset_of(status) {
        Some((offset, is_i32)) => (
            status_value(status),
            Some(value_is_fractional(status, offset, is_i32)),
        ),
        None => (None, None),
    };

    ObjectFacts {
        applier_index: applier_index_for_status(status),
        source_ids: if ids_ok { Some(source_ids) } else { None },
        value,
        value_is_fraction,
    }
}

fn status_value(status: *const u8) -> Option<f32> {
    let (offset, is_i32) = value_offset_of(status)?;
    let at = status.wrapping_add(offset);
    if is_i32 {
        safe_read(at as *const i32).map(|v| v as f32)
    } else {
        safe_read(at as *const f32)
    }
}

/// Summing is the engine's own rule: two DEF DOWNs at 0.10 and 0.20 act as 0.30.
fn summed_value_in_component(comp: *const usize, kind_id: u32) -> Option<(f32, bool)> {
    let base = comp as *const u8;
    let begin = safe_read(base.wrapping_add(COMPONENT_VECTOR_BEGIN_OFFSET) as *const usize)?;
    let end = vector_end(comp)?;
    if begin == 0 || end < begin {
        return None;
    }
    let count = (end - begin) / std::mem::size_of::<usize>();
    if count > MAX_STATUSES_PER_CLEAR {
        return None;
    }

    let mut total = 0.0f32;
    let mut fractional = false;
    for i in 0..count {
        let slot = (begin as *const u8).wrapping_add(i * std::mem::size_of::<usize>());
        let status = safe_read(slot as *const *const u8)?;
        if status.is_null() {
            continue;
        }
        if safe_read(status.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32)? != kind_id {
            continue;
        }
        let (offset, is_i32) = value_offset_of(status)?;
        fractional = value_is_fractional(status, offset, is_i32);
        total += status_value(status)?;
    }
    Some((total, fractional))
}

fn is_stackable(kind_id: u32) -> bool {
    STACKABLE_KINDS.binary_search(&kind_id).is_ok()
}

/// Apply out-struct (`rdx`): `{ u8 applied; Status* @+0x08 }`. Failure writes `{0, NULL}`.
const APPLY_OUT_STATUS_OFFSET: usize = 0x08;

/// Walk bound; a torn read of the two vector pointers can produce an absurd count.
const MAX_STATUSES_PER_CLEAR: usize = 256;

/// u64::MAX: a repeat refresh never re-emits. Aura buffs hammer `applyStatus`.
const REFRESH_MIN_GAP_MS: u64 = u64::MAX;

const REFRESH_SLOTS: usize = 64;

struct RefreshGate {
    key: [AtomicU64; REFRESH_SLOTS],
    at_ms: [AtomicU64; REFRESH_SLOTS],
}

impl RefreshGate {
    const fn new() -> Self {
        Self {
            key: [const { AtomicU64::new(u64::MAX) }; REFRESH_SLOTS],
            at_ms: [const { AtomicU64::new(0) }; REFRESH_SLOTS],
        }
    }

    /// Fibonacci hash; a plain modulo would ignore the actor half of the key.
    fn slot(key: u64) -> usize {
        const PHI: u64 = 0x9E37_79B9_7F4A_7C15;
        (key.wrapping_mul(PHI) >> (64 - REFRESH_SLOTS.trailing_zeros() as u64)) as usize
    }

    fn should_emit(&self, actor_index: u32, kind_id: u32, now_ms: u64, min_gap_ms: u64) -> bool {
        let key = ((actor_index as u64) << 32) | kind_id as u64;
        let slot = Self::slot(key);

        let held = self.key[slot].load(Ordering::Relaxed);
        if held == key {
            let last = self.at_ms[slot].load(Ordering::Relaxed);
            if now_ms.saturating_sub(last) < min_gap_ms {
                return false;
            }
        } else {
            self.key[slot].store(key, Ordering::Relaxed);
        }

        self.at_ms[slot].store(now_ms, Ordering::Relaxed);
        true
    }

    fn forget(&self, actor_index: u32, kind_id: u32) {
        let key = ((actor_index as u64) << 32) | kind_id as u64;
        let slot = Self::slot(key);
        let _ = self.key[slot].compare_exchange(
            key,
            u64::MAX,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

static REFRESH_GATE: RefreshGate = RefreshGate::new();

static CLOCK_BASE: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    CLOCK_BASE.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn actor_index_for_status(status: *const u8) -> Option<u32> {
    actor_index_at_handle(status, STATUS_TARGET_INFO_OFFSET)
}

fn applier_index_for_status(status: *const u8) -> Option<u32> {
    actor_index_at_handle(status, STATUS_APPLIER_INFO_OFFSET)
}

/// Every hop fails closed, and the caller decides what a `None` costs: the
/// target's resolver drops the event, the applier's reports no source.
fn actor_index_at_handle(status: *const u8, offset: usize) -> Option<u32> {
    let info = safe_read(status.wrapping_add(offset) as *const *const u8)
        .filter(|info| !info.is_null())?;

    let instance =
        safe_read(info.wrapping_add(ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const *const usize)
            .filter(|instance| !instance.is_null())?;

    safe_read::<usize>(instance as *const usize)?;

    // A transform or summon body reports its owner, same as the damage path.
    let type_id = actor_type_id(instance);
    if let Some((_parent_type, parent_index)) = get_source_parent(type_id, instance) {
        return Some(parent_index);
    }

    Some(probe_actor(instance).id)
}

fn vector_end(comp: *const usize) -> Option<usize> {
    safe_read((comp as *const u8).wrapping_add(COMPONENT_VECTOR_END_OFFSET) as *const usize)
}

/// The stack depth of `kind_id` on this component, and whether `needle` is still
/// in the vector. `applyStatus` never writes the level (its caller does, after it
/// returns), so a mid-apply read sees 0 and clamps up to 1.
fn stack_depth_in_component(
    comp: *const usize,
    kind_id: u32,
    needle: *const u8,
) -> Option<(u32, u32, bool)> {
    let base = comp as *const u8;
    let begin = safe_read(base.wrapping_add(COMPONENT_VECTOR_BEGIN_OFFSET) as *const usize)?;
    let end = vector_end(comp)?;

    if begin == 0 || end < begin {
        return None;
    }

    let count = (end - begin) / std::mem::size_of::<usize>();
    if count > MAX_STATUSES_PER_CLEAR {
        return None;
    }

    let stackable = is_stackable(kind_id);
    let mut depth = 0u32;
    let mut objects = 0u32;
    let mut found = false;

    for i in 0..count {
        let slot = (begin as *const u8).wrapping_add(i * std::mem::size_of::<usize>());
        let status = safe_read(slot as *const *const u8)?;
        if status.is_null() {
            continue;
        }
        if status == needle {
            found = true;
        }
        if safe_read(status.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32)? != kind_id {
            continue;
        }
        objects = objects.saturating_add(1);
        if !stackable {
            continue;
        }
        let level = safe_read(status.wrapping_add(STATUS_STACK_LEVEL_OFFSET) as *const i32)?;
        let max = safe_read(status.wrapping_add(STATUS_MAX_LEVEL_OFFSET) as *const i32)?;
        let level = level.clamp(1, if max >= 1 { max } else { i32::MAX });
        depth = depth.saturating_add(level as u32);
    }

    if objects == 0 {
        return Some((0, 0, found));
    }
    Some((if stackable { depth.max(1) } else { 1 }, objects, found))
}

/// `fn(ExStatus* comp, Out* out, CEntityInfo* applier, u32 kind_id, ...11 stack)`.
/// Arity is 15 and every argument must be forwarded: an under-declared detour
/// makes the original read args 5-15 out of OUR stack frame.
#[allow(clippy::type_complexity)]
type ApplyStatusFunc = unsafe extern "system" fn(
    *const usize,
    *const u8,
    *const usize,
    u32,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
) -> usize;

// retour's `Function` trait stops at 14 arguments, so this hook uses the
// untyped `RawDetour` and these statics instead of `static_detour!`.
static APPLY_TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
static APPLY_TX: OnceLock<event::Tx> = OnceLock::new();

#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn apply_status_detour(
    comp: *const usize,
    out: *const u8,
    applier: *const usize,
    kind_id: u32,
    a5: u64,
    a6: u64,
    a7: u64,
    a8: u64,
    a9: u64,
    a10: u64,
    a11: u64,
    a12: u64,
    a13: u64,
    a14: u64,
    a15: u64,
) -> usize {
    let kind_before = stack_depth_in_component(comp, kind_id, std::ptr::null()).map(|(n, _, _)| n);

    let trampoline = APPLY_TRAMPOLINE.load(Ordering::Acquire);
    if trampoline == 0 {
        return 0;
    }
    let original: ApplyStatusFunc = std::mem::transmute(trampoline);

    let ret = original(
        comp, out, applier, kind_id, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15,
    );

    let Some(tx) = APPLY_TX.get() else {
        return ret;
    };

    // The `Status*`, not the `u8`, is the success signal: a failed application
    // writes `{0, NULL}`. Gating on the `u8` instead discarded 89% of real
    // applications, Invincibility and Mirror Image among them.
    let status = match safe_read(out.wrapping_add(APPLY_OUT_STATUS_OFFSET) as *const *const u8) {
        Some(status) if !status.is_null() => status,
        _ => return ret,
    };

    let Some(actor_index) = actor_index_for_status(status) else {
        return ret;
    };

    let remaining_secs =
        safe_read(status.wrapping_add(STATUS_REMAINING_SECS_OFFSET) as *const f32).unwrap_or(0.0);
    let full_duration_secs =
        safe_read(status.wrapping_add(STATUS_FULL_DURATION_SECS_OFFSET) as *const f32);
    let is_permanent =
        safe_read(status.wrapping_add(STATUS_PERMANENT_OFFSET) as *const u8).map(|v| v != 0);

    let stacks = stack_depth_in_component(comp, kind_id, std::ptr::null()).map(|(n, _, _)| n);

    let is_refresh = match (kind_before, stacks) {
        (Some(before), Some(after)) => after <= before,
        _ => false,
    };

    if is_refresh && !REFRESH_GATE.should_emit(actor_index, kind_id, now_ms(), REFRESH_MIN_GAP_MS) {
        return ret;
    }

    let summed = summed_value_in_component(comp, kind_id);
    let value_total = summed.map(|(v, _)| v);
    let facts = object_facts(status);

    PENDING_APPLY.with(|slot| slot.set(Some((actor_index, kind_id, value_total))));

    if let Some((value, false)) = summed {
        if value != 0.0 {
            VALUE_WATCH.arm(actor_index, kind_id, comp, value, now_ms());
        }
    }

    let _ = tx.send(Message::OnStatusApplied(StatusAppliedEvent {
        actor_index,
        status_id: kind_id,
        remaining_secs,
        is_refresh,
        stacks,
        value_total,
        value_is_fraction: facts.value_is_fraction.or(summed.map(|(_, f)| f)),
        applier_index: facts.applier_index,
        source_ids: facts.source_ids,
        value: facts.value,
        full_duration_secs,
        is_permanent,
    }));

    ret
}

pub struct OnStatusApplyHook {
    tx: event::Tx,
}

impl OnStatusApplyHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address(ON_STATUS_APPLY_SIG)
            .or_else(|_| process.search_address(ON_STATUS_APPLY_SIG_BACKUP))
            .map_err(|e| anyhow!("Could not find status_apply: {e}"))?;

        let _ = APPLY_TX.set(self.tx.clone());

        #[cfg(feature = "console")]
        println!("Found status apply");

        unsafe {
            let detour = RawDetour::new(address as *const (), apply_status_detour as *const ())?;

            // Publish the trampoline BEFORE enable(): the detour is live on
            // every game thread the moment it is enabled.
            APPLY_TRAMPOLINE.store(detour.trampoline() as *const () as usize, Ordering::Release);
            detour.enable()?;

            // Leaked: dropping would free the trampoline under a call in flight.
            std::mem::forget(detour);
        }

        Ok(())
    }
}

const WATCH_SLOTS: usize = 32;

const WATCH_MIN_GAP_MS: u64 = 200;

/// Watches statuses whose value moves with nothing to announce it: a shield
/// absorbs damage through a per-class setter with no shared function to hook.
struct ValueWatch {
    key: [AtomicU64; WATCH_SLOTS],
    comp: [AtomicUsize; WATCH_SLOTS],
    last: [AtomicU32; WATCH_SLOTS],
    at_ms: [AtomicU64; WATCH_SLOTS],
}

impl ValueWatch {
    const fn new() -> Self {
        Self {
            key: [const { AtomicU64::new(u64::MAX) }; WATCH_SLOTS],
            comp: [const { AtomicUsize::new(0) }; WATCH_SLOTS],
            last: [const { AtomicU32::new(0) }; WATCH_SLOTS],
            at_ms: [const { AtomicU64::new(0) }; WATCH_SLOTS],
        }
    }

    fn slot(key: u64) -> usize {
        const PHI: u64 = 0x9E37_79B9_7F4A_7C15;
        (key.wrapping_mul(PHI) >> (64 - WATCH_SLOTS.trailing_zeros() as u64)) as usize
    }

    fn key_of(actor_index: u32, kind_id: u32) -> u64 {
        ((actor_index as u64) << 32) | kind_id as u64
    }

    fn arm(&self, actor_index: u32, kind_id: u32, comp: *const usize, value: f32, now_ms: u64) {
        let key = Self::key_of(actor_index, kind_id);
        let slot = Self::slot(key);
        self.key[slot].store(key, Ordering::Relaxed);
        self.comp[slot].store(comp as usize, Ordering::Relaxed);
        self.last[slot].store(value.to_bits(), Ordering::Relaxed);
        self.at_ms[slot].store(now_ms, Ordering::Relaxed);
    }

    fn forget(&self, actor_index: u32, kind_id: u32) {
        let key = Self::key_of(actor_index, kind_id);
        let slot = Self::slot(key);
        if self.key[slot].load(Ordering::Relaxed) == key {
            self.key[slot].store(u64::MAX, Ordering::Relaxed);
            self.comp[slot].store(0, Ordering::Relaxed);
        }
    }
}

static VALUE_WATCH: ValueWatch = ValueWatch::new();

/// Called from the damage hook. Reports any watched status whose value moved.
pub fn note_damage_to_actor(tx: &event::Tx, actor_index: u32) {
    let now = now_ms();

    for slot in 0..WATCH_SLOTS {
        let key = VALUE_WATCH.key[slot].load(Ordering::Relaxed);
        if key == u64::MAX || (key >> 32) as u32 != actor_index {
            continue;
        }

        let at = VALUE_WATCH.at_ms[slot].load(Ordering::Relaxed);
        if now.saturating_sub(at) < WATCH_MIN_GAP_MS {
            continue;
        }

        let comp = VALUE_WATCH.comp[slot].load(Ordering::Relaxed) as *const usize;
        if comp.is_null() {
            continue;
        }
        let kind_id = key as u32;

        let Some((value, is_fraction)) = summed_value_in_component(comp, kind_id) else {
            continue;
        };

        if VALUE_WATCH.key[slot].load(Ordering::Relaxed) != key {
            continue;
        }
        VALUE_WATCH.at_ms[slot].store(now, Ordering::Relaxed);

        let previous = VALUE_WATCH.last[slot].load(Ordering::Relaxed);
        if previous == value.to_bits() {
            continue;
        }
        VALUE_WATCH.last[slot].store(value.to_bits(), Ordering::Relaxed);

        let stacks = stack_depth_in_component(comp, kind_id, std::ptr::null())
            .map(|(n, _, _)| n)
            .unwrap_or(1)
            .max(1);

        for status in component_statuses_of_kind(comp, kind_id) {
            let facts = object_facts(status);
            let _ = tx.send(Message::OnStatusStacksChanged(StatusStacksChangedEvent {
                actor_index,
                status_id: kind_id,
                stacks,
                value_total: Some(value),
                value_is_fraction: facts.value_is_fraction.or(Some(is_fraction)),
                applier_index: facts.applier_index,
                source_ids: facts.source_ids,
                value: facts.value,
            }));
        }
    }
}

fn component_statuses_of_kind(comp: *const usize, kind_id: u32) -> Vec<*const u8> {
    let mut out = Vec::new();
    let base = comp as *const u8;
    let Some(begin) = safe_read(base.wrapping_add(COMPONENT_VECTOR_BEGIN_OFFSET) as *const usize)
    else {
        return out;
    };
    let Some(end) = vector_end(comp) else {
        return out;
    };
    if begin == 0 || end < begin {
        return out;
    }
    let count = (end - begin) / std::mem::size_of::<usize>();
    if count > MAX_STATUSES_PER_CLEAR {
        return out;
    }

    for i in 0..count {
        let slot = (begin as *const u8).wrapping_add(i * std::mem::size_of::<usize>());
        let Some(status) = safe_read(slot as *const *const u8) else {
            break;
        };
        if status.is_null() {
            continue;
        }
        if safe_read(status.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32) != Some(kind_id) {
            continue;
        }
        out.push(status);
    }

    out
}

thread_local! {
    /// What the apply detour last reported; the grant wrapper runs on this thread.
    static PENDING_APPLY: std::cell::Cell<Option<(u32, u32, Option<f32>)>> =
        const { std::cell::Cell::new(None) };
}

// Interior landmarks; each DELTA is its anchor's offset from the function start.
const ON_STATUS_GRANT_SIG: &str =
    "41 89 b7 b0 00 00 00 c4 c1 7a 11 87 b4 00 00 00 c4 c1 7a 11 8f b8";
const ON_STATUS_GRANT_SIG_DELTA: usize = 0x2AC;

const ON_STATUS_GRANT_SIG_BACKUP: &str =
    "41 8b 87 b0 00 00 00 39 c8 0f 4c c8 41 89 4f 58 ff c9 c5 fa 2a c1";
const ON_STATUS_GRANT_SIG_BACKUP_DELTA: usize = 0x1A2;

/// `fn(ExStatus* comp, Out* out, u32 kind_id, CEntityInfo* applier, ...14 stack)`.
/// Arity is 18 and every argument must be forwarded, as on `ApplyStatusFunc`.
/// Note the kind is third here and the applier fourth.
#[allow(clippy::type_complexity)]
type GrantStatusFunc = unsafe extern "system" fn(
    *const usize,
    *const u8,
    u32,
    *const usize,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
) -> usize;

static GRANT_TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
static GRANT_TX: OnceLock<event::Tx> = OnceLock::new();

/// `applyStatus` does not write the value; this wrapper does, after it returns,
/// so an apply-time read misses the object being applied. The correction goes
/// out on the stacks-changed channel, and only when the total actually moved.
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn grant_status_detour(
    comp: *const usize,
    out: *const u8,
    kind_id: u32,
    applier: *const usize,
    a5: u64,
    a6: u64,
    a7: u64,
    a8: u64,
    a9: u64,
    a10: u64,
    a11: u64,
    a12: u64,
    a13: u64,
    a14: u64,
    a15: u64,
    a16: u64,
    a17: u64,
    a18: u64,
) -> usize {
    let trampoline = GRANT_TRAMPOLINE.load(Ordering::Acquire);
    if trampoline == 0 {
        return 0;
    }
    let original: GrantStatusFunc = std::mem::transmute(trampoline);

    PENDING_APPLY.with(|slot| slot.set(None));

    let ret = original(
        comp, out, kind_id, applier, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17,
        a18,
    );

    let Some(tx) = GRANT_TX.get() else {
        return ret;
    };

    let Some((actor_index, applied_kind, applied_value)) = PENDING_APPLY.with(|slot| slot.take())
    else {
        return ret;
    };
    if applied_kind != kind_id {
        return ret;
    }

    let status = match safe_read(out.wrapping_add(APPLY_OUT_STATUS_OFFSET) as *const *const u8) {
        Some(status) if !status.is_null() => status,
        _ => return ret,
    };

    let facts = object_facts(status);
    let summed = summed_value_in_component(comp, kind_id);
    let value_total = summed.map(|(v, _)| v);

    // Bit comparison: NaN != NaN would make an unreadable value emit forever.
    let corrected = match (applied_value, value_total) {
        (Some(before), Some(after)) => before.to_bits() != after.to_bits(),
        (None, Some(_)) => true,
        _ => false,
    };
    if !corrected {
        return ret;
    }

    let stacks = stack_depth_in_component(comp, kind_id, std::ptr::null())
        .map(|(n, _, _)| n)
        .unwrap_or(1)
        .max(1);

    let _ = tx.send(Message::OnStatusStacksChanged(StatusStacksChangedEvent {
        actor_index,
        status_id: kind_id,
        stacks,
        value_total,
        value_is_fraction: facts.value_is_fraction.or(summed.map(|(_, f)| f)),
        applier_index: facts.applier_index,
        source_ids: facts.source_ids,
        value: facts.value,
    }));

    ret
}

pub struct OnStatusGrantHook {
    tx: event::Tx,
}

impl OnStatusGrantHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_STATUS_GRANT_SIG)
            .map(|a| a - ON_STATUS_GRANT_SIG_DELTA)
            .or_else(|_| {
                process
                    .search_address_start(ON_STATUS_GRANT_SIG_BACKUP)
                    .map(|a| a - ON_STATUS_GRANT_SIG_BACKUP_DELTA)
            })
            .map_err(|e| anyhow!("Could not find status_grant: {e}"))?;

        let _ = GRANT_TX.set(self.tx.clone());

        #[cfg(feature = "console")]
        println!("Found status grant");

        unsafe {
            let detour = RawDetour::new(address as *const (), grant_status_detour as *const ())?;

            GRANT_TRAMPOLINE.store(detour.trampoline() as *const () as usize, Ordering::Release);
            detour.enable()?;
            std::mem::forget(detour);
        }

        Ok(())
    }
}

type DestroyStatusFunc = unsafe extern "system" fn(*const usize, *const u8, u8) -> usize;
type ClearAllStatusFunc = unsafe extern "system" fn(*const usize) -> usize;

type StacksChangedFunc =
    unsafe extern "system" fn(*const u8, u32, u32, i32, i32) -> usize;

static_detour! {
    static OnStatusDestroy: unsafe extern "system" fn(*const usize, *const u8, u8) -> usize;
    static OnStatusClearAll: unsafe extern "system" fn(*const usize) -> usize;
    static OnStatusStacksChanged:
        unsafe extern "system" fn(*const u8, u32, u32, i32, i32) -> usize;
}

struct CapturedRemoval {
    actor_index: u32,
    status_id: u32,
    stacks: Option<u32>,
    stacks_before: Option<u32>,
    value_total: Option<f32>,
    value_is_fraction: Option<bool>,
    applier_index: Option<u32>,
    source_ids: Option<[u32; 3]>,
    value: Option<f32>,
}

#[derive(Clone)]
pub struct OnStatusDestroyHook {
    tx: event::Tx,
}

impl OnStatusDestroyHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_STATUS_DESTROY_SIG)
            .map_err(|e| anyhow!("Could not find status_destroy: {e}"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found status destroy");

        unsafe {
            let func: DestroyStatusFunc = std::mem::transmute(address);
            OnStatusDestroy.initialize(func, move |comp, status, notify| {
                cloned_self.run(comp, status, notify)
            })?;
            OnStatusDestroy.enable()?;
        }

        Ok(())
    }

    /// `fn(ExStatus* comp, Status* st, bool notify_network)`. `notify` is 1
    /// when this client originated the removal; not used to filter, or every
    /// remote party member's statuses would disappear.
    fn run(&self, comp: *const usize, status: *const u8, notify: u8) -> usize {
        // Read before the call: the original frees the Status before it returns.
        let captured = if status.is_null() {
            None
        } else {
            let status_id = safe_read(status.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32);
            match (actor_index_for_status(status), status_id) {
                (Some(actor_index), Some(status_id)) => {
                    let summed = summed_value_in_component(comp, status_id);
                    let value_total = summed.map(|(v, _)| v);
                    let value_is_fraction = summed.map(|(_, f)| f);
                    let facts = object_facts(status);
                    let applier_index = facts.applier_index;
                    let source_ids = facts.source_ids;
                    let own_value = facts.value;
                    let value_is_fraction = facts.value_is_fraction.or(value_is_fraction);
                    match stack_depth_in_component(comp, status_id, status) {
                        // `stacks` is the depth that remains once this object goes.
                        Some((depth, objects, true)) => {
                            let remaining = if is_stackable(status_id) {
                                let going = safe_read(
                                    status.wrapping_add(STATUS_STACK_LEVEL_OFFSET) as *const i32,
                                )
                                .map(|l| l.max(1) as u32)
                                .unwrap_or(depth);
                                depth.saturating_sub(going)
                            } else if objects > 1 {
                                // Another source's copy is still on the target.
                                1
                            } else {
                                0
                            };
                            Some(CapturedRemoval {
                                actor_index,
                                status_id,
                                stacks: Some(remaining),
                                stacks_before: Some(depth),
                                value_total,
                                value_is_fraction,
                                applier_index,
                                source_ids,
                                value: own_value,
                            })
                        }
                        // clearAll already unlinked and reported this one.
                        Some((_, _, false)) => None,
                        None => Some(CapturedRemoval {
                            actor_index,
                            status_id,
                            stacks: None,
                            stacks_before: None,
                            value_total,
                            value_is_fraction,
                            applier_index,
                            source_ids,
                            value: own_value,
                        }),
                    }
                }
                _ => None,
            }
        };

        let ret = unsafe { OnStatusDestroy.call(comp, status, notify) };

        if let Some(removal) = captured {
            REFRESH_GATE.forget(removal.actor_index, removal.status_id);
            VALUE_WATCH.forget(removal.actor_index, removal.status_id);
            let _ = self.tx.send(Message::OnStatusRemoved(StatusRemovedEvent {
                actor_index: removal.actor_index,
                status_id: removal.status_id,
                stacks: removal.stacks,
                stacks_before: removal.stacks_before,
                value_total: removal.value_total,
                value_is_fraction: removal.value_is_fraction,
                applier_index: removal.applier_index,
                source_ids: removal.source_ids,
                value: removal.value,
            }));
        }

        ret
    }
}

/// Death, respawn and entity teardown free every status at once, without
/// going through `destroyStatus`.
#[derive(Clone)]
pub struct OnStatusClearAllHook {
    tx: event::Tx,
}

impl OnStatusClearAllHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_STATUS_CLEAR_ALL_SIG)
            .map_err(|e| anyhow!("Could not find status_clear_all: {e}"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found status clear all");

        unsafe {
            let func: ClearAllStatusFunc = std::mem::transmute(address);
            OnStatusClearAll.initialize(func, move |comp| cloned_self.run(comp))?;
            OnStatusClearAll.enable()?;
        }

        Ok(())
    }

    fn run(&self, comp: *const usize) -> usize {
        // Walk before the call: the original frees every entry.
        let cleared = snapshot_component_statuses(comp);

        let ret = unsafe { OnStatusClearAll.call(comp) };

        for (actor_index, status_id, facts) in cleared {
            REFRESH_GATE.forget(actor_index, status_id);
            VALUE_WATCH.forget(actor_index, status_id);
            let _ = self.tx.send(Message::OnStatusRemoved(StatusRemovedEvent {
                actor_index,
                status_id,
                stacks: Some(0),
                stacks_before: None,
                value_total: Some(0.0),
                value_is_fraction: facts.value_is_fraction,
                applier_index: facts.applier_index,
                source_ids: facts.source_ids,
                value: facts.value,
            }));
        }

        ret
    }
}

#[derive(Clone)]
pub struct OnStatusStacksChangedHook {
    tx: event::Tx,
}

impl OnStatusStacksChangedHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_STATUS_STACKS_SIG)
            .map_err(|e| anyhow!("Could not find status_stacks_changed: {e}"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found status stacks changed");

        unsafe {
            let func: StacksChangedFunc = std::mem::transmute(address);
            OnStatusStacksChanged.initialize(func, move |st, mode, tag, new_count, delta| {
                cloned_self.run(st, mode, tag, new_count, delta)
            })?;
            OnStatusStacksChanged.enable()?;
        }

        Ok(())
    }

    /// `fn(Status* st, u32 mode, u32 tag, i32 new_level, i32 delta)`. Mode 1
    /// is the removal notify out of `destroyStatus`, already reported by the
    /// destroy hook, so it is ignored here.
    fn run(&self, st: *const u8, mode: u32, tag: u32, new_level: i32, delta: i32) -> usize {
        // Read before the call: the removal path frees the object.
        let captured = if mode != 0 || st.is_null() {
            None
        } else {
            match (
                safe_read(st.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32),
                actor_index_for_status(st),
            ) {
                (Some(status_id), Some(actor_index)) => {
                    if new_level > 0 {
                        Some((actor_index, status_id, new_level as u32, object_facts(st)))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        let ret = unsafe { OnStatusStacksChanged.call(st, mode, tag, new_level, delta) };

        if let Some((actor_index, status_id, stacks, facts)) = captured {
            let _ = self
                .tx
                .send(Message::OnStatusStacksChanged(StatusStacksChangedEvent {
                    actor_index,
                    status_id,
                    stacks,
                    // No component pointer here, so there is no vector to sum.
                    value_total: None,
                    value_is_fraction: facts.value_is_fraction,
                    applier_index: facts.applier_index,
                    source_ids: facts.source_ids,
                    value: facts.value,
                }));
        }

        let _ = delta;
        ret
    }
}

fn snapshot_component_statuses(comp: *const usize) -> Vec<(u32, u32, ObjectFacts)> {
    let mut cleared = Vec::new();

    let Some(begin) =
        safe_read((comp as *const u8).wrapping_add(COMPONENT_VECTOR_BEGIN_OFFSET) as *const usize)
    else {
        return cleared;
    };
    let Some(end) = vector_end(comp) else {
        return cleared;
    };

    if begin == 0 || end < begin {
        return cleared;
    }

    let count = (end - begin) / std::mem::size_of::<usize>();
    if count > MAX_STATUSES_PER_CLEAR {
        return cleared;
    }

    for i in 0..count {
        let slot = (begin as *const u8).wrapping_add(i * std::mem::size_of::<usize>());
        let Some(status) = safe_read(slot as *const *const u8) else {
            continue;
        };
        if status.is_null() {
            continue;
        }
        let Some(status_id) = safe_read(status.wrapping_add(STATUS_KIND_ID_OFFSET) as *const u32)
        else {
            continue;
        };
        let Some(actor_index) = actor_index_for_status(status) else {
            continue;
        };
        cleared.push((actor_index, status_id, object_facts(status)));
    }

    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: u64 = 1000;
    const ACTOR: u32 = 0x8000_0000;
    const DMG_CUT: u32 = 0x4;

    #[test]
    fn the_first_refresh_of_a_pair_always_gets_through() {
        let gate = RefreshGate::new();

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
    }

    #[test]
    fn refreshes_inside_the_gap_are_suppressed() {
        let gate = RefreshGate::new();
        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));

        let suppressed = (1..=116)
            .filter(|ms| !gate.should_emit(ACTOR, DMG_CUT, *ms, GAP))
            .count();

        assert_eq!(suppressed, 116);
    }

    #[test]
    fn a_refresh_gets_through_once_the_gap_has_passed() {
        let gate = RefreshGate::new();

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
        assert!(!gate.should_emit(ACTOR, DMG_CUT, 999, GAP));
        assert!(gate.should_emit(ACTOR, DMG_CUT, 1_000, GAP));
        assert!(!gate.should_emit(ACTOR, DMG_CUT, 1_999, GAP));
        assert!(gate.should_emit(ACTOR, DMG_CUT, 2_000, GAP));
    }

    #[test]
    fn each_actor_and_kind_is_gated_independently() {
        let gate = RefreshGate::new();

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
        assert!(gate.should_emit(ACTOR + 1, DMG_CUT, 0, GAP));
        assert!(gate.should_emit(ACTOR, DMG_CUT + 1, 0, GAP));
        assert!(!gate.should_emit(ACTOR, DMG_CUT, 1, GAP));
        assert!(!gate.should_emit(ACTOR + 1, DMG_CUT, 1, GAP));
    }

    #[test]
    fn a_slot_collision_costs_saving_and_never_an_event() {
        let gate = RefreshGate::new();
        let key = |actor: u32, kind: u32| ((actor as u64) << 32) | kind as u64;

        let base = key(ACTOR, DMG_CUT);
        let (other_actor, other_kind) = (0..4u32)
            .flat_map(|a| (0..4096u32).map(move |k| (ACTOR + a, k)))
            .find(|(a, k)| {
                let candidate = key(*a, *k);
                candidate != base && RefreshGate::slot(candidate) == RefreshGate::slot(base)
            })
            .expect("some pair must collide into 64 slots");

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
        assert!(gate.should_emit(other_actor, other_kind, 1, GAP));
        assert!(gate.should_emit(ACTOR, DMG_CUT, 2, GAP));
    }

    #[test]
    fn every_party_member_holding_one_aura_gets_its_own_slot() {
        let slots: std::collections::HashSet<usize> = (0..4)
            .map(|slot| RefreshGate::slot(((ACTOR + slot) as u64) << 32 | DMG_CUT as u64))
            .collect();

        assert_eq!(slots.len(), 4, "each party member must map to its own slot");
    }

    #[test]
    fn a_removal_lets_the_next_application_through_immediately() {
        let gate = RefreshGate::new();

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
        assert!(!gate.should_emit(ACTOR, DMG_CUT, 100, GAP));

        gate.forget(ACTOR, DMG_CUT);

        assert!(gate.should_emit(ACTOR, DMG_CUT, 200, GAP));
    }

    #[test]
    fn forgetting_a_pair_leaves_another_pairs_gap_alone() {
        let gate = RefreshGate::new();
        let other_kind = DMG_CUT + 1;

        assert!(gate.should_emit(ACTOR, DMG_CUT, 0, GAP));
        assert!(gate.should_emit(ACTOR, other_kind, 0, GAP));

        gate.forget(ACTOR, DMG_CUT);

        assert!(gate.should_emit(ACTOR, DMG_CUT, 10, GAP), "forgotten pair emits");
        assert!(!gate.should_emit(ACTOR, other_kind, 10, GAP), "untouched pair still gated");
    }

    #[test]
    fn only_stackable_kinds_are_stackable() {
        assert!(is_stackable(0x09), "Mirror Image stacks");
        assert!(is_stackable(0x3B), "Stackable DEF DOWN stacks");
        assert!(!is_stackable(0x38), "DMG Cap UP does NOT stack");
        assert!(!is_stackable(0x03), "plain DEF DOWN is below the range");
        assert_eq!(STACKABLE_KINDS.len(), 68);
    }

    #[test]
    fn the_value_getter_decoder_reads_both_shapes() {
        assert_eq!(
            decode_value_getter([0xC5, 0xFA, 0x10, 0x81, 0xB8, 0x00, 0x00, 0x00, 0xC3]),
            Some((0xB8, false)),
            "vmovss leaf"
        );
        assert_eq!(
            decode_value_getter([0xC5, 0xFA, 0x2A, 0x81, 0xB0, 0x00, 0x00, 0x00, 0xC3]),
            Some((0xB0, true)),
            "vcvtsi2ss leaf"
        );
        assert_eq!(
            decode_value_getter([0xC5, 0xFA, 0x10, 0x81, 0xC8, 0x00, 0x00, 0x00, 0xC3]),
            Some((0xC8, false))
        );
    }

    #[test]
    fn a_getter_that_is_not_a_plain_leaf_reports_no_value() {
        assert_eq!(
            decode_value_getter([0xC5, 0xF8, 0x57, 0xC0, 0xC3, 0xCC, 0xCC, 0xCC, 0xCC]),
            None,
            "a zero-returning getter has no field to read"
        );
        assert_eq!(
            decode_value_getter([0xC5, 0xFA, 0x10, 0x81, 0xB8, 0x00, 0x00, 0x00, 0x48]),
            None,
            "must be a leaf"
        );
        assert_eq!(
            decode_value_getter([0xC5, 0xFA, 0x10, 0x81, 0x00, 0x00, 0x01, 0x00, 0xC3]),
            None,
            "0x10000 is not a member offset"
        );
    }

    #[test]
    fn the_stackable_table_is_sorted() {
        let mut sorted = STACKABLE_KINDS;
        sorted.sort_unstable();
        assert_eq!(sorted, STACKABLE_KINDS);
    }
}
