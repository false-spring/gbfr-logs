use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use death::OnDeathHook;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;

use crate::{event, process::Process};

use self::{
    area::OnAreaEnterHook,
    damage::{OnProcessDamageHook, OnProcessDotHook},
    player::OnLoadPlayerIdentityHook,
    quest::{OnLoadQuestHook, OnQuestAbandonHook, OnQuestCompleteHook},
    sba::{
        OnAttemptSBAHook, OnCheckSBACollisionHook, OnHandleSBAUpdateHook, OnRemoteSBAUpdateHook,
        OnSBAResetBroadcastHook,
    },
};

pub use self::player::resolve_source_identity;

pub use self::actor::{actor_idx, actor_type_id, get_source_parent};
pub(crate) use self::actor::{
    parent_actor_idx, probe_actor, read_player_key, source_is_local_for_dedup,
    vfunc_slot_readable, ENTITY_SPECIFIED_INSTANCE_OFFSET, INSTANCE_RECORD_OFFSET,
    RECORD_KEY_MIRROR_OFFSET, RECORD_SNAPSHOT_PTR_OFFSET, SNAPSHOT_PARTY_INDEX_OFFSET,
    TYPE_ID_VFUNC_SLOT,
};
pub(crate) use self::gate::is_local_sim_only_class;

mod actor;
mod area;
mod conflux;
mod damage;
mod death;
mod ffi;
mod gate;
mod gbfr_hash;
mod globals;
mod heal;
mod link;
mod network_user;
mod player;
mod player_directory;
mod quest;
mod rtti;
mod sba;
mod sba_cause;
mod status;
mod subname;

macro_rules! setup_or_skip {
    ($hook:expr, $process:expr, $name:literal) => {
        match $hook.setup($process) {
            Ok(()) => log::info!("[hook-install] {} ok", $name),
            Err(e) => log::warn!("[hook-install] {} UNAVAILABLE on this build: {}", $name, e),
        }
    };
}

macro_rules! setup_required {
    ($failures:ident, $hook:expr, $process:expr, $name:literal) => {
        if let Err(e) = $hook.setup($process) {
            $failures.push(($name, e));
        }
    };
}

pub(crate) fn warn_stale_rva(flag: &AtomicBool, what: &str) {
    if !flag.swap(true, Ordering::Relaxed) {
        log::warn!("[anchor] {what} no longer validates — data path degraded");
    }
}

/// Returns the absolute VA of the slot, or `None` if the signature no longer
/// matches. There is no baked address to fall back to, so a caller that gets
/// `None` abstains. `degraded` says what goes missing without it.
pub(crate) fn resolve_global_slot(
    process: &Process,
    signature: &str,
    what: &str,
    degraded: &str,
) -> Option<usize> {
    match process.search_address(signature) {
        Ok(slot) => {
            log::info!(
                "[anchor] {what} resolved to {:#x}",
                slot.wrapping_sub(process.base_address)
            );
            Some(slot)
        }
        Err(e) => {
            log::warn!(
                "[anchor] {what} signature did not resolve ({e}); it is UNAVAILABLE for this \
                 session — {degraded}. See docs/HOOK-REDISCOVERY.md."
            );
            None
        }
    }
}

pub(crate) struct AnchorReport {
    pub checked: usize,
    pub failed: Vec<String>,
}

impl AnchorReport {
    pub(crate) fn new() -> Self {
        AnchorReport { checked: 0, failed: Vec::new() }
    }

    pub(crate) fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.checked += 1;
        if !ok {
            self.failed.push(what());
        }
    }

    pub(crate) fn absorb(&mut self, other: AnchorReport) {
        self.checked += other.checked;
        self.failed.extend(other.failed);
    }
}

fn validate_anchors(process: &Process) {
    let mut report = AnchorReport::new();
    report.absorb(actor::validate_anchors());
    report.absorb(quest::validate_anchors());
    report.absorb(conflux::validate_anchors(process));
    report.absorb(player::validate_anchors(process));

    let ok = report.checked - report.failed.len();
    if report.failed.is_empty() {
        log::info!("[anchor] {ok}/{} validated", report.checked);
    } else {
        log::warn!(
            "[anchor] {ok}/{} validated — FAILED: {}. These are addresses derived at startup \
             from RTTI names and byte signatures; nothing falls back to a baked constant, so \
             the affected data paths are OFF rather than wrong. See docs/HOOK-REDISCOVERY.md.",
            report.checked,
            report.failed.join(", ")
        );
    }
}

