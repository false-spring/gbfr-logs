use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicUsize};

use anyhow::{anyhow, Result};
use protocol::{Message, PartyRosterEvent, PlayerIdentityEvent, RosterMember};
use retour::static_detour;

use crate::{
    event,
    hooks::{
        ffi::{MasteryNodeRow, OverMasterySlot, SigilEntry, SummonSlot, TraitPair},
        player_directory::{
            announced_identities, character_type_for_key, identity_store, last_roster_sig,
            local_network_user, network_user_for_enw, reset_announced,
        },
        safe_read, warn_stale_rva, AnchorReport, ENTITY_SPECIFIED_INSTANCE_OFFSET,
        INSTANCE_RECORD_OFFSET, RECORD_KEY_MIRROR_OFFSET, RECORD_SNAPSHOT_PTR_OFFSET,
        SNAPSHOT_PARTY_INDEX_OFFSET,
    },
    process::Process,
};

// A source instance pointer no longer says which player it is. The bridge is
// the identity record's key at +0x5EA8, which equals the actor's +0x1AB40.

/// Record offset (u32): source type. `< 4` is read below as "is_online" and is
/// confirmed to include you. Do not read 4 or 5 as "local" anywhere else.
pub(crate) const RECORD_SOURCE_TYPE_OFFSET: usize = 0x5EAC;
/// Record offset: pointer to the replicated-loadout sub-object. NULL for you.
const RECORD_LOADOUT_SUB_OFFSET: usize = 0x5DC8;
/// Loadout-sub offset (u32): the owner's eNwPlayerId, same domain as
/// NetworkUserPlayFab+0x68. eNw permutes the party order. Never join by index.
const SUB_MEMBER_ENW_OFFSET: usize = 0x3514;
/// Loadout-sub offset (u8): set when a local CPU owns this loadout. Its id is
/// meaningless.
const SUB_CPU_FLAG_OFFSET: usize = 0x356B;
const SNAPSHOT_CHARACTER_NAME_OFFSET: usize = 0x1E8;
const SNAPSHOT_DISPLAY_NAME_OFFSET: usize = 0x208;

/// Snapshot offset: sigil array, 13 slots of 0x24 bytes, pre-2.0 layout.
const SNAPSHOT_SIGIL_ARRAY_OFFSET: usize = 0x0;
const SNAPSHOT_SIGIL_SLOTS: usize = 13;
/// Record offset (u32): level. rec+0x00..0x18 is the pre-2.0 `PlayerStats`.
const RECORD_LEVEL_OFFSET: usize = 0x00;
const RECORD_LEVEL_WIRE_OFFSET: usize = 0x5B44;
const RECORD_TOTAL_HP_OFFSET: usize = 0x04;
const RECORD_TOTAL_ATTACK_OFFSET: usize = 0x08;
const RECORD_STUN_POWER_OFFSET: usize = 0x10;
const RECORD_CRITICAL_RATE_OFFSET: usize = 0x14;
const RECORD_TOTAL_POWER_OFFSET: usize = 0x18;
const RECORD_SUMMON_SLOT_OFFSETS: [usize; 4] = [0x5DD8, 0x5DF4, 0x5E10, 0x5E2C];
/// Record offset of the 400-slot x 0x38 mastery/limit_bonus array.
const RECORD_MASTERY_ARRAY_OFFSET: usize = 0x138;
const MASTERY_ARRAY_SLOTS: usize = 400;
const RECORD_OVER_MASTERY_BLOCK_OFFSET: usize = 0x58B8;
const OVER_MASTERY_SLOTS: usize = 4;
/// Record offset (u32): master level 0..=55. The game splits this one number
/// into Master Lvl and Mastery Break.
const RECORD_MASTER_LEVEL_OFFSET: usize = 0x5B60;
const RECORD_WEAPON_ID_OFFSET: usize = 0x54;
/// Record offset (u32): the CURRENT weapon level, not a cap. rec+0x5C is the
/// internal ForgedWeaponLevel.
const RECORD_WEAPON_LEVEL_OFFSET: usize = 0xA8;
const RECORD_WEAPON_PLUS_MARKS_OFFSET: usize = 0x68;
const RECORD_WEAPON_AWAKENING_OFFSET: usize = 0x6C;
const RECORD_WEAPON_TRANSCENDENCE_OFFSET: usize = 0x90;
const RECORD_WEAPON_TRAIT_PAIRS_OFFSET: usize = 0xF4;
const RECORD_WRIGHTSTONE_TRAIT_PAIRS_OFFSET: usize = 0x70;
/// Record offsets (f32): three EXPERIMENTAL DMG-cap channels, meaning unproven.
const RECORD_DMG_CAP_CHANNEL_OFFSETS: [usize; 3] = [0x28, 0x30, 0x34];
const SNAPSHOT_SKILL_LOADOUT_OFFSET: usize = 0x1D4;
const SKILL_LOADOUT_SLOTS: usize = 4;
const RECORD_EFFECTIVE_TRAIT_MAP_OFFSET: usize = 0x5D78;
const EFFECTIVE_MAP_SENTINEL_PTR_OFFSET: usize = 0x08;
const EFFECTIVE_NODE_NEXT_OFFSET: usize = 0x00;
const EFFECTIVE_NODE_HASH_OFFSET: usize = 0x10;
const EFFECTIVE_NODE_LEVEL_OFFSET: usize = 0x18;
/// A corrupt or concurrently mutated list must not spin. Real lists are ~22.
const EFFECTIVE_TRAIT_MAX_ITERS: usize = 256;
/// The game's empty-hash sentinel. Id fields read this when a slot is empty.
const EMPTY_HASH_SENTINEL: u32 = 0x887AE0B0;

