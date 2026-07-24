use anyhow::{anyhow, Result};
use protocol::{
    LinkAttackChanceEvent, LinkTimeEndEvent, LinkTimeStartEvent, Message,
};
use retour::static_detour;

use crate::{event, process::Process};

const ON_LINK_TIME_START_SIG: &str = "b9 09 00 00 00 e8 ? ? ? ? c5 f9 6e f0";

const LINK_TIME_START_ANCHOR_DELTA: usize = 0x58;

const ON_LINK_TIME_END_SIG: &str =
    "56 57 53 48 83 ec 20 89 d3 48 89 ce e8 ? ? ? ? 48 8b 8e 70 02 00 00 c5 fa 10 0d";

const ON_LINK_ATTACK_CHANCE_SIG: &str = "55 41 57 41 56 41 55 41 54 56 57 53 48 81 ec e8 02 00 00 \
     48 8d ac 24 80 00 00 00 c5 f8 29 bd 50 02 00 00 c5 f8 29 b5 40 02 00 00 \
     48 c7 85 38 02 00 00 fe ff ff ff 4d";

type OnLinkTimeStartFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnLinkTimeEndFunc = unsafe extern "system" fn(*const usize, u32) -> usize;
type OnLinkAttackChanceFunc =
    unsafe extern "system" fn(*const usize, *const usize, *const usize, u32) -> usize;

static_detour! {
    static OnLinkTimeStart: unsafe extern "system" fn(*const usize) -> usize;
    static OnLinkTimeEnd: unsafe extern "system" fn(*const usize, u32) -> usize;
    static OnLinkAttackChance:
        unsafe extern "system" fn(*const usize, *const usize, *const usize, u32) -> usize;
}

/// Fires once per window on every client; peers reach it from the `{2,33}` receive case.
#[derive(Clone)]
pub struct OnLinkTimeStartHook {
    tx: event::Tx,
}

impl OnLinkTimeStartHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLinkTimeStartHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let anchor = process
            .search_address_start(ON_LINK_TIME_START_SIG)
            .map_err(|e| anyhow!("Could not find link_time_start anchor: {e}"))?;

        let address = anchor
            .checked_sub(LINK_TIME_START_ANCHOR_DELTA)
            .ok_or_else(|| anyhow!("link_time_start anchor {anchor:#x} underflowed"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found link time start");

        unsafe {
            let func: OnLinkTimeStartFunc = std::mem::transmute(address);
            OnLinkTimeStart.initialize(func, move |mgr| cloned_self.run(mgr))?;
            OnLinkTimeStart.enable()?;
        }

        Ok(())
    }

    fn run(&self, mgr: *const usize) -> usize {
        let ret = unsafe { OnLinkTimeStart.call(mgr) };

        let _ = self.tx.send(Message::OnLinkTimeStart(LinkTimeStartEvent {}));

        ret
    }
}

/// Every way a link time can end routes through this; peers reach it from the `{2,35}` receive case.
#[derive(Clone)]
pub struct OnLinkTimeEndHook {
    tx: event::Tx,
}

impl OnLinkTimeEndHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLinkTimeEndHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_LINK_TIME_END_SIG)
            .map_err(|e| anyhow!("Could not find link_time_end: {e}"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found link time end");

        unsafe {
            let func: OnLinkTimeEndFunc = std::mem::transmute(address);
            OnLinkTimeEnd.initialize(func, move |mgr, reason| cloned_self.run(mgr, reason))?;
            OnLinkTimeEnd.enable()?;
        }

        Ok(())
    }

    fn run(&self, mgr: *const usize, reason: u32) -> usize {
        let ret = unsafe { OnLinkTimeEnd.call(mgr, reason) };

        let _ = self
            .tx
            .send(Message::OnLinkTimeEnd(LinkTimeEndEvent { reason }));

        ret
    }
}

/// The link-attack prompt, not the window itself.
#[derive(Clone)]
pub struct OnLinkAttackChanceHook {
    tx: event::Tx,
}

impl OnLinkAttackChanceHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLinkAttackChanceHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let address = process
            .search_address_start(ON_LINK_ATTACK_CHANCE_SIG)
            .map_err(|e| anyhow!("Could not find link_attack_chance: {e}"))?;

        let cloned_self = self.clone();

        #[cfg(feature = "console")]
        println!("Found link attack chance");

        unsafe {
            let func: OnLinkAttackChanceFunc = std::mem::transmute(address);
            OnLinkAttackChance
                .initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;
            OnLinkAttackChance.enable()?;
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, a2: *const usize, a3: *const usize, a4: u32) -> usize {
        let ret = unsafe { OnLinkAttackChance.call(a1, a2, a3, a4) };

        let _ = self
            .tx
            .send(Message::OnLinkAttackChance(LinkAttackChanceEvent {}));

        ret
    }
}
