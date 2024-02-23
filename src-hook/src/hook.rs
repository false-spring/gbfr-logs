use anyhow::{anyhow, Result};
use log::{info, warn};
use pelite::{
    pattern,
    pe64::{Pe, PeView},
};
use retour::static_detour;

use crate::process::Process;

type ProcessDamageEventFunc = unsafe extern "system" fn(usize, usize, usize, u8) -> usize;
type ProcessDotEventFunc = unsafe extern "system" fn(usize, usize) -> usize;
type OnEnterAreaFunc = unsafe extern "system" fn(u32, usize, u8) -> usize;

static_detour! {
    static ProcessDamageEvent: unsafe extern "system" fn(usize, usize, usize, u8) -> usize;
    static ProcessDotEvent: unsafe extern "system" fn(usize, usize) -> usize;
    static OnEnterArea: unsafe extern "system" fn(u32, usize, u8) -> usize;
}

const PROCESS_DAMAGE_EVENT_SIG: &str = "e8 $ { ' } 66 83 bc 24 ? ? ? ? ?";
const PROCESS_DOT_EVENT_SIG: &str = "44 89 74 24 ? 48 ? ? ? ? 48 ? ? e8 $ { ' } 4c";
const ON_ENTER_AREA_SIG: &str = "e8 $ { ' } c5 ? ? ? c5 f8 29 45 ? c7 45 ? ? ? ? ?";
const P_QWORD_1467572B0_SIG: &str = "48 ? ? $ { ' } 83 66 ? ? 48 ? ?";

unsafe fn process_damage_event(a1: usize, a2: usize, a3: usize, a4: u8) -> usize {
    info!("process_damage_event({}, {}, {}, {})", a1, a2, a3, a4);

    let ret = ProcessDamageEvent.call(a1, a2, a3, a4);
    ret
}

unsafe fn process_dot_event(a1: usize, a2: usize) -> usize {
    info!("process_dot_event({}, {})", a1, a2);

    let ret = ProcessDotEvent.call(a1, a2);
    ret
}

unsafe fn on_enter_area(a1: u32, a2: usize, a3: u8) -> usize {
    info!("on_enter_area({}, {:x}, {})", a1, a2, a3);

    let ret = OnEnterArea.call(a1, a2, a3);
    ret
}

pub fn init() -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    // See https://github.com/nyaoouo/GBFR-ACT/blob/5801c193de2f474764b55b7c6b759c3901dc591c/injector.py#L1773-L1809
    if let Ok(process_dmg_evt) = search(&process, PROCESS_DAMAGE_EVENT_SIG) {
        info!("Found process_dmg_evt");
        unsafe {
            let func: ProcessDamageEventFunc = std::mem::transmute(process_dmg_evt);
            ProcessDamageEvent
                .initialize(func, |a1, a2, a3, a4| process_damage_event(a1, a2, a3, a4))?;
            ProcessDamageEvent.enable()?;
        }
    } else {
        warn!("Could not find process_dmg_evt");
    }
    if let Ok(process_dot_evt) = search(&process, PROCESS_DOT_EVENT_SIG) {
        info!("Found process_dot_evt");
        unsafe {
            let func: ProcessDotEventFunc = std::mem::transmute(process_dot_evt);
            ProcessDotEvent.initialize(func, |a1, a2| process_dot_event(a1, a2))?;
            ProcessDotEvent.enable()?;
        }
    } else {
        warn!("Could not find process_dot_evt");
    }
    if let Ok(on_enter_area_evt) = search(&process, ON_ENTER_AREA_SIG) {
        info!("Found on_enter_area");
        unsafe {
            let func: OnEnterAreaFunc = std::mem::transmute(on_enter_area_evt);
            OnEnterArea.initialize(func, |a1, a2, a3| on_enter_area(a1, a2, a3))?;
            OnEnterArea.enable()?;
        }
    } else {
        warn!("Could not find on_enter_area");
    }
    #[allow(non_snake_case)]
    if let Ok(_p_qword_1467572B0) = search(&process, P_QWORD_1467572B0_SIG) {
        info!("Found p_qword_1467572B0");
    } else {
        warn!("Could not find p_qword_1467572B0");
    }

    Ok(())
}

/// Searches and returns the RVAs of the function that matches the given signature pattern.
fn search(process: &Process, signature_pattern: &str) -> Result<usize> {
    let view = unsafe { PeView::module(process.module_handle.0 as *const u8) };
    let scanner = view.scanner();
    let pattern = pattern::parse(signature_pattern)?;

    let mut addrs = [0; 8];

    let mut matches = scanner.matches_code(&pattern);

    let mut first_addr = None;

    // addrs[0] = RVA of where the match was found.
    // addrs[1] = RVA of the function being called.
    while matches.next(&mut addrs) {
        info!("Found match {:?} for pattern: {}", addrs, signature_pattern);
        first_addr = Some(process.base_address + addrs[1] as usize);
    }

    first_addr.ok_or(anyhow!(
        "Could not find match for pattern: {}",
        signature_pattern
    ))
}