const ON_LOAD_PLAYER_IDENTITY_SIG: &str = "55 41 57 41 56 41 54 56 57 53 48 83 ec 70 48 8d 6c 24 70 48 c7 45 f8 fe ff ff ff 80 b9 bc 5e 00 00 00";

#[derive(Clone, Default)]
pub(crate) struct PlayerIdentity {
    party_index: u8,
    character_name: CString,
    display_name: CString,
    character_type: u32,
    is_online: bool,
    enw: Option<u8>,
    sigils: Vec<protocol::Sigil>,
    weapon_info: Option<protocol::WeaponInfo>,
    summon_info: Option<protocol::SummonInfo>,
    skill_loadout: Vec<u32>,
    over_mastery: Vec<protocol::OverMasteryLine>,
    master_trait_flags: Vec<protocol::MasterTraitFlag>,
    effective_traits: Vec<protocol::EffectiveTrait>,
    player_stats: Option<protocol::PlayerStats>,
    master_level: Option<u32>,
}

// IDENTITY_STORE lives in `player_directory`, keyed by (party slot, record
// pointer). The slot is unique per lobby where the player key is not.

type OnLoadPlayerIdentityFunc = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static OnLoadPlayerIdentity: unsafe extern "system" fn(*const usize) -> usize;
}

#[derive(Clone)]
pub struct OnLoadPlayerIdentityHook {
    tx: event::Tx,
}

impl OnLoadPlayerIdentityHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLoadPlayerIdentityHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_load_player_identity) =
            process.search_address_start(ON_LOAD_PLAYER_IDENTITY_SIG)
        {
            #[cfg(feature = "console")]
            println!("Found on load player identity");

            unsafe {
                let func: OnLoadPlayerIdentityFunc =
                    std::mem::transmute(on_load_player_identity);
                OnLoadPlayerIdentity
                    .initialize(func, move |record| cloned_self.run(record))?;
                OnLoadPlayerIdentity.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_load_player_identity"));
        }

        Ok(())
    }

    fn run(&self, record: *const usize) -> usize {
        let ret = unsafe { OnLoadPlayerIdentity.call(record) };

        if let Some((_player_key, identity)) = read_identity_from_record(record) {
            if let Ok(mut store) = identity_store().lock() {
                store.insert((identity.party_index, record as usize), identity);
            }

            // A refresh coincides with a load, which is when the parser clears
            // its party rows. Re-announce, or the names stay blank all session.
            reset_announced();

            self.emit_roster_if_changed();
        }

        ret
    }

    fn emit_roster_if_changed(&self) {
        let members = build_roster();
        if members.is_empty() {
            return;
        }

        // Suppress the title screen roster. Those records exist before a save
        // is loaded, carry no display name, and show a phantom party at launch.
        if !members.iter().any(|m| !m.display_name.as_bytes().is_empty()) {
            return;
        }

        let sig: Vec<(u8, u32, Vec<u8>)> = members
            .iter()
            .map(|m| {
                (
                    m.party_index,
                    m.character_type,
                    m.display_name.as_bytes().to_vec(),
                )
            })
            .collect();

        let changed = match last_roster_sig().lock() {
            Ok(mut last) if *last != sig => {
                *last = sig;
                true
            }
            _ => false,
        };

        if changed {
            let _ = self.tx.send(Message::PartyRoster(PartyRosterEvent { members }));
        }
    }
}

