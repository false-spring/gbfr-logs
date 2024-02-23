use anyhow::Result;
use log::info;
use pelite::{
    pattern,
    pe64::{Pe, PeView},
};

use crate::process::Process;

/// Searches and returns the RVAs of the function that matches the given signature pattern.
pub fn search(signature_pattern: &str) -> Result<()> {
    let process = Process::with_name("granblue_fantasy_relink.exe")?;
    let view = unsafe { PeView::module(process.module_handle.0 as *const u8) };
    let scanner = view.scanner();
    let pattern = pattern::parse(signature_pattern)?;

    let mut addrs = [0; 8];

    let mut matches = scanner.matches_code(&pattern);

    // addrs[0] = RVA of where the match was found.
    // addrs[1] = RVA of the function being called.
    while matches.next(&mut addrs) {
        info!("Found match {:?} for pattern: {}", addrs, signature_pattern);
    }

    Ok(())
}
