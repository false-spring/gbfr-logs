use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::{event, process::Process};

use super::{globals::SBA_OFFSET, parent_actor_idx, safe_read, sba_cause, vfunc_slot_readable};

/// Chain burst manager singleton definition `*(0x7C24F48)`
const CHAIN_BURST_CTOR_SIG: &str =
    "41 b8 70 02 00 00 48 89 f1 31 d2 e8 ? ? ? ? 48 89 35 $ { ' } c5 f9 ef c0";
/// Load inside the SBA collision hook
const CHAIN_BURST_LOAD_SIG: &str =
    "48 8b 3d $ { ' } 48 89 f9 e8 ? ? ? ? 84 c0 74 47 48 8b 46 08";

/// Chain burst state machine
const CHAIN_BURST_STATE_OFFSET: usize = 0x1E8;
/// A 0.2333s (7/30) tail timer armed by `ChainBurst::Teardown`
/// so that trailing hits still count as inside the window.
/// Included here because we need to track `ChainBurst::IsActive()`
const CHAIN_BURST_TAIL_OFFSET: usize = 0x1D4;

/// VA of the global slot holding the chain-burst manager
static CHAIN_BURST_SLOT: AtomicUsize = AtomicUsize::new(0);

pub fn setup_chain_burst_manager(process: &Process) -> Result<()> {
    let from_ctor = process.search_address(CHAIN_BURST_CTOR_SIG)?;
    let from_load = process.search_address(CHAIN_BURST_LOAD_SIG)?;
    if from_ctor != from_load {
        return Err(anyhow!(
            "chain-burst manager AOBs disagree ({from_ctor:#x} vs {from_load:#x})"
        ));
    }

    CHAIN_BURST_SLOT.store(from_ctor, Ordering::Relaxed);

    #[cfg(feature = "console")]
    println!("Found chain-burst manager global: {:#x}", from_ctor);

    Ok(())
}

/// Tracked for Celestial Aqua bonus windows. Excludes `IsActive()`'s
/// summon-call arm: the post-burst reaction states already cover the
/// Primal Burst, and the arm would admit ordinary summon calls.
pub(crate) fn chain_burst_active() -> Option<bool> {
    let slot = CHAIN_BURST_SLOT.load(Ordering::Relaxed);
    if slot == 0 {
        return None;
    }
    let manager = safe_read::<usize>(slot as *const usize).filter(|&p| p != 0)? as *const u8;

    let state = safe_read::<i32>(manager.wrapping_add(CHAIN_BURST_STATE_OFFSET) as *const i32)?;
    let tail = safe_read::<f32>(manager.wrapping_add(CHAIN_BURST_TAIL_OFFSET) as *const f32)?;

    Some(state != 0 || tail > 0.0)
}

type OnSBAUpdateFunc = unsafe extern "system" fn(
    *const usize, // rcx: a1 = SBA struct (entity + SBA_OFFSET)
    f32,          // xmm1: gauge add value
    u32,          // r8d
    u8,           // r9b: gauge-already-full (setae)
    u8,           // [rsp+0x20]
    f32,          // [rsp+0x28]
    u8,           // [rsp+0x30]
    u8,           // [rsp+0x38]
    u8,           // [rsp+0x40]
    u8,           // [rsp+0x48]
    u8,           // [rsp+0x50]
) -> usize;
type OnSBAAttemptFunc = unsafe extern "system" fn(*const usize, f32) -> usize;
type OnCheckSBACollisionFunc = unsafe extern "system" fn(*const usize, f32) -> usize;
// type OnContinueSBAChainFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;
type OnRemoteSBAUpdateFunc =
    unsafe extern "system" fn(*const usize, *const usize, f32, f32) -> usize;
// New ER function to hook: net_send component ptr (= entity + 0x4350), mode (2 = "performed SBA"), value.
type OnSBAResetBroadcastFunc = unsafe extern "system" fn(*const usize, u32, f32) -> usize;

static_detour! {
    static OnSBAUpdate: unsafe extern "system" fn(*const usize, f32, u32, u8, u8, f32, u8, u8, u8, u8, u8) -> usize;
    static OnSBAAttempt: unsafe extern "system" fn(*const usize, f32) -> usize;
    static OnCheckSBACollision: unsafe extern "system" fn(*const usize, f32) -> usize;
    static OnRemoteSBAUpdate: unsafe extern "system" fn(*const usize, *const usize, f32, f32) -> usize;
    static OnSBAResetBroadcast: unsafe extern "system" fn(*const usize, u32, f32) -> usize;
}

const ON_HANDLE_SBA_UPDATE_SIG: &str =
    "e8 $ { ' } c4 c1 78 2e f8 0f 83 ? ? ? ? c5 fa 10 46 ? c5 f8 2e 86 80 00 00 00";