fn build_roster() -> Vec<RosterMember> {
    let Ok(store) = identity_store().lock() else {
        return Vec::new();
    };

    let mut by_slot: HashMap<u8, PlayerIdentity> = HashMap::new();
    for ((slot, _record), identity) in store.iter() {
        match by_slot.get(slot) {
            Some(existing)
                if (existing.is_online && !identity.is_online)
                    || (existing.enw.is_some() && identity.enw.is_none()) => {}
            _ => {
                by_slot.insert(*slot, identity.clone());
            }
        }
    }

    let local_slot = local_party_index();

    let mut members: Vec<RosterMember> = by_slot
        .into_iter()
        .map(|(slot, identity)| {
            // The join only resolves remote members, so your own row falls back.
            let network = identity
                .enw
                .and_then(network_user_for_enw)
                .or_else(|| (Some(slot) == local_slot).then(local_network_user).flatten());
            RosterMember {
                party_index: slot,
                character_type: identity.character_type,
                display_name: identity.display_name,
                network_user_name: network.as_ref().and_then(|n| n.name.clone()),
                network_user_id: network.and_then(|n| n.entity_id),
                is_online: identity.is_online,
            }
        })
        .collect();

    members.sort_by_key(|m| m.party_index);
    members
}

fn read_identity_from_record(record: *const usize) -> Option<(u32, PlayerIdentity)> {
    let player_key = safe_read::<u32>(byte_offset::<u32>(record, RECORD_KEY_MIRROR_OFFSET))?;

    let snapshot =
        safe_read::<usize>(byte_offset::<usize>(record, RECORD_SNAPSHOT_PTR_OFFSET))? as *const u8;
    if snapshot.is_null() {
        return None;
    }

    let party_index_raw =
        safe_read::<u32>(byte_offset::<u32>(snapshot as *const usize, SNAPSHOT_PARTY_INDEX_OFFSET))?;

    // Inactive placeholder records read 0xFFFFFFFF. Skip anything not a slot.
    if party_index_raw > 3 {
        return None;
    }

    let is_online =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_SOURCE_TYPE_OFFSET)).unwrap_or(4) < 4;

    let name_bytes = safe_read::<[u8; 16]>(byte_offset::<[u8; 16]>(
        snapshot as *const usize,
        SNAPSHOT_CHARACTER_NAME_OFFSET,
    ))
    .unwrap_or([0u8; 16]);
    let character_name = CStr::from_bytes_until_nul(&name_bytes)
        .map(|cstr| cstr.to_owned())
        .unwrap_or_else(|_| CString::new("").unwrap());

    let display_name = read_vbuffer_guarded(byte_offset::<usize>(
        snapshot as *const usize,
        SNAPSHOT_DISPLAY_NAME_OFFSET,
    ));

    let effective_traits = read_effective_traits_from_record(record);
    let weapon_info = read_weapon_info_from_record(record)
        .map(|weapon_info| patch_stale_trait5_level(weapon_info, &effective_traits));

    Some((
        player_key,
        PlayerIdentity {
            party_index: party_index_raw as u8,
            character_name,
            display_name,
            enw: read_enw_from_record(record),
            character_type: character_type_for_key(player_key).unwrap_or(0),
            is_online,
            sigils: read_sigils_from_snapshot(snapshot),
            weapon_info,
            summon_info: read_summons_from_record(record),
            skill_loadout: read_skill_loadout_from_snapshot(snapshot),
            over_mastery: read_over_mastery_from_record(record),
            master_trait_flags: read_master_trait_flags_from_record(record),
            effective_traits,
            player_stats: read_player_stats(record),
            master_level: read_master_level_from_record(record),
        },
    ))
}

/// Resolves the global holding the CEntityInfo* of the character you control.
const CONTROLLED_CHARACTER_INFO_SIG: &str =
    "48 8b 05 $ { ' } ff c9 4c 8b 4a 48 4d 8b 0c c9 4d 39 d1 0f 85 ? ? ? ? \
     48 8b 52 20 48 8b 0c ca 48 39 c1";

static CONTROLLED_CHARACTER_INFO_SLOT: AtomicUsize = AtomicUsize::new(0);

static CONTROLLED_CHAR_RVA_WARNED: AtomicBool = AtomicBool::new(false);

