use std::sync::atomic::Ordering;

use crate::{event, hooks::globals::QUEST_STATE_PTR, process::Process};
use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

type OnEnterAreaFunc = unsafe extern "system" fn(u32, *const usize, u8, *const usize) -> usize;

static_detour! {
    static OnEnterArea: unsafe extern "system" fn(u32, *const usize, u8, *const usize) -> usize;
}

const ON_ENTER_AREA_SIG: &str = "e8 $ { ' } c5 ? ? ? c5 f8 29 45 ? c7 45 ? ? ? ? ?";

/// Handles tracking whenever the player enters a new area.
#[derive(Clone)]
pub struct OnAreaEnterHook {
    tx: event::Tx,
}

impl OnAreaEnterHook {
    pub fn new(tx: event::Tx) -> Self {
        OnAreaEnterHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        if let Ok(on_enter_area_evt) = process.search_address(ON_ENTER_AREA_SIG) {
            let cloned_self = self.clone();

            #[cfg(feature = "console")]
            println!("Found on enter area");

            unsafe {
                let func: OnEnterAreaFunc = std::mem::transmute(on_enter_area_evt);

                OnEnterArea
                    .initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;

                OnEnterArea.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_enter_area"));
        }

        Ok(())
    }

    fn run(&self, a1: u32, a2: *const usize, a3: u8, a4: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("on enter area");

        let quest_state_ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);

        if quest_state_ptr != std::ptr::null_mut() {
            let quest_state = unsafe { quest_state_ptr.read() };

            let quest_id = quest_state.quest_id;
            let timer = quest_state.elapsed_time;

            let _ = self.tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
                last_known_quest_id: quest_id,
                last_known_elapsed_time_in_secs: timer,
            }));
        } else {
            let _ = self.tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
                last_known_quest_id: 0,
                last_known_elapsed_time_in_secs: 0,
            }));
        }

        let ret = unsafe { OnEnterArea.call(a1, a2, a3, a4) };

        ret
    }
}
