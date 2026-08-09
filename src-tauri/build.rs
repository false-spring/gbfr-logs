use std::fs;

use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    // Stage the built hook.dll next to the manifest: `src-tauri/hook.dll` is
    // the bundle resource (see tauri.conf.json), so this copy is what ships.
    // The rerun-if-changed keeps cargo re-running this script after a hook
    // rebuild (tauri_build's own directives disable the default rerun rule);
    // without it a stale hook.dll ships against a newer parser and mismatched
    // bincode messages get dropped silently.
    println!("cargo:rerun-if-changed=../target/release/hook.dll");
    let copied = fs::copy("../target/release/hook.dll", "hook.dll");

    if cfg!(debug_assertions) {
        // Best-effort: a debug build (including `cargo test`) may have no
        // release hook, and `tauri dev` injects straight from target/release.
        let _ = copied;

        tauri_build::build();
    } else {
        // Strict: this copy goes into the installer.
        copied.expect(
            "cannot stage hook.dll into the bundle — build it first with \
             `cargo build --release --package hook` (npm run build does this)",
        );

        let windows = WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

        tauri_build::try_build(Attributes::new().windows_attributes(windows))
            .expect("Could not build Tauri app.")
    }
}
