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
