use std::fs;
use std::process::Command;

use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    // Build hook library
    println!("cargo:rerun-if-changed=../src-hook/src");
    let hook_lib_path = fs::canonicalize("../src-hook").unwrap();

    Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(&hook_lib_path)
        .status()
        .expect("Could not build hook library.");

    // Copy the built library to the tauri app directory
    let _ = fs::copy(hook_lib_path.join("target/release/hook.dll"), "hook.dll");

    if cfg!(debug_assertions) {
        tauri_build::build();
    } else {
        let windows = WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

        tauri_build::try_build(Attributes::new().windows_attributes(windows))
            .expect("Could not build Tauri app.")
    }
}
