//! Resolves class vtables by RTTI name instead of by hardcoded RVA. A class
//! that does not resolve yields nothing, so the feature keyed on it stays off.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use pelite::pe64::{Pe, PeView};

use crate::process::Process;

static VTABLES: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();

const EXPECTED_VTABLE_CLASSES: usize = 22;

const _: () = assert!(
    super::actor::SUMMON_VTABLE_CLASSES.len() + super::quest::FLOW_VTABLE_CLASSES.len() + 1
        == EXPECTED_VTABLE_CLASSES,
    "the RTTI class tables no longer add up to EXPECTED_VTABLE_CLASSES"
);

pub(crate) fn setup(process: &Process) {
    let mut wanted: Vec<&'static str> = Vec::new();
    wanted.extend(super::actor::SUMMON_VTABLE_CLASSES.iter().copied());
    wanted.extend(super::quest::FLOW_VTABLE_CLASSES.iter().copied());
    wanted.push(super::conflux::CLEAR_ENDLESS_QUEST_VTABLE_NAME);

    let resolved = resolve_vtables(process, &wanted);
    let found = resolved.len();
    // Publish before the report below, which reads the map back.
    let _ = VTABLES.set(resolved);

    if found == EXPECTED_VTABLE_CLASSES {
        log::info!("[rtti] resolved {found}/{EXPECTED_VTABLE_CLASSES} vtables by class name");
    } else {
        log::warn!(
            "[rtti] resolved {found}/{EXPECTED_VTABLE_CLASSES} vtables by class name — \
             MISSING: {}. Nothing falls back to a baked address any more, so every feature \
             keyed on a missing class is OFF for this session. See docs/HOOK-REDISCOVERY.md.",
            unresolved(&wanted).join(", ")
        );
    }
}

pub(crate) fn resolved_vtable(name: &'static str) -> Option<usize> {
    VTABLES.get()?.get(name).copied()
}

pub(crate) fn unresolved(names: &[&'static str]) -> Vec<&'static str> {
    names
        .iter()
        .copied()
        .filter(|name| resolved_vtable(name).is_none())
        .collect()
}

/// `RTTICompleteObjectLocator` offsets, x64. Every field here is an RVA.
const COL_SIZE: usize = 0x18;
const COL_SIGNATURE: usize = 0x00;
const COL_OFFSET: usize = 0x04;
const COL_TYPE_DESCRIPTOR: usize = 0x0C;
const COL_SELF: usize = 0x14;

const TYPE_DESCRIPTOR_NAME: usize = 0x10;

const MAX_NAME_LEN: usize = 512;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

/// `wanted` holds MANGLED names, matched with full string equality. Never relax
/// this to a prefix test. `.?AVSo0000@@` is a strict prefix of
/// `.?AVSo0000EyeLaserAction@BT@@`, so a prefix match binds the wrong class.
pub(crate) fn resolve_vtables(
    process: &Process,
    wanted: &[&'static str],
) -> HashMap<&'static str, usize> {
    let mut resolved = HashMap::new();

    let view = unsafe { PeView::module(process.module_handle.0 as *const u8) };

    let Some(rdata) = view.section_headers().by_name(".rdata") else {
        return resolved;
    };
    let section_rva = rdata.VirtualAddress as usize;
    let section_len = rdata.VirtualSize as usize;

    // SAFETY: the module stays mapped and the bounds come from its own section header.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            (process.base_address + section_rva) as *const u8,
            section_len,
        )
    };

    let opt = view.optional_header();
    let code = (opt.BaseOfCode as usize)..(opt.BaseOfCode as usize + opt.SizeOfCode as usize);

    let mut col_to_name: HashMap<u64, &'static str> = HashMap::new();
    let mut at = 0usize;
    while at + COL_SIZE <= bytes.len() {
        let col_rva = (section_rva + at) as u32;
        if u32_at(bytes, at + COL_SIGNATURE) == Some(1)
            && u32_at(bytes, at + COL_SELF) == Some(col_rva)
        {
            if let Some(name) = wanted_name(&view, bytes, at, wanted) {
                if u32_at(bytes, at + COL_OFFSET) == Some(0) {
                    col_to_name.insert(process.base_address as u64 + col_rva as u64, name);
                }
            }
        }
        at += 4;
    }

    // MSVC stores a class's COL address in the qword 8 bytes before its vtable.
    let mut ambiguous: HashSet<&'static str> = HashSet::new();
    let mut at = 0usize;
    while at + 16 <= bytes.len() {
        if let Some(name) = u64_at(bytes, at).and_then(|v| col_to_name.get(&v)) {
            let slot0 = u64_at(bytes, at + 8).unwrap_or(0) as usize;
            let slot0_rva = slot0.wrapping_sub(process.base_address);
            if code.contains(&slot0_rva) {
                let vtable_rva = section_rva + at + 8;
                if resolved.insert(*name, vtable_rva).is_some() {
                    ambiguous.insert(*name);
                }
            }
        }
        at += 8;
    }

    for name in ambiguous {
        log::warn!("[rtti] {name} matched more than one vtable — ignoring");
        resolved.remove(name);
    }

    resolved
}

fn wanted_name(
    view: &PeView,
    bytes: &[u8],
    at: usize,
    wanted: &[&'static str],
) -> Option<&'static str> {
    let td_rva = u32_at(bytes, at + COL_TYPE_DESCRIPTOR)?;
    let name_rva = td_rva.checked_add(TYPE_DESCRIPTOR_NAME as u32)?;
    let name: &[u8] = view.derva_c_str(name_rva).ok()?.as_ref();
    if name.len() > MAX_NAME_LEN {
        return None;
    }
    wanted.iter().copied().find(|w| w.as_bytes() == name)
}
