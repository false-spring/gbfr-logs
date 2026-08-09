use anyhow::anyhow;
use pelite::{
    pattern,
    pe64::{Pe, PeView},
};
use thiserror::Error;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Process32FirstW, Process32NextW, MODULEENTRY32W,
    PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Process was not found with that name")]
    ProcessNotFound,
    #[error("Could not snapshot process")]
    ProcessSnapshotError(windows::core::Error),
    #[error("Could not snapshot process memory")]
    ModuleSnapshotError(windows::core::Error),
}

pub struct Process {
    pub base_address: usize,
    pub module_handle: HMODULE,
}

impl Process {
    /// Finds a process by its name.
    pub fn with_name(name: &str) -> Result<Process, ProcessError> {
        let mut found_process = None;

        unsafe {
            let snapshot_handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(ProcessError::ProcessSnapshotError)?;

            let mut process = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..PROCESSENTRY32W::default()
            };

            if Process32FirstW(snapshot_handle, &mut process).is_ok() {
                loop {
                    if Process32NextW(snapshot_handle, &mut process).is_ok() {
                        let process_name = String::from_utf16_lossy(&process.szExeFile)
                            .trim_end_matches('\u{0}')
                            .to_string();

                        if process_name == name {
                            let module_snapshot = CreateToolhelp32Snapshot(
                                TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                                process.th32ProcessID,
                            )
                            .map_err(ProcessError::ModuleSnapshotError)?;

                            let mut module_entry = MODULEENTRY32W {
                                dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                                ..MODULEENTRY32W::default()
                            };

                            if Module32FirstW(module_snapshot, &mut module_entry).is_ok() {
                                let module_name = String::from_utf16_lossy(&process.szExeFile)
                                    .trim_end_matches('\u{0}')
                                    .to_string();

                                if module_name == name {
                                    let base_address = module_entry.modBaseAddr as usize;
                                    let module_handle = module_entry.hModule;

                                    found_process = Some(Process {
                                        base_address,
                                        module_handle,
                                    });
                                }
                            } else {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        found_process.ok_or(ProcessError::ProcessNotFound)
    }

    /// Absolute VA bounds of the module's executable code, `[start, end)`.
    pub fn code_range(&self) -> (usize, usize) {
        let view = unsafe { PeView::module(self.module_handle.0 as *const u8) };
        let opt = view.optional_header();
        let start = self.base_address + opt.BaseOfCode as usize;
        (start, start + opt.SizeOfCode as usize)
    }

    /// Absolute VA bounds of the whole mapped image, `[base, base+SizeOfImage)`.
    pub fn image_range(&self) -> (usize, usize) {
        let view = unsafe { PeView::module(self.module_handle.0 as *const u8) };
        let size = view.optional_header().SizeOfImage as usize;
        (self.base_address, self.base_address + size)
    }

    /// Searches and returns the RVAs of the function that matches the given
    /// signature pattern. Keeps the LAST match; `search_slice` keeps the FIRST.
    pub fn search_address(&self, signature_pattern: &str) -> anyhow::Result<usize> {
        let view = unsafe { PeView::module(self.module_handle.0 as *const u8) };
        let scanner = view.scanner();
        let pattern = pattern::parse(signature_pattern)?;

        let mut addrs = [0; 8];

        let mut matches = scanner.matches_code(&pattern);

        let mut chosen = None;
        let mut count = 0usize;

        // addrs[0] = RVA of where the match was found.
        // addrs[1] = RVA of the function being called.
        while matches.next(&mut addrs) {
            count += 1;
            chosen = Some(self.base_address + addrs[1] as usize);
        }

        if count > 1 {
            log::warn!(
                "signature is NOT unique ({count} matches), binding to the LAST — \
                 verify it is the intended target: {signature_pattern}"
            );
        }

        chosen.ok_or(anyhow!(
            "Could not find match for pattern: {}",
            signature_pattern
        ))
    }

    /// Absolute address of where a pattern's match begins. Keeps the LAST match.
    pub fn search_address_start(&self, signature_pattern: &str) -> anyhow::Result<usize> {
        let view = unsafe { PeView::module(self.module_handle.0 as *const u8) };
        let scanner = view.scanner();
        let pattern = pattern::parse(signature_pattern)?;

        let mut addrs = [0; 8];

        let mut matches = scanner.matches_code(&pattern);

        let mut chosen = None;
        let mut count = 0usize;

        // pelite prepends `Save(0)`, so addrs[0] is where the match starts.
        while matches.next(&mut addrs) {
            count += 1;
            chosen = Some(self.base_address + addrs[0] as usize);
        }

        if count > 1 {
            log::warn!(
                "prologue signature is NOT unique ({count} matches), binding to the LAST — \
                 verify it is the intended target: {signature_pattern}"
            );
        }

        chosen.ok_or(anyhow!(
            "Could not find match for pattern: {}",
            signature_pattern
        ))
    }

    /// Searches and returns the value of the type `T` that matches the given
    /// signature pattern. Takes the FIRST match.
    pub fn search_slice<T>(&self, signature_pattern: &str) -> anyhow::Result<T> {
        let view = unsafe { PeView::module(self.module_handle.0 as *const u8) };
        let scanner = view.scanner();
        let pattern = pattern::parse(signature_pattern)?;
        let mut addrs = [0; 8];
        let matches = scanner.matches_code(&pattern).next(&mut addrs);

        if matches {
            let addr = self.base_address + addrs[1] as usize;
            let ptr = addr as *const T;
            Ok(unsafe { ptr.read_unaligned() })
        } else {
            Err(anyhow!(
                "Could not find match for pattern: {}",
                signature_pattern
            ))
        }
    }
}
