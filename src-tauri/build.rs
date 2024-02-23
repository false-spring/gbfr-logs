use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    let windows = WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

    tauri_build::try_build(Attributes::new().windows_attributes(windows))
        .expect("Could not build Tauri app.")
}