pub fn setup_hooks(tx: event::Tx) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    let mut failures: Vec<(&str, anyhow::Error)> = Vec::new();

    if let Err(e) = globals::setup_globals(&process) {
        failures.push(("setup_globals", e));
    }

    // Resolve every class vtable by RTTI name before anything reads one. The
    // quest flow and Conflux installs below compare a resolved function against
    // a vtable slot, and fail closed if the map is empty.
    rtti::setup(&process);
    actor::setup_summon_vtables();

    player::setup_player_globals(&process);

    if let Err(_e) = subname::setup_subname_registry(&process) {
        #[cfg(feature = "console")]
        println!("subname registry unavailable on this build: {}", _e);
    }

    /* Damage Events */
    setup_required!(failures, OnProcessDamageHook::new(tx.clone()), &process, "on_process_damage");
    setup_required!(failures, OnProcessDotHook::new(tx.clone()), &process, "on_process_dot");
    setup_required!(failures, OnDeathHook::new(tx.clone()), &process, "on_death");

    /* Player Data */
    setup_or_skip!(
        OnLoadPlayerIdentityHook::new(tx.clone()),
        &process,
        "on_load_player_identity"
    );

    setup_or_skip!(
        network_user::OnPopulateNetworkUserHook::new(),
        &process,
        "populate_network_user"
    );

    /* Quest + Area Tracking */
    setup_required!(failures, OnAreaEnterHook::new(tx.clone()), &process, "on_area_enter");
    setup_required!(failures, OnLoadQuestHook::new(tx.clone()), &process, "on_load_quest");
    setup_required!(failures, OnQuestCompleteHook::new(tx.clone()), &process, "on_quest_complete");
    setup_or_skip!(
        OnQuestAbandonHook::new(tx.clone()),
        &process,
        "on_quest_abandon"
    );
    // Fires at boss death, once per cleared run, quest repeats included. It gets
    // there before `on_quest_complete`, which stays installed as a latched backup.
    setup_or_skip!(
        quest::OnQuestStateFunnelHook::new(tx.clone()),
        &process,
        "quest_state_funnel"
    );

    if let Err(_e) = quest::setup_quest_state_global(&process) {
        #[cfg(feature = "console")]
        println!("Quest-state global unavailable on this build: {}", _e);
    }

    if let Err(_e) = sba::setup_chain_burst_manager(&process) {
        #[cfg(feature = "console")]
        println!("Chain-burst manager unavailable on this build: {}", _e);
    }

    /* Conflux (endless mode) run structure */
    if let Err(_e) = conflux::setup_conflux_state(&process) {
        #[cfg(feature = "console")]
        println!("Conflux run state unavailable on this build: {}", _e);
    }
    setup_or_skip!(
        conflux::OnConfluxSurveyStartHook::new(tx.clone()),
        &process,
        "on_conflux_survey_start"
    );
    setup_or_skip!(
        conflux::OnConfluxSurveyEndHook::new(tx.clone()),
        &process,
        "on_conflux_survey_end"
    );
    setup_or_skip!(
        conflux::OnConfluxAreaClearHook::new(tx.clone()),
        &process,
        "on_conflux_area_clear"
    );
    setup_or_skip!(
        conflux::OnConfluxAdvanceHook::new(tx.clone()),
        &process,
        "on_conflux_advance"
    );

    /* SBA */
    // With SBA_OFFSET unresolved the gauge hooks take the gauge struct itself
    // for the entity and call through garbage. Refuse the install.
    if globals::SBA_OFFSET.load(Ordering::Relaxed) == 0 {
        failures.push((
            "sba_offset",
            anyhow::anyhow!("unresolved; the two gauge hooks were not installed"),
        ));
    } else {
        setup_required!(failures, OnHandleSBAUpdateHook::new(tx.clone()), &process, "on_handle_sba_update");
        setup_required!(failures, OnRemoteSBAUpdateHook::new(tx.clone()), &process, "on_remote_sba_update");
    }
    setup_required!(failures, OnAttemptSBAHook::new(tx.clone()), &process, "on_attempt_sba");
    setup_required!(failures, OnCheckSBACollisionHook::new(tx.clone()), &process, "on_check_sba_collision");
    setup_or_skip!(
        OnSBAResetBroadcastHook::new(tx.clone()),
        &process,
        "on_sba_reset_broadcast"
    );

    setup_or_skip!(sba_cause::OnRegisterHitHook::new(), &process, "on_register_hit");
    setup_or_skip!(
        sba_cause::OnPerfectBlockEffectsHook::new(),
        &process,
        "on_perfect_block_effects"
    );
    setup_or_skip!(
        sba_cause::OnPerfectDodgeEffectsHook::new(),
        &process,
        "on_perfect_dodge_effects"
    );
    setup_or_skip!(sba_cause::OnTakeHitHook::new(), &process, "on_take_hit");
    setup_or_skip!(sba_cause::OnSbaChainGrantHook::new(), &process, "on_sba_chain_grant");
    setup_or_skip!(
        sba_cause::OnRemoteDodgeReplayHook::new(tx.clone()),
        &process,
        "on_remote_dodge_replay"
    );
    setup_or_skip!(
        sba_cause::OnRemoteBlockReplayHook::new(tx.clone()),
        &process,
        "on_remote_block_replay"
    );

    setup_or_skip!(
        link::OnLinkTimeStartHook::new(tx.clone()),
        &process,
        "on_link_time_start"
    );
    setup_or_skip!(
        link::OnLinkTimeEndHook::new(tx.clone()),
        &process,
        "on_link_time_end"
    );
    setup_or_skip!(
        link::OnLinkAttackChanceHook::new(tx.clone()),
        &process,
        "on_link_attack_chance"
    );

    setup_or_skip!(
        status::OnStatusApplyHook::new(tx.clone()),
        &process,
        "on_status_apply"
    );
    setup_or_skip!(
        status::OnStatusDestroyHook::new(tx.clone()),
        &process,
        "on_status_destroy"
    );
    setup_or_skip!(
        status::OnStatusClearAllHook::new(tx.clone()),
        &process,
        "on_status_clear_all"
    );
    setup_or_skip!(
        status::OnStatusStacksChangedHook::new(tx.clone()),
        &process,
        "on_status_stacks_changed"
    );
    setup_or_skip!(
        status::OnStatusGrantHook::new(tx.clone()),
        &process,
        "on_status_grant"
    );

    setup_or_skip!(heal::OnHealHook::new(tx.clone()), &process, "on_heal");

    validate_anchors(&process);

    if failures.is_empty() {
        Ok(())
    } else {
        let detail = failures
            .iter()
            .map(|(name, err)| format!("{name}: {err}"))
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow::anyhow!(
            "{} required hook(s) failed to install: {}",
            failures.len(),
            detail
        ))
    }
}

