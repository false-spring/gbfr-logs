//! Actor identity: whose row does an instance's damage belong to.

use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

use super::{globals, player, rtti, safe_read, v_func, warn_stale_rva, AnchorReport};

type GetEntityHashID0x58 = unsafe extern "system" fn(*const usize, *const u32) -> *const usize;

#[inline(always)]
pub fn actor_type_id(actor_ptr: *const usize) -> u32 {
    let mut type_id: u32 = 0;

    unsafe {
        v_func::<GetEntityHashID0x58>(actor_ptr, TYPE_ID_VFUNC_SLOT)(actor_ptr, &mut type_id as *mut u32);
    }

    type_id
}

pub(crate) const ACTOR_PLAYER_KEY_OFFSET: usize = 0x1AB40;

/// The u32 after the player key holds this when the key is real.
const ACTOR_PLAYER_KEY_SENTINEL: u32 = 0x887AE0B0;

use protocol::PLAYER_ID_BASE;

const INSTANCE_ID_MASK: u32 = 0x7FFF_FFFF;

fn derive_instance_id(actor_ptr: *const usize, vtable: Option<usize>) -> u32 {
    let mut x = actor_ptr as u64;
    x ^= (vtable.unwrap_or(0) as u64).rotate_left(32);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as u32) & INSTANCE_ID_MASK
}

pub(crate) fn read_player_key(actor_ptr: *const usize) -> Option<u32> {
    let [key, sentinel] = safe_read::<[u32; 2]>(
        (actor_ptr as *const u8).wrapping_add(ACTOR_PLAYER_KEY_OFFSET) as *const [u32; 2],
    )?;

    (key != 0 && sentinel == ACTOR_PLAYER_KEY_SENTINEL).then_some(key)
}

fn read_vtable_ptr(actor_ptr: *const usize) -> Option<usize> {
    safe_read(actor_ptr as *const usize)
}

/// Instance offset of the embedded profile record.
pub(crate) const INSTANCE_RECORD_OFFSET: usize = 0x15030;
pub(crate) const RECORD_KEY_MIRROR_OFFSET: usize = 0x5EA8;
pub(crate) const RECORD_SNAPSHOT_PTR_OFFSET: usize = 0x5E60;
pub(crate) const SNAPSHOT_PARTY_INDEX_OFFSET: usize = 0x22C;
/// Entity/CEntityInfo -> `m_pSpecifiedInstance`.
pub(crate) const ENTITY_SPECIFIED_INSTANCE_OFFSET: usize = 0x70;
pub(crate) const TYPE_ID_VFUNC_SLOT: usize = 0x58;

fn read_player_slot_for_key(actor_ptr: *const usize, key: u32) -> Option<(u32, u8, usize)> {
    let record = (actor_ptr as *const u8).wrapping_add(INSTANCE_RECORD_OFFSET);

    let rec_key = safe_read::<u32>(record.wrapping_add(RECORD_KEY_MIRROR_OFFSET) as *const u32)?;
    if rec_key != key {
        return None;
    }

    let snapshot =
        safe_read::<*const u8>(record.wrapping_add(RECORD_SNAPSHOT_PTR_OFFSET) as *const *const u8)?;
    if snapshot.is_null() {
        return None;
    }
    let slot = safe_read::<u32>(snapshot.wrapping_add(SNAPSHOT_PARTY_INDEX_OFFSET) as *const u32)?;
    (slot <= 3).then_some((key, slot as u8, record as usize))
}

/// Is this damage source owned by THIS client, rather than by an online peer?
/// `None` means the question does not apply or could not be answered, and
/// callers must fail open and keep the event.
///
/// `read_player_key` gates the record walk. `INSTANCE_RECORD_OFFSET` is only
/// valid on the shared player base class, and `safe_read` does not know that:
/// on a summon pet it returns whatever bytes are mapped there.
pub(crate) fn source_owner_is_local(actor_ptr: *const usize) -> Option<bool> {
    read_player_key(actor_ptr)?;
    let record = (actor_ptr as *const u8).wrapping_add(INSTANCE_RECORD_OFFSET);

    // Ask the lobby first. A local CPU companion and a real online peer look
    // alike on every other byte of the record.
    match player::network_locality(record as *const usize) {
        player::NetworkLocality::Decided(is_local) => return Some(is_local),
        // The record has a network id but our own side of the comparison is
        // missing. Abstain. The fallback below is wrong for this population,
        // not merely weaker.
        player::NetworkLocality::Unresolved => return None,
        player::NetworkLocality::NoLoadoutSub => {}
    }

    // You are never replicated to yourself, and nobody is replicated offline,
    // so with no loadout sub attached `src_type` is the only signal left.
    let src_type =
        safe_read::<u32>(record.wrapping_add(player::RECORD_SOURCE_TYPE_OFFSET) as *const u32)?;
    Some(src_type < 4)
}

