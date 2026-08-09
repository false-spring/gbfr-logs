use anyhow::{anyhow, Result};
use retour::static_detour;

use crate::{event, process::Process};

use super::{player_directory, probe_actor, read_player_key, safe_read};

// The 5th arg is a real stack arg; the detour must be declared 5-wide.
type ChangeStateFunc =
    unsafe extern "system" fn(*const usize, u32, u32, u32, u32) -> usize;

static_detour! {
    static OnDeathEvent: unsafe extern "system" fn(*const usize, u32, u32, u32, u32) -> usize;
}

const ON_DEATH_EVENT_SIG: &str = "e8 $ { ' } b9 80 3d 00 00 48 03 4f 10 b2 01";

/// Entity state id registered by `ExPlayerDie`.
const DIE_STATE: u32 = 0x48;
const STATE_ID_OFFSET: usize = 0x40;

#[derive(Clone)]
pub struct OnDeathHook {
    tx: event::Tx,
}

impl OnDeathHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_death_event) = process.search_address(ON_DEATH_EVENT_SIG) {
            #[cfg(feature = "console")]
            println!("Found on death event");

            unsafe {
                let func: ChangeStateFunc = std::mem::transmute(on_death_event);
                OnDeathEvent.initialize(func, move |entity, new_state, kind, flag, a5| {
                    cloned_self.run(entity, new_state, kind, flag, a5)
                })?;
                OnDeathEvent.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_death_event"));
        }

        Ok(())
    }

    fn run(&self, entity: *const usize, new_state: u32, kind: u32, flag: u32, a5: u32) -> usize {
        // Sample the outgoing state first; the game re-posts 0x48 if you're hit while dead
        let prev_state = read_state_id(entity);

        let ret = unsafe { OnDeathEvent.call(entity, new_state, kind, flag, a5) };

        if new_state == DIE_STATE && prev_state != Some(DIE_STATE) {
            let post_state = read_state_id(entity);
            if post_state == Some(DIE_STATE) {
                if let Some(player_key) = read_player_key(entity) {
                    let probe = probe_actor(entity);
                    let slot = probe.slot.map(|(_key, slot, _record)| slot);
                    let event = protocol::Message::OnDeathEvent(protocol::OnDeathEvent {
                        actor_index: probe.id,
                        death_counter: player_directory::bump_death_count(player_key, slot),
                    });

                    let _ = self.tx.send(event);
                }
            }
        }

        ret
    }
}

fn read_state_id(entity: *const usize) -> Option<u32> {
    safe_read((entity as *const u8).wrapping_add(STATE_ID_OFFSET) as *const u32)
}
