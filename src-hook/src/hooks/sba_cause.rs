//! Attribution for SBA gauge gains.
//!
//! The SBA gauge delta itself is measured in `sba.rs` by straddling the game's
//! update call, which is exact. What that call cannot tell us is *why* the SBA
//! gauge moved: its `a2` is the requested add (rescaled by three modifiers and
//! clamped before it lands) and its `a3` is a 0/1 scaling switch that is 0 at
//! 18 of its 21 call sites. Neither identifies a cause.
//!
//! So the cause is captured one level up instead. Detours on the functions that
//! *produce* a gain park what they know on a thread-local for the duration of
//! the call they wrap; `OnHandleSBAUpdateHook` reads it back when the gauge
//! actually moves. Seven producers are modelled:
//!
//! |                     detour | covers                                       |
//! | -------------------------- | -------------------------------------------- |
//! |          `on_register_hit` | generating SBA by hitting the enemy          |
//! | `on_perfect_block_effects` | perfect guard, credited to 611               |
//! | `on_perfect_dodge_effects` | perfect dodge, credited to 610               |
//! |              `on_take_hit` | generating SBA by being hit (1%/0.7%)        |
//! |       `on_sba_chain_grant` | the flat +10% when someone else lands an SBA |
//! |   `on_remote_dodge_replay` | a peer's perfect dodge, replayed locally     |
//! |   `on_remote_block_replay` | a peer's perfect guard, replayed locally     |
//!
//! Only the two replay hooks take a `tx` and emit an event of their own; the
//! other five emit nothing and just park a cause on the thread-local.
//!
//! Everything else resolves to a named `SbaCause` variant rather than a guess.
//! Under-reporting attribution is recoverable with better heuristics.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use protocol::SbaCause;
use retour::static_detour;

use crate::process::Process;

use super::{
    damage, parent_actor_idx, safe_read, vfunc_slot_readable, ENTITY_SPECIFIED_INSTANCE_OFFSET,
    TYPE_ID_VFUNC_SLOT,
};

/// The perfect-dodge counter's internal shot id (0x262/610), sibling of
/// `damage::PERFECT_BLOCK_COUNTER_ACTION_ID` (0x263/611).
/// Synthesized (replacing the usual DamageInstance::action_id of -1)
/// so that it gets logged and rendered correctly.
pub(crate) const PERFECT_DODGE_COUNTER_ACTION_ID: u32 = 0x262;

const DAMAGE_SOURCE_ENTITY_OFFSET: usize = 0x18;
const PDODGE_PBLOCK_OWNER_OFFSET: usize = 0x10;

const ON_REGISTER_HIT_SIG: &str = "41 57 41 56 41 55 41 54 56 57 55 53 48 83 ec 38 \
     80 ba 58 01 00 00 00 0f 85 ? ? ? ? c5 fa 10 05 ? ? ? ?";

const ON_PERFECT_BLOCK_EFFECTS_SIG: &str = "55 41 57 41 56 56 57 53 48 81 ec 68 04 00 00 \
     48 8d ac 24 80 00 00 00 c5 78 29 85 d0 03 00 00";
const ON_PERFECT_DODGE_EFFECTS_SIG: &str = "55 41 56 56 57 53 48 81 ec 30 02 00 00 \
     48 8d ac 24 80 00 00 00 c5 78 29 95 a0 01 00 00";

const ON_TAKE_HIT_SIG: &str = "55 41 57 41 56 41 55 41 54 56 57 53 48 81 ec 58 04 00 00      48 8d ac 24 80 00 00 00 c5 78 29 bd c0";
const ON_FLAT_GRANT_SIG: &str =
    "55 41 56 56 57 53 48 81 ec 90 01 00 00 48 8d ac 24 80 00 00 00 c5 f8 29";
const ON_REMOTE_DODGE_REPLAY_SIG: &str = "55 56 57 48 81 ec 80 01 00 00 48 8d ac 24 80 00 00 00 \
     c5 f8 29 b5 f0 00 00 00 48 c7 85 e8 00 00 00 fe ff ff ff 48 89 ce c5 f8 57 c0 c5 f8 29 45";
