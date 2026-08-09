//! Quest-flavor ("sub-name") target naming (e.g. "Furycane Alterphase")
//! `enemy.tbl` gives each class a 16-slot array of name-key hashes,
//! and the quest's content-registry entry picks the slot.

use std::collections::HashMap;
use std::mem::offset_of;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;

use crate::process::Process;

use super::ffi::QuestState;
use super::gbfr_hash::gbfr_hash;
use super::globals::QUEST_STATE_PTR;
use super::safe_read;

const ENEMY_SUBNAMES_JSON: &str = include_str!("../../../src-tauri/assets/enemy-subnames.json");

const QM_GLOBAL_SIG: &str = "48 8b 0d $ { ' } 89 d3 c1 ea 10 89 d8 c1 e8 18 44 0f b6 c3";

static QM_GLOBAL_SLOT: AtomicUsize = AtomicUsize::new(0);

// MSVC unordered_map header on the QM singleton, then node/entry field offsets.
const QM_MAP_SENTINEL: usize = 0xA0;
const QM_MAP_BUCKETS: usize = 0xB0;
const QM_MAP_MASK: usize = 0xC8;
const NODE_NEXT: usize = 0x8;
const NODE_KEY: usize = 0x10;
const NODE_VALUE: usize = 0x18;

const SLOT_STRIDE: usize = 0x10;
const SLOT_COUNT: usize = 9;
const SLOT_INDEX_OFFSET: usize = 0xC;
const SLOT_UID_EMPTY: u32 = 0xFFFF_FFFF;

const MAX_BUCKET_WALK: usize = 64;
const MAX_SANE_MASK: u64 = 0x0010_0000;

static SUBNAME_HEADER_WARNED: AtomicBool = AtomicBool::new(false);

pub fn setup_subname_registry(process: &Process) -> Result<()> {
    let addr = process.search_address(QM_GLOBAL_SIG)?;
    QM_GLOBAL_SLOT.store(addr, Ordering::Relaxed);

    #[cfg(feature = "console")]
    println!("Found quest content-registry global (subnames): {:#x}", addr);

    Ok(())
}

fn category_code(prefix: &str) -> Option<u32> {
    match prefix {
        "Em" => Some(0x0002),
        "We" => Some(0x0103),
        "Ba" => Some(0x000F),
        "Bh" => Some(0x000E),
        "Np" => Some(0x010A),
        "So" => Some(0x010D),
        _ => None,
    }
}

fn class_token_to_uid(token: &str) -> Option<u32> {
    if token.len() != 6 {
        return None;
    }
    let cat = category_code(&token[..2])?;
    let num = u32::from_str_radix(&token[2..], 16).ok()?;
    Some((cat << 16) | num)
}

// Variant numbering bumps the tens hex digit and keeps the units digit,
// so Em7220 folds to Em7200 but Em7221 to Em7201.
fn variant_base_uid(uid: u32) -> Option<u32> {
    let base = uid & 0xFFFF_FF0F;
    (base != uid).then_some(base)
}

// Story-only slots are baked as bare 8-digit lowercase hex.
fn slot_value_to_hash(value: &str) -> u32 {
    if value.len() == 8 && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
        u32::from_str_radix(value, 16).unwrap_or(0)
    } else {
        gbfr_hash(value)
    }
}

fn subname_table() -> &'static HashMap<u32, [u32; 16]> {
    static TABLE: OnceLock<HashMap<u32, [u32; 16]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let parsed: HashMap<String, Vec<Option<String>>> =
            serde_json::from_str(ENEMY_SUBNAMES_JSON).unwrap_or_default();

        let mut table = HashMap::with_capacity(parsed.len());
        for (class, slots) in parsed {
            let Some(uid) = class_token_to_uid(&class) else {
                continue;
            };
            let mut hashes = [0u32; 16];
            for (i, slot) in slots.iter().take(16).enumerate() {
                if let Some(value) = slot {
                    hashes[i] = slot_value_to_hash(value);
                }
            }
            table.insert(uid, hashes);
        }
        table
    })
}

