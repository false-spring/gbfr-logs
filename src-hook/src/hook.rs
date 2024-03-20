use std::{
    ffi::{CStr, CString},
    ptr,
    sync::atomic::{AtomicPtr, AtomicU32, Ordering},
};

use anyhow::{anyhow, Context, Result};
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
type OnEnterAreaFunc = unsafe extern "system" fn(u32, *const usize, u8, *const usize) -> usize;
type OnLoadPlayerFunc = unsafe extern "system" fn(*const usize) -> usize;
type OnLoadQuestState = unsafe extern "system" fn(*const usize) -> usize;
type OnShowResultScreen = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static ProcessDamageEvent: unsafe extern "system" fn(*const usize, *const usize, *const usize, u8) -> usize;
    static ProcessDotEvent: unsafe extern "system" fn(*const usize, *const usize) -> usize;
    static OnEnterArea: unsafe extern "system" fn(u32, *const usize, u8, *const usize) -> usize;
    static OnLoadPlayer: unsafe extern "system" fn(*const usize) -> usize;
    static OnLoadQuestState: unsafe extern "system" fn(*const usize) -> usize;
    static OnShowResultScreen: unsafe extern "system" fn(*const usize) -> usize;
}

static QUEST_STATE_PTR: AtomicPtr<QuestState> = AtomicPtr::new(ptr::null_mut());
static PLAYER_DATA_OFFSET: AtomicU32 = AtomicU32::new(0);
static WEAPON_OFFSET: AtomicU32 = AtomicU32::new(0);
static OVERMASTERY_OFFSET: AtomicU32 = AtomicU32::new(0);
static SIGIL_OFFSET: AtomicU32 = AtomicU32::new(0);

const PROCESS_DAMAGE_EVENT_SIG: &str = "e8 $ { ' } 66 83 bc 24 ? ? ? ? ?";
const PROCESS_DOT_EVENT_SIG: &str = "44 89 74 24 ? 48 ? ? ? ? 48 ? ? e8 $ { ' } 4c";
const ON_ENTER_AREA_SIG: &str = "e8 $ { ' } c5 ? ? ? c5 f8 29 45 ? c7 45 ? ? ? ? ?";
const ON_LOAD_PLAYER: &str = "49 89 ce e8 $ { ' } 31 ff 85 c0 ? ? ? ? ? ? 49 8b 46 28";
const ON_LOAD_QUEST_STATE: &str =
    "48 8b 0d ? ? ? ? e8 $ { ' } c5 fb 12 ? ? ? ? ? c5 f8 11 ? ? ? ? ? c5 f8 11 ? ? ? ? ? 48 83 c4 48";
const ON_SHOW_RESULT_SCREEN_SIG: &str =
    "e8 $ { ' } b8 ? ? ? ? 23 87 ? ? 00 00 3d 00 00 60 00 0f 94 c0";

type GetEntityHashID0x58 = unsafe extern "system" fn(*const usize, *const u32) -> *const usize;

struct VBuffer(*const usize);

impl VBuffer {
    fn ptr(&self) -> *const usize {
        if self.max_size() > 0xf {
            unsafe { self.0.read() as *const usize }
        } else {
            self.0
        }
    }

    fn used_size(&self) -> usize {
        unsafe { self.0.byte_add(0x10).read() }
    }

    fn max_size(&self) -> usize {
        unsafe { self.0.byte_add(0x18).read() }
    }

    fn raw(&self) -> CString {
        let bytes =
            unsafe { std::slice::from_raw_parts(self.ptr() as *const u8, self.used_size()) };

        unsafe { CString::from_vec_unchecked(bytes.to_vec()) }
    }
}

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
fn parent_specified_instance_at(actor_ptr: *const usize, offset: usize) -> Option<*const usize> {
    unsafe {
        let info = (actor_ptr.byte_add(offset) as *const *const *const usize).read_unaligned();

        if info == std::ptr::null() {
            return None;
        }

        Some(info.byte_add(0x70).read())
    }
}

