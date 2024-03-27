use std::ffi::CString;

#[derive(Debug)]
#[repr(C)]
pub struct QuestState {
    pub quest_id: u32,        // 0x00
    padding_640: [u8; 0x644], // 0x004 - 0x648
    pub elapsed_time: u32,    // 0x648
}

#[derive(Debug)]
#[repr(C)]
pub struct SigilEntry {
    pub first_trait_id: u32,
    pub first_trait_level: u32,
    pub second_trait_id: u32,
    pub second_trait_level: u32,
    pub sigil_id: u32,
    pub equipped_character: u32,
    pub sigil_level: u32,
    pub acquisition_count: u32,
    pub notification_enum: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct SigilList {
    pub sigils: [SigilEntry; 12], // 0x00
    unk_1b0: u32,                 //0x01B0
    unk_1b4: u32,                 //0x01B4
    unk_1b8: u32,                 //0x01B8
    unk_1bc: u32,                 //0x01BC
    unk_1c0: u32,                 //0x01C0
    unk_1c4: u32,                 //0x01C4
    /// 0 == local, 1 == online
    pub is_online: u32, //0x01C8
    unk_1cc: u32,                 //0x01CC
    unk_1d0: u32,                 //0x01D0
    unk_1d4: u32,                 //0x01D4
    unk_1d8: u32,                 //0x01D8
    unk_1dc: u32,                 //0x01DC
    unk_1e0: u32,                 //0x01E0
    unk_1e4: u32,                 //0x01E4
    pub character_name: [u8; 16], //0x01E8
    padding_1f8: [u8; 16],        //0x01F8
    pub display_name: [u8; 16],   //0x0208
    padding_218: [u8; 24],        //0x0218
    pub party_index: u32,         //0x0230
}

#[derive(Debug)]
#[repr(C)]
pub struct PlayerStats {
    pub level: u32,
    pub total_health: u32,
    pub total_attack: u32,
    pub unk_0c: u32,
    pub stun_power: f32,
    pub critical_rate: f32,
    pub total_power: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct WeaponInfo {
    unk_00: u32,
    /// Weapon ID Hash
    pub weapon_id: u32,
    pub weapon_ap_tree: u32,
    unk_0c: u32,
    pub weapon_exp: u32,
    /// How many uncap stars the weapon has
    pub star_level: u32,
    /// Number of plus marks on the weapon
    pub plus_marks: u32,
    /// Weapon's awakening level
    pub awakening_level: u32,
    /// First trait ID
    pub trait_1_id: u32,
    /// First trait level
    pub trait_1_level: u32,
    /// Second trait ID
    pub trait_2_id: u32,
    /// Second trait level
    pub trait_2_level: u32,
    /// Third trait ID
    pub trait_3_id: u32,
    /// Third trait level
    pub trait_3_level: u32,
    /// Wrightstone used on the weapon
    pub wrightstone_id: u32,
    unk_3c: u32,
    /// Current weapon level
    pub weapon_level: u32,
    /// Weapon's HP Stats (before plus marks)
    pub weapon_hp: u32,
    /// Weapon's Attack Stats (before plus marks)
    pub weapon_attack: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct Overmastery {
    /// Overmastery Stats ID type
    pub id: u32,
    /// Flags
    pub flags: u32,
    unk_08: u32,
    /// Value for the overmastery
    pub value: f32,
}

#[derive(Debug)]
#[repr(C)]
pub struct Overmasteries {
    pub stats: [Overmastery; 4],
}

pub struct VBuffer(pub *const usize);

impl VBuffer {
    pub fn ptr(&self) -> *const usize {
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

    pub fn raw(&self) -> CString {
        let bytes =
            unsafe { std::slice::from_raw_parts(self.ptr() as *const u8, self.used_size()) };

        unsafe { CString::from_vec_unchecked(bytes.to_vec()) }
    }
}
