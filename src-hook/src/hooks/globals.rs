use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32};

use anyhow::{Context, Result};

use crate::hooks::ffi::QuestState;
use crate::process::Process;

pub static QUEST_STATE_PTR: AtomicPtr<QuestState> = AtomicPtr::new(ptr::null_mut());
pub static PLAYER_DATA_OFFSET: AtomicU32 = AtomicU32::new(0);
pub static WEAPON_OFFSET: AtomicU32 = AtomicU32::new(0);
pub static OVERMASTERY_OFFSET: AtomicU32 = AtomicU32::new(0);
pub static SIGIL_OFFSET: AtomicU32 = AtomicU32::new(0);
pub static SBA_OFFSET: AtomicU32 = AtomicU32::new(0);

pub fn setup_globals(process: &Process) -> Result<()> {
    let player_data_offset = process
        .search_slice::<u32>("3d b0 e0 7a 88 0f ? ? ? ? ? b8 b0 e0 7a 88 48 8d 8e '")
        .context("Could not find player_data_offset")?;

    #[cfg(feature = "console")]
    println!("player_data_offset: {:x}", player_data_offset);

    PLAYER_DATA_OFFSET.store(player_data_offset, std::sync::atomic::Ordering::Relaxed);

    let sigil_offset = process
        .search_slice::<u32>("8b 01 eb 02 31 c0 49 8b 8c 24 ' ? ? ? ? 89 81 ? ? ? ?")
        .context("Could not find sigil offset")?;

    #[cfg(feature = "console")]
    println!("sigil offsets: {:x}", sigil_offset);

    SIGIL_OFFSET.store(
        player_data_offset + sigil_offset,
        std::sync::atomic::Ordering::Relaxed,
    );

    let weapon_offset = process
        .search_slice::<u8>("48 ? ? ' ? 48 ? ? ? 48 ? ? e8 ? ? ? ? 31 ?")
        .context("Could not find weapon offset")?;

    #[cfg(feature = "console")]
    println!("weapon_offset: {:x}", weapon_offset);

    WEAPON_OFFSET.store(
        player_data_offset + weapon_offset as u32,
        std::sync::atomic::Ordering::Relaxed,
    );

    let overmastery_offset = process
        .search_slice::<u32>("49 8D 8C 24 ' ? ? ? ? 48 8D 93 ? ? ? ? E8 ? ? ? ?")
        .context("Could not find overmastery offset")?;

    #[cfg(feature = "console")]
    println!("overmastery_offset: {:x}", overmastery_offset);

    OVERMASTERY_OFFSET.store(
        player_data_offset + overmastery_offset,
        std::sync::atomic::Ordering::Relaxed,
    );

    let sba_offset = process
        .search_slice::<u32>("7E ? C5 FA 59 81 ? ? ? ? 48 81 C1 ' ? ? ? ? C5 F8 54 0D ? ? ? ?")
        .context("Could not find sba offset")?;

    #[cfg(feature = "console")]
    println!("sba_offset: {:x}", sba_offset);

    SBA_OFFSET.store(sba_offset, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}