fn local_party_index() -> Option<u8> {
    let slot = CONTROLLED_CHARACTER_INFO_SLOT.load(std::sync::atomic::Ordering::Relaxed);
    if slot == 0 {
        return None;
    }

    let cinfo = match safe_read::<usize>(slot as *const usize) {
        Some(cinfo) => cinfo,
        None => {
            warn_stale_rva(&CONTROLLED_CHAR_RVA_WARNED, "controlled-character global (player.rs)");
            return None;
        }
    };
    if cinfo == 0 {
        return None;
    }

    let instance =
        safe_read::<usize>((cinfo + ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const usize)?;
    if instance == 0 {
        return None;
    }

    let record = instance.wrapping_add(INSTANCE_RECORD_OFFSET) as *const u8;
    let snapshot = safe_read::<*const u8>(
        record.wrapping_add(RECORD_SNAPSHOT_PTR_OFFSET) as *const *const u8,
    )?;
    if snapshot.is_null() {
        return None;
    }

    let party =
        safe_read::<u32>(snapshot.wrapping_add(SNAPSHOT_PARTY_INDEX_OFFSET) as *const u32)?;
    (party <= 3).then_some(party as u8)
}

fn read_enw_from_record(record: *const usize) -> Option<u8> {
    let sub =
        safe_read::<usize>(byte_offset::<usize>(record, RECORD_LOADOUT_SUB_OFFSET))? as *const u8;
    if sub.is_null() {
        return None;
    }

    let cpu_flag = safe_read::<u8>(sub.wrapping_add(SUB_CPU_FLAG_OFFSET))?;
    if cpu_flag != 0 {
        return None;
    }

    let enw = safe_read::<u32>(sub.wrapping_add(SUB_MEMBER_ENW_OFFSET) as *const u32)?;
    if enw > 3 {
        return None;
    }

    Some(enw as u8)
}

const LOBBY_PTR_SIG: &str =
    "48 8b 0d $ { ' } c5 fa 10 81 00 cd 06 00 c5 f8 2e 05 ? ? ? ? 75 06";

static LOBBY_PTR_SLOT: AtomicUsize = AtomicUsize::new(0);
/// Lobby offset (u8): your own slot in the table below. Not an eNwPlayerId.
const LOBBY_LOCAL_SLOT_OFFSET: usize = 0x6CCE8;
/// Lobby offset: 4 u32 entries mapping a context slot to its real eNwPlayerId.
const LOBBY_TABLE_OFFSET: usize = 0x6C828;

static LOBBY_RVA_WARNED: AtomicBool = AtomicBool::new(false);

static LOBBY_SLOT_INSANE_WARNED: AtomicBool = AtomicBool::new(false);

/// A miss stores 0, which every reader below treats as unavailable.
pub(crate) fn setup_player_globals(process: &Process) {
    for (sig, what, degraded, slot) in [
        (
            CONTROLLED_CHARACTER_INFO_SIG,
            "controlled-character global (player.rs)",
            "the local player's row carries no PlayFab entity id",
            &CONTROLLED_CHARACTER_INFO_SLOT,
        ),
        (
            LOBBY_PTR_SIG,
            "lobby singleton (player.rs)",
            "local-vs-remote classification abstains, so the damage hook's \
             duplicate-frame filter stays open for player-owned sources",
            &LOBBY_PTR_SLOT,
        ),
    ] {
        let resolved = super::resolve_global_slot(process, sig, what, degraded).unwrap_or(0);
        slot.store(resolved, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(crate) fn validate_anchors(process: &Process) -> AnchorReport {
    let mut report = AnchorReport::new();
    let (img_lo, img_hi) = process.image_range();

    for (slot, name) in [
        (&CONTROLLED_CHARACTER_INFO_SLOT, "controlled-character global"),
        (&LOBBY_PTR_SLOT, "lobby singleton"),
    ] {
        let addr = slot.load(std::sync::atomic::Ordering::Relaxed);
        let ok = addr != 0
            && (img_lo..img_hi).contains(&addr)
            && safe_read::<usize>(addr as *const usize).is_some();
        report.check(ok, || {
            format!("{name} {:#x} (player.rs)", addr.wrapping_sub(process.base_address))
        });
    }

    report
}

pub(crate) enum NetworkLocality {
    Decided(bool),
    NoLoadoutSub,
    /// The test applies but its inputs could not be read. The caller must
    /// abstain. Nothing here licenses a weaker guess.
    Unresolved,
}

/// The game's own local-vs-remote test. Compare the record's network id against
/// the lobby table's entry for your own slot, never against a fixed constant.
/// Offline both sides read 0xFFFFFFFF, so everyone comes out local.
pub(crate) fn network_locality(record: *const usize) -> NetworkLocality {
    let Some(sub) = safe_read::<usize>(byte_offset::<usize>(record, RECORD_LOADOUT_SUB_OFFSET))
    else {
        return NetworkLocality::Unresolved;
    };
    let sub = sub as *const u8;
    if sub.is_null() {
        return NetworkLocality::NoLoadoutSub;
    }
    let Some(source_enw) = safe_read::<u32>(sub.wrapping_add(SUB_MEMBER_ENW_OFFSET) as *const u32)
    else {
        return NetworkLocality::Unresolved;
    };

    let slot = LOBBY_PTR_SLOT.load(std::sync::atomic::Ordering::Relaxed);
    if slot == 0 {
        return NetworkLocality::Unresolved;
    }
    let lobby = match safe_read::<usize>(slot as *const usize) {
        Some(lobby) => lobby,
        None => {
            warn_stale_rva(&LOBBY_RVA_WARNED, "lobby singleton global (player.rs)");
            return NetworkLocality::Unresolved;
        }
    };
    if lobby == 0 {
        return NetworkLocality::Unresolved;
    }
    let Some(local_slot) = safe_read::<u8>((lobby + LOBBY_LOCAL_SLOT_OFFSET) as *const u8) else {
        return NetworkLocality::Unresolved;
    };
    if local_slot > 3 {
        // A slot outside 0..=3 means this reads but is not a lobby.
        warn_stale_rva(
            &LOBBY_SLOT_INSANE_WARNED,
            "lobby local-slot index out of range (player.rs — the lobby pointer is not a lobby)",
        );
        return NetworkLocality::Unresolved;
    }
    let Some(my_enw) =
        safe_read::<u32>((lobby + LOBBY_TABLE_OFFSET + 4 * local_slot as usize) as *const u32)
    else {
        return NetworkLocality::Unresolved;
    };

    NetworkLocality::Decided(source_enw == my_enw)
}

fn read_skill_loadout_from_snapshot(snapshot: *const u8) -> Vec<u32> {
    let Some(ids) = safe_read::<[u32; SKILL_LOADOUT_SLOTS]>(byte_offset::<
        [u32; SKILL_LOADOUT_SLOTS],
    >(
        snapshot as *const usize,
        SNAPSHOT_SKILL_LOADOUT_OFFSET,
    )) else {
        return Vec::new();
    };

    ids.iter()
        .map(|&id| if id == EMPTY_HASH_SENTINEL { 0 } else { id })
        .collect()
}

fn read_over_mastery_from_record(record: *const usize) -> Vec<protocol::OverMasteryLine> {
    let Some(slots) = safe_read::<[OverMasterySlot; OVER_MASTERY_SLOTS]>(byte_offset::<
        [OverMasterySlot; OVER_MASTERY_SLOTS],
    >(
        record,
        RECORD_OVER_MASTERY_BLOCK_OFFSET,
    )) else {
        return Vec::new();
    };

    slots
        .iter()
        .map(|slot| protocol::OverMasteryLine {
            id: slot.param_hash,
            rank: if slot.rank_bit == 0 {
                0
            } else {
                slot.rank_bit.trailing_zeros() + 1
            },
            value: slot.value,
        })
        .collect()
}

fn read_master_level_from_record(record: *const usize) -> Option<u32> {
    let value = safe_read::<u32>(byte_offset::<u32>(record, RECORD_MASTER_LEVEL_OFFSET))?;
    (value <= protocol::MASTER_LEVEL_MAX).then_some(value)
}

/// Flag rows are recognized by SHAPE, never by index, because the limit_bonus
/// prefix length varies per character. A flag row is the empty-hash sentinel at
/// `marker` with 0 or 1 at `flag`. A catalog row has a real hash at `marker`.
fn classify_master_trait_row(row: &MasteryNodeRow) -> Option<protocol::MasterTraitFlag> {
    if row.marker != EMPTY_HASH_SENTINEL {
        return None;
    }
    if row.flag > 1 {
        return None;
    }
    if row.hash == EMPTY_HASH_SENTINEL || row.hash == 0 {
        return None;
    }

    Some(protocol::MasterTraitFlag {
        hash: row.hash,
        on: row.flag == 1,
    })
}

fn read_master_trait_flags_from_record(record: *const usize) -> Vec<protocol::MasterTraitFlag> {
    let Some(rows) = safe_read::<[MasteryNodeRow; MASTERY_ARRAY_SLOTS]>(byte_offset::<
        [MasteryNodeRow; MASTERY_ARRAY_SLOTS],
    >(
        record,
        RECORD_MASTERY_ARRAY_OFFSET,
    )) else {
        return Vec::new();
    };

    rows.iter().filter_map(classify_master_trait_row).collect()
}

fn read_effective_traits_from_record(record: *const usize) -> Vec<protocol::EffectiveTrait> {
    let emap = (record as *const u8).wrapping_add(RECORD_EFFECTIVE_TRAIT_MAP_OFFSET);

    let Some(sentinel) =
        safe_read::<u64>(emap.wrapping_add(EFFECTIVE_MAP_SENTINEL_PTR_OFFSET) as *const u64)
    else {
        return Vec::new();
    };
    if sentinel == 0 {
        return Vec::new();
    }

    let Some(first) =
        safe_read::<u64>((sentinel as usize + EFFECTIVE_NODE_NEXT_OFFSET) as *const u64)
    else {
        return Vec::new();
    };

    let mut traits = Vec::new();
    let mut node = first;
    let mut iters = 0;

    while node != 0 && node != sentinel && iters < EFFECTIVE_TRAIT_MAX_ITERS {
        let Some(hash) = safe_read::<u32>((node as usize + EFFECTIVE_NODE_HASH_OFFSET) as *const u32)
        else {
            break;
        };
        let Some(level) =
            safe_read::<u32>((node as usize + EFFECTIVE_NODE_LEVEL_OFFSET) as *const u32)
        else {
            break;
        };

        if hash != 0 && hash != EMPTY_HASH_SENTINEL {
            traits.push(protocol::EffectiveTrait { hash, level });
        }

        let Some(next) = safe_read::<u64>((node as usize + EFFECTIVE_NODE_NEXT_OFFSET) as *const u64)
        else {
            break;
        };
        node = next;
        iters += 1;
    }

    traits
}

fn read_sigils_from_snapshot(snapshot: *const u8) -> Vec<protocol::Sigil> {
    let Some(entries) = safe_read::<[SigilEntry; SNAPSHOT_SIGIL_SLOTS]>(byte_offset::<
        [SigilEntry; SNAPSHOT_SIGIL_SLOTS],
    >(
        snapshot as *const usize,
        SNAPSHOT_SIGIL_ARRAY_OFFSET,
    )) else {
        return Vec::new();
    };

    entries.iter().map(protocol::Sigil::from).collect()
}

fn read_player_stats(record: *const usize) -> Option<protocol::PlayerStats> {
    let level = safe_read::<u32>(byte_offset::<u32>(record, RECORD_LEVEL_OFFSET))
        .filter(|level| (1..=999).contains(level))
        .or_else(|| safe_read::<u32>(byte_offset::<u32>(record, RECORD_LEVEL_WIRE_OFFSET)))?;

    let total_hp =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_TOTAL_HP_OFFSET)).unwrap_or(0);
    let total_attack =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_TOTAL_ATTACK_OFFSET)).unwrap_or(0);
    let stun_power =
        safe_read::<f32>(byte_offset::<f32>(record, RECORD_STUN_POWER_OFFSET)).unwrap_or(0.0);
    let critical_rate =
        safe_read::<f32>(byte_offset::<f32>(record, RECORD_CRITICAL_RATE_OFFSET)).unwrap_or(0.0);
    let total_power =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_TOTAL_POWER_OFFSET)).unwrap_or(0);

    let mut dmg_cap_channels = [0.0f32; 3];
    for (chan, &offset) in dmg_cap_channels.iter_mut().zip(RECORD_DMG_CAP_CHANNEL_OFFSETS.iter()) {
        *chan = safe_read::<f32>(byte_offset::<f32>(record, offset)).unwrap_or(0.0);
    }

    Some(protocol::PlayerStats {
        level,
        total_hp,
        total_attack,
        stun_power,
        critical_rate,
        total_power,
        dmg_cap_channels,
    })
}

