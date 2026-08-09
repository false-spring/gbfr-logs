use std::mem::offset_of;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::{
    event,
    hooks::{
        ffi::QuestState, globals::QUEST_STATE_PTR, player_directory, rtti, safe_read, AnchorReport,
    },
    process::Process,
};

type OnLoadQuestStateFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnQuestEndFinalizerFunc = unsafe extern "system" fn(*const usize) -> usize;
// rcx = flow, dl = deleting-flag. MUST be 2-arg so the flag reaches the
// original, or the flow object is leaked / double-freed.
type OnFlowDtorFunc = unsafe extern "system" fn(*const usize, usize) -> usize;
type OnTrialQuitFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnTrialTeardownFunc = unsafe extern "system" fn(*const usize, usize, usize) -> usize;

// The QM session-state funnel (RVA 0x634830). rcx = the quest singleton, rdx =
// a pointer to the dispatched 32-bit session-state id, r8d and r9b are flags.
// The body reads those four registers and no caller stack, so 4 args is safe.
type OnQuestStateFunnelFunc = unsafe extern "system" fn(*const usize, *const u32, u32, u32) -> usize;

static_detour! {
    static OnLoadQuestState: unsafe extern "system" fn(*const usize) -> usize;
    static OnQuestEndFinalizer: unsafe extern "system" fn(*const usize) -> usize;
    static OnFlowDtorQuest: unsafe extern "system" fn(*const usize, usize) -> usize;
    static OnFlowDtorEndless: unsafe extern "system" fn(*const usize, usize) -> usize;
    static OnFlowDtorFate: unsafe extern "system" fn(*const usize, usize) -> usize;
    static OnTrialQuit: unsafe extern "system" fn(*const usize) -> usize;
    static OnTrialTeardown: unsafe extern "system" fn(*const usize, usize, usize) -> usize;
}

static_detour! {
    static OnQuestStateFunnel: unsafe extern "system" fn(*const usize, *const u32, u32, u32) -> usize;
}

// Set by the funnel at boss death so the finalizer's later emit stays quiet.
static QUEST_COMPLETE_EMITTED: AtomicBool = AtomicBool::new(false);

static LAST_LOADED_QUEST_ID: AtomicU32 = AtomicU32::new(0);

const ON_LOAD_QUEST_STATE: &str =
    "48 8b 0d ? ? ? ? e8 $ { ' } c5 fb 12 ? ? ? ? ? c5 f8 11 ? ? ? ? ? c5 f8 11 ? ? ? ? ?";
const QUEST_END_FINALIZER_SIG: &str =
    "48 89 f1 e8 $ { ' } b8 00 00 f0 00 23 46 40 3d 00 00 60 00";
const FLOW_DTOR_QUEST_SIG: &str =
    "56 57 48 83 ec 28 89 d7 48 89 ce e8 c0 fa ff ff 85 ff 74 08 48 89 f1 e8 50 e3 0d 04";
const FLOW_DTOR_ENDLESS_SIG: &str =
    "55 56 57 53 48 83 ec 38 48 8d 6c 24 30 48 c7 45 00 fe ff ff ff 89 d7 48 89 ce 48 8d 05 ? ? ? ? 48 89 81 10 08 00 00";
const FLOW_QUEST_VTABLE_NAME: &str = ".?AVReceptionQuestFlow@quest@stage@@";
const FLOW_ENDLESS_VTABLE_NAME: &str = ".?AVReceptionEndlessModeFlow@quest@stage@@";
const FLOW_FATE_VTABLE_NAME: &str = ".?AVReceptionFateEpisodeFlow@quest@stage@@";
pub(crate) const FLOW_VTABLE_CLASSES: &[&str] = &[
    FLOW_QUEST_VTABLE_NAME,
    FLOW_ENDLESS_VTABLE_NAME,
    FLOW_FATE_VTABLE_NAME,
];
const FLOW_DTOR_FATE_SIG: &str =
    "56 57 48 83 ec 28 89 d7 48 89 ce e8 40 e5 ff ff 85 ff 74 08 48 89 f1 e8 f0 df 0d 04";

const TRIAL_TEARDOWN_CALLSITE_SIG: &str =
    "89 be e0 3d 06 00 48 89 f1 31 d2 41 b0 01 e8 $ { ' } c6 86 92 3a 06 00 00";
const TRIAL_TEARDOWN_PROLOGUE_SIG: &str =
    "55 41 57 41 56 41 55 41 54 56 57 53 48 81 ec 88 00 00 00 48 8d ac 24 80 00 00 00 c5 f8 29 75 f0 48 c7 45 e8 fe ff ff ff 44 88 45 d7 88 55 c8 49 89 cd ba 05 00 00 00";

