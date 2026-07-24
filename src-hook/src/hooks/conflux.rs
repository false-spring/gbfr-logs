//! The Conflux (internally "endless mode"). Keeps one log per survey.

use std::mem::offset_of;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::hooks::ffi::QuestState;
use crate::hooks::{player_directory, quest, rtti, safe_read, AnchorReport};
use crate::{event, process::Process};

const ENDLESS_STATE_SIG: &str =
    "48 8b 0d $ { ' } 31 c0 83 b9 e0 2d 00 00 01 0f 94 c0 83 f0 03 c3";

const ENDLESS_BEGIN_A_SIG: &str =
    "55 41 57 41 56 56 57 53 48 83 ec 58 48 8d 6c 24 50 48 c7 45 00 fe ff ff ff \
     89 d3 48 89 ce 4c 8d 79 10 8b 05 ? ? ? ?";
const ENDLESS_BEGIN_B_SIG: &str =
    "55 41 57 41 56 41 55 41 54 56 57 53 48 81 ec 18 02 00 00 48 8d ac 24 80 00 00 00 \
     c5 f8 29 bd 80 01 00 00 c5 f9 7f b5 70 01 00 00 48 c7 85 68 01 00 00 fe ff ff ff \
     48 8b 35 ? ? ? ? 80 be 59 02 00 00 00";

const ENDLESS_END_SIG: &str =
    "55 41 57 41 56 41 55 41 54 56 57 53 48 83 ec 58 48 8d 6c 24 50 48 c7 45 00 fe ff ff ff \
     48 89 cb 48 8b 0d ? ? ? ? 48 8d 93 b8 24 00 00";

const ENDLESS_ADVANCE_SIG: &str =
    "55 41 57 41 56 41 54 56 57 53 48 83 ec 60 48 8d 6c 24 60 48 c7 45 f8 fe ff ff ff \
     89 d7 48 89 ce 0f b6 05 ? ? ? ? 85 c0";

const CLEAR_ENDLESS_QUEST_SIG: &str =
    "55 41 57 41 56 41 55 41 54 56 57 53 48 83 ec 58 48 8d 6c 24 50 c5 f8 29 75 f0 \
     48 c7 45 e8 fe ff ff ff 48 89 cf 48 8b 05 ? ? ? ? 48 8b 88 10 02 00 00";
/// `ClearEndlessModeQuest`'s vtable, by mangled RTTI name; slot 1 is `execute`.
pub(crate) const CLEAR_ENDLESS_QUEST_VTABLE_NAME: &str = ".?AVClearEndlessModeQuest@quest@stage@@";
const CLEAR_ENDLESS_QUEST_VTABLE_SLOT: usize = 1;

fn clear_endless_quest_execute_slot(process: &Process) -> Option<*const usize> {
    let vtable = rtti::resolved_vtable(CLEAR_ENDLESS_QUEST_VTABLE_NAME)?;
    Some(
        (process.base_address
            + vtable
            + CLEAR_ENDLESS_QUEST_VTABLE_SLOT * std::mem::size_of::<usize>())
            as *const usize,
    )
}

const STATE_IN_SURVEY_OFFSET: usize = 0x2DE0;
const STATE_CONTENT_ID_OFFSET: usize = 0x24B8;
const STATE_AREA_OFFSET: usize = 0x8;
const STATE_CYCLE_OFFSET: usize = 0xC;
const STATE_AREA_COUNT_OFFSET: usize = 0x2DE8;
const STATE_CYCLE_COUNT_OFFSET: usize = 0x2DEC;

static ENDLESS_STATE_SLOT: AtomicUsize = AtomicUsize::new(0);

static SURVEY_OPEN: AtomicBool = AtomicBool::new(false);

/// Sum of the game's per-area quest timers. Time between areas does not count.
static SURVEY_ELAPSED_SECS: AtomicU32 = AtomicU32::new(0);

type OnEndlessBeginAFunc = unsafe extern "system" fn(*const usize, u32) -> usize;
type OnEndlessBeginBFunc = unsafe extern "system" fn() -> usize;
type OnEndlessEndFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnClearEndlessQuestFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnEndlessAdvanceFunc = unsafe extern "system" fn(*const usize, u32) -> usize;