const ON_REMOTE_BLOCK_REPLAY_SIG: &str = "55 56 57 48 81 ec 90 01 00 00 48 8d ac 24 80 00 00 00 \
     c5 f8 29 b5 00 01 00 00 48 c7 85 f8 00 00 00 fe ff ff ff 48 89 ce c5 f8 57 c0 c5 f8 29 45";
const REMOTE_DODGE_OWNER_OFFSET: usize = 0x56F0;

type OnRegisterHitFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;
type OnCounterEffectsFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;
type OnSingleArgFunc = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static OnRegisterHit: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnPerfectBlockEffects: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnPerfectDodgeEffects: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnTakeHit: unsafe extern "system" fn(*const usize, *const usize, u32, u64) -> usize;
    static OnFlatGrant: unsafe extern "system" fn(
        *const usize, f32, u32, u32, u64, u64, u64, u64) -> usize;
    static OnRemoteDodgeReplay: unsafe extern "system" fn(*const usize) -> usize;
    static OnRemoteBlockReplay: unsafe extern "system" fn(*const usize) -> usize;
}

static GATE_INSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum Pending {
    Hit(*const usize),
    PerfectBlock(*const usize),
    PerfectDodge(*const usize),
    DamageTaken,
    FlatGrant,
}

thread_local! {
    static SBA_CAUSE: Cell<Option<Pending>> = const { Cell::new(None) };
}

// Used to disambiguate different causes in DamageEvent
struct CauseGuard(Option<Pending>);

impl CauseGuard {
    #[inline(always)]
    fn set(pending: Pending) -> Self {
        CauseGuard(SBA_CAUSE.with(|c| c.replace(Some(pending))))
    }
}

impl Drop for CauseGuard {
    #[inline(always)]
    fn drop(&mut self) {
        SBA_CAUSE.with(|c| c.set(self.0));
    }
}

pub(crate) fn resolve_remote(_gaining_idx: u32) -> SbaCause {
    SbaCause::Remote
}

fn grouping_idx_of(ptr: *const usize) -> Option<u32> {
    if ptr.is_null() || !vfunc_slot_readable(ptr, TYPE_ID_VFUNC_SLOT) {
        return None;
    }
    Some(parent_actor_idx(ptr))
}