const ON_ATTEMPT_SBA_SIG: &str = "e8 $ { ' } 48 8d 8e ? ? ff ff c7 44 24 38 00 00 80 3f";
const ON_CHECK_SBA_COLLISION_SIG: &str = "e8 $ { ' } 84 c0 74 ? 48 83 c4 40 5e c3 8b 8e ? ? ff ff";
const ON_SBA_RESET_BROADCAST_SIG: &str = "c7 80 ac 32 00 00 00 00 00 00 b9 50 43 00 00 48 03 4e f8 c5 e8 57 d2 ba 02 00 00 00 e8 $ { ' }";
// ER's function takes (player_obj*, msg*); the detour's two trailing f32 args are
// ignored by the callee and never dereferenced here, so the 4-arg type is kept.
const ON_HANDLE_REMOTE_SBA_UPDATE_SIG: &str =
    "48 8b 8f ? ? ? ? 48 89 f2 e8 $ { ' } e9 ? ? ? ? 48 8b 8f ? ? ? ? 8b 56 1c c6 44 24 20 01";

/// Gets called when your SBA gauge value needs to update with a given value.
#[derive(Clone)]
pub struct OnHandleSBAUpdateHook {
    tx: event::Tx,
}

impl OnHandleSBAUpdateHook {
    pub fn new(tx: event::Tx) -> Self {
        OnHandleSBAUpdateHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_sba_update_original) = process.search_address(ON_HANDLE_SBA_UPDATE_SIG) {
            #[cfg(feature = "console")]
            println!("found on sba update");

            let cloned_self = self.clone();

            unsafe {
                let func: OnSBAUpdateFunc = std::mem::transmute(on_sba_update_original);
                OnSBAUpdate.initialize(func, move |a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11| {
                    cloned_self.run(a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11)
                })?;
                OnSBAUpdate.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_sba_update"));
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        a1: *const usize,
        a2: f32,
        a3: u32,
        a4: u8,
        a5: u8,
        a6: f32,
        a7: u8,
        a8: u8,
        a9: u8,
        a10: u8,
        a11: u8,
    ) -> usize {
        let sba_offset = SBA_OFFSET.load(Ordering::Relaxed);

        let entity_ptr = unsafe { a1.byte_sub(sba_offset as usize) };

        let source_parent_idx = parent_actor_idx(entity_ptr);

        let sba_value_ptr = unsafe { a1.byte_add(0x7C) } as *const f32;
        let old_sba_value = unsafe { sba_value_ptr.read() };

        let ret = unsafe { OnSBAUpdate.call(a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11) };

        let new_sba_value = unsafe { sba_value_ptr.read() };
        let sba_added = f32::max(new_sba_value - old_sba_value, 0.0);

        let cause = sba_cause::resolve_for(source_parent_idx);

        if new_sba_value == 0.0 && old_sba_value > 0.0 {
            #[cfg(feature = "console")]
            println!("on perform sba: player_index={}", source_parent_idx);

            let payload = Message::OnPerformSBA(protocol::OnPerformSBAEvent {
                actor_index: source_parent_idx,
            });

            let _ = self.tx.send(payload);
        } else {
            let payload = Message::OnUpdateSBA(protocol::OnUpdateSBAEvent {
                actor_index: source_parent_idx,
                sba_value: new_sba_value,
                sba_added,
                cause,
            });

            let _ = self.tx.send(payload);
        }

        ret
    }
}

/// Called when your first try to attempt your SBA, and sets you into "casting SBA" state.
#[derive(Clone)]
pub struct OnAttemptSBAHook {
    tx: event::Tx,
}

impl OnAttemptSBAHook {
    pub fn new(tx: event::Tx) -> Self {
        OnAttemptSBAHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_sba_attempt_original) = process.search_address(ON_ATTEMPT_SBA_SIG) {
            #[cfg(feature = "console")]
            println!("found on sba attempt");

            let cloned_self = self.clone();

            unsafe {
                let func: OnSBAAttemptFunc = std::mem::transmute(on_sba_attempt_original);
                OnSBAAttempt.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
                OnSBAAttempt.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_sba_attempt"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: f32) -> usize {
        let ret = unsafe { OnSBAAttempt.call(a1, a2) };

        let entity_ptr = unsafe { a1.byte_add(0x10).read() } as *const usize;

        let source_parent_idx = parent_actor_idx(entity_ptr);

        #[cfg(feature = "console")]
        println!("on sba attempt: player_index={}", source_parent_idx);

        let payload = Message::OnAttemptSBA(protocol::OnAttemptSBAEvent {
            actor_index: source_parent_idx,
        });

        let _ = self.tx.send(payload);

        ret
    }
}

/// Gets called when you're in "casting SBA state" once per game update interval until your SBA lands on
/// the target (or you miss)
/// ONLY WORKS FOR LOCAL.
#[derive(Clone)]
pub struct OnCheckSBACollisionHook {
    tx: event::Tx,
}

impl OnCheckSBACollisionHook {
    pub fn new(tx: event::Tx) -> Self {
        OnCheckSBACollisionHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_check_sba_collision_original) =
            process.search_address(ON_CHECK_SBA_COLLISION_SIG)
        {
            #[cfg(feature = "console")]
            println!("found on check sba collision");

            let cloned_self = self.clone();

            unsafe {
                let func: OnCheckSBACollisionFunc =
                    std::mem::transmute(on_check_sba_collision_original);
                OnCheckSBACollision.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
                OnCheckSBACollision.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_check_sba_collision"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: f32) -> usize {
        let ret = unsafe { OnCheckSBACollision.call(a1, a2) };

        if ret != 0 {
            let entity_ptr = unsafe { a1.byte_add(0x10).read() } as *const usize;

            let source_parent_idx = parent_actor_idx(entity_ptr);

            #[cfg(feature = "console")]
            println!("on perform sba: player_index={}", source_parent_idx);

            let payload = Message::OnPerformSBA(protocol::OnPerformSBAEvent {
                actor_index: source_parent_idx,
            });

            let _ = self.tx.send(payload);
        }

        ret
    }
}

/// Gets called when you connect your SBA with an active SBA chain (2/3/4)
/// or land a solo SBA — ER routes both through the same mode-2 broadcast.
#[derive(Clone)]
pub struct OnSBAResetBroadcastHook {
    tx: event::Tx,
}

impl OnSBAResetBroadcastHook {
    pub fn new(tx: event::Tx) -> Self {
        OnSBAResetBroadcastHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_sba_reset_broadcast_original) =
            process.search_address(ON_SBA_RESET_BROADCAST_SIG)
        {
            #[cfg(feature = "console")]
            println!("found on sba reset broadcast");

            let cloned_self = self.clone();

            unsafe {
                let func: OnSBAResetBroadcastFunc =
                    std::mem::transmute(on_sba_reset_broadcast_original);
                OnSBAResetBroadcast.initialize(func, move |a1, a2, a3| cloned_self.run(a1, a2, a3))?;
                OnSBAResetBroadcast.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_sba_reset_broadcast"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: u32, a3: f32) -> usize {
        let ret = unsafe { OnSBAResetBroadcast.call(a1, a2, a3) };

        if a2 == 2 {
            let entity_ptr = unsafe { a1.byte_sub(0x4350) };

            if !vfunc_slot_readable(entity_ptr, 0x58) {
                return ret;
            }

            let source_parent_idx = parent_actor_idx(entity_ptr);

            #[cfg(feature = "console")]
            println!("on perform sba (reset broadcast): player_index={}", source_parent_idx);

            let payload = Message::OnPerformSBA(protocol::OnPerformSBAEvent {
                actor_index: source_parent_idx,
            });

            let _ = self.tx.send(payload);
        }

        ret
    }
}

#[derive(Clone)]
pub struct OnRemoteSBAUpdateHook {
    tx: event::Tx,
}

impl OnRemoteSBAUpdateHook {
    pub fn new(tx: event::Tx) -> Self {
        OnRemoteSBAUpdateHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_remote_sba_update_original) =
            process.search_address(ON_HANDLE_REMOTE_SBA_UPDATE_SIG)
        {
            #[cfg(feature = "console")]
            println!("found on remote sba update");

            let cloned_self = self.clone();

            unsafe {
                let func: OnRemoteSBAUpdateFunc =
                    std::mem::transmute(on_remote_sba_update_original);
                OnRemoteSBAUpdate
                    .initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;
                OnRemoteSBAUpdate.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_remote_sba_update"));
        }

        Ok(())
    }