// Called for players, enemies and sub-entities alike, so it must never fault
// and must return a stable id per distinct instance. There is no cache, so a
// player whose party slot is unreadable gets a pointer-derived id and keeps it.
#[inline]
pub fn actor_idx(actor_ptr: *const usize) -> u32 {
    probe_actor(actor_ptr).id
}

pub(crate) struct ActorProbe {
    pub id: u32,
    pub slot: Option<(u32, u8, usize)>,
}

pub(crate) fn probe_actor(actor_ptr: *const usize) -> ActorProbe {
    let key = read_player_key(actor_ptr);

    if let Some((player_key, slot, record)) =
        key.and_then(|key| read_player_slot_for_key(actor_ptr, key))
    {
        return ActorProbe {
            id: PLAYER_ID_BASE | u32::from(slot),
            slot: Some((player_key, slot, record)),
        };
    }

    ActorProbe {
        id: derive_instance_id(actor_ptr, read_vtable_ptr(actor_ptr)),
        slot: None,
    }
}

/// Every summon body class that stores its summoner at +0xFE8, by mangled RTTI
/// name. `rtti::setup` turns each name into a live vtable RVA at startup. A
/// class that does not resolve is absent, and its damage goes uncredited.
pub(crate) const SUMMON_VTABLE_CLASSES: &[&str] = &[
    ".?AVBehaviorSummonObjectBase@@", // generic/data-driven body
    ".?AVSo0000@@",                   // Lucilius
    ".?AVSo4e00@@",                   // Albacore
    ".?AVSo6400@@",                   // Wheel of Fate
    ".?AVSo0200@@",                   // Rolan
    ".?AVSo2001@@",                   // Silverslime var.
    ".?AVSo4502@@",                   // Lilith var.
    ".?AVSo4500@@",                   // Lilith
    ".?AVSo4c00@@",                   // Managarmr Nihilla
    ".?AVSo1d00@@",                   // Quakadile
    ".?AVSo9200@@",                   // Beelzebub
    ".?AVSo0d00@@",                   // Goblin Soldier
    ".?AVSo4f00@@",                   // Hope-Filled Skydwellers
    ".?AVSo5600@@",                   // Mellose Clan
    ".?AVSo5700@@",                   // Crew Alliance Rafale
    ".?AVSo5f01@@",                   // Cat var.
    ".?AVSo1100@@",                   // Goblin Warrior
    ".?AVSo1100Base@@",               // generic body
];

static SUMMON_VTABLE_RVAS: OnceLock<Vec<usize>> = OnceLock::new();

/// Call once, after `rtti::setup`.
pub(crate) fn setup_summon_vtables() {
    let mut rvas: Vec<usize> = SUMMON_VTABLE_CLASSES
        .iter()
        .filter_map(|&class| rtti::resolved_vtable(class))
        .collect();
    // Sorted here, never by hand. `is_summon_base_vtable` binary-searches this.
    rvas.sort_unstable();

    let missing = rtti::unresolved(SUMMON_VTABLE_CLASSES);
    if !missing.is_empty() {
        log::warn!(
            "[rtti] {}/{} summon body classes resolved — MISSING: {}. Damage from those \
             summons is credited to NOBODY (their two-hop summoner walk fails closed); \
             the meter will show a party total lower than the game's.",
            rvas.len(),
            SUMMON_VTABLE_CLASSES.len(),
            missing.join(", ")
        );
    }

    if SUMMON_VTABLE_RVAS.set(rvas).is_err() {
        log::warn!(
            "[rtti] summon vtables were read before resolution; \
             the table stays EMPTY for this session and no summon damage is attributed"
        );
    }
}

/// Empty until `setup_summon_vtables` runs, and empty is the fail-closed
/// answer. No pointer is ever mistaken for a summon body.
fn summon_vtable_rvas() -> &'static [usize] {
    SUMMON_VTABLE_RVAS.get_or_init(Vec::new)
}

static SUMMON_VTABLE_RVA_WARNED: AtomicBool = AtomicBool::new(false);

pub(crate) fn validate_anchors() -> AnchorReport {
    let mut report = AnchorReport::new();

    let missing = rtti::unresolved(SUMMON_VTABLE_CLASSES);
    report.check(missing.is_empty(), || {
        format!(
            "{}/{} summon body classes unresolved: {} (actor.rs — that summon's damage \
             goes uncredited)",
            missing.len(),
            SUMMON_VTABLE_CLASSES.len(),
            missing.join(", ")
        )
    });

    report.check(
        summon_vtable_rvas().windows(2).all(|w| w[0] < w[1]),
        || "summon vtable table not sorted (actor.rs binary_search)".to_string(),
    );

    report
}