// FNV-1a-64 over the content id's 4 little-endian bytes, the game's own key mix.
fn fnv1a64_u32(id: u32) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in id.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

fn registry_flavor_index(content_id: u32, uid: u32) -> Option<u32> {
    // Slot-array offset by content-id family; endless-mode entries have a
    // shorter header.
    let slot_base: usize = match content_id & 0xF0_0000 {
        0x40_0000 => 0x40,
        0x80_0000 => 0x24,
        _ => return None,
    };

    let slot_va = QM_GLOBAL_SLOT.load(Ordering::Relaxed);
    if slot_va == 0 {
        return None;
    }
    let qm = safe_read(slot_va as *const usize).filter(|&p| p != 0)? as *const u8;

    let sentinel = safe_read(qm.wrapping_add(QM_MAP_SENTINEL) as *const usize)?;
    let buckets = safe_read(qm.wrapping_add(QM_MAP_BUCKETS) as *const usize)?;
    let mask = safe_read(qm.wrapping_add(QM_MAP_MASK) as *const u64)?;
    if sentinel == 0 || buckets == 0 || mask == 0 || mask > MAX_SANE_MASK {
        // An insane mask means the QM_MAP_* offsets went stale.
        if mask > MAX_SANE_MASK {
            super::warn_stale_rva(&SUBNAME_HEADER_WARNED, "subname map-header offsets (subname.rs)");
        }
        return None;
    }

    let bucket = buckets + ((fnv1a64_u32(content_id) & mask) as usize) * 0x10;
    let last = safe_read(bucket as *const usize)?;
    let mut node = safe_read((bucket + 8) as *const usize)?;
    if node == sentinel {
        return None;
    }

    let mut entry: usize = 0;
    for _ in 0..MAX_BUCKET_WALK {
        if safe_read((node + NODE_KEY) as *const u32)? == content_id {
            entry = safe_read((node + NODE_VALUE) as *const usize)?;
            break;
        }
        if node == last {
            break;
        }
        node = safe_read((node + NODE_NEXT) as *const usize)?;
        if node == 0 || node == sentinel {
            break;
        }
    }
    if entry == 0 {
        return None;
    }

    let mut found: Option<u32> = None;
    for i in 0..SLOT_COUNT {
        let slot = entry + slot_base + i * SLOT_STRIDE;
        let Some(slot_uid) = safe_read(slot as *const u32) else {
            continue;
        };
        if slot_uid != uid || slot_uid == SLOT_UID_EMPTY {
            continue;
        }
        let Some(index) = safe_read((slot + SLOT_INDEX_OFFSET) as *const u32) else {
            continue;
        };
        if index > 15 {
            continue;
        }
        match found {
            None => found = Some(index),
            Some(previous) if previous != index => return None,
            Some(_) => {}
        }
    }
    found
}

static FLAVOR_CACHE: OnceLock<Mutex<HashMap<(u32, u32), u32>>> = OnceLock::new();

