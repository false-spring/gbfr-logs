use tauri_build::WindowsAttributes;

fn main() {
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("manifest.xml"));

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("Could not build Tauri app.")
}