fn is_summon_base_vtable(summon: *const usize) -> bool {
    let base = globals::MODULE_BASE.load(std::sync::atomic::Ordering::Relaxed);
    if base == 0 {
        return false;
    }
    let Some(vtable) = safe_read(summon as *const usize) else {
        return false;
    };
    let Some(rva) = (vtable as usize).checked_sub(base) else {
        return false;
    };
    summon_vtable_rvas().binary_search(&rva).is_ok()
}

fn two_hop_summoner(
    source: *const usize,
    hop1_idx_off: usize,
    hop1_ptr_off: usize,
) -> Option<*const usize> {
    let summon = parent_specified_instance_at(source, {
        gated_parent_offset(source, hop1_idx_off, hop1_ptr_off)?
    })?;

    // Re-type by comparing the vtable pointer. Never call a vfunc on a pointer
    // that came out of the sweep.
    if !is_summon_base_vtable(summon) {
        warn_stale_rva(&SUMMON_VTABLE_RVA_WARNED, "summon-base vtable table (actor.rs)");
        return None;
    }

    let owner = parent_specified_instance_at(summon, {
        gated_parent_offset(summon, 0xFE0, 0xFE8)?
    })?;

    if !vfunc_slot_readable(owner, TYPE_ID_VFUNC_SLOT) {
        return None;
    }
    read_player_key(owner)?;
    Some(owner)
}

/// One hop only; `resolve_source_parent_typed` walks the chain.
#[inline(always)]
fn resolve_source_parent_hop(source_type_id: u32, source: *const usize) -> Option<*const usize> {
    match source_type_id {
        // SoAhrimanBaseLaser
        0x8FE0DF11 => return two_hop_summoner(source, 0x4F0, 0x4F8),
        // We8090 / We8091 / We8170 (em8000 "Seofon" sword entities)
        0xAE1F95D9 | 0xAE1E9FFC | 0x2E0DE3A8 => return two_hop_summoner(source, 0xFE0, 0xFE8),
        _ => {}
    }

    let parent_offset = match source_type_id {
        0x2AF678E8 => 0xE58, // Pl0700Ghost -> Pl0700 (Ferry)
        0x8364C8BC => 0x4E8, // Pl0700GhostSatellite (Ferry's Umlauf) -> Pl0700
        0x5B1AB457 => 0x4E0, // Wp2290 -> Pl2200 (Seofon)
        0xF5755C0E => 0x1CA98, // Pl2000 (Id dragon) -> Id human (Pl1900)
        0xC9F45042 => {
            // Wp1890 (Cagliostro's sled) -> Pl1800.
            gated_parent_offset(source, 0x550, 0x558)?
        }
        // Summon bodies store their summoner as an entity handle {+0xFE0 idx, +0xFE8 ptr}.
        0xD2E5407A // So0000  (Lucilius)
        | 0x6D068BDE // So0200  (Rolan)
        | 0x69893920 // So0d00  (Goblin Soldier)
        | 0x1D3EDC63 // So1100  (Goblin Warrior)
        | 0xAE913DE3 // So1100Base (generic summon body)
        | 0xDFEC5706 // So1d00  (Quakadile)
        | 0x34894579 // So2001  (Silverslime var.)
        | 0x1DB19581 // So4500  (Lilith)
        | 0x9F394F85 // So4502  (Lilith var.)
        | 0x65294C5C // So4c00  (Managarmr Nihilla)
        | 0x18617D59 // So4e00  (Albacore)
        | 0x925ADE1B // So4f00  (Hope-Filled Skydwellers)
        | 0x6093301C // So5600  (Mellose Clan)
        | 0xA22E16CF // So5700  (Crew Alliance Rafale)
        | 0x0F617FF0 // So5f01  (Cat var.)
        | 0xF065D8B8 // So6400  (Wheel of Fate)
        | 0x5395CE93 // So9200  (Beelzebub)
        | 0xB0792857 // BehaviorSummonObjectBase (generic summon body)
        => gated_parent_offset(source, 0xFE0, 0xFE8)?,
        0x3B5133C4 => {
            // Pl8000 (controllable-summon spawner).
            gated_parent_offset(source, 0x23E0, 0x23E8)?
        }
        _ => return None,
    };

    let parent = parent_specified_instance_at(source, parent_offset)?;
    if !vfunc_slot_readable(parent, TYPE_ID_VFUNC_SLOT) {
        return None;
    }
    Some(parent)
}