/// The other pre-2.0 numeric fields stay zeroed on purpose. rec+0xAC/+0xB0 are
/// weapon HP and ATK at the L150 cap row, not current-level stats. rec+0xDC and
/// rec+0xE0 are transcension bonus sums, rec+0xCC the awakening ATK sum.
fn read_weapon_info_from_record(record: *const usize) -> Option<protocol::WeaponInfo> {
    let weapon_pairs = safe_read::<[TraitPair; 5]>(byte_offset::<[TraitPair; 5]>(
        record,
        RECORD_WEAPON_TRAIT_PAIRS_OFFSET,
    ));
    let wrightstone_pairs = safe_read::<[TraitPair; 3]>(byte_offset::<[TraitPair; 3]>(
        record,
        RECORD_WRIGHTSTONE_TRAIT_PAIRS_OFFSET,
    ));

    if weapon_pairs.is_none() && wrightstone_pairs.is_none() {
        return None;
    }

    let empty = TraitPair {
        trait_id: EMPTY_HASH_SENTINEL,
        level: 0,
    };
    let weapon = weapon_pairs.unwrap_or([empty; 5]);
    let wrightstone = wrightstone_pairs.unwrap_or([empty; 3]);

    let weapon_id =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_WEAPON_ID_OFFSET)).unwrap_or(0);
    let weapon_level =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_WEAPON_LEVEL_OFFSET)).unwrap_or(0);
    let plus_marks =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_WEAPON_PLUS_MARKS_OFFSET)).unwrap_or(0);
    let transcendence_level =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_WEAPON_TRANSCENDENCE_OFFSET))
            .unwrap_or(0);
    let awakening_level_er =
        safe_read::<u32>(byte_offset::<u32>(record, RECORD_WEAPON_AWAKENING_OFFSET)).unwrap_or(0);

    Some(protocol::WeaponInfo {
        weapon_id,
        star_level: 0,
        plus_marks,
        awakening_level: 0,
        trait_1_id: weapon[0].trait_id,
        trait_1_level: weapon[0].level,
        trait_2_id: weapon[1].trait_id,
        trait_2_level: weapon[1].level,
        trait_3_id: weapon[2].trait_id,
        trait_3_level: weapon[2].level,
        trait_4_id: weapon[3].trait_id,
        trait_4_level: weapon[3].level,
        trait_5_id: weapon[4].trait_id,
        trait_5_level: weapon[4].level,
        wrightstone_id: 0,
        weapon_level,
        transcendence_level,
        awakening_level_er,
        weapon_hp: 0,
        weapon_attack: 0,
        wrightstone_trait_1_id: wrightstone[0].trait_id,
        wrightstone_trait_1_level: wrightstone[0].level,
        wrightstone_trait_2_id: wrightstone[1].trait_id,
        wrightstone_trait_2_level: wrightstone[1].level,
        wrightstone_trait_3_id: wrightstone[2].trait_id,
        wrightstone_trait_3_level: wrightstone[2].level,
    })
}