pub fn resolve_flavor_type_id(uid: u32) -> Option<u32> {
    // Instance variants have no array of their own; quests author the flavor
    // on the base class.
    let (lookup_uid, slots) = match subname_table().get(&uid) {
        Some(slots) => (uid, slots),
        None => {
            let base = variant_base_uid(uid)?;
            (base, subname_table().get(&base)?)
        }
    };

    let quest_state = QUEST_STATE_PTR.load(Ordering::Relaxed);
    if quest_state.is_null() {
        return None;
    }
    let quest_id = safe_read(
        (quest_state as *const u8).wrapping_add(offset_of!(QuestState, quest_id)) as *const u32,
    )?;
    if quest_id == 0 {
        return None;
    }

    let cache = FLAVOR_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(&hash) = cache.get(&(quest_id, uid)) {
        return (hash != 0).then_some(hash);
    }

    let hash = match registry_flavor_index(quest_id, lookup_uid) {
        // Index 0 is the class's own base name, and a row may repeat it.
        Some(index) if index > 0 && slots[index as usize] != 0 && slots[index as usize] != slots[0] => {
            let resolved = slots[index as usize];
            #[cfg(feature = "console")]
            println!(
                "target flavor: quest={:#x} uid={:#010x} base={:#010x} index={} -> {:#010x}",
                quest_id, uid, lookup_uid, index, resolved
            );
            resolved
        }
        _ => 0,
    };

    cache.insert((quest_id, uid), hash);
    (hash != 0).then_some(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_matches_reference() {
        assert_eq!(fnv1a64_u32(0), {
            let mut h: u64 = 0xCBF2_9CE4_8422_2325;
            for _ in 0..4 {
                h = h.wrapping_mul(0x0000_0100_0000_01B3);
            }
            h
        });
    }

    #[test]
    fn class_token_uid_encoding() {
        assert_eq!(class_token_to_uid("Em8200"), Some(0x0002_8200));
        assert_eq!(class_token_to_uid("Em0000"), Some(0x0002_0000));
        assert_eq!(class_token_to_uid("We7700"), Some(0x0103_7700));
        assert_eq!(class_token_to_uid("Em76a0"), Some(0x0002_76A0));
        assert_eq!(class_token_to_uid("Pl0700Ghost"), None);
    }

    #[test]
    fn baked_table_resolves_beelzebub() {
        let table = subname_table();
        let slots = table.get(&0x0002_8200).expect("Em8200 row missing");
        assert_eq!(slots[0], gbfr_hash("Em8200"));
        assert_eq!(slots[12], gbfr_hash("Em8200_01"));
        assert_eq!(slots[12], 0xBE14_BCD5);
    }

    #[test]
    fn baked_table_keeps_cross_class_slots() {
        let table = subname_table();
        let slots = table.get(&0x0002_0000).expect("Em0000 row missing");
        assert_eq!(slots[7], gbfr_hash("Em0201_11"));
        assert_ne!(slots[7], gbfr_hash("Em0000_07"));
    }

    #[test]
    fn variant_uid_folds_to_its_base_class() {
        assert_eq!(variant_base_uid(0x0002_7220), Some(0x0002_7200));
        assert_eq!(variant_base_uid(0x0002_7221), Some(0x0002_7201));
        assert_eq!(variant_base_uid(0x0002_7210), Some(0x0002_7200));
        assert_eq!(variant_base_uid(0x0002_7211), Some(0x0002_7201));
        assert_eq!(variant_base_uid(0x0002_7200), None);
        assert_eq!(variant_base_uid(0x0002_7201), None);
        assert_eq!(variant_base_uid(0x0002_8200), None);
        assert_eq!(variant_base_uid(0x0103_7710).unwrap() >> 16, 0x0103);
        assert_eq!(variant_base_uid(0x0002_7220).unwrap() & 0xFF00, 0x7200);
    }

    #[test]
    fn variant_fold_reaches_furycane_alterphase() {
        let table = subname_table();
        assert!(table.get(&0x0002_7220).is_none());
        assert!(table.get(&0x0002_7221).is_none());

        let furycane = gbfr_hash("Em7200_02");
        for (variant, base) in [(0x0002_7220u32, 0x0002_7200u32), (0x0002_7221, 0x0002_7201)] {
            let folded = variant_base_uid(variant).expect("variant folds to a base");
            assert_eq!(folded, base);
            let slots = table.get(&folded).expect("base row present");
            assert_eq!(slots[12], furycane, "index 12 is Furycane Alterphase");
        }
    }

    #[test]
    fn raw_hash_slots_parse() {
        assert_eq!(slot_value_to_hash("57946db3"), 0x5794_6DB3);
        assert_eq!(slot_value_to_hash("Em8200_01"), gbfr_hash("Em8200_01"));
    }
}
