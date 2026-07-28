// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::AtomicBool;

use log::{info, LevelFilter};
use tauri::{AppHandle, LogicalSize, Manager, Size};
use tauri_plugin_log::LogTarget;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

use gbfr_logs::{commands, db, hook_listener, tray};

use commands::{AlwaysOnTop, ClickThrough, DebugMode};

fn show_window(app: &AppHandle) {
    let windows = app.windows();

    for window in windows.values() {
        let _ = window.show();
    }
}

fn main() {
    info!("Starting application..");

    // Setup the database.
    db::setup_db().expect("Failed to setup database");

    db::backfill_style_columns();

    info!("Database setup complete, launching application..");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([LogTarget::Folder("logs".into()), LogTarget::Stdout])
                .level(LevelFilter::Warn)
                .level_for("tao", LevelFilter::Error)
                .build(),
        )
        .manage(AlwaysOnTop(AtomicBool::new(true)))
        .manage(ClickThrough(AtomicBool::new(false)))
        .manage(DebugMode(AtomicBool::new(false)))
        .system_tray(tray::system_tray_with_menu())
        .on_system_tray_event(tray::menu_tray_handler)
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                // "Closing" only hides the window, so the plugin's on-exit save never fires.
                let _ = event.window().app_handle().save_window_state(StateFlags::all());
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::fetch_encounter_state,
            commands::fetch_enemy_encounter_state,
            commands::fetch_logs,
            commands::bug_report_payload,
            commands::delete_logs,
            commands::delete_all_logs,
            commands::toggle_always_on_top,
            commands::export_damage_log_to_file,
            commands::set_debug_mode,
        ])
        .setup(|app| {
            // Clamp restored sizes up to the config's own minimums; a size saved
            // under an older, smaller minimum forces a manual resize on launch.
            let window_floors: Vec<(String, f64, f64)> = app
                .config()
                .tauri
                .windows
                .iter()
                .filter_map(|config| {
                    Some((
                        config.label.clone(),
                        config.min_width?,
                        config.min_height?,
                    ))
                })
                .collect();

            for (label, min_w, min_h) in window_floors {
                if let Some(window) = app.get_window(&label) {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    if let Ok(size) = window.inner_size() {
                        let logical = size.to_logical::<f64>(scale);
                        let width = logical.width.max(min_w);
                        let height = logical.height.max(min_h);
                        if width > logical.width || height > logical.height {
                            log::info!(
                                "[window] {label} restored at {:.0}x{:.0}, below its {min_w:.0}x{min_h:.0} \
                                 minimum — scaling up",
                                logical.width,
                                logical.height
                            );
                            let _ = window.set_size(Size::Logical(LogicalSize { width, height }));
                        }
                    }
                }
            }

            // Perform the game hook check in a separate thread.
            tauri::async_runtime::spawn(hook_listener::check_and_perform_hook(app.handle()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