/// A backup or offline member's weapon block keeps a real trait5 id but never
/// refreshes its level, so it stays 0 while the effective-trait map on the same
/// record has the right one. Only trait5 is gated this way.
fn patch_stale_trait5_level(
    mut weapon_info: protocol::WeaponInfo,
    effective_traits: &[protocol::EffectiveTrait],
) -> protocol::WeaponInfo {
    if weapon_info.trait_5_id != EMPTY_HASH_SENTINEL && weapon_info.trait_5_level == 0 {
        if let Some(effective) = effective_traits
            .iter()
            .find(|t| t.hash == weapon_info.trait_5_id)
        {
            weapon_info.trait_5_level = effective.level;
        }
    }
    weapon_info
}

fn read_summons_from_record(record: *const usize) -> Option<protocol::SummonInfo> {
    let summons: Vec<protocol::SummonSlot> = RECORD_SUMMON_SLOT_OFFSETS
        .iter()
        .filter_map(|&offset| {
            let slot = safe_read::<SummonSlot>(byte_offset::<SummonSlot>(record, offset))?;
            Some(protocol::SummonSlot {
                id: slot.id,
                trait_id: slot.trait_id,
                trait_level: slot.trait_level,
                equip_bonus_id: slot.equip_bonus_id,
                equip_bonus_level: slot.equip_bonus_level,
            })
        })
        .collect();

    if summons.is_empty() {
        return None;
    }

    Some(protocol::SummonInfo { summons })
}

