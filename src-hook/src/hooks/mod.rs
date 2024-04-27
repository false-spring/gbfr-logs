use anyhow::Result;

use crate::{event, process::Process};

use self::{
    area::OnAreaEnterHook,
    damage::{OnProcessDamageHook, OnProcessDotHook},
    player::OnLoadPlayerHook,
    quest::{OnLoadQuestHook, OnQuestCompleteHook},
    sba::{
        OnAttemptSBAHook, OnCheckSBACollisionHook, OnContinueSBAChainHook, OnHandleSBAUpdateHook,
        OnRemoteSBAUpdateHook,
    },
};

mod area;
mod damage;
mod ffi;
mod globals;
mod player;
mod quest;
mod sba;

type GetEntityHashID0x58 = unsafe extern "system" fn(*const usize, *const u32) -> *const usize;

pub fn setup_hooks(tx: event::Tx) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    globals::setup_globals(&process)?;

    /* Damage Events */
    OnProcessDamageHook::new(tx.clone()).setup(&process)?;
    OnProcessDotHook::new(tx.clone()).setup(&process)?;

    /* Player Data */
    OnLoadPlayerHook::new(tx.clone()).setup(&process)?;

    /* Quest + Area Tracking */
    OnAreaEnterHook::new(tx.clone()).setup(&process)?;
    OnLoadQuestHook::new().setup(&process)?;
    OnQuestCompleteHook::new(tx.clone()).setup(&process)?;

    /* SBA */
    OnHandleSBAUpdateHook::new(tx.clone()).setup(&process)?;
    OnRemoteSBAUpdateHook::new(tx.clone()).setup(&process)?;
    OnAttemptSBAHook::new(tx.clone()).setup(&process)?;
    OnCheckSBACollisionHook::new(tx.clone()).setup(&process)?;
    OnContinueSBAChainHook::new(tx.clone()).setup(&process)?;

    Ok(())
}

#[inline(always)]
pub unsafe fn v_func<T: Sized>(ptr: *const usize, offset: usize) -> T {
    ((ptr.read() as *const usize).byte_add(offset) as *const T).read()
}

#[inline(always)]
pub fn actor_type_id(actor_ptr: *const usize) -> u32 {
    let mut type_id: u32 = 0;

    unsafe {
        v_func::<GetEntityHashID0x58>(actor_ptr, 0x58)(actor_ptr, &mut type_id as *mut u32);
    }

    type_id
}

#[inline(always)]
pub fn actor_idx(actor_ptr: *const usize) -> u32 {
    unsafe { (actor_ptr.byte_add(0x170) as *const u32).read() }
}

// Returns the parent entity of the source entity if necessary.
pub fn get_source_parent(source_type_id: u32, source: *const usize) -> Option<(u32, u32)> {
    match source_type_id {
        // Pl0700Ghost -> Pl0700
        0x2AF678E8 => {
            let parent_instance = parent_specified_instance_at(source, 0xE48)?;

            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Pl0700GhostSatellite -> Pl0700
        0x8364C8BC => {
            let parent_instance = parent_specified_instance_at(source, 0x508)?;

            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Wp1890: Cagliostro's Ouroboros Dragon Sled -> Pl1800
        0xC9F45042 => {
            let parent_instance = parent_specified_instance_at(source, 0x578)?;
            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Pl2000: Id's Dragon Form -> Pl1900
        0xF5755C0E => {
            let parent_instance = parent_specified_instance_at(source, 0xD338)?;
            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Wp2290: Seofon's Avatar
        0x5B1AB457 => {
            let parent_instance = parent_specified_instance_at(source, 0x500)?;
            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        _ => None,
    }
}

// Returns the specified instance of the parent entity.
// ptr+offset: Entity
// *(ptr+offset) + 0x70: m_pSpecifiedInstance (Pl0700, Pl1200, etc.)
fn parent_specified_instance_at(actor_ptr: *const usize, offset: usize) -> Option<*const usize> {
    unsafe {
        let info = (actor_ptr.byte_add(offset) as *const *const *const usize).read_unaligned();

        if info.is_null() {
            return None;
        }

        Some(info.byte_add(0x70).read())
    }
}
