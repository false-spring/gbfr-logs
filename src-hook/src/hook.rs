use anyhow::{anyhow, Result};
use log::warn;
use pelite::{
    pattern,
    pe64::{Pe, PeView},
};
use protocol::{ActionType, Actor, DamageEvent, Message};
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
// const P_QWORD_1467572B0_SIG: &str = "48 ? ? $ { ' } 83 66 ? ? 48 ? ?";

type GetEntityHashID0x58 = unsafe extern "system" fn(*const usize, *const u32) -> *const usize;

#[inline(always)]
unsafe fn v_func<T: Sized>(ptr: *const usize, offset: usize) -> T {
    ((ptr.read() as *const usize).byte_add(offset) as *const T).read()
}

fn actor_type_id(actor_ptr: *const usize) -> u32 {
    let mut type_id: u32 = 0;

    unsafe {
        v_func::<GetEntityHashID0x58>(actor_ptr, 0x58)(actor_ptr, &mut type_id as *mut u32);
    }

    type_id
}

#[inline(always)]
fn actor_idx(actor_ptr: *const usize) -> u32 {
    unsafe { (actor_ptr.byte_add(0x170) as *const u32).read() }
}

// Returns the specified instance of the parent entity.
// ptr+offset: Entity
// *(ptr+offset) + 0x70: m_pSpecifiedInstance (Pl0700, Pl1200, etc.)
fn parent_specified_instance_at(actor_ptr: *const usize, offset: usize) -> *const usize {
    unsafe {
        (actor_ptr.byte_add(offset) as *const *const *const usize)
            .read()
            .byte_add(0x70)
            .read()
    }
}

unsafe fn process_damage_event(
    tx: event::Tx,
    a1: *const usize, // RCX: EmBehaviourBase instance
    a2: *const usize, // RDX
    a3: *const usize, // R8
    a4: u8,           // R9
) -> usize {
    let original_value = ProcessDamageEvent.call(a1, a2, a3, a4);

    // Target is the instance of the actor being damaged.
    // For example: Instance of the Em2700 class.
    let target_specified_instance_ptr: usize = *(*a1.byte_add(0x08) as *const usize);

    // This points to the first Entity instance in the 'a2' entity list.
    let source_entity_ptr = (a2.byte_add(0x18) as *const *const usize).read();

    // @TODO(false): For some reason, online + Ferry's Umlauf skill pet can return a null pointer here.
    // Possible data race with online?
    if source_entity_ptr == std::ptr::null() {
        return original_value;
    }

    // entity->m_pSpecifiedInstance, offset 0x70 from entity pointer.
    // Returns the specific class instance of the source entity. (e.g. Instance of Pl1200 / Pl0700Ghost)
    let source_specified_instance_ptr: usize = *(source_entity_ptr.byte_add(0x70) as *const usize);
    let damage: i32 = (a2.byte_add(0xD0) as *const i32).read();

    if original_value == 0 || damage <= 0 {
        return original_value;
    }

    let flags: u64 = (a2.byte_add(0xD8) as *const u64).read();

    let action_type: ActionType = if ((1 << 7 | 1 << 50) & flags) != 0 {
        ActionType::LinkAttack
    } else if ((1 << 13 | 1 << 14) & flags) != 0 {
        ActionType::SBA
    } else if ((1 << 15) & flags) != 0 {
        ActionType::SupplementaryDamage((a2.byte_add(0x154) as *const u32).read())
    } else {
        ActionType::Normal((a2.byte_add(0x154) as *const u32).read())
    };

    // Get the source actor's type ID.
    let source_type_id = actor_type_id(source_specified_instance_ptr as *const usize);
    let source_idx = actor_idx(source_specified_instance_ptr as *const usize);

    // If the source_type is any of the following, then we need to get their parent entity.
    let (source_parent_type_id, source_parent_idx) = get_source_parent(
        source_type_id,
        source_specified_instance_ptr as *const usize,
        source_idx,
    );
    let target_type_id: u32 = actor_type_id(target_specified_instance_ptr as *const usize);
    let target_idx = actor_idx(target_specified_instance_ptr as *const usize);

    let event = Message::DamageEvent(DamageEvent {
        source: Actor {
            index: source_idx,
            actor_type: source_type_id,
            parent_index: source_parent_idx,
            parent_actor_type: source_parent_type_id,
        },
        target: Actor {
            index: target_idx,
            actor_type: target_type_id,
            parent_index: target_idx,
            parent_actor_type: target_type_id,
        },
        damage,
        flags,
        action_id: action_type,
    });

    let _ = tx.send(event);

    original_value
}