#[inline(always)]
pub unsafe fn v_func<T: Sized>(ptr: *const usize, offset: usize) -> T {
    ((ptr.read() as *const usize).byte_add(offset) as *const T).read()
}

/// Guarded read: a bad address returns `None` instead of faulting the game.
pub(crate) fn safe_read<T: Copy>(addr: *const T) -> Option<T> {
    if addr.is_null() {
        return None;
    }

    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut bytes_read: usize = 0;
    let size = std::mem::size_of::<T>();

    let ok = unsafe {
        ReadProcessMemory(
            HANDLE(-1),
            addr as *const std::ffi::c_void,
            value.as_mut_ptr() as *mut std::ffi::c_void,
            size,
            Some(&mut bytes_read as *mut usize),
        )
    };

    if ok.is_ok() && bytes_read == size {
        Some(unsafe { value.assume_init() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::AnchorReport;

    #[test]
    fn a_report_counts_every_check() {
        let mut report = AnchorReport::new();
        report.check(true, || "a".to_string());
        report.check(false, || "b".to_string());
        report.check(true, || "c".to_string());

        assert_eq!(report.checked, 3);
        assert_eq!(report.failed, vec!["b".to_string()]);
    }

    #[test]
    fn a_passing_check_never_formats_its_label() {
        let mut report = AnchorReport::new();
        report.check(true, || panic!("label formatted for a passing check"));
        assert_eq!(report.checked, 1);
        assert!(report.failed.is_empty());
    }

    #[test]
    fn absorbing_sums_counts_and_concatenates_failures() {
        let mut a = AnchorReport::new();
        a.check(true, || unreachable!());
        a.check(false, || "from-a".to_string());

        let mut b = AnchorReport::new();
        b.check(false, || "from-b".to_string());

        a.absorb(b);

        assert_eq!(a.checked, 3);
        assert_eq!(a.failed, vec!["from-a".to_string(), "from-b".to_string()]);
    }
}
