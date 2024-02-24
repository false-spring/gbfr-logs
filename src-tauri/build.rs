use std::fs;
use std::process::Command;

use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    // Build hook library
    let hook_lib_path = fs::canonicalize("../src-hook").unwrap();

    Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(&hook_lib_path)
        .status()
        .expect("Could not build hook library.");

    // Copy the built library to the tauri app directory
    fs::copy(hook_lib_path.join("target/release/hook.dll"), "hook.dll")
        .expect("Could not copy hook library.");

    let windows = WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

    tauri_build::try_build(Attributes::new().windows_attributes(windows))
        .expect("Could not build Tauri app.")
}
