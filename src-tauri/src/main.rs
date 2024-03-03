// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, path::Path};

use dll_syringe::{process::OwnedProcess, Syringe};
use futures::io::AsyncReadExt;
use interprocess::os::windows::named_pipe::tokio::MsgReaderPipeStream;
use parser::{EncounterState, Parser};
use rusqlite::params_from_iter;
use serde::Serialize;
use tauri::{AppHandle, Manager};

mod db;
mod parser;

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
    dps_chart: HashMap<u32, Vec<i32>>,
    chart_len: usize,
}

#[tauri::command]
fn fetch_encounter_state(id: u64) -> Result<EncounterStateResponse, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let serialized_parser: Vec<u8> = stmt
        .query_row([id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let parser: Parser =
        protocol::bincode::deserialize(&serialized_parser).map_err(|e| e.to_string())?;

    let duration = parser.encounter_state.end_time - parser.encounter_state.start_time;

    let mut player_dps: HashMap<u32, Vec<i32>> = HashMap::new();

    const DPS_INTERVAL: i64 = 5 * 1_000;

    for player in parser.encounter_state.party.values() {
        player_dps.insert(
            player.index,
            vec![0; (duration / DPS_INTERVAL) as usize + 1],
        );
    }

    for (timestamp, damage_event) in parser.damage_event_log.iter() {
        let index = ((timestamp - parser.encounter_state.start_time) / DPS_INTERVAL) as usize;
        if let Some(chart) = player_dps.get_mut(&damage_event.source.parent_index) {
            chart[index] += damage_event.damage;
        }
    }

    Ok(EncounterStateResponse {
        encounter_state: parser.encounter_state,
        dps_chart: player_dps,
        chart_len: (duration / DPS_INTERVAL) as usize + 1,
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

fn main() {
    // Setup the database.
    db::setup_db().expect("Failed to setup database");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            load_parse_log_from_file,
            fetch_encounter_state,
            fetch_logs,
            delete_logs
        ])
        .setup(|app| {
            // Perform the game hook check in a separate thread.
            tauri::async_runtime::spawn(check_and_perform_hook(app.handle()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
