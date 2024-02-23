use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Process32FirstW, Process32NextW, MODULEENTRY32W,
    PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};

#[derive(Debug)]
pub enum ProcessError {
    ProcessNotFound,
    ProcessSnapshotError(windows::core::Error),
    ModuleSnapshotError(windows::core::Error),
}

pub struct Process {
    pub pid: u32,
    pub name: String,
    pub base_address: usize,
}

impl Process {
    /// Finds a process by its name.
    pub fn with_name(name: &str) -> Result<Process, ProcessError> {
        let mut found_process = None;

        unsafe {
            let snapshot_handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(ProcessError::ProcessSnapshotError)?;

            let mut process = PROCESSENTRY32W::default();
            process.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot_handle, &mut process).is_ok() {
                loop {
                    if Process32NextW(snapshot_handle, &mut process).is_ok() {
                        let process_name = String::from_utf16_lossy(&process.szExeFile)
                            .trim_end_matches('\u{0}')
                            .to_string();

                        if process_name == name {
                            println!("Found process!");

                            let module_snapshot = CreateToolhelp32Snapshot(
                                TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                                process.th32ProcessID,
                            )
                            .map_err(ProcessError::ModuleSnapshotError)?;
                            let mut module_entry = MODULEENTRY32W::default();
                            module_entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

                            if Module32FirstW(module_snapshot, &mut module_entry).is_ok() {
                                let module_name = String::from_utf16_lossy(&process.szExeFile)
                                    .trim_end_matches('\u{0}')
                                    .to_string();

                                if module_name == name {
                                    let base_address = module_entry.modBaseAddr as usize;

                                    found_process = Some(Process {
                                        pid: process.th32ProcessID,
                                        name: process_name,
                                        base_address,
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
}