/// Add support for recursive calls to identify edge cases
/// such as Id (Dragonform) summoning Beelzebub
const MAX_PARENT_HOPS: usize = 3;

#[inline(always)]
fn resolve_source_parent_typed(
    source_type_id: u32,
    source: *const usize,
) -> Option<(*const usize, u32)> {
    let mut current = source;
    let mut current_type = source_type_id;
    let mut resolved = None;

    for _ in 0..MAX_PARENT_HOPS {
        let Some(parent) = resolve_source_parent_hop(current_type, current) else {
            break;
        };

        let Some(type_fn) = validated_vfunc(parent, TYPE_ID_VFUNC_SLOT) else {
            break;
        };
        let parent_type = actor_type_id_via(parent, type_fn);
        resolved = Some((parent, parent_type));

        if std::ptr::eq(parent, current) {
            break;
        }
        current = parent;
        current_type = parent_type;
    }

    resolved
}

#[inline(always)]
fn resolve_source_parent_ptr(source_type_id: u32, source: *const usize) -> Option<*const usize> {
    resolve_source_parent_typed(source_type_id, source).map(|(parent, _)| parent)
}

#[inline(always)]
pub fn get_source_parent(source_type_id: u32, source: *const usize) -> Option<(u32, u32)> {
    let (parent, parent_type) = resolve_source_parent_typed(source_type_id, source)?;
    Some((parent_type, actor_idx(parent)))
}

pub(crate) fn source_owner_via_parent_is_local(
    source_type_id: u32,
    source: *const usize,
) -> Option<bool> {
    let parent = resolve_source_parent_ptr(source_type_id, source)?;
    source_owner_is_local(parent)
}

pub(crate) fn source_is_local_for_dedup(source_type_id: u32, source: *const usize) -> Option<bool> {
    source_owner_via_parent_is_local(source_type_id, source)
        .or_else(|| source_owner_is_local(source))
}

/// Owner handles are {idx, CEntityInfo*, serial} triples that can be legitimately empty.
#[inline(always)]
fn gated_parent_offset(
    source: *const usize,
    idx_offset: usize,
    ptr_offset: usize,
) -> Option<usize> {
    match safe_read((source as *const u8).wrapping_add(idx_offset) as *const u32) {
        Some(idx) if idx != 0 => Some(ptr_offset),
        _ => None,
    }
}

/// Callers that invoke the result must call through THIS pointer;
/// re-walking the vtable raw reopens the probe-crash window.
fn validated_vfunc(instance: *const usize, offset: usize) -> Option<*const usize> {
    let vtable: *const usize = safe_read(instance as *const *const usize)?;
    if vtable.is_null() {
        return None;
    }
    match safe_read::<usize>((vtable as *const u8).wrapping_add(offset) as *const usize) {
        Some(slot) if slot != 0 => Some(slot as *const usize),
        _ => None,
    }
}

pub(crate) fn vfunc_slot_readable(instance: *const usize, offset: usize) -> bool {
    validated_vfunc(instance, offset).is_some()
}

pub(crate) fn parent_actor_idx(entity_ptr: *const usize) -> u32 {
    let source_idx = actor_idx(entity_ptr);
    let Some(type_fn) = validated_vfunc(entity_ptr, 0x58) else {
        return source_idx;
    };
    let source_type_id = actor_type_id_via(entity_ptr, type_fn);
    let (_, source_parent_idx) =
        get_source_parent(source_type_id, entity_ptr).unwrap_or((source_type_id, source_idx));
    source_parent_idx
}

fn actor_type_id_via(instance: *const usize, type_fn: *const usize) -> u32 {
    let mut type_id: u32 = 0;
    unsafe {
        let func: GetEntityHashID0x58 = std::mem::transmute(type_fn);
        func(instance, &mut type_id as *mut u32);
    }
    type_id
}

