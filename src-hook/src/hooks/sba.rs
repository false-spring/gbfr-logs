use std::sync::atomic::Ordering;

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::{event, process::Process};

use super::{actor_idx, actor_type_id, get_source_parent, globals::SBA_OFFSET};

type OnSBAUpdateFunc = unsafe extern "system" fn(*const usize, f32, u32, u8, u32, u8) -> usize;
type OnSBAAttemptFunc = unsafe extern "system" fn(*const usize, f32) -> usize;
type OnCheckSBACollisionFunc = unsafe extern "system" fn(*const usize, f32) -> usize;
type OnContinueSBAChainFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;
type OnRemoteSBAUpdateFunc =
    unsafe extern "system" fn(*const usize, *const usize, f32, f32) -> usize;

static_detour! {
    static OnSBAUpdate: unsafe extern "system" fn(*const usize, f32, u32, u8, u32, u8) -> usize;
    static OnSBAAttempt: unsafe extern "system" fn(*const usize, f32) -> usize;
    static OnCheckSBACollision: unsafe extern "system" fn(*const usize, f32) -> usize;
    static OnContinueSBAChain: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnRemoteSBAUpdate: unsafe extern "system" fn(*const usize, *const usize, f32, f32) -> usize;
}

const ON_HANDLE_SBA_UPDATE_SIG: &str = "e8 $ { ' } c5 fa 10 46 ? c5 f8 2e 86 80 00 00 00";
const ON_ATTEMPT_SBA_SIG: &str = "e8 $ { ' } 48 8d 8e ? ? ff ff c7 44 24 38 00 00 80 3f";
const ON_CHECK_SBA_COLLISION_SIG: &str = "e8 $ { ' } 84 c0 0f 85 f0 00 00 ? 8b 8e ? ? ff ff";
const ON_CONTINUE_SBA_CHAIN_SIG: &str = "e8 $ { ' } 48 8b 53 ? 48 8d 82 ? ? ? ?";
const ON_HANDLE_REMOTE_SBA_UPDATE_SIG: &str =
    "48 8b 8f ? ? ? ? 4c 89 e2 e8 $ { ' } e9 ? ? ? ? 48 81 c7 ? ? ? ? 48 89 f9";

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
                OnSBAUpdate.initialize(func, move |a1, a2, a3, a4, a5, a6| {
                    cloned_self.run(a1, a2, a3, a4, a5, a6)
                })?;
                OnSBAUpdate.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_sba_update"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: f32, a3: u32, a4: u8, a5: u32, a6: u8) -> usize {
        let sba_offset = SBA_OFFSET.load(Ordering::Relaxed);

        let entity_ptr = unsafe { a1.byte_sub(sba_offset as usize) };

        let source_idx = actor_idx(entity_ptr);
        let source_type_id = actor_type_id(entity_ptr);
        let (_, source_parent_idx) =
            get_source_parent(source_type_id, entity_ptr).unwrap_or((source_type_id, source_idx));

        let sba_value_ptr = unsafe { a1.byte_add(0x7C) } as *const f32;
        let old_sba_value = unsafe { sba_value_ptr.read() };

        let ret = unsafe { OnSBAUpdate.call(a1, a2, a3, a4, a5, a6) };

        let new_sba_value = unsafe { sba_value_ptr.read() };
        let sba_added = f32::max(new_sba_value - old_sba_value, 0.0);

        if new_sba_value == 0.0 {
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

        let source_idx = actor_idx(entity_ptr);
        let source_type_id = actor_type_id(entity_ptr);
        let (_, source_parent_idx) =
            get_source_parent(source_type_id, entity_ptr).unwrap_or((source_type_id, source_idx));

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

            let source_idx = actor_idx(entity_ptr);
            let source_type_id = actor_type_id(entity_ptr);
            let (_, source_parent_idx) = get_source_parent(source_type_id, entity_ptr)
                .unwrap_or((source_type_id, source_idx));

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
#[derive(Clone)]
pub struct OnContinueSBAChainHook {
    tx: event::Tx,
}

impl OnContinueSBAChainHook {
    pub fn new(tx: event::Tx) -> Self {
        OnContinueSBAChainHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_continue_sba_chain_original) =
            process.search_address(ON_CONTINUE_SBA_CHAIN_SIG)
        {
            #[cfg(feature = "console")]
            println!("found on continue sba chain");

            let cloned_self = self.clone();

            unsafe {
                let func: OnContinueSBAChainFunc =
                    std::mem::transmute(on_continue_sba_chain_original);
                OnContinueSBAChain.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
                OnContinueSBAChain.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_continue_sba_chain"));
        }

        Ok(())
    }

    fn run(&self, player_entity: *const usize, a2: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!(
            "on continue sba chain: player_entity={:p}, a2={:p}",
            player_entity, a2
        );

        let ret = unsafe { OnContinueSBAChain.call(player_entity, a2) };

        let source_idx = actor_idx(player_entity);
        let source_type_id = actor_type_id(player_entity);
        let (_, source_parent_idx) = get_source_parent(source_type_id, player_entity)
            .unwrap_or((source_type_id, source_idx));

        let payload = Message::OnContinueSBAChain(protocol::OnContinueSBAChainEvent {
            actor_index: source_parent_idx,
        });

        let _ = self.tx.send(payload);

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

        let source_idx = actor_idx(player_entity);
        let source_type_id = actor_type_id(player_entity);
        let (_, source_parent_idx) = get_source_parent(source_type_id, player_entity)
            .unwrap_or((source_type_id, source_idx));

        let new_sba_value = unsafe { sba_value_ptr.read() };
        let sba_added = f32::max(new_sba_value - old_sba_value, 0.0);

        // If the SBA value is 0, then the player has performed an SBA and this is resetting their SBA.
        if new_sba_value == 0.0 {
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
            });

            let _ = self.tx.send(payload);
        }

        ret
    }
}