// A1: DoT Instance (StatusPl2300ParalysisArrow)
// *A1+0x00 -> StatusAilmentPoison : StatusBase
// A1+0x18->targetEntityInfo : CEntityInfo (Target entity of the DoT, what is being damaged)
// A1+0x30->sourceEntityInfo : CEntityInfo (Source entity of the DoT, who applied it)
// A1+0x50->duration : float (How much time is left for the DoT)
unsafe fn process_dot_event(tx: event::Tx, dot_instance: *const usize, a2: *const usize) -> usize {
    let original_value = ProcessDotEvent.call(dot_instance, a2);

    // @TODO(false): There's a better way to check null pointers with Option type, but I'm too dumb to figure it out right now.
    let target_info = dot_instance.byte_add(0x18).read() as *const usize;
    let source_info = dot_instance.byte_add(0x30).read() as *const usize;

    if target_info == std::ptr::null() || source_info == std::ptr::null() {
        return original_value;
    }

    let target = target_info.byte_add(0x70).read() as *const usize;
    let source = source_info.byte_add(0x70).read() as *const usize;

    if target == std::ptr::null() || source == std::ptr::null() {
        return original_value;
    }

    let dmg = (a2 as *const i32).read();

    let source_idx = actor_idx(source);
    let source_type_id = actor_type_id(source);

    let target_idx = actor_idx(target);
    let target_type_id = actor_type_id(target);

    let (source_parent_type_id, source_parent_idx) =
        get_source_parent(source_type_id, source, source_idx);

    // @TODO(false): There should be a way to get the type of DoT being applied. Too dumb to find it right now.
    let event = Message::DamageEvent(DamageEvent {
        source: Actor {
            index: source_idx,
            actor_type: source_type_id,
            parent_index: source_parent_idx,
            parent_actor_type: source_parent_type_id,
        },
        target: Actor {
            index: target_idx,
            actor_type: target_type_id,
            parent_index: target_idx,
            parent_actor_type: target_type_id,
        },
        damage: dmg,
        flags: 0,
        action_id: ActionType::DamageOverTime(0),
    });

    let _ = tx.send(event);

    original_value
}

// Returns the parent entity of the source entity if necessary.
fn get_source_parent(source_type_id: u32, source: *const usize, source_idx: u32) -> (u32, u32) {
    let (source_parent_type_id, source_parent_idx) = match source_type_id {
        // Pl0700Ghost -> Pl0700
        0x2AF678E8 => {
            let parent_instance = parent_specified_instance_at(source, 0xE48);

            (actor_type_id(parent_instance), actor_idx(parent_instance))
        }
        // Pl0700GhostSatellite -> Pl0700
        0x8364C8BC => {
            let parent_instance = parent_specified_instance_at(source, 0x508);

            (actor_type_id(parent_instance), actor_idx(parent_instance))
        }
        // Wp1890: Cagliostro's Ouroboros Dragon Sled -> Pl1800
        0xC9F45042 => {
            let parent_instance = parent_specified_instance_at(source, 0x578);
            (actor_type_id(parent_instance), actor_idx(parent_instance))
        }
        // Pl2000: Id's Dragon Form -> Pl1900
        0xF5755C0E => {
            let parent_instance = parent_specified_instance_at(source, 0xD028);
            (actor_type_id(parent_instance), actor_idx(parent_instance))
        }
        _ => (source_type_id, source_idx),
    };
    (source_parent_type_id, source_parent_idx)
}

unsafe fn on_enter_area(tx: event::Tx, a1: u32, a2: *const usize, a3: u8) -> usize {
    let ret = OnEnterArea.call(a1, a2, a3);
    let _ = tx.send(Message::OnAreaEnter);
    ret
}

pub fn init(tx: event::Tx) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    // See https://github.com/nyaoouo/GBFR-ACT/blob/5801c193de2f474764b55b7c6b759c3901dc591c/injector.py#L1773-L1809
    if let Ok(process_dmg_evt) = search(&process, PROCESS_DAMAGE_EVENT_SIG) {
        let tx = tx.clone();
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
        let tx = tx.clone();
        unsafe {
            let func: ProcessDotEventFunc = std::mem::transmute(process_dot_evt);
            ProcessDotEvent
                .initialize(func, move |a1, a2| process_dot_event(tx.clone(), a1, a2))?;
            ProcessDotEvent.enable()?;
        }
    } else {
        warn!("Could not find process_dot_evt");
    }

    if let Ok(on_enter_area_evt) = search(&process, ON_ENTER_AREA_SIG) {
        let tx = tx.clone();
        unsafe {
            let func: OnEnterAreaFunc = std::mem::transmute(on_enter_area_evt);
            OnEnterArea.initialize(func, move |a1, a2, a3| {
                on_enter_area(tx.clone(), a1, a2, a3)
            })?;
            OnEnterArea.enable()?;
        }
    } else {
        warn!("Could not find on_enter_area");
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
