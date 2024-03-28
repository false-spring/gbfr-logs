use std::ffi::{CStr, CString};

use anyhow::{anyhow, Result};
use protocol::Message;
use retour::static_detour;

use crate::{
    event,
    hooks::{
        actor_type_id,
        ffi::{Overmasteries, PlayerStats, SigilList, VBuffer, WeaponInfo},
        globals::{OVERMASTERY_OFFSET, PLAYER_DATA_OFFSET, SIGIL_OFFSET, WEAPON_OFFSET},
    },
    process::Process,
};

type OnLoadPlayerFunc = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static OnLoadPlayer: unsafe extern "system" fn(*const usize) -> usize;
}

#[derive(Clone)]
pub struct OnLoadPlayerHook {
    tx: event::Tx,
}

impl OnLoadPlayerHook {
    pub fn new(tx: event::Tx) -> Self {
        OnLoadPlayerHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_load_player_original) =
            process.search_address("49 89 ce e8 $ { ' } 31 ff 85 c0 ? ? ? ? ? ? 49 8b 46 28")
        {
            #[cfg(feature = "console")]
            println!("Found on load player");

            unsafe {
                let func: OnLoadPlayerFunc = std::mem::transmute(on_load_player_original);
                OnLoadPlayer.initialize(func, move |a1| cloned_self.run(a1))?;
                OnLoadPlayer.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_load_player"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("on load player: {:p}", a1);

        let ret = unsafe { OnLoadPlayer.call(a1) };

        let player_idx = unsafe { a1.byte_add(0x170).read() } as u32;

        let player_offset = PLAYER_DATA_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
        let weapon_offset = WEAPON_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
        let overmastery_offset = OVERMASTERY_OFFSET.load(std::sync::atomic::Ordering::Relaxed);
        let sigil_offset = SIGIL_OFFSET.load(std::sync::atomic::Ordering::Relaxed);

        let raw_player_stats = std::ptr::NonNull::new(
            unsafe { a1.byte_add(player_offset as usize) } as *mut PlayerStats,
        );

        let raw_weapon_info = std::ptr::NonNull::new(
            unsafe { a1.byte_add(weapon_offset as usize) } as *mut WeaponInfo,
        );

        let raw_overmastery_info =
            std::ptr::NonNull::new(
                unsafe { a1.byte_add(overmastery_offset as usize) } as *mut Overmasteries
            );

        let sigil_list = std::ptr::NonNull::new(
            unsafe { a1.byte_add(sigil_offset as usize).read() } as *mut SigilList,
        );

        if let (
            Some(raw_player_stats),
            Some(weapon_info),
            Some(overmastery_info),
            Some(sigil_list),
        ) = (
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

            let _ = self.tx.send(payload);
        }

        ret
    }
}
