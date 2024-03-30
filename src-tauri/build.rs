use std::fs;

use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    if cfg!(debug_assertions) {
        // Copy the built library to the tauri app directory
        let _ = fs::copy("../target/release/hook.dll", "hook.dll");

        tauri_build::build();
    } else {
        let windows = WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

        tauri_build::try_build(Attributes::new().windows_attributes(windows))
            .expect("Could not build Tauri app.")
    }
}
