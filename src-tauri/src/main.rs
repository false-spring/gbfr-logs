// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use dll_syringe::{process::OwnedProcess, Syringe};
use futures::io::AsyncReadExt;
use interprocess::os::windows::named_pipe::tokio::MsgReaderPipeStream;
use parser::{
    constants::{CharacterType, EnemyType},
    EncounterState, Parser,
};
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use tauri::{
    api::dialog::blocking::FileDialogBuilder, AppHandle, CustomMenuItem, Manager, State,
    SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};

mod db;
mod parser;

// @TODO(false): Swap these results to return a proper error type, instead of stringified errors.

#[tauri::command]
async fn export_damage_log_to_file(id: u32, options: ParseOptions) -> Result<(), String> {
    let file_path = FileDialogBuilder::new()
        .add_filter("csv", &["csv"])
        .set_file_name(&format!("{id}_damage_log.csv"))
        .set_title("Export Damage Log")
        .save_file()
        .ok_or("No file selected!")?;

    let conn = db::connect_to_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT data FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let blob: Vec<u8> = stmt
        .query_row([id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let parser = Parser::from_blob(&blob).map_err(|e| e.to_string())?;
    let file = File::create(&file_path).map_err(|e| e.to_string())?;

    // @TODO(false): Split formatting into a separate function.
    let mut writer = std::io::BufWriter::new(file);

    writer
        .write_all(
            b"timestamp,source_type,source_index,target_type,target_index,action_id,flags,damage\n",
        )
        .map_err(|e| e.to_string())?;

    for damage_event in parser.damage_event_log.iter() {
        let target_type = EnemyType::from_hash(damage_event.1.target.parent_actor_type);

        if options.targets.is_empty() || options.targets.contains(&target_type) {
            let timestamp = damage_event.0 - parser.encounter_state.start_time;

            let line = format!(
                "{},{},{},{},{},{:?},{},{}\n",
                timestamp,
                CharacterType::from_hash(damage_event.1.source.parent_actor_type),
                damage_event.1.source.parent_index,
                EnemyType::from_hash(damage_event.1.target.parent_actor_type),
                damage_event.1.target.parent_index,
                damage_event.1.action_id,
                damage_event.1.flags,
                damage_event.1.damage
            );

            writer
                .write_all(line.as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }

    writer.flush().map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    logs: Vec<LogEntry>,
    page: u32,
    page_count: u32,
    log_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    /// The ID of the log entry.
    id: u64,
    /// The name of the log.
    name: String,
    /// Milliseconds since UNIX epoch.
    time: i64,
    /// Duration of the encounter in milliseconds.
    duration: i64,
}

#[tauri::command]
fn fetch_logs(page: Option<u32>) -> Result<SearchResult, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let page = page.unwrap_or(1);
    let per_page = 10;
    let offset = page.saturating_sub(1) * per_page;

    let mut stmt = conn
        .prepare("SELECT id, name, time, duration FROM logs ORDER BY time DESC LIMIT ? OFFSET ?")
        .map_err(|e| e.to_string())?;

    let logs = stmt
        .query_map([per_page, offset], |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                time: row.get(2)?,
                duration: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let log_count: u32 = conn
        .query_row_and_then("SELECT COUNT(*) FROM logs", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let page_count = (log_count as f64 / per_page as f64).ceil() as u32;

    Ok(SearchResult {
        logs,
        page,
        page_count,
        log_count,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EncounterStateResponse {
    encounter_state: EncounterState,
    targets: Vec<EnemyType>,
    dps_chart: HashMap<u32, Vec<i32>>,
    chart_len: usize,
}

#[derive(Debug, Deserialize)]
struct ParseOptions {
    targets: Vec<EnemyType>,
}

#[tauri::command]
fn fetch_encounter_state(id: u64, options: ParseOptions) -> Result<EncounterStateResponse, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let blob: Vec<u8> = stmt
        .query_row([id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut parser: Parser = Parser::from_blob(&blob).map_err(|e| e.to_string())?;

    parser.reparse(&options.targets);

    let duration = parser.encounter_state.end_time - parser.encounter_state.start_time;
    let mut player_dps: HashMap<u32, Vec<i32>> = HashMap::new();

    const DPS_INTERVAL: i64 = 5 * 1_000;

    for player in parser.encounter_state.party.values() {
        player_dps.insert(
            player.index,
            vec![0; (duration / DPS_INTERVAL) as usize + 1],
        );
    }

    let mut targets = Vec::new();

    for (timestamp, damage_event) in parser.damage_event_log.iter() {
        let index = ((timestamp - parser.encounter_state.start_time) / DPS_INTERVAL) as usize;
        let target_type = EnemyType::from_hash(damage_event.target.parent_actor_type);

        if !targets.contains(&target_type) {
            targets.push(target_type);
        }

        if let Some(chart) = player_dps.get_mut(&damage_event.source.parent_index) {
            // Check to see if the target is in the list of targets to filter by.
            if options.targets.is_empty() || options.targets.contains(&target_type) {
                chart[index] += damage_event.damage;
            }
        }
    }

    Ok(EncounterStateResponse {
        encounter_state: parser.encounter_state,
        dps_chart: player_dps,
        chart_len: (duration / DPS_INTERVAL) as usize + 1,
        targets,
    })
}

#[tauri::command]
fn delete_logs(ids: Vec<u64>) -> Result<(), String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;

    let id_params: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let param = id_params.join(",");

    let sql = format!("DELETE FROM logs WHERE id IN ({})", param);
    let mut statement = conn.prepare_cached(&sql).map_err(|e| e.to_string())?;
    statement
        .execute(params_from_iter(ids))
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn load_parse_log_from_file(path: String) -> Result<Parser, String> {
    Parser::load_parse_log_from_file(&path).map_err(|e| e.to_string())
}

// Continuously check for the game process and inject the DLL when found.
async fn check_and_perform_hook(app: AppHandle) {
    loop {
        match OwnedProcess::find_first_by_name("granblue_fantasy_relink.exe") {
            Some(target) => {
                let syringe = Syringe::for_process(target);
                let debug_dll_path = Path::new("hook-dbg.dll");
                let mut dll_path = Path::new("hook.dll");

                // If the debug DLL is present, use it instead.
                if debug_dll_path.exists() {
                    dll_path = debug_dll_path;
                }

                let _ = syringe.inject(dll_path);
                let _ = app.emit_all("success-alert", "Found game..");

                connect_and_run_parser(app);

                break;
            }
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    }
}

// Connect to the game hook event channel and listen for damage events.
fn connect_and_run_parser(app: AppHandle) {
    let window = app.get_window("main").expect("Window not found");
    let database = db::connect_to_db().expect("Could not connect to database");
    let mut state = Parser::new(Some(window.clone()), database);

    tauri::async_runtime::spawn(async move {
        loop {
            match MsgReaderPipeStream::connect(protocol::PIPE_NAME) {
                Ok(mut stream) => {
                    let _ = app.emit_all("success-alert", "Connnected to game!");
                    let mut buffer = [0; 1024];
                    while let Ok(msg) = stream.read(&mut buffer).await {
                        if let Ok(msg) =
                            protocol::bincode::deserialize::<protocol::Message>(&buffer[..msg])
                        {
                            match msg {
                                protocol::Message::DamageEvent(event) => {
                                    state.on_damage_event(event);
                                }
                                protocol::Message::OnAreaEnter => {
                                    state.on_area_enter_event();
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });
}

fn system_tray_with_menu() -> SystemTray {
    let meter = CustomMenuItem::new("open_meter", "Open Meter");
    let logs = CustomMenuItem::new("open_logs", "Open Logs");
    let always_on_top = CustomMenuItem::new("always_on_top", "Always on top");
    let toggle_clickthrough = CustomMenuItem::new("toggle_clickthrough", "Clickthrough");
    let quit = CustomMenuItem::new("quit", "Quit");

    let menu = SystemTrayMenu::new()
        .add_item(meter)
        .add_item(logs)
        .add_item(always_on_top)
        .add_item(toggle_clickthrough)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);
    SystemTray::new().with_menu(menu)
}

fn toggle_window_visibility(handle: &AppHandle, id: &str, focus: Option<bool>) {
    if let Some(window) = handle.get_window(id) {
        if let Some(focus_value) = focus {
            if focus_value {
                window.set_focus().unwrap();
            }
        }

        match window.is_visible().unwrap() {
            true => window.hide().unwrap(),
            false => window.show().unwrap(),
        }
    }
}

struct AlwaysOnTop(AtomicBool);
#[tauri::command]
fn toggle_always_on_top(window: tauri::Window, state: State<AlwaysOnTop>) {
    let always_on_top = &state.0;
    let new_state = !always_on_top.load(Ordering::Acquire);
    always_on_top.store(new_state, Ordering::Release);
    window.set_always_on_top(new_state).unwrap();
}

struct ClickThrough(AtomicBool);
#[tauri::command]
fn toggle_clickthrough(window: tauri::Window, state: State<ClickThrough>) {
    let click_through = &state.0;
    let new_state = !click_through.load(Ordering::Acquire);
    click_through.store(new_state, Ordering::Release);
    window.set_ignore_cursor_events(new_state).unwrap();
}

fn menu_tray_handler(handle: &AppHandle, event: SystemTrayEvent) {
    let should_focus = true;
    match event {
        SystemTrayEvent::LeftClick { .. } => {
            toggle_window_visibility(handle, "main", Some(should_focus))
        }
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            "open_meter" => toggle_window_visibility(handle, "main", Some(should_focus)),
            "open_logs" => toggle_window_visibility(handle, "logs", Some(should_focus)),
            "toggle_clickthrough" => toggle_clickthrough(
                handle.get_window("main").unwrap(),
                handle.state::<ClickThrough>(),
            ),
            "always_on_top" => toggle_always_on_top(
                handle.get_window("main").unwrap(),
                handle.state::<AlwaysOnTop>(),
            ),
            "quit" => handle.exit(0),

            _ => {}
        },
        _ => {} // Ignore rest of the events.
    }
}

fn main() {
    // Setup the database.
    db::setup_db().expect("Failed to setup database");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AlwaysOnTop(AtomicBool::new(false)))
        .manage(ClickThrough(AtomicBool::new(false)))
        .system_tray(system_tray_with_menu())
        .on_system_tray_event(menu_tray_handler)
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_parse_log_from_file,
            fetch_encounter_state,
            fetch_logs,
            delete_logs,
            toggle_always_on_top,
            export_damage_log_to_file
        ])
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .setup(|app| {
            // Perform the game hook check in a separate thread.
            tauri::async_runtime::spawn(check_and_perform_hook(app.handle()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
