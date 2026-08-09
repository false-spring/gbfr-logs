use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicUsize};

use anyhow::{Context, Result};

use crate::hooks::ffi::QuestState;
use crate::process::Process;

pub static QUEST_STATE_PTR: AtomicPtr<QuestState> = AtomicPtr::new(ptr::null_mut());
pub static SBA_OFFSET: AtomicU32 = AtomicU32::new(0);
pub static MODULE_BASE: AtomicUsize = AtomicUsize::new(0);

pub fn setup_globals(process: &Process) -> Result<()> {
    MODULE_BASE.store(process.base_address, std::sync::atomic::Ordering::Relaxed);

    // The captured displacement is 0x3230, the gauge struct. Its value is at +0x7C.
    let sba_offset = process
        .search_slice::<u32>("c5 fa 59 ? ? ? ? ? 48 81 c7 ' ? ? ? ? c5 f8 54 0d")
        .context("Could not find sba offset")?;

    #[cfg(feature = "console")]
    println!("sba_offset: {:x}", sba_offset);

    SBA_OFFSET.store(sba_offset, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}