    fn run(&self, player_entity: *const usize, a2: *const usize, a3: f32, a4: f32) -> usize {
        let sba_offset = SBA_OFFSET.load(Ordering::Relaxed);
        let sba_value_ptr =
            unsafe { player_entity.byte_add(sba_offset as usize).byte_add(0x7C) } as *const f32;
        let old_sba_value = unsafe { sba_value_ptr.read() };

        let ret = unsafe { OnRemoteSBAUpdate.call(player_entity, a2, a3, a4) };

        let source_parent_idx = parent_actor_idx(player_entity);

        let new_sba_value = unsafe { sba_value_ptr.read() };
        let sba_added = f32::max(new_sba_value - old_sba_value, 0.0);

        // If the SBA value is 0, then the player has performed an SBA and this is resetting their SBA.
        if new_sba_value == 0.0 && old_sba_value > 0.0 {
            #[cfg(feature = "console")]
            println!("on perform sba: player_index={}", source_parent_idx);

            let payload = Message::OnPerformSBA(protocol::OnPerformSBAEvent {
                actor_index: source_parent_idx,
            });

            let _ = self.tx.send(payload);
        } else {
            let payload = Message::OnUpdateSBA(protocol::OnUpdateSBAEvent {
                actor_index: source_parent_idx,
                sba_value: new_sba_value,
                sba_added,
                // Resolve SBA cause separately for remote players
                cause: sba_cause::resolve_remote(source_parent_idx),
            });

            let _ = self.tx.send(payload);
        }

        ret
    }
}