static_detour! {
    static OnEndlessBeginA: unsafe extern "system" fn(*const usize, u32) -> usize;
    static OnEndlessBeginB: unsafe extern "system" fn() -> usize;
    static OnEndlessEnd: unsafe extern "system" fn(*const usize) -> usize;
    static OnClearEndlessQuest: unsafe extern "system" fn(*const usize) -> usize;
    static OnEndlessAdvance: unsafe extern "system" fn(*const usize, u32) -> usize;
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ConfluxState {
    pub in_survey: bool,
    pub content_id: u32,
    pub area: u32,
    pub cycle: u32,
    pub area_count: u32,
    pub cycle_count: u32,
}

impl ConfluxState {
    fn at_final_area(&self) -> bool {
        self.area_count != 0
            && self.cycle_count != 0
            && self.area == self.area_count
            && self.cycle == self.cycle_count
    }
}

pub fn setup_conflux_state(process: &Process) -> Result<()> {
    let slot = process.search_address(ENDLESS_STATE_SIG)?;
    ENDLESS_STATE_SLOT.store(slot, Ordering::Relaxed);

    #[cfg(feature = "console")]
    println!("Found Conflux state global: {:#x}", slot);

    Ok(())
}

pub(crate) fn snapshot() -> Option<ConfluxState> {
    let slot_va = ENDLESS_STATE_SLOT.load(Ordering::Relaxed);
    if slot_va == 0 {
        return None;
    }
    let state = safe_read(slot_va as *const usize).filter(|&p| p != 0)? as *const u8;

    let u32_at =
        |offset: usize| safe_read::<u32>(state.wrapping_add(offset) as *const u32).unwrap_or(0);

    Some(ConfluxState {
        in_survey: u32_at(STATE_IN_SURVEY_OFFSET) == 1,
        content_id: u32_at(STATE_CONTENT_ID_OFFSET),
        area: u32_at(STATE_AREA_OFFSET),
        cycle: u32_at(STATE_CYCLE_OFFSET),
        area_count: u32_at(STATE_AREA_COUNT_OFFSET),
        cycle_count: u32_at(STATE_CYCLE_COUNT_OFFSET),
    })
}

pub(crate) fn in_survey() -> bool {
    snapshot().is_some_and(|s| s.in_survey)
}

fn area_elapsed_secs() -> u32 {
    let Some(state) = quest::quest_state() else {
        return 0;
    };
    safe_read::<u32>(
        (state as *const u8).wrapping_add(offset_of!(QuestState, elapsed_time)) as *const u32,
    )
    .unwrap_or(0)
}

fn begin_survey(tx: &event::Tx) {
    let Some(state) = snapshot().filter(|s| s.in_survey) else {
        return;
    };
    if SURVEY_OPEN.swap(true, Ordering::Relaxed) {
        return;
    }
    SURVEY_ELAPSED_SECS.store(0, Ordering::Relaxed);

    log::info!(
        "[conflux] survey start: content={:#x} area={}/{} cycle={}/{}",
        state.content_id,
        state.area,
        state.area_count,
        state.cycle,
        state.cycle_count
    );

    let _ = tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
        last_known_quest_id: state.content_id,
        last_known_elapsed_time_in_secs: 0,
    }));
}

fn end_survey(tx: &event::Tx, state: Option<ConfluxState>) {
    if !SURVEY_OPEN.swap(false, Ordering::Relaxed) {
        return;
    }

    let state = state.unwrap_or_default();
    let quest_id = state.content_id;
    let elapsed_time_in_secs = SURVEY_ELAPSED_SECS.swap(0, Ordering::Relaxed);

    if state.at_final_area() {
        log::info!(
            "[conflux] survey end: CLEARED at content={:#x} area={}/{} cycle={}/{} quest-time={}s",
            quest_id,
            state.area,
            state.area_count,
            state.cycle,
            state.cycle_count,
            elapsed_time_in_secs
        );
        let _ = tx.send(Message::OnQuestComplete(protocol::QuestCompleteEvent {
            quest_id,
            elapsed_time_in_secs,
        }));
    } else {
        log::info!(
            "[conflux] survey end: LEFT at content={:#x} area={}/{} cycle={}/{} quest-time={}s",
            quest_id,
            state.area,
            state.area_count,
            state.cycle,
            state.cycle_count,
            elapsed_time_in_secs
        );
        let _ = tx.send(Message::OnQuestAbandon(protocol::QuestAbandonEvent {
            quest_id,
            elapsed_time_in_secs,
        }));
    }
}