fn owner_grouping_idx(base: *const u8, offset: usize, hop_specified: bool) -> Option<u32> {
    let mut ptr =
        safe_read::<*const usize>(base.wrapping_add(offset) as *const *const usize).filter(|p| !p.is_null())?;

    if hop_specified {
        ptr = safe_read::<*const usize>(
            (ptr as *const u8).wrapping_add(ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const *const usize,
        )
        .filter(|p| !p.is_null())?;
    }

    // `parent_actor_idx` walks the vtable raw; refuse to hand it anything whose
    // type slot doesn't read back.
    if !vfunc_slot_readable(ptr, TYPE_ID_VFUNC_SLOT) {
        return None;
    }

    Some(parent_actor_idx(ptr))
}

pub(crate) fn resolve_for(gaining_idx: u32) -> SbaCause {
    let Some(pending) = SBA_CAUSE.with(|c| c.get()) else {
        return if GATE_INSTALLED.load(Ordering::Relaxed) {
            SbaCause::Unknown
        } else {
            SbaCause::HookUnavailable
        };
    };

    match pending {
        Pending::Hit(instance) => resolve_hit(instance, gaining_idx),
        Pending::PerfectBlock(sub) => resolve_counter(
            owner_grouping_idx(sub as *const u8, PDODGE_PBLOCK_OWNER_OFFSET, false),
            gaining_idx,
            damage::PERFECT_BLOCK_COUNTER_ACTION_ID,
            false,
        ),
        Pending::PerfectDodge(sub) => resolve_counter(
            owner_grouping_idx(sub as *const u8, PDODGE_PBLOCK_OWNER_OFFSET, false),
            gaining_idx,
            PERFECT_DODGE_COUNTER_ACTION_ID,
            false,
        ),
        Pending::DamageTaken => SbaCause::DamageTaken,
        Pending::FlatGrant => SbaCause::ChainGrant,
    }
}

// Resolve SBA from landing a hit
fn resolve_hit(instance: *const usize, gaining_idx: u32) -> SbaCause {
    let base = instance as *const u8;

    let Some(source_idx) = owner_grouping_idx(base, DAMAGE_SOURCE_ENTITY_OFFSET, true) else {
        return SbaCause::Unknown;
    };
    if source_idx != gaining_idx {
        return SbaCause::Unknown;
    }

    let (Some(flags), Some(action_id)) = (
        safe_read::<u64>(base.wrapping_add(0xE8) as *const u64),
        safe_read::<u32>(base.wrapping_add(0x16C) as *const u32),
    ) else {
        return SbaCause::Unknown;
    };

    let action = damage::classify_action(flags, action_id);

    if matches!(action, protocol::ActionType::SupplementaryDamage(_)) {
        return SbaCause::Unknown;
    }

    SbaCause::Action(action)
}

/// Resolve pblock/pdodge
fn resolve_counter(
    owner_idx: Option<u32>,
    gaining_idx: u32,
    counter_action_id: u32,
    inferred: bool,
) -> SbaCause {
    let Some(owner_idx) = owner_idx else {
        return SbaCause::Unknown;
    };
    if owner_idx != gaining_idx {
        return SbaCause::Unknown;
    }

    let action = protocol::ActionType::Normal(counter_action_id);
    if inferred {
        SbaCause::Inferred(action)
    } else {
        SbaCause::Action(action)
    }
}

// --- Hooks -----------------------------------------------------------------
//
// None of these take an `event::Tx`: they emit nothing. Their entire job is to
// park a pointer for the duration of the call they wrap.
//
// `-> usize` on void callees follows the precedent in `sba.rs`
// (`OnSBAResetBroadcastHook`): rax is caller-saved and the call sites ignore it.

#[derive(Clone)]
pub struct OnRegisterHitHook {}

impl OnRegisterHitHook {
    pub fn new() -> Self {
        OnRegisterHitHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let Ok(addr) = process.search_address_start(ON_REGISTER_HIT_SIG) else {
            return Err(anyhow!("Could not find on_register_hit"));
        };

        #[cfg(feature = "console")]
        println!("found on register hit");

        let cloned_self = self.clone();

        unsafe {
            let func: OnRegisterHitFunc = std::mem::transmute(addr);
            OnRegisterHit.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
            OnRegisterHit.enable()?;
        }

        GATE_INSTALLED.store(true, Ordering::Relaxed);

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize) -> usize {
        // Two thread-local operations and nothing else — this fires on every
        // registered hit in the process. All decoding is deferred to
        // `resolve_for`, which runs only when the gauge actually moves.
        let _cause = CauseGuard::set(Pending::Hit(a2));
        unsafe { OnRegisterHit.call(a1, a2) }
    }
}

impl Default for OnRegisterHitHook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct OnPerfectBlockEffectsHook {}

impl OnPerfectBlockEffectsHook {
    pub fn new() -> Self {
        OnPerfectBlockEffectsHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let Ok(addr) = process.search_address_start(ON_PERFECT_BLOCK_EFFECTS_SIG) else {
            return Err(anyhow!("Could not find on_perfect_block_effects"));
        };

        #[cfg(feature = "console")]
        println!("found on perfect block effects");

        let cloned_self = self.clone();

        unsafe {
            let func: OnCounterEffectsFunc = std::mem::transmute(addr);
            OnPerfectBlockEffects.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
            OnPerfectBlockEffects.enable()?;
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize) -> usize {
        // `a2` is the BLOCKED hit's DamageInstance — it describes the enemy's
        // attack, not the player's action, which is exactly why the counter is
        // tagged with the synthesized 611 instead of anything read from it.
        let _cause = CauseGuard::set(Pending::PerfectBlock(a1));
        unsafe { OnPerfectBlockEffects.call(a1, a2) }
    }
}

impl Default for OnPerfectBlockEffectsHook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct OnPerfectDodgeEffectsHook {}

impl OnPerfectDodgeEffectsHook {
    pub fn new() -> Self {
        OnPerfectDodgeEffectsHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let Ok(addr) = process.search_address_start(ON_PERFECT_DODGE_EFFECTS_SIG) else {
            return Err(anyhow!("Could not find on_perfect_dodge_effects"));
        };

        #[cfg(feature = "console")]
        println!("found on perfect dodge effects");

        let cloned_self = self.clone();

        unsafe {
            let func: OnCounterEffectsFunc = std::mem::transmute(addr);
            OnPerfectDodgeEffects.initialize(func, move |a1, a2| cloned_self.run(a1, a2))?;
            OnPerfectDodgeEffects.enable()?;
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize) -> usize {
        let _cause = CauseGuard::set(Pending::PerfectDodge(a1));
        unsafe { OnPerfectDodgeEffects.call(a1, a2) }
    }
}

impl Default for OnPerfectDodgeEffectsHook {
    fn default() -> Self {
        Self::new()
    }
}

/// Parks `DamageTaken` for the duration of the take-hit call.
///
/// Hand-written rather than generated by `parking_hook!` because that macro
/// assumes a single-argument callee; these two have real arities that must be
/// forwarded verbatim.
#[derive(Clone)]
pub struct OnTakeHitHook {}

impl OnTakeHitHook {
    pub fn new() -> Self {
        OnTakeHitHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let Ok(addr) = process.search_address_start(ON_TAKE_HIT_SIG) else {
            return Err(anyhow!("Could not find on_take_hit"));
        };

        #[cfg(feature = "console")]
        println!("found on take hit");

        let cloned_self = self.clone();

        unsafe {
            let func: unsafe extern "system" fn(*const usize, *const usize, u32, u64) -> usize =
                std::mem::transmute(addr);
            OnTakeHit.initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;
            OnTakeHit.enable()?;
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize, a3: u32, a4: u64) -> usize {
        let _cause = CauseGuard::set(Pending::DamageTaken);
        unsafe { OnTakeHit.call(a1, a2, a3, a4) }
    }
}

impl Default for OnTakeHitHook {
    fn default() -> Self {
        Self::new()
    }
}

/// Parks `FlatGrant` around the percent-of-max granter.
#[derive(Clone)]
pub struct OnSbaChainGrantHook {}

impl OnSbaChainGrantHook {
    pub fn new() -> Self {
        OnSbaChainGrantHook {}
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let Ok(addr) = process.search_address_start(ON_FLAT_GRANT_SIG) else {
            return Err(anyhow!("Could not find on_sba_chain_grant"));
        };

        #[cfg(feature = "console")]
        println!("found on sba chain grant");

        let cloned_self = self.clone();

        unsafe {
            #[allow(clippy::type_complexity)]
            let func: unsafe extern "system" fn(
                *const usize,
                f32,
                u32,
                u32,
                u64,
                u64,
                u64,
                u64,
            ) -> usize = std::mem::transmute(addr);
            OnFlatGrant.initialize(func, move |a1, a2, a3, a4, a5, a6, a7, a8| {
                cloned_self.run(a1, a2, a3, a4, a5, a6, a7, a8)
            })?;
            OnFlatGrant.enable()?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        a1: *const usize,
        a2: f32,
        a3: u32,
        a4: u32,
        a5: u64,
        a6: u64,
        a7: u64,
        a8: u64,
    ) -> usize {
        // All eight forwarded unchanged — a5..a8 are incoming STACK args (one
        // of which the callee reads back as an f32), so they are passed as raw
        // 8-byte slots rather than modelled.
        let _cause = CauseGuard::set(Pending::FlatGrant);
        unsafe { OnFlatGrant.call(a1, a2, a3, a4, a5, a6, a7, a8) }
    }
}

impl Default for OnSbaChainGrantHook {
    fn default() -> Self {
        Self::new()
    }
}

/// Online party member pblock/pdodge
macro_rules! replay_hook {
    ($name:ident, $detour:ident, $sig:ident, $label:literal, $owner_off:expr, $action:expr) => {
        #[derive(Clone)]
        pub struct $name {
            tx: crate::event::Tx,
        }

        impl $name {
            pub fn new(tx: crate::event::Tx) -> Self {
                $name { tx }
            }

            pub fn setup(&self, process: &Process) -> Result<()> {
                let Ok(addr) = process.search_address_start($sig) else {
                    return Err(anyhow!(concat!("Could not find ", $label)));
                };
                #[cfg(feature = "console")]
                println!(concat!("found ", $label));
                let cloned_self = self.clone();
                unsafe {
                    let func: OnSingleArgFunc = std::mem::transmute(addr);
                    $detour.initialize(func, move |a1| cloned_self.run(a1))?;
                    $detour.enable()?;
                }
                Ok(())
            }

            fn run(&self, a1: *const usize) -> usize {
                let owner = read_owner(a1, $owner_off);
                // A handler that fires but cannot name its owner has nothing to
                // credit the counter to, so it emits nothing rather than
                // guessing at a peer.
                if let Some(actor) = grouping_idx_of(owner) {
                    let _ = self.tx.send(protocol::Message::OnPeerCounter(
                        protocol::PeerCounterEvent { actor_index: actor, action_id: $action },
                    ));
                }
                unsafe { $detour.call(a1) }
            }
        }

    };
}

replay_hook!(
    OnRemoteDodgeReplayHook,
    OnRemoteDodgeReplay,
    ON_REMOTE_DODGE_REPLAY_SIG,
    "on remote perfect dodge replay",
    REMOTE_DODGE_OWNER_OFFSET,
    PERFECT_DODGE_COUNTER_ACTION_ID
);

replay_hook!(
    OnRemoteBlockReplayHook,
    OnRemoteBlockReplay,
    ON_REMOTE_BLOCK_REPLAY_SIG,
    "on remote perfect block replay",
    PDODGE_PBLOCK_OWNER_OFFSET,
    damage::PERFECT_BLOCK_COUNTER_ACTION_ID
);

/// Guarded deref of a replay handler's owner slot. Returns null rather than
/// `Option` so `Pending` stays `Copy`; a null resolves to `Unknown` downstream.
fn read_owner(a1: *const usize, offset: usize) -> *const usize {
    safe_read::<*const usize>((a1 as *const u8).wrapping_add(offset) as *const *const usize)
        .unwrap_or(std::ptr::null())
}

#[cfg(test)]
mod tests {
    use super::*;

    // No `exclusive()` guard and no sleeping anywhere in here, deliberately.
    // The state under test is thread-local, so cargo test's per-test threads
    // isolate it for free — unlike the damage hook's global maps, which do need
    // serializing. Nothing here reads a clock.

    fn peek() -> Option<Pending> {
        SBA_CAUSE.with(|c| c.get())
    }

    fn is_hit(p: Option<Pending>) -> bool {
        matches!(p, Some(Pending::Hit(_)))
    }

    fn is_block(p: Option<Pending>) -> bool {
        matches!(p, Some(Pending::PerfectBlock(_)))
    }

    /// The load-bearing one. A naive clear-on-drop implementation passes every
    /// other test in this module and fails this.
    #[test]
    fn guard_restores_the_enclosing_cause_on_drop() {
        assert!(peek().is_none());

        let outer = CauseGuard::set(Pending::Hit(0x1000 as *const usize));
        assert!(is_hit(peek()));

        {
            let _inner = CauseGuard::set(Pending::PerfectBlock(0x2000 as *const usize));
            assert!(is_block(peek()));
        }

        // The enclosing cause must be back, not cleared.
        assert!(is_hit(peek()), "inner guard erased the enclosing cause");

        drop(outer);
        assert!(peek().is_none());
    }

    #[test]
    fn guard_restores_from_an_empty_stack() {
        assert!(peek().is_none());
        {
            let _g = CauseGuard::set(Pending::PerfectDodge(0x3000 as *const usize));
            assert!(matches!(peek(), Some(Pending::PerfectDodge(_))));
        }
        assert!(peek().is_none(), "outermost guard left a stale cause behind");
    }

    #[test]
    fn an_empty_stack_is_named_not_guessed() {
        assert!(peek().is_none());

        GATE_INSTALLED.store(false, Ordering::Relaxed);
        assert_eq!(resolve_for(7), SbaCause::HookUnavailable);

        GATE_INSTALLED.store(true, Ordering::Relaxed);
        assert_eq!(resolve_for(7), SbaCause::Unknown);

        GATE_INSTALLED.store(false, Ordering::Relaxed);
    }

    /// Pointers that cannot be read must resolve to `Unknown`, never to an
    /// action. `safe_read` fails closed on these bogus addresses, so this
    /// exercises the unreadable arm without a live process.
    #[test]
    fn an_unreadable_cause_resolves_to_unknown() {
        let _g = CauseGuard::set(Pending::Hit(0x10 as *const usize));
        assert_eq!(resolve_for(7), SbaCause::Unknown);
    }

    #[test]
    fn counter_ids_are_the_documented_shot_ids() {
        assert_eq!(PERFECT_DODGE_COUNTER_ACTION_ID, 0x262);
        assert_eq!(damage::PERFECT_BLOCK_COUNTER_ACTION_ID, 0x263);
        // 610 / 611 as the UI and the locale table see them.
        assert_eq!(PERFECT_DODGE_COUNTER_ACTION_ID, 610);
        assert_eq!(damage::PERFECT_BLOCK_COUNTER_ACTION_ID, 611);
    }

    #[test]
    fn damage_taken_resolves_without_an_owner_check() {
        let _g = CauseGuard::set(Pending::DamageTaken);
        // Any actor id at all, including one that owns nothing on this stack.
        assert_eq!(resolve_for(7), SbaCause::DamageTaken);
        assert_eq!(resolve_for(1234), SbaCause::DamageTaken);
    }

    /// A joined cause and a read cause must stay distinguishable all the way to
    /// the wire — that separation is the entire reason `Inferred` exists.
    #[test]
    fn inferred_never_collapses_into_action() {
        let dodge = protocol::ActionType::Normal(PERFECT_DODGE_COUNTER_ACTION_ID);
        assert_ne!(SbaCause::Inferred(dodge), SbaCause::Action(dodge));

        // And the resolver picks the right one for each path.
        assert_eq!(
            resolve_counter(Some(5), 5, PERFECT_DODGE_COUNTER_ACTION_ID, false),
            SbaCause::Action(dodge)
        );
        assert_eq!(
            resolve_counter(Some(5), 5, PERFECT_DODGE_COUNTER_ACTION_ID, true),
            SbaCause::Inferred(dodge)
        );
    }

    /// A counter belonging to someone else is refused whether it was read or
    /// joined — the mismatch arm must not have an inferred loophole.
    #[test]
    fn a_counter_for_another_actor_is_refused_on_both_paths() {
        for inferred in [false, true] {
            assert_eq!(
                resolve_counter(Some(9), 5, PERFECT_DODGE_COUNTER_ACTION_ID, inferred),
                SbaCause::Unknown
            );
        }
        // An unreadable owner likewise fails closed rather than crediting.
        assert_eq!(
            resolve_counter(None, 5, PERFECT_DODGE_COUNTER_ACTION_ID, true),
            SbaCause::Unknown
        );
    }

    #[test]
    fn sba_cause_defaults_to_not_classified() {
        // Old saved logs deserialize through this default; it must not be
        // mistakable for a live "we looked and failed" answer.
        assert_eq!(SbaCause::default(), SbaCause::NotClassified);
        assert_ne!(SbaCause::default(), SbaCause::Unknown);
    }
}