fn process_damage_event(
    tx: event::Tx,
    a1: *const usize, // RCX: EmBehaviourBase instance
    a2: *const usize, // RDX
    a3: *const usize, // R8
    a4: u8,           // R9
) -> usize {
    let original_value = unsafe { ProcessDamageEvent.call(a1, a2, a3, a4) };

    // Target is the instance of the actor being damaged.
    // For example: Instance of the Em2700 class.
    let target_specified_instance_ptr: usize = unsafe { *(*a1.byte_add(0x08) as *const usize) };

    // This points to the first Entity instance in the 'a2' entity list.
    let source_entity_ptr = unsafe { (a2.byte_add(0x18) as *const *const usize).read() };

    // @TODO(false): For some reason, online + Ferry's Umlauf skill pet can return a null pointer here.
    // Possible data race with online?
    if source_entity_ptr == std::ptr::null() {
        return original_value;
    }

    // entity->m_pSpecifiedInstance, offset 0x70 from entity pointer.
    // Returns the specific class instance of the source entity. (e.g. Instance of Pl1200 / Pl0700Ghost)
    let source_specified_instance_ptr: usize =
        unsafe { *(source_entity_ptr.byte_add(0x70) as *const usize) };
    let damage: i32 = unsafe { (a2.byte_add(0xD0) as *const i32).read() };

    if original_value == 0 || damage <= 0 {
        return original_value;
    }

    let flags: u64 = unsafe { (a2.byte_add(0xD8) as *const u64).read() };

    let action_type: ActionType = if ((1 << 7 | 1 << 50) & flags) != 0 {
        ActionType::LinkAttack
    } else if ((1 << 13 | 1 << 14) & flags) != 0 {
        ActionType::SBA
    } else if ((1 << 15) & flags) != 0 {
        let skill_id = unsafe { (a2.byte_add(0x154) as *const u32).read() };
        ActionType::SupplementaryDamage(skill_id)
    } else {
        let skill_id = unsafe { (a2.byte_add(0x154) as *const u32).read() };
        ActionType::Normal(skill_id)
    };

    // Get the source actor's type ID.
    let source_type_id = actor_type_id(source_specified_instance_ptr as *const usize);
    let source_idx = actor_idx(source_specified_instance_ptr as *const usize);

    // If the source_type is any of the following, then we need to get their parent entity.
    let (source_parent_type_id, source_parent_idx) = get_source_parent(
        source_type_id,
        source_specified_instance_ptr as *const usize,
    )
    .unwrap_or((source_type_id, source_idx));

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
        get_source_parent(source_type_id, source).unwrap_or((source_type_id, source_idx));

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
fn get_source_parent(source_type_id: u32, source: *const usize) -> Option<(u32, u32)> {
    match source_type_id {
        // Pl0700Ghost -> Pl0700
        0x2AF678E8 => {
            let parent_instance = parent_specified_instance_at(source, 0xE48)?;

            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Pl0700GhostSatellite -> Pl0700
        0x8364C8BC => {
            let parent_instance = parent_specified_instance_at(source, 0x508)?;

            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Wp1890: Cagliostro's Ouroboros Dragon Sled -> Pl1800
        0xC9F45042 => {
            let parent_instance = parent_specified_instance_at(source, 0x578)?;
            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        // Pl2000: Id's Dragon Form -> Pl1900
        0xF5755C0E => {
            let parent_instance = parent_specified_instance_at(source, 0xD138)?;
            Some((actor_type_id(parent_instance), actor_idx(parent_instance)))
        }
        _ => None,
    }
}

fn on_enter_area(tx: event::Tx, a1: u32, a2: *const usize, a3: u8, a4: *const usize) -> usize {
    #[cfg(feature = "console")]
    println!("on enter area");

    let quest_state_ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);

    if quest_state_ptr != std::ptr::null_mut() {
        let quest_state = unsafe { quest_state_ptr.read() };

        let quest_id = quest_state.quest_id;
        let timer = quest_state.elapsed_time;

        let _ = tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
            last_known_quest_id: quest_id,
            last_known_elapsed_time_in_secs: timer,
        }));
    } else {
        let _ = tx.send(Message::OnAreaEnter(protocol::AreaEnterEvent {
            last_known_quest_id: 0,
            last_known_elapsed_time_in_secs: 0,
        }));
    }

    let ret = unsafe { OnEnterArea.call(a1, a2, a3, a4) };

    ret
}