pub(crate) fn note_boss_clear(tx: &event::Tx) {
    let Some(state) = snapshot().filter(|s| s.in_survey) else {
        return;
    };
    if !SURVEY_OPEN.load(Ordering::Relaxed) {
        return;
    }

    log::info!(
        "[conflux] boss down: content={:#x} area={}/{} cycle={}/{}",
        state.content_id,
        state.area,
        state.area_count,
        state.cycle,
        state.cycle_count
    );

    let _ = tx.send(Message::OnConfluxBossClear(protocol::ConfluxBossClearEvent {
        content_id: state.content_id,
        area: state.area,
        cycle: state.cycle,
        area_count: state.area_count,
        cycle_count: state.cycle_count,
    }));
}

#[derive(Clone)]
pub struct OnConfluxSurveyStartHook {
    tx: event::Tx,
}

impl OnConfluxSurveyStartHook {
    pub fn new(tx: event::Tx) -> Self {
        OnConfluxSurveyStartHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let mut installed = 0u8;

        if let Ok(addr) = process.search_address_start(ENDLESS_BEGIN_A_SIG) {
            let tx = self.tx.clone();
            unsafe {
                let func: OnEndlessBeginAFunc = std::mem::transmute(addr);
                OnEndlessBeginA.initialize(func, move |state, arg| {
                    let ret = OnEndlessBeginA.call(state, arg);
                    begin_survey(&tx);
                    ret
                })?;
                OnEndlessBeginA.enable()?;
            }
            installed += 1;
        }

        if let Ok(addr) = process.search_address_start(ENDLESS_BEGIN_B_SIG) {
            let tx = self.tx.clone();
            unsafe {
                let func: OnEndlessBeginBFunc = std::mem::transmute(addr);
                OnEndlessBeginB.initialize(func, move || {
                    let ret = OnEndlessBeginB.call();
                    begin_survey(&tx);
                    ret
                })?;
                OnEndlessBeginB.enable()?;
            }
            installed += 1;
        }

        if installed == 0 {
            return Err(anyhow!("Could not find either Conflux survey-start path"));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct OnConfluxSurveyEndHook {
    tx: event::Tx,
}

impl OnConfluxSurveyEndHook {
    pub fn new(tx: event::Tx) -> Self {
        OnConfluxSurveyEndHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let addr = process.search_address_start(ENDLESS_END_SIG)?;

        let tx = self.tx.clone();
        unsafe {
            let func: OnEndlessEndFunc = std::mem::transmute(addr);
            OnEndlessEnd.initialize(func, move |state| {
                // The original zeroes the whole state, so read it first.
                end_survey(&tx, snapshot());
                OnEndlessEnd.call(state)
            })?;
            OnEndlessEnd.enable()?;
        }

        Ok(())
    }
}

/// Startup check for the `ClearEndlessModeQuest` vtable. It reads slot 1, which
/// the RTTI resolver never looks at, so it is not a repeat of resolution.
pub(crate) fn validate_anchors(process: &Process) -> AnchorReport {
    let mut report = AnchorReport::new();
    let (code_lo, code_hi) = process.code_range();

    let entry = clear_endless_quest_execute_slot(process).and_then(safe_read::<usize>);
    report.check(
        entry.is_some_and(|f| (code_lo..code_hi).contains(&f)),
        || {
            format!(
                "{CLEAR_ENDLESS_QUEST_VTABLE_NAME} slot {CLEAR_ENDLESS_QUEST_VTABLE_SLOT} \
                 (conflux.rs — no Conflux per-area boundary)"
            )
        },
    );

    report
}

#[derive(Clone)]
pub struct OnConfluxAreaClearHook {
    tx: event::Tx,
}

impl OnConfluxAreaClearHook {
    pub fn new(tx: event::Tx) -> Self {
        OnConfluxAreaClearHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let addr = process.search_address_start(CLEAR_ENDLESS_QUEST_SIG)?;

        // Fail closed: a wrong address here is a detour on an unrelated
        // function. An unresolved class fails the same way, with no RVA to fall
        // back on.
        let slot = clear_endless_quest_execute_slot(process).ok_or_else(|| {
            anyhow!("{CLEAR_ENDLESS_QUEST_VTABLE_NAME} did not resolve by RTTI name")
        })?;
        match safe_read::<usize>(slot) {
            Some(entry) if entry == addr => {}
            _ => {
                return Err(anyhow!(
                    "ClearEndlessModeQuest vtable slot does not match the resolved address"
                ))
            }
        }

        let tx = self.tx.clone();
        unsafe {
            let func: OnClearEndlessQuestFunc = std::mem::transmute(addr);
            OnClearEndlessQuest.initialize(func, move |node| {
                // Read the area timer before the original restarts the clock.
                let area_elapsed_time_in_secs = area_elapsed_secs();
                let ret = OnClearEndlessQuest.call(node);

                if let Some(state) = snapshot().filter(|s| s.in_survey) {
                    log::info!(
                        "[conflux] area cleared: content={:#x} area={}/{} cycle={}/{} area-time={}s",
                        state.content_id,
                        state.area,
                        state.area_count,
                        state.cycle,
                        state.cycle_count,
                        area_elapsed_time_in_secs
                    );

                    SURVEY_ELAPSED_SECS
                        .fetch_add(area_elapsed_time_in_secs, Ordering::Relaxed);

                    if SURVEY_OPEN.load(Ordering::Relaxed) {
                        let _ = tx.send(Message::OnConfluxAreaClear(
                            protocol::ConfluxAreaClearEvent {
                                content_id: state.content_id,
                                area: state.area,
                                cycle: state.cycle,
                                area_count: state.area_count,
                                cycle_count: state.cycle_count,
                                area_elapsed_time_in_secs,
                            },
                        ));
                    }

                    player_directory::on_encounter_boundary(
                        player_directory::Boundary::ConfluxArea,
                    );
                }
                ret
            })?;
            OnClearEndlessQuest.enable()?;
        }

        Ok(())
    }
}

/// Nothing consumes this event yet, but saved surveys contain it, so the
/// protocol variant has to stay.
#[derive(Clone)]
pub struct OnConfluxAdvanceHook {
    tx: event::Tx,
}

impl OnConfluxAdvanceHook {
    pub fn new(tx: event::Tx) -> Self {
        OnConfluxAdvanceHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let addr = process.search_address_start(ENDLESS_ADVANCE_SIG)?;

        let tx = self.tx.clone();
        unsafe {
            let func: OnEndlessAdvanceFunc = std::mem::transmute(addr);
            OnEndlessAdvance.initialize(func, move |state, arg| {
                let ret = OnEndlessAdvance.call(state, arg);

                if let Some(state) = snapshot().filter(|s| s.in_survey) {
                    log::info!(
                        "[conflux] advanced to: content={:#x} area={}/{} cycle={}/{}",
                        state.content_id,
                        state.area,
                        state.area_count,
                        state.cycle,
                        state.cycle_count
                    );

                    if SURVEY_OPEN.load(Ordering::Relaxed) {
                        let _ = tx.send(Message::OnConfluxAdvance(protocol::ConfluxAdvanceEvent {
                            content_id: state.content_id,
                            area: state.area,
                            cycle: state.cycle,
                            area_count: state.area_count,
                            cycle_count: state.cycle_count,
                        }));
                    }
                }

                ret
            })?;
            OnEndlessAdvance.enable()?;
        }

        Ok(())
    }
}