const TRIAL_QUIT_PROLOGUE_SIG: &str =
    "56 57 55 53 48 83 ec 28 48 8b 81 a0 00 00 00 80 78 48 00 0f 84 99 01 00 00";
const TRIAL_QUIT_CALLSITE_SIG: &str =
    "48 8b 35 ? ? ? ? 0f b6 59 60 48 8b 0d ? ? ? ? e8 $ { ' } 88 9e 0e 01 00 00 80 3d ? ? ? ? 05";

const QUEST_STATE_FUNNEL_SIG: &str =
    "48 8b 0d ? ? ? ? 48 89 fa 45 89 e8 45 31 c9 e8 $ { ' } 48 8b 0d";

const QUEST_STATE_GLOBAL_SIG: &str =
    "48 8b 0d $ { ' } e8 ? ? ? ? c5 fb 12 05 ? ? ? ? c5 f8 11 87 94 06 00 00";

static QUEST_STATE_GLOBAL_SLOT: AtomicUsize = AtomicUsize::new(0);

pub fn setup_quest_state_global(process: &Process) -> Result<()> {
    let slot = process.search_address(QUEST_STATE_GLOBAL_SIG)?;
    QUEST_STATE_GLOBAL_SLOT.store(slot, Ordering::Relaxed);

    #[cfg(feature = "console")]
    println!("Found quest-state global: {:#x}", slot);

    Ok(())
}

pub(crate) fn quest_state() -> Option<*const QuestState> {
    let cached = QUEST_STATE_PTR.load(Ordering::Relaxed);
    if !cached.is_null() {
        return Some(cached);
    }

    let slot_va = QUEST_STATE_GLOBAL_SLOT.load(Ordering::Relaxed);
    if slot_va == 0 {
        return None;
    }
    safe_read(slot_va as *const usize)
        .filter(|&p| p != 0)
        .map(|p| p as *const QuestState)
}

const QM_TRIAL_MANAGER_OFFSET: usize = 0xA0;
const TRIAL_MGR_ARMED_OFFSET: usize = 0x48;
const TRIAL_MGR_DELEGATE_COUNT_OFFSET: usize = 0x20;

/// Called while loading into a quest.
#[derive(Clone)]
pub struct OnLoadQuestHook {
    tx: event::Tx,
}

impl OnLoadQuestHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLoadQuestHook { tx }
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

        let prev_quest_id = LAST_LOADED_QUEST_ID.load(Ordering::Relaxed);
        if prev_quest_id != 0 {
            let _ = self.tx.send(Message::OnQuestAbandon(protocol::QuestAbandonEvent {
                quest_id: prev_quest_id,
                elapsed_time_in_secs: 0,
            }));
        }

        let ret = unsafe { OnLoadQuestState.call(a1) };
        // a1 is the quest manager singleton and is itself the QuestState base.
        let quest_state_ptr = a1 as *mut QuestState;

        if quest_state_ptr.is_null() {
            return ret;
        }

        QUEST_STATE_PTR.store(quest_state_ptr, std::sync::atomic::Ordering::Relaxed);

        QUEST_COMPLETE_EMITTED.store(false, Ordering::Relaxed);

        let new_quest_id = unsafe { quest_state_ptr.read() }.quest_id;
        LAST_LOADED_QUEST_ID.store(new_quest_id, Ordering::Relaxed);

        if new_quest_id != 0 {
            let _ = self.tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
                last_known_quest_id: new_quest_id,
                last_known_elapsed_time_in_secs: 0,
            }));
        }

        player_directory::on_encounter_boundary(player_directory::Boundary::QuestLoad);

        ret
    }
}

/// Called on the Quest Cleared screen. It fires exactly once per clear,
/// including on quest repeats, and never on a retire or abandon.
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

        if let Ok(quest_end_finalizer) = process.search_address(QUEST_END_FINALIZER_SIG) {
            #[cfg(feature = "console")]
            println!("Found quest-end finalizer");

            unsafe {
                let func: OnQuestEndFinalizerFunc = std::mem::transmute(quest_end_finalizer);
                OnQuestEndFinalizer.initialize(func, move |flow| cloned_self.run(flow))?;
                OnQuestEndFinalizer.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find quest-end finalizer"));
        }

        Ok(())
    }

    fn run(&self, flow: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("quest-end finalizer");

        // Let the game finalize first: this call freezes the clear time and sets
        // the freeze latch, so the fields read below are only final afterwards.
        let ret = unsafe { OnQuestEndFinalizer.call(flow) };

        let quest_state_ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);
        if quest_state_ptr.is_null() {
            return ret;
        }

        // A Conflux survey is one encounter; conflux.rs saves it, not this hook.
        if super::conflux::in_survey() {
            return ret;
        }

        let quest_state = unsafe { quest_state_ptr.read() };
        if quest_state.freeze_flag == 1
            && quest_state.quest_id != 0
            && !QUEST_COMPLETE_EMITTED.swap(true, Ordering::Relaxed)
        {
            let _ = self
                .tx
                .send(Message::OnQuestComplete(protocol::QuestCompleteEvent {
                    quest_id: quest_state.quest_id,
                    elapsed_time_in_secs: quest_state.elapsed_time,
                }));
        }

        ret
    }
}

