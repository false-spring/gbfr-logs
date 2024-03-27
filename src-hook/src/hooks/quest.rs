use std::sync::atomic::Ordering;

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::{
    event,
    hooks::{ffi::QuestState, globals::QUEST_STATE_PTR},
    process::Process,
};

type OnLoadQuestStateFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnShowResultScreenFunc = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static OnLoadQuestState: unsafe extern "system" fn(*const usize) -> usize;
    static OnShowResultScreen: unsafe extern "system" fn(*const usize) -> usize;
}

const ON_LOAD_QUEST_STATE: &str =
    "48 8b 0d ? ? ? ? e8 $ { ' } c5 fb 12 ? ? ? ? ? c5 f8 11 ? ? ? ? ? c5 f8 11 ? ? ? ? ? 48 83 c4 48";
const ON_SHOW_RESULT_SCREEN_SIG: &str =
    "e8 $ { ' } b8 ? ? ? ? 23 87 ? ? 00 00 3d 00 00 60 00 0f 94 c0";

/// Called while loading into a quest.
#[derive(Clone)]
pub struct OnLoadQuestHook {}

impl OnLoadQuestHook {
    pub fn new() -> Self {
        OnLoadQuestHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_load_quest_state) = process.search_address(ON_LOAD_QUEST_STATE) {
            #[cfg(feature = "console")]
            println!("Found on load quest state");

            unsafe {
                let func: OnLoadQuestStateFunc = std::mem::transmute(on_load_quest_state);
                OnLoadQuestState.initialize(func, move |a1| cloned_self.run(a1))?;
                OnLoadQuestState.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_load_quest_state"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("on load quest state");

        let ret = unsafe { OnLoadQuestState.call(a1) };
        let quest_state_ptr = unsafe { a1.byte_add(0x1D8) } as *mut QuestState;

        if quest_state_ptr == std::ptr::null_mut() {
            return ret;
        }

        QUEST_STATE_PTR.store(quest_state_ptr, std::sync::atomic::Ordering::Relaxed);

        ret
    }
}

/// Called whenever the result screen is shown for the quest.
#[derive(Clone)]
pub struct OnQuestCompleteHook {
    tx: event::Tx,
}

impl OnQuestCompleteHook {
    pub fn new(tx: event::Tx) -> Self {
        OnQuestCompleteHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_show_result_screen) = process.search_address(ON_SHOW_RESULT_SCREEN_SIG) {
            #[cfg(feature = "console")]
            println!("Found on show result screen");

            unsafe {
                let func: OnShowResultScreenFunc = std::mem::transmute(on_show_result_screen);
                OnShowResultScreen.initialize(func, move |a1| cloned_self.run(a1))?;
                OnShowResultScreen.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_show_result_screen"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("on show result screen");

        let quest_state_ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);

        if quest_state_ptr != std::ptr::null_mut() {
            let quest_state = unsafe { quest_state_ptr.read() };
            let quest_id = quest_state.quest_id;
            let timer = quest_state.elapsed_time;

            let _ = self
                .tx
                .send(Message::OnQuestComplete(protocol::QuestCompleteEvent {
                    quest_id,
                    elapsed_time_in_secs: timer,
                }));
        }

        let ret = unsafe { OnShowResultScreen.call(a1) };

        ret
    }
}