const MAX_DISPLAY_NAME_LEN: usize = 0x100;

/// The game's VBuffer layout: +0x00 data or heap pointer, +0x10 used size,
/// +0x18 max size, SSO threshold 0xF. The game's own read builds a slice from
/// an uncapped length. Guard and cap it, or a stale snapshot crashes the game.
fn read_vbuffer_guarded(vbuf: *const usize) -> CString {
    let Some(used_size) = safe_read::<usize>(byte_offset::<usize>(vbuf, 0x10)) else {
        return CString::default();
    };
    let Some(max_size) = safe_read::<usize>(byte_offset::<usize>(vbuf, 0x18)) else {
        return CString::default();
    };
    if used_size == 0 || used_size > MAX_DISPLAY_NAME_LEN || used_size > max_size {
        return CString::default();
    }

    let data = if max_size > 0xf {
        match safe_read::<usize>(vbuf) {
            Some(ptr) if ptr != 0 => ptr as *const u8,
            _ => return CString::default(),
        }
    } else {
        vbuf as *const u8
    };

    let bytes = if let Some(chunk) =
        safe_read::<[u8; MAX_DISPLAY_NAME_LEN]>(data as *const [u8; MAX_DISPLAY_NAME_LEN])
    {
        let mut v = chunk[..used_size].to_vec();
        if let Some(nul) = v.iter().position(|&b| b == 0) {
            v.truncate(nul);
        }
        v
    } else {
        let mut v = Vec::with_capacity(used_size);
        for i in 0..used_size {
            let Some(byte) = safe_read::<u8>(data.wrapping_add(i)) else {
                return CString::default();
            };
            if byte == 0 {
                break;
            }
            v.push(byte);
        }
        v
    };

    CString::new(bytes).unwrap_or_default()
}