#[inline(always)]
fn parent_specified_instance_at(actor_ptr: *const usize, offset: usize) -> Option<*const usize> {
    let info: *const usize = safe_read(actor_ptr.wrapping_byte_add(offset) as *const *const usize)?;
    if info.is_null() {
        return None;
    }
    let instance: *const usize = safe_read((info as *const u8).wrapping_add(ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const *const usize)?;
    if instance.is_null() {
        return None;
    }
    Some(instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_BUF_SIZE: usize = INSTANCE_RECORD_OFFSET + RECORD_KEY_MIRROR_OFFSET + 4;

    fn fake_instance(fields: Option<(u32, u32)>) -> *const usize {
        let buf: &'static mut [u8] = Box::leak(vec![0u8; FAKE_BUF_SIZE].into_boxed_slice());

        if let Some((key, sentinel)) = fields {
            buf[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4]
                .copy_from_slice(&key.to_le_bytes());
            buf[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
                .copy_from_slice(&sentinel.to_le_bytes());
        }

        buf.as_ptr() as *const usize
    }

    fn fake_player_with_slot(key: u32, slot: u8) -> *const usize {
        let size = SNAP_OFF + SNAPSHOT_PARTY_INDEX_OFFSET + 4;
        let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
        let base = buf.as_ptr() as usize;

        buf[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4].copy_from_slice(&key.to_le_bytes());
        buf[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
            .copy_from_slice(&ACTOR_PLAYER_KEY_SENTINEL.to_le_bytes());

        let rec_key = INSTANCE_RECORD_OFFSET + RECORD_KEY_MIRROR_OFFSET;
        buf[rec_key..rec_key + 4].copy_from_slice(&key.to_le_bytes());

        let rec_snap = INSTANCE_RECORD_OFFSET + RECORD_SNAPSHOT_PTR_OFFSET;
        buf[rec_snap..rec_snap + 8].copy_from_slice(&((base + SNAP_OFF) as u64).to_le_bytes());
        let party = SNAP_OFF + SNAPSHOT_PARTY_INDEX_OFFSET;
        buf[party..party + 4].copy_from_slice(&(slot as u32).to_le_bytes());

        buf.as_ptr() as *const usize
    }

    const SNAP_OFF: usize = INSTANCE_RECORD_OFFSET + RECORD_SNAPSHOT_PTR_OFFSET + 0x100;

    fn fake_loading_player(key: u32) -> *const usize {
        let size = SNAP_OFF + SNAPSHOT_PARTY_INDEX_OFFSET + 4;
        let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());

        buf[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4]
            .copy_from_slice(&key.to_le_bytes());
        buf[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
            .copy_from_slice(&ACTOR_PLAYER_KEY_SENTINEL.to_le_bytes());

        buf.as_ptr() as *const usize
    }

    fn finish_loading(ptr: *const usize, key: u32, slot: u8) {
        let base = ptr as usize;
        unsafe {
            ((base + INSTANCE_RECORD_OFFSET + RECORD_KEY_MIRROR_OFFSET) as *mut u32)
                .write_unaligned(key);
            ((base + INSTANCE_RECORD_OFFSET + RECORD_SNAPSHOT_PTR_OFFSET) as *mut u64)
                .write_unaligned((base + SNAP_OFF) as u64);
            ((base + SNAP_OFF + SNAPSHOT_PARTY_INDEX_OFFSET) as *mut u32)
                .write_unaligned(slot as u32);
        }
    }

    #[test]
    fn concrete_actor_instances_receive_distinct_ids() {
        let a = actor_idx(0x1000 as *const usize);
        let b = actor_idx(0x2000 as *const usize);

        assert_ne!(a, b);
        assert_eq!(a, actor_idx(0x1000 as *const usize));
        assert_eq!(b, actor_idx(0x2000 as *const usize));
    }

    #[test]
    fn a_players_id_is_exactly_their_party_slot() {
        let key = 0xDEAD_0001;
        for slot in 0..=3u8 {
            assert_eq!(
                actor_idx(fake_player_with_slot(key, slot)),
                PLAYER_ID_BASE | u32::from(slot)
            );
        }
    }

    #[test]
    fn same_key_different_slots_do_not_merge() {
        let key = 0xDEAD_1000;
        assert_ne!(
            actor_idx(fake_player_with_slot(key, 0)),
            actor_idx(fake_player_with_slot(key, 2))
        );
    }

    #[test]
    fn different_keys_in_one_slot_collapse_to_that_slot() {
        let a = actor_idx(fake_player_with_slot(0xAAAA_0000, 2));
        let b = actor_idx(fake_player_with_slot(0xBBBB_0000, 2));

        assert_eq!(a, b);
        assert_eq!(a, PLAYER_ID_BASE | 2);
    }

    #[test]
    fn player_ids_never_collide_with_instance_derived_ids() {
        assert!(actor_idx(fake_player_with_slot(0xDEAD_1001, 1)) >= PLAYER_ID_BASE);

        for ptr in [
            fake_instance(None),
            fake_loading_player(0xDEAD_1002),
            0x1000 as *const usize,
        ] {
            assert!(actor_idx(ptr) < PLAYER_ID_BASE);
        }
    }

    #[test]
    fn key_without_sentinel_never_reaches_the_slot_path() {
        let key = 0xDEAD_0003;
        let impostor = fake_player_with_slot(key, 1);
        assert_eq!(actor_idx(impostor), PLAYER_ID_BASE | 1);

        restamp_key(impostor, key, 0);
        assert!(actor_idx(impostor) < PLAYER_ID_BASE);
    }

    // The three tests below pin behaviour that got worse when the actor
    // registry went away. They assert the cost, not the ideal.

    #[test]
    fn slotless_forms_of_one_player_no_longer_reunite() {
        let key = 0xDEAD_1002;
        let base = actor_idx(fake_player_with_slot(key, 3));
        let slotless = actor_idx(fake_instance(Some((key, ACTOR_PLAYER_KEY_SENTINEL))));

        assert_ne!(base, slotless);
        assert!(slotless < PLAYER_ID_BASE);
    }

    #[test]
    fn two_instances_of_one_player_split_while_slotless() {
        let key = 0xDEAD_0001;
        let first = fake_instance(Some((key, ACTOR_PLAYER_KEY_SENTINEL)));
        let second = fake_instance(Some((key, ACTOR_PLAYER_KEY_SENTINEL)));

        assert_ne!(actor_idx(first), actor_idx(second));
    }

    #[test]
    fn a_loading_players_id_changes_once_its_slot_appears() {
        let key = 0xDEAD_2001;
        let p = fake_loading_player(key);

        let while_loading = actor_idx(p);
        assert!(while_loading < PLAYER_ID_BASE);

        finish_loading(p, key, 1);
        let once_loaded = actor_idx(p);

        assert_ne!(while_loading, once_loaded);
        assert_eq!(once_loaded, PLAYER_ID_BASE | 1);
    }

    fn restamp_key(ptr: *const usize, key: u32, sentinel: u32) {
        let base = ptr as usize;
        unsafe {
            ((base + ACTOR_PLAYER_KEY_OFFSET) as *mut u32).write_unaligned(key);
            ((base + ACTOR_PLAYER_KEY_OFFSET + 4) as *mut u32).write_unaligned(sentinel);
        }
    }

    fn restamp_vtable(ptr: *const usize, vtable: usize) {
        unsafe {
            (ptr as *mut usize).write_unaligned(vtable);
        }
    }

    #[test]
    fn a_recycled_player_pointer_follows_the_live_record() {
        let key = 0xDEAD_3000;
        let ptr = fake_player_with_slot(key, 0);
        assert_eq!(actor_idx(ptr), PLAYER_ID_BASE);

        finish_loading(ptr, key, 3);
        assert_eq!(actor_idx(ptr), PLAYER_ID_BASE | 3);
    }

    #[test]
    fn a_recycled_player_pointer_hosting_a_keyless_actor_gets_a_fresh_id() {
        let ptr = fake_player_with_slot(0xDEAD_3001, 2);
        let player_id = actor_idx(ptr);

        restamp_key(ptr, 0, 0);
        let enemy_id = actor_idx(ptr);

        assert_ne!(player_id, enemy_id);
        assert!(enemy_id < PLAYER_ID_BASE);
        assert_eq!(enemy_id, actor_idx(ptr));
    }

    #[test]
    fn recycled_keyless_pointer_with_a_different_vtable_gets_a_fresh_id() {
        let ptr = fake_instance(None);
        restamp_vtable(ptr, 0x1111_1111);
        let firewyrm_id = actor_idx(ptr);
        assert_eq!(firewyrm_id, actor_idx(ptr));

        restamp_vtable(ptr, 0x2222_2222);
        let icewyrm_id = actor_idx(ptr);

        assert_ne!(firewyrm_id, icewyrm_id);
        assert_eq!(icewyrm_id, actor_idx(ptr));
    }

    #[test]
    fn recycled_keyless_pointer_with_the_same_vtable_keeps_its_id() {
        let ptr = fake_instance(None);
        restamp_vtable(ptr, 0x3333_3333);
        let first_id = actor_idx(ptr);

        restamp_vtable(ptr, 0x3333_3333);
        let second_id = actor_idx(ptr);

        assert_eq!(first_id, second_id);
    }

    #[test]
    fn source_owner_is_local_fails_closed_for_a_non_player_source() {
        let pet = fake_instance(None);
        assert_eq!(source_owner_is_local(pet), None);
    }

    #[test]
    fn source_owner_is_local_reads_the_record_for_a_real_player() {
        let key = 0xDEAD_4000;
        let ptr = fake_player_with_slot(key, 1);
        let src_type_addr =
            (ptr as usize + INSTANCE_RECORD_OFFSET + player::RECORD_SOURCE_TYPE_OFFSET) as *mut u32;

        unsafe { src_type_addr.write_unaligned(1) };
        assert_eq!(source_owner_is_local(ptr), Some(true));

        unsafe { src_type_addr.write_unaligned(4) };
        assert_eq!(source_owner_is_local(ptr), Some(false));
    }

    const PL2000_DRAGON_TYPE_ID: u32 = 0xF5755C0E;
    const PL2000_PARENT_OFFSET: usize = 0x1CA98;

    fn fake_dragonform_child(parent_src_type: u32) -> *const usize {
        let human_size = INSTANCE_RECORD_OFFSET + player::RECORD_SOURCE_TYPE_OFFSET + 4;
        let human: &'static mut [u8] = Box::leak(vec![0u8; human_size].into_boxed_slice());
        let human_addr = human.as_ptr() as usize;
        install_type_id_vfunc(human_addr as *const usize, reports_pl1900);

        human[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4]
            .copy_from_slice(&0xBEEF_0001u32.to_le_bytes());
        human[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
            .copy_from_slice(&ACTOR_PLAYER_KEY_SENTINEL.to_le_bytes());

        // The record is zeroed there, so `network_locality` reports NoLoadoutSub.
        let src = INSTANCE_RECORD_OFFSET + player::RECORD_SOURCE_TYPE_OFFSET;
        human[src..src + 4].copy_from_slice(&parent_src_type.to_le_bytes());

        let info: &'static mut [u8] =
            Box::leak(vec![0u8; ENTITY_SPECIFIED_INSTANCE_OFFSET + 8].into_boxed_slice());
        info[ENTITY_SPECIFIED_INSTANCE_OFFSET..ENTITY_SPECIFIED_INSTANCE_OFFSET + 8]
            .copy_from_slice(&(human_addr as u64).to_le_bytes());

        let child_size = PL2000_PARENT_OFFSET + 8;
        let child: &'static mut [u8] = Box::leak(vec![0u8; child_size].into_boxed_slice());
        child[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4]
            .copy_from_slice(&0xBEEF_9000u32.to_le_bytes());
        child[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
            .copy_from_slice(&ACTOR_PLAYER_KEY_SENTINEL.to_le_bytes());
        child[PL2000_PARENT_OFFSET..PL2000_PARENT_OFFSET + 8]
            .copy_from_slice(&(info.as_ptr() as u64).to_le_bytes());

        child.as_ptr() as *const usize
    }

    #[test]
    fn dragonform_child_resolves_via_parent_owner_not_its_borrowed_key() {
        let remote_child = fake_dragonform_child(4);

        assert_eq!(source_owner_is_local(remote_child), Some(true));
        assert_eq!(
            source_is_local_for_dedup(PL2000_DRAGON_TYPE_ID, remote_child),
            Some(false)
        );
    }

    #[test]
    fn local_dragonform_child_still_resolves_local_via_owner() {
        let local_child = fake_dragonform_child(1);
        assert_eq!(
            source_is_local_for_dedup(PL2000_DRAGON_TYPE_ID, local_child),
            Some(true)
        );
    }

    #[test]
    fn parent_actor_idx_fails_closed_when_the_type_slot_is_unreadable() {
        let unmapped = 0x1000 as *const usize;
        assert_eq!(parent_actor_idx(unmapped), actor_idx(unmapped));

        let vtable_less_player = fake_player_with_slot(0xFEED_0001, 2);
        assert_eq!(parent_actor_idx(vtable_less_player), PLAYER_ID_BASE | 2);
    }

    const PL1900_HUMAN_TYPE_ID: u32 = 0x8056_ABCD;
    const SUMMON_TYPE_ID: u32 = 0x5395_CE93;
    const SUMMON_HANDLE_IDX_OFFSET: usize = 0xFE0;
    const SUMMON_HANDLE_PTR_OFFSET: usize = 0xFE8;

    unsafe extern "system" fn reports_pl2000(_: *const usize, out: *const u32) -> *const usize {
        (out as *mut u32).write_unaligned(PL2000_DRAGON_TYPE_ID);
        std::ptr::null()
    }

    unsafe extern "system" fn reports_pl1900(_: *const usize, out: *const u32) -> *const usize {
        (out as *mut u32).write_unaligned(PL1900_HUMAN_TYPE_ID);
        std::ptr::null()
    }

    fn install_type_id_vfunc(instance: *const usize, type_fn: GetEntityHashID0x58) {
        let vtable: &'static mut [u8] =
            Box::leak(vec![0u8; TYPE_ID_VFUNC_SLOT + 8].into_boxed_slice());
        vtable[TYPE_ID_VFUNC_SLOT..TYPE_ID_VFUNC_SLOT + 8]
            .copy_from_slice(&(type_fn as usize as u64).to_le_bytes());
        unsafe { (instance as *mut u64).write_unaligned(vtable.as_ptr() as u64) };
    }

    fn fake_entity_info(instance: *const usize) -> *const usize {
        let info: &'static mut [u8] =
            Box::leak(vec![0u8; ENTITY_SPECIFIED_INSTANCE_OFFSET + 8].into_boxed_slice());
        info[ENTITY_SPECIFIED_INSTANCE_OFFSET..ENTITY_SPECIFIED_INSTANCE_OFFSET + 8]
            .copy_from_slice(&(instance as u64).to_le_bytes());
        info.as_ptr() as *const usize
    }

    fn fake_summon_owned_by_dragonform(slot: u8) -> (*const usize, *const usize) {
        let human = fake_player_with_slot(0xBEEF_1900, slot);
        install_type_id_vfunc(human, reports_pl1900);

        let dragon: &'static mut [u8] =
            Box::leak(vec![0u8; PL2000_PARENT_OFFSET + 8].into_boxed_slice());
        dragon[ACTOR_PLAYER_KEY_OFFSET..ACTOR_PLAYER_KEY_OFFSET + 4]
            .copy_from_slice(&0xBEEF_2000u32.to_le_bytes());
        dragon[ACTOR_PLAYER_KEY_OFFSET + 4..ACTOR_PLAYER_KEY_OFFSET + 8]
            .copy_from_slice(&ACTOR_PLAYER_KEY_SENTINEL.to_le_bytes());
        dragon[PL2000_PARENT_OFFSET..PL2000_PARENT_OFFSET + 8]
            .copy_from_slice(&(fake_entity_info(human) as u64).to_le_bytes());
        let dragon = dragon.as_ptr() as *const usize;
        install_type_id_vfunc(dragon, reports_pl2000);

        let summon: &'static mut [u8] =
            Box::leak(vec![0u8; SUMMON_HANDLE_PTR_OFFSET + 8].into_boxed_slice());
        summon[SUMMON_HANDLE_IDX_OFFSET..SUMMON_HANDLE_IDX_OFFSET + 4]
            .copy_from_slice(&1u32.to_le_bytes());
        summon[SUMMON_HANDLE_PTR_OFFSET..SUMMON_HANDLE_PTR_OFFSET + 8]
            .copy_from_slice(&(fake_entity_info(dragon) as u64).to_le_bytes());

        (summon.as_ptr() as *const usize, dragon)
    }

    #[test]
    fn summon_called_in_dragonform_files_under_the_human_player() {
        let (summon, dragon) = fake_summon_owned_by_dragonform(2);

        let one_hop = resolve_source_parent_hop(SUMMON_TYPE_ID, summon).unwrap();
        assert!(std::ptr::eq(one_hop, dragon));
        assert!(actor_idx(dragon) < PLAYER_ID_BASE);

        assert_eq!(
            get_source_parent(SUMMON_TYPE_ID, summon),
            Some((PL1900_HUMAN_TYPE_ID, PLAYER_ID_BASE | 2))
        );
    }

    #[test]
    fn a_source_owned_directly_by_a_player_still_stops_at_that_player() {
        let (_, dragon) = fake_summon_owned_by_dragonform(3);

        assert_eq!(
            get_source_parent(PL2000_DRAGON_TYPE_ID, dragon),
            Some((PL1900_HUMAN_TYPE_ID, PLAYER_ID_BASE | 3))
        );
    }

    #[test]
    fn a_source_with_no_arm_resolves_no_parent() {
        let player = fake_player_with_slot(0xFEED_2002, 1);
        install_type_id_vfunc(player, reports_pl1900);

        assert_eq!(get_source_parent(PL1900_HUMAN_TYPE_ID, player), None);
    }

    #[test]
    fn a_self_owning_chain_terminates() {
        let dragon: &'static mut [u8] =
            Box::leak(vec![0u8; PL2000_PARENT_OFFSET + 8].into_boxed_slice());
        let dragon = dragon.as_ptr() as *const usize;
        install_type_id_vfunc(dragon, reports_pl2000);
        let info = fake_entity_info(dragon);
        unsafe {
            ((dragon as usize + PL2000_PARENT_OFFSET) as *mut u64).write_unaligned(info as u64)
        };

        assert_eq!(
            get_source_parent(PL2000_DRAGON_TYPE_ID, dragon),
            Some((PL2000_DRAGON_TYPE_ID, actor_idx(dragon)))
        );
    }
}