#[derive(Debug)]
#[repr(C)]
struct QuestState {
    quest_id: u32,            // 0x00
    padding_640: [u8; 0x644], // 0x004 - 0x648
    elapsed_time: u32,        // 0x648
}

fn on_load_quest_state(a1: *const usize) -> usize {
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

fn on_show_result_screen(tx: event::Tx, a1: *const usize) -> usize {
    #[cfg(feature = "console")]
    println!("on show result screen");

    let quest_state_ptr = QUEST_STATE_PTR.load(Ordering::Relaxed);

    if quest_state_ptr != std::ptr::null_mut() {
        let quest_state = unsafe { quest_state_ptr.read() };
        let quest_id = quest_state.quest_id;
        let timer = quest_state.elapsed_time;

        let _ = tx.send(Message::OnQuestComplete(protocol::QuestCompleteEvent {
            quest_id,
            elapsed_time_in_secs: timer,
        }));
    }

    let ret = unsafe { OnShowResultScreen.call(a1) };

    ret
}

#[derive(Debug)]
#[repr(C)]
struct SigilEntry {
    first_trait_id: u32,
    first_trait_level: u32,
    second_trait_id: u32,
    second_trait_level: u32,
    sigil_id: u32,
    equipped_character: u32,
    sigil_level: u32,
    acquisition_count: u32,
    notification_enum: u32,
}

#[derive(Debug)]
#[repr(C)]
struct SigilList {
    sigils: [SigilEntry; 12], // 0x00
    unk_1b0: u32,             //0x01B0
    unk_1b4: u32,             //0x01B4
    unk_1b8: u32,             //0x01B8
    unk_1bc: u32,             //0x01BC
    unk_1c0: u32,             //0x01C0
    unk_1c4: u32,             //0x01C4
    /// 0 == local, 1 == online
    is_online: u32, //0x01C8
    unk_1cc: u32,             //0x01CC
    unk_1d0: u32,             //0x01D0
    unk_1d4: u32,             //0x01D4
    unk_1d8: u32,             //0x01D8
    unk_1dc: u32,             //0x01DC
    unk_1e0: u32,             //0x01E0
    unk_1e4: u32,             //0x01E4
    character_name: [u8; 16], //0x01E8
    padding_1f8: [u8; 16],    //0x01F8
    display_name: [u8; 16],   //0x0208
    padding_218: [u8; 24],    //0x0218
    party_index: u32,         //0x0230
}

#[derive(Debug)]
#[repr(C)]
struct PlayerStats {
    level: u32,
    total_health: u32,
    total_attack: u32,
    unk_0c: u32,
    stun_power: f32,
    critical_rate: f32,
    total_power: u32,
}

#[derive(Debug)]
#[repr(C)]
struct WeaponInfo {
    unk_00: u32,
    /// Weapon ID Hash
    weapon_id: u32,
    weapon_ap_tree: u32,
    unk_0c: u32,
    weapon_exp: u32,
    /// How many uncap stars the weapon has
    star_level: u32,
    /// Number of plus marks on the weapon
    plus_marks: u32,
    /// Weapon's awakening level
    awakening_level: u32,
    /// First trait ID
    trait_1_id: u32,
    /// First trait level
    trait_1_level: u32,
    /// Second trait ID
    trait_2_id: u32,
    /// Second trait level
    trait_2_level: u32,
    /// Third trait ID
    trait_3_id: u32,
    /// Third trait level
    trait_3_level: u32,
    /// Wrightstone used on the weapon
    wrightstone_id: u32,
    unk_3c: u32,
    /// Current weapon level
    weapon_level: u32,
    /// Weapon's HP Stats (before plus marks)
    weapon_hp: u32,
    /// Weapon's Attack Stats (before plus marks)
    weapon_attack: u32,
}

#[derive(Debug)]
#[repr(C)]
struct Overmastery {
    /// Overmastery Stats ID type
    id: u32,
    /// Flags
    flags: u32,
    unk_08: u32,
    /// Value for the overmastery
    value: f32,
}

#[derive(Debug)]
#[repr(C)]
struct Overmasteries {
    stats: [Overmastery; 4],
}

fn on_load_player(tx: event::Tx, a1: *const usize) -> usize {
    #[cfg(feature = "console")]
    println!("on load player: {:p}", a1);

    let ret = unsafe { OnLoadPlayer.call(a1) };

    let player_idx = unsafe { a1.byte_add(0x170).read() } as u32;

    let player_offset = PLAYER_DATA_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
    let weapon_offset = WEAPON_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
    let overmastery_offset = OVERMASTERY_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
    let sigil_offset = SIGIL_OFFSET.load(std::sync::atomic::Ordering::Relaxed);

    let raw_player_stats =
        std::ptr::NonNull::new(unsafe { a1.byte_add(player_offset as usize) } as *mut PlayerStats);

    let raw_weapon_info =
        std::ptr::NonNull::new(unsafe { a1.byte_add(weapon_offset as usize) } as *mut WeaponInfo);

    let raw_overmastery_info =
        std::ptr::NonNull::new(
            unsafe { a1.byte_add(overmastery_offset as usize) } as *mut Overmasteries
        );

    let sigil_list = std::ptr::NonNull::new(
        unsafe { a1.byte_add(sigil_offset as usize).read() } as *mut SigilList
    );

    if let (Some(raw_player_stats), Some(weapon_info), Some(overmastery_info), Some(sigil_list)) = (
        raw_player_stats,
        raw_weapon_info,
        raw_overmastery_info,
        sigil_list,
    ) {
        let character_type = actor_type_id(a1);
        let player_stats = unsafe { raw_player_stats.as_ref() };
        let weapon_info = unsafe { weapon_info.as_ref() };
        let overmastery_info = unsafe { overmastery_info.as_ref() };
        let sigil_list = unsafe { sigil_list.as_ref() };

        if (sigil_list.party_index as u8) == 0xFF && sigil_list.is_online == 0 {
            return ret;
        }

        let sigils = sigil_list
            .sigils
            .iter()
            .map(|sigil| protocol::Sigil {
                first_trait_id: sigil.first_trait_id,
                first_trait_level: sigil.first_trait_level,
                second_trait_id: sigil.second_trait_id,
                second_trait_level: sigil.second_trait_level,
                sigil_id: sigil.sigil_id,
                equipped_character: sigil.equipped_character,
                sigil_level: sigil.sigil_level,
                acquisition_count: sigil.acquisition_count,
                notification_enum: sigil.notification_enum,
            })
            .collect();

        let character_name = CStr::from_bytes_until_nul(&sigil_list.character_name)
            .ok()
            .map(|cstr| cstr.to_owned())
            .unwrap_or(CString::new("").unwrap());

        let display_name =
            VBuffer(std::ptr::addr_of!(sigil_list.display_name) as *const usize).raw();

        let weapon_info = protocol::WeaponInfo {
            weapon_id: weapon_info.weapon_id,
            star_level: weapon_info.star_level,
            plus_marks: weapon_info.plus_marks,
            awakening_level: weapon_info.awakening_level,
            trait_1_id: weapon_info.trait_1_id,
            trait_1_level: weapon_info.trait_1_level,
            trait_2_id: weapon_info.trait_2_id,
            trait_2_level: weapon_info.trait_2_level,
            trait_3_id: weapon_info.trait_3_id,
            trait_3_level: weapon_info.trait_3_level,
            wrightstone_id: weapon_info.wrightstone_id,
            weapon_level: weapon_info.weapon_level,
            weapon_hp: weapon_info.weapon_hp,
            weapon_attack: weapon_info.weapon_attack,
        };

        let overmastery_info = protocol::OvermasteryInfo {
            overmasteries: overmastery_info
                .stats
                .iter()
                .map(|overmastery| protocol::Overmastery {
                    id: overmastery.id,
                    flags: overmastery.flags,
                    value: overmastery.value,
                })
                .collect(),
        };

        let payload = Message::PlayerLoadEvent(protocol::PlayerLoadEvent {
            sigils,
            character_name,
            display_name,
            actor_index: player_idx,
            is_online: sigil_list.is_online != 0,
            party_index: sigil_list.party_index as u8,
            player_stats: protocol::PlayerStats {
                level: player_stats.level,
                total_hp: player_stats.total_health,
                total_attack: player_stats.total_attack,
                stun_power: player_stats.stun_power,
                critical_rate: player_stats.critical_rate,
                total_power: player_stats.total_power,
            },
            character_type,
            weapon_info,
            overmastery_info,
        });

        #[cfg(feature = "console")]
        println!("sending player load event: {:?}", payload);

        let _ = tx.send(payload);
    }

    ret
}

pub fn init(tx: event::Tx) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;

    let player_data_offset = search_slice::<u32>(
        &process,
        "3d b0 e0 7a 88 0f ? ? ? ? ? b8 b0 e0 7a 88 48 8d 8e '",
    )
    .context("Could not find player_data_offset")?;

    #[cfg(feature = "console")]
    println!("player_data_offset: {:x}", player_data_offset);

    PLAYER_DATA_OFFSET.store(player_data_offset, std::sync::atomic::Ordering::Relaxed);

    let sigil_offset = search_slice::<u32>(
        &process,
        "8b 01 eb 02 31 c0 49 8b 8c 24 ' ? ? ? ? 89 81 ? ? ? ?",
    )
    .context("Could not find sigil offset")?;

    #[cfg(feature = "console")]
    println!("sigil offsets: {:x}", sigil_offset);

    SIGIL_OFFSET.store(
        player_data_offset + sigil_offset,
        std::sync::atomic::Ordering::Relaxed,
    );

    let weapon_offset = search_slice::<u8>(&process, "48 ? ? ' ? 48 ? ? ? 48 ? ? e8 ? ? ? ? 31 ?")
        .context("Could not find weapon offset")?;

    #[cfg(feature = "console")]
    println!("weapon_offset: {:x}", weapon_offset);

    WEAPON_OFFSET.store(
        player_data_offset + weapon_offset as u32,
        std::sync::atomic::Ordering::Relaxed,
    );

    let overmastery_offset = search_slice::<u32>(
        &process,
        "49 8D 8C 24 ' ? ? ? ? 48 8D 93 ? ? ? ? E8 ? ? ? ?",
    )
    .context("Could not find overmastery offset")?;

    #[cfg(feature = "console")]
    println!("overmastery_offset: {:x}", overmastery_offset);

    OVERMASTERY_OFFSET.store(
        player_data_offset + overmastery_offset,
        std::sync::atomic::Ordering::Relaxed,
    );

    // See https://github.com/nyaoouo/GBFR-ACT/blob/5801c193de2f474764b55b7c6b759c3901dc591c/injector.py#L1773-L1809
    if let Ok(process_dmg_evt) = search_address(&process, PROCESS_DAMAGE_EVENT_SIG) {
        #[cfg(feature = "console")]
        println!("Found process dmg event");

        let tx = tx.clone();
        unsafe {
            let func: ProcessDamageEventFunc = std::mem::transmute(process_dmg_evt);
            ProcessDamageEvent.initialize(func, move |a1, a2, a3, a4| {
                process_damage_event(tx.clone(), a1, a2, a3, a4)
            })?;
            ProcessDamageEvent.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find process_dmg_evt"));
    }

    if let Ok(process_dot_evt) = search_address(&process, PROCESS_DOT_EVENT_SIG) {
        #[cfg(feature = "console")]
        println!("Found process dot event");

        let tx = tx.clone();
        unsafe {
            let func: ProcessDotEventFunc = std::mem::transmute(process_dot_evt);
            ProcessDotEvent
                .initialize(func, move |a1, a2| process_dot_event(tx.clone(), a1, a2))?;
            ProcessDotEvent.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find process_dot_evt"));
    }

    if let Ok(on_enter_area_evt) = search_address(&process, ON_ENTER_AREA_SIG) {
        #[cfg(feature = "console")]
        println!("Found on enter area");

        let tx = tx.clone();
        unsafe {
            let func: OnEnterAreaFunc = std::mem::transmute(on_enter_area_evt);
            OnEnterArea.initialize(func, move |a1, a2, a3, a4| {
                on_enter_area(tx.clone(), a1, a2, a3, a4)
            })?;
            OnEnterArea.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find on_enter_area"));
    }

    if let Ok(on_load_player_original) = search_address(&process, ON_LOAD_PLAYER) {
        #[cfg(feature = "console")]
        println!("Found on load player");

        let tx = tx.clone();
        unsafe {
            let func: OnLoadPlayerFunc = std::mem::transmute(on_load_player_original);
            OnLoadPlayer.initialize(func, move |a1| on_load_player(tx.clone(), a1))?;
            OnLoadPlayer.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find on_load_player"));
    }

    if let Ok(on_load_quest_state_original) = search_address(&process, ON_LOAD_QUEST_STATE) {
        #[cfg(feature = "console")]
        println!("Found on load quest state");

        unsafe {
            let func: OnLoadQuestState = std::mem::transmute(on_load_quest_state_original);
            OnLoadQuestState.initialize(func, move |a1| on_load_quest_state(a1))?;
            OnLoadQuestState.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find on_load_quest_state"));
    }

    if let Ok(on_show_result_screen_original) = search_address(&process, ON_SHOW_RESULT_SCREEN_SIG)
    {
        #[cfg(feature = "console")]
        println!("found on show result screen");

        let tx = tx.clone();

        unsafe {
            let func: OnShowResultScreen = std::mem::transmute(on_show_result_screen_original);
            OnShowResultScreen.initialize(func, move |a1| on_show_result_screen(tx.clone(), a1))?;
            OnShowResultScreen.enable()?;
        }
    } else {
        return Err(anyhow!("Could not find on_show_result_screen"));
    }

    Ok(())
}

fn search_slice<T>(process: &Process, signature_pattern: &str) -> Result<T> {
    let view = unsafe { PeView::module(process.module_handle.0 as *const u8) };
    let scanner = view.scanner();
    let pattern = pattern::parse(signature_pattern)?;
    let mut addrs = [0; 8];
    let matches = scanner.matches_code(&pattern).next(&mut addrs);

    if matches {
        let addr = process.base_address + addrs[1] as usize;
        let ptr = addr as *const T;
        Ok(unsafe { ptr.read_unaligned() })
    } else {
        return Err(anyhow!(
            "Could not find match for pattern: {}",
            signature_pattern
        ));
    }
}

/// Searches and returns the RVAs of the function that matches the given signature pattern.
fn search_address(process: &Process, signature_pattern: &str) -> Result<usize> {
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