/// Returns `Some` only the first time an `actor_index` maps to a slot, or when
/// that mapping changes, so callers can send the event on every hit.
pub fn resolve_source_identity(
    source_slot: Option<(u32, u8, usize)>,
    actor_index: u32,
    character_type: u32,
) -> Option<PlayerIdentityEvent> {
    let (_player_key, slot, record_ptr) = source_slot?;

    {
        let announced = announced_identities().lock().ok()?;
        if announced.get(&actor_index) == Some(&slot) {
            return None;
        }
    }

    let identity = {
        let store = identity_store().lock().ok()?;
        match store.get(&(slot, record_ptr)) {
            Some(identity) => identity.clone(),
            None => store
                .iter()
                .filter(|((s, _), _)| *s == slot)
                .map(|(_, identity)| identity)
                .max_by_key(|identity| (identity.enw.is_some(), identity.is_online))?
                .clone(),
        }
    };

    {
        let mut announced = announced_identities().lock().ok()?;
        if announced.insert(actor_index, slot) == Some(slot) {
            return None;
        }
    }

    let network = identity
        .enw
        .and_then(network_user_for_enw)
        .or_else(|| {
            (Some(identity.party_index) == local_party_index())
                .then(local_network_user)
                .flatten()
        });

    Some(PlayerIdentityEvent {
        actor_index,
        party_index: identity.party_index,
        character_name: identity.character_name,
        display_name: identity.display_name,
        character_type,
        is_online: identity.is_online,
        sigils: identity.sigils,
        weapon_info: identity.weapon_info,
        summon_info: identity.summon_info,
        skill_loadout: identity.skill_loadout,
        over_mastery: identity.over_mastery,
        master_trait_flags: identity.master_trait_flags,
        effective_traits: identity.effective_traits,
        player_stats: identity.player_stats,
        network_user_id: network.as_ref().and_then(|n| n.entity_id.clone()),
        network_user_name: network.and_then(|n| n.name),
        master_level: identity.master_level,
    })
}

#[inline(always)]
fn byte_offset<T>(ptr: *const usize, offset: usize) -> *const T {
    (ptr as *const u8).wrapping_add(offset) as *const T
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(hash: u32, flag: u32, marker: u32) -> MasteryNodeRow {
        MasteryNodeRow {
            hash,
            flag,
            marker,
            rest: [0u8; 0x2C],
        }
    }

    #[test]
    fn master_level_splits_into_level_and_break() {
        assert_eq!(protocol::split_master_level(0), (0, 0));
        assert_eq!(protocol::split_master_level(32), (32, 0));
        assert_eq!(protocol::split_master_level(50), (50, 0));
        assert_eq!(protocol::split_master_level(51), (50, 1));
        assert_eq!(protocol::split_master_level(55), (50, 5));
    }

    #[test]
    fn master_level_past_the_table_is_refused() {
        let accept = |value: u32| (value <= protocol::MASTER_LEVEL_MAX).then_some(value);
        assert_eq!(accept(0), Some(0));
        assert_eq!(accept(55), Some(55));
        assert_eq!(accept(56), None);
        assert_eq!(accept(u32::MAX), None);
    }

    #[test]
    fn master_trait_flag_rows_classified_by_shape() {
        // Skillboard flag rows: {SBE hash, 0|1, sentinel}.
        let on = classify_master_trait_row(&row(0x3A583A9C, 1, EMPTY_HASH_SENTINEL))
            .expect("allocated flag row");
        assert_eq!(on.hash, 0x3A583A9C);
        assert!(on.on);

        let off = classify_master_trait_row(&row(0xFCE3D57D, 0, EMPTY_HASH_SENTINEL))
            .expect("unallocated flag row");
        assert_eq!(off.hash, 0xFCE3D57D);
        assert!(!off.on);

        // limit_bonus catalog row: a real param hash at +0x08.
        assert!(classify_master_trait_row(&row(0x45C65767, 0x0303, 0x9A97C049)).is_none());
        assert!(classify_master_trait_row(&row(0x45C65767, 1, 0x9A97C049)).is_none());

        assert!(classify_master_trait_row(&row(0x3A583A9C, 0x0303, EMPTY_HASH_SENTINEL)).is_none());

        assert!(classify_master_trait_row(&row(
            EMPTY_HASH_SENTINEL,
            EMPTY_HASH_SENTINEL,
            EMPTY_HASH_SENTINEL
        ))
        .is_none());
        assert!(classify_master_trait_row(&row(EMPTY_HASH_SENTINEL, 0, EMPTY_HASH_SENTINEL)).is_none());
        assert!(classify_master_trait_row(&row(0, 0, EMPTY_HASH_SENTINEL)).is_none());
    }
}
