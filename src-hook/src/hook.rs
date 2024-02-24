use anyhow::{anyhow, Result};
use log::{info, warn};
use pelite::{
    pattern,
    pe64::{Pe, PeView},
};
use protocol::{ActionType, Actor, Message};
use retour::static_detour;

use crate::event;
use crate::process::Process;

type ProcessDamageEventFunc =
    unsafe extern "system" fn(*const usize, *const usize, *const usize, u8) -> usize;
type ProcessDotEventFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;
type OnEnterAreaFunc = unsafe extern "system" fn(u32, *const usize, u8) -> usize;

static_detour! {
    static ProcessDamageEvent: unsafe extern "system" fn(*const usize, *const usize, *const usize, u8) -> usize;
    static ProcessDotEvent: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnEnterArea: unsafe extern "system" fn(u32, *const usize, u8) -> usize;
}

const PROCESS_DAMAGE_EVENT_SIG: &str = "e8 $ { ' } 66 83 bc 24 ? ? ? ? ?";
const PROCESS_DOT_EVENT_SIG: &str = "44 89 74 24 ? 48 ? ? ? ? 48 ? ? e8 $ { ' } 4c";
const ON_ENTER_AREA_SIG: &str = "e8 $ { ' } c5 ? ? ? c5 f8 29 45 ? c7 45 ? ? ? ? ?";
const P_QWORD_1467572B0_SIG: &str = "48 ? ? $ { ' } 83 66 ? ? 48 ? ?";

type IA10x40 = unsafe extern "system" fn(
    *const usize,
    *const usize,
    *const usize,
    *const usize,
    *const usize,
) -> u32;
type IActor0x58 = unsafe extern "system" fn(*const usize, *const u32) -> *const usize;

unsafe fn v_func<T: Sized>(ptr: *const usize, offset: usize) -> T {
    ((ptr.read() as *const usize).byte_add(offset) as *const T).read()
}

fn actor_type_id(actor_ptr: *const usize) -> u32 {
    let mut type_id: u32 = 0;

    unsafe {
        v_func::<IActor0x58>(actor_ptr, 0x58)(actor_ptr, &mut type_id as *mut u32);
    }

    type_id
}

unsafe fn process_damage_event(
    tx: event::Tx,
    a1: *const usize,
    a2: *const usize,
    a3: *const usize,
    a4: u8,
) -> usize {
    // ah yes, just rust things
    let target: usize = *(*a1.byte_add(0x08) as *const usize);
    let source: usize = *((*a2.byte_add(0x18) as *const usize).byte_add(0x70) as *const usize);

    let ignore = !(a4 > 0
        || v_func::<IA10x40>(a1, 0x40)(
            a1 as *const usize,
            a2 as *const usize,
            std::ptr::null(),
            target as *const usize,
            source as *const usize,
        ) > 0);

    let original_value = ProcessDamageEvent.call(a1, a2, a3, a4);

    if ignore {
        return original_value;
    }

    let damage: i32 = (a2.byte_add(0xD0) as *const i32).read();
    let flags: u64 = (a2.byte_add(0xD8) as *const u64).read();
    let action_type: ActionType = if ((1 << 7 | 1 << 50) & flags) != 0 {
        ActionType::LinkAttack
    } else if ((1 << 13 | 1 << 14) & flags) != 0 {
        ActionType::SBA
    } else {
        ActionType::Normal((a2.byte_add(0x154) as *const u32).read())
    };

    // Get the source actor's type ID.
    let source_type_id = actor_type_id(source as *const usize);
    let target_type_id: u32 = actor_type_id(target as *const usize);

    let event = Message::DamageEvent {
        source: Actor {
            actor_type: source_type_id,
        },
        target: Actor {
            actor_type: target_type_id,
        },
        damage,
        flags,
        action_id: action_type,
    };

    let _ = tx.send(event);

    original_value
}

unsafe fn process_dot_event(a1: *const usize, a2: *const usize) -> usize {
    // @TODO(false): Implement DOT tracking.
    let ret = ProcessDotEvent.call(a1, a2);
    ret
}

unsafe fn on_enter_area(a1: u32, a2: *const usize, a3: u8) -> usize {
    // @TODO(false): Implement area change event.
    let ret = OnEnterArea.call(a1, a2, a3);
    ret
}

pub fn init(tx: event::Tx) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    // See https://github.com/nyaoouo/GBFR-ACT/blob/5801c193de2f474764b55b7c6b759c3901dc591c/injector.py#L1773-L1809
    if let Ok(process_dmg_evt) = search(&process, PROCESS_DAMAGE_EVENT_SIG) {
        info!("Found process_dmg_evt");
        unsafe {
            let func: ProcessDamageEventFunc = std::mem::transmute(process_dmg_evt);
            ProcessDamageEvent.initialize(func, move |a1, a2, a3, a4| {
                process_damage_event(tx.clone(), a1, a2, a3, a4)
            })?;
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

/// Uninstalls the hooks.
pub fn uninstall() {
    unsafe {
        ProcessDamageEvent.disable().unwrap();
        ProcessDotEvent.disable().unwrap();
        OnEnterArea.disable().unwrap();
    }
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
        first_addr = Some(process.base_address + addrs[1] as usize);
    }

    first_addr.ok_or(anyhow!(
        "Could not find match for pattern: {}",
        signature_pattern
    ))
}