/// Called when a quest is torn down without completing: a retire or abandon
/// from the pause menu, a forced abandon, or a network leave. The freeze latch
/// tells those apart from a completion: 0 = abandon, 1 = completion dismissal.
#[derive(Clone)]
pub struct OnQuestAbandonHook {
    tx: event::Tx,
}

impl OnQuestAbandonHook {
    pub fn new(tx: event::Tx) -> Self {
        OnQuestAbandonHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let mut installed = 0u8;

        if let Some(addr) =
            resolve_validated_dtor(process, FLOW_DTOR_QUEST_SIG, FLOW_QUEST_VTABLE_NAME)
        {
            #[cfg(feature = "console")]
            println!("Found quest-flow dtor (ReceptionQuestFlow)");

            let cloned_self = self.clone();
            unsafe {
                let func: OnFlowDtorFunc = std::mem::transmute(addr);
                OnFlowDtorQuest.initialize(func, move |flow, del_flag| {
                    let snapshot = snapshot_quest_state();
                    let ret = OnFlowDtorQuest.call(flow, del_flag);
                    cloned_self.emit_abandon_if_left(snapshot);
                    ret
                })?;
                OnFlowDtorQuest.enable()?;
            }
            installed += 1;
        }

        if let Some(addr) =
            resolve_validated_dtor(process, FLOW_DTOR_ENDLESS_SIG, FLOW_ENDLESS_VTABLE_NAME)
        {
            #[cfg(feature = "console")]
            println!("Found quest-flow dtor (ReceptionEndlessModeFlow)");

            let cloned_self = self.clone();
            unsafe {
                let func: OnFlowDtorFunc = std::mem::transmute(addr);
                OnFlowDtorEndless.initialize(func, move |flow, del_flag| {
                    let snapshot = snapshot_quest_state();
                    let ret = OnFlowDtorEndless.call(flow, del_flag);
                    cloned_self.emit_abandon_if_left(snapshot);
                    ret
                })?;
                OnFlowDtorEndless.enable()?;
            }
            installed += 1;
        }

        if let Some(addr) =
            resolve_validated_dtor(process, FLOW_DTOR_FATE_SIG, FLOW_FATE_VTABLE_NAME)
        {
            #[cfg(feature = "console")]
            println!("Found quest-flow dtor (ReceptionFateEpisodeFlow)");

            let cloned_self = self.clone();
            unsafe {
                let func: OnFlowDtorFunc = std::mem::transmute(addr);
                OnFlowDtorFate.initialize(func, move |flow, del_flag| {
                    let snapshot = snapshot_quest_state();
                    let ret = OnFlowDtorFate.call(flow, del_flag);
                    cloned_self.emit_abandon_if_left(snapshot);
                    ret
                })?;
                OnFlowDtorFate.enable()?;
            }
            installed += 1;
        }

        if let (Ok(callsite), Ok(prologue)) = (
            process.search_address(TRIAL_QUIT_CALLSITE_SIG),
            process.search_address_start(TRIAL_QUIT_PROLOGUE_SIG),
        ) {
            if callsite == prologue {
                #[cfg(feature = "console")]
                println!("Found trial-quit choke point (training room)");

                let cloned_self = self.clone();
                unsafe {
                    let func: OnTrialQuitFunc = std::mem::transmute(callsite);
                    OnTrialQuit.initialize(func, move |qm| {
                        adopt_quest_state_ptr(qm);

                        // Emit first: the original may not return.
                        if trial_is_armed(qm) {
                            #[cfg(feature = "console")]
                            println!("trial quit -> emitting abandon");

                            let _ = cloned_self.tx.send(Message::OnQuestAbandon(
                                protocol::QuestAbandonEvent {
                                    quest_id: 0,
                                    elapsed_time_in_secs: 0,
                                },
                            ));
                        }

                        OnTrialQuit.call(qm)
                    })?;
                    OnTrialQuit.enable()?;
                }
                installed += 1;
            }
        }

        // quest_id 0 and elapsed 0 tell the parser to keep its own id and timer.
        if let (Ok(callsite), Ok(prologue)) = (
            process.search_address(TRIAL_TEARDOWN_CALLSITE_SIG),
            process.search_address_start(TRIAL_TEARDOWN_PROLOGUE_SIG),
        ) {
            if callsite == prologue {
                #[cfg(feature = "console")]
                println!("Found trial session-teardown (fires at trial START, e.g. Restart)");

                let cloned_self = self.clone();
                unsafe {
                    let func: OnTrialTeardownFunc = std::mem::transmute(callsite);
                    OnTrialTeardown.initialize(func, move |qm, completed, force| {
                        adopt_quest_state_ptr(qm);

                        #[cfg(feature = "console")]
                        println!("trial teardown -> emitting abandon");

                        let _ = cloned_self.tx.send(Message::OnQuestAbandon(
                            protocol::QuestAbandonEvent {
                                quest_id: 0,
                                elapsed_time_in_secs: 0,
                            },
                        ));

                        OnTrialTeardown.call(qm, completed, force)
                    })?;
                    OnTrialTeardown.enable()?;
                }
                installed += 1;
            }
        }

        if installed == 0 {
            return Err(anyhow!("Could not find any quest-over hook"));
        }

        Ok(())
    }

    fn emit_abandon_if_left(&self, snapshot: Option<QuestState>) {
        if super::conflux::in_survey() {
            return;
        }

        let (quest_id, elapsed_time_in_secs) = match snapshot {
            Some(quest_state) => (
                quest_state.quest_id,
                if quest_state.freeze_flag == 0 {
                    quest_state.elapsed_time
                } else {
                    0
                },
            ),
            None => (0, 0),
        };

        let _ = self
            .tx
            .send(Message::OnQuestAbandon(protocol::QuestAbandonEvent {
                quest_id,
                elapsed_time_in_secs,
            }));
    }
}

// Safe during the dtor: the singleton is a global, not the flow being destroyed.
fn snapshot_quest_state() -> Option<QuestState> {
    let ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);
    (!ptr.is_null()).then(|| unsafe { ptr.read() })
}

/// Caches the QM singleton for a parser that attached mid-quest.
fn adopt_quest_state_ptr(qm: *const usize) {
    if qm.is_null() || !QUEST_STATE_PTR.load(Ordering::Relaxed).is_null() {
        return;
    }

    let Some(quest_id) = safe_read::<u32>(
        (qm as *const u8).wrapping_add(offset_of!(QuestState, quest_id)) as *const u32,
    ) else {
        return;
    };

    QUEST_STATE_PTR.store(qm as *mut QuestState, Ordering::Relaxed);

    if quest_id != 0 {
        let _ = LAST_LOADED_QUEST_ID.compare_exchange(
            0,
            quest_id,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    #[cfg(feature = "console")]
    println!("adopted quest state singleton (quest={quest_id:x}) — attached mid-quest");
}

/// The primary per-clear quest-complete signal. It fires first, at boss death,
/// once per cleared run and once per quest-repeat iteration. Emit gate: the
/// freeze latch is set, the quest id is live, and the dispatched state id equals
/// it (during a quest the session state holds the quest id itself, the 0x4xxxxx
/// family). It sets `QUEST_COMPLETE_EMITTED` instead of reading it.
#[derive(Clone)]
pub struct OnQuestStateFunnelHook {
    tx: event::Tx,
}

impl OnQuestStateFunnelHook {
    pub fn new(tx: event::Tx) -> Self {
        OnQuestStateFunnelHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let funnel = process.search_address(QUEST_STATE_FUNNEL_SIG)?;
        #[cfg(feature = "console")]
        println!("Found quest session-state funnel");

        let cloned_self = self.clone();
        unsafe {
            let func: OnQuestStateFunnelFunc = std::mem::transmute(funnel);
            OnQuestStateFunnel
                .initialize(func, move |qm, state_ptr, r8, r9| {
                    cloned_self.run(qm, state_ptr, r8, r9)
                })?;
            OnQuestStateFunnel.enable()?;
        }
        Ok(())
    }

    fn run(&self, qm: *const usize, state_ptr: *const u32, r8: u32, r9: u32) -> usize {
        adopt_quest_state_ptr(qm);

        let new_state = safe_read::<u32>(state_ptr).unwrap_or(0);
        // The original zeroes these on teardown paths, so snapshot first.
        let snapshot = snapshot_quest_state();

        #[cfg(feature = "console")]
        {
            let base = qm as *const u8;
            let quest_id = safe_read::<u32>(base.wrapping_add(0xDC8) as *const u32).unwrap_or(0);
            let elapsed = safe_read::<u32>(base.wrapping_add(0xAC8) as *const u32).unwrap_or(0);
            let freeze = safe_read::<u8>(base.wrapping_add(0xADC)).unwrap_or(0xFF);
            let dd4 = safe_read::<u32>(base.wrapping_add(0xDD4) as *const u32).unwrap_or(0xFFFF_FFFF);
            let e2c = safe_read::<u32>(base.wrapping_add(0xE2C) as *const u32).unwrap_or(0xFFFF_FFFF);
            println!(
                "QSTATE -> {new_state:06x} (r8={r8:x} r9={r9:x}) | quest={quest_id:x} t={elapsed}s frz={freeze} dd4={dd4} e2c={e2c}"
            );
        }

        let ret = unsafe { OnQuestStateFunnel.call(qm, state_ptr, r8, r9) };

        if let Some(quest_state) = snapshot {
            if quest_state.freeze_flag == 1
                && quest_state.quest_id != 0
                && new_state == quest_state.quest_id
            {
                if super::conflux::in_survey() {
                    super::conflux::note_boss_clear(&self.tx);
                    return ret;
                }

                QUEST_COMPLETE_EMITTED.store(true, Ordering::Relaxed);
                let _ = self
                    .tx
                    .send(Message::OnQuestComplete(protocol::QuestCompleteEvent {
                        quest_id: quest_state.quest_id,
                        elapsed_time_in_secs: quest_state.elapsed_time,
                    }));

                // The game reallocates the party's actor instances per repeat, so
                // a stale by-instance entry would hand a new player a prior
                // owner's id. Cleared after the emit, once the parser is Stopped,
                // so tail events under fresh ids mint no ghost rows.
                player_directory::on_encounter_boundary(player_directory::Boundary::RepeatRun);

                #[cfg(feature = "console")]
                println!(
                    "QSTATE emitted quest complete (quest={:x} t={}s)",
                    quest_state.quest_id, quest_state.elapsed_time
                );
            }
        }

        ret
    }
}

fn trial_is_armed(qm: *const usize) -> bool {
    let Some(mgr) = safe_read::<usize>(
        (qm as *const u8).wrapping_add(QM_TRIAL_MANAGER_OFFSET) as *const usize,
    ) else {
        return false;
    };
    if mgr == 0 {
        return false;
    }

    let mgr = mgr as *const u8;
    let armed = safe_read::<u8>(mgr.wrapping_add(TRIAL_MGR_ARMED_OFFSET)).unwrap_or(0);
    let delegates = safe_read::<usize>(
        mgr.wrapping_add(TRIAL_MGR_DELEGATE_COUNT_OFFSET) as *const usize,
    )
    .unwrap_or(0);

    armed != 0 && delegates != 0
}

/// Startup check that all three quest-flow classes resolved. A missing one
/// takes its dtor hook with it, and `setup` still returns `Ok`.
pub(crate) fn validate_anchors() -> AnchorReport {
    let mut report = AnchorReport::new();

    let missing = rtti::unresolved(FLOW_VTABLE_CLASSES);
    report.check(missing.is_empty(), || {
        format!(
            "{}/{} quest-flow classes unresolved: {} (quest.rs — no abandon/retire save \
             for that flow)",
            missing.len(),
            FLOW_VTABLE_CLASSES.len(),
            missing.join(", ")
        )
    });

    report
}

/// Resolves a flow dtor by AOB and checks it against slot 0 of the class vtable,
/// which is located by RTTI name. `None` means skip the install: no match, an
/// unresolved class, or a mismatch. Hooking a wrong address crashes the game.
fn resolve_validated_dtor(process: &Process, sig: &str, class: &'static str) -> Option<usize> {
    let addr = process.search_address_start(sig).ok()?;

    let Some(vtable_rva) = rtti::resolved_vtable(class) else {
        log::warn!(
            "[rtti] {class} did not resolve; its flow dtor is NOT hooked — \
             a retire/abandon of that quest kind saves no log"
        );
        return None;
    };

    match safe_read::<usize>((process.base_address + vtable_rva) as *const usize) {
        Some(slot0) if slot0 == addr => Some(addr),
        other => {
            log::warn!(
                "[rtti] {class} vtable {vtable_rva:#x} slot 0 is {other:x?}, but the dtor AOB \
                 resolved {:#x}; NOT hooking it",
                addr.wrapping_sub(process.base_address)
            );
            None
        }
    }
}
