// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Context;
use db::logs::LogEntry;
use dll_syringe::{process::OwnedProcess, Syringe};
use interprocess::os::windows::named_pipe::tokio::RecvPipeStream;
use log::{info, LevelFilter};
use parser::{
    constants::{CharacterType, EnemyType},
    v1::{self, PlayerData},
};
use protocol::Message;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use tauri::{
    api::dialog::blocking::FileDialogBuilder, AppHandle, CustomMenuItem, LogicalSize, Manager,
    Size, State, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};
use tauri_plugin_log::LogTarget;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

mod db;
mod parser;

struct AlwaysOnTop(AtomicBool);
struct ClickThrough(AtomicBool);
struct DebugMode(AtomicBool);

#[tauri::command]
fn set_debug_mode(app: AppHandle, state: State<DebugMode>, enabled: bool) {
    if let Some(window) = app.get_window("logs") {
        if enabled {
            window.open_devtools()
        } else {
            window.close_devtools()
        }
    }

    state.0.store(enabled, Ordering::Release);
}

#[tauri::command]
async fn delete_all_logs() -> Result<(), String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM logs", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn export_damage_log_to_file(id: u32, options: ParseOptions) -> Result<(), String> {
    let file_path = FileDialogBuilder::new()
        .add_filter("csv", &["csv"])
        .set_file_name(&format!("{id}_damage_log.csv"))
        .set_title("Export Damage Log")
        .save_file()
        .ok_or("No file selected!")?;

    let conn = db::connect_to_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT data, version FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let (blob, version): (Vec<u8>, u8) = stmt
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("Failed to fetch log from database")
        .map_err(|e| e.to_string())?;

    let parser = parser::deserialize_version(&blob, version).map_err(|e| e.to_string())?;

    let file = File::create(file_path).map_err(|e| e.to_string())?;

    // @TODO(false): Split formatting into a separate function.
    let mut writer = std::io::BufWriter::new(file);

    writeln!(
        writer,
        "timestamp,source_type,child_source_type,source_index,target_type,target_index,action_id,flags,damage"
    )
    .map_err(|e| e.to_string())?;

    for (event_ts, event) in parser.encounter.event_log() {
        if let Message::DamageEvent(damage_event) = event {
            let timestamp = event_ts - parser.start_time();
            let target_type = EnemyType::from_hash(damage_event.target.parent_actor_type);
            let parent_character_type =
                CharacterType::from_hash(damage_event.source.parent_actor_type);
            let child_character_type = CharacterType::from_hash(damage_event.source.actor_type);

            if options.targets.is_empty() || options.targets.contains(&target_type) {
                writeln!(
                    writer,
                    "{},{},{},{},{},{},{},{},{}",
                    timestamp,
                    parent_character_type,
                    child_character_type,
                    damage_event.source.parent_index,
                    target_type,
                    damage_event.target.parent_index,
                    damage_event.action_id,
                    damage_event.flags,
                    damage_event.damage
                )
                .map_err(|e| e.to_string())?;
            }
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
    log_count: i32,
    /// IDs of the enemies that can be filtered by.
    enemy_ids: Vec<u32>,
    /// IDs of the quests that can be filtered by.
    quest_ids: Vec<u32>,
}

#[tauri::command]
fn fetch_logs(
    page: Option<u32>,
    filter_by_enemy_id: Option<u32>,
    filter_by_quest_id: Option<u32>,
    sort_direction: Option<String>,
    sort_type: Option<String>,
    quest_completed: Option<bool>,
) -> Result<SearchResult, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let page = page.unwrap_or(1);
    let per_page = 10;
    let offset = page.saturating_sub(1) * per_page;

    let sort_type_param = sort_type
        .map(|s| match s.as_str() {
            "time" => db::logs::SortType::Time,
            "duration" => db::logs::SortType::Duration,
            "quest-elapsed-time" => db::logs::SortType::QuestElapsedTime,
            _ => db::logs::SortType::Time,
        })
        .unwrap_or(db::logs::SortType::Time);

    let sort_direction_param = sort_direction
        .map(|s| match s.as_str() {
            "asc" => db::logs::SortDirection::Ascending,
            _ => db::logs::SortDirection::Descending,
        })
        .unwrap_or(db::logs::SortDirection::Descending);

    let logs = db::logs::get_logs(
        &conn,
        filter_by_enemy_id,
        filter_by_quest_id,
        per_page,
        offset,
        &sort_type_param,
        &sort_direction_param,
        quest_completed,
    )
    .map_err(|e| e.to_string())?;

    let log_count = db::logs::get_logs_count(
        &conn,
        filter_by_enemy_id,
        filter_by_quest_id,
        quest_completed,
    )
    .map_err(|e| e.to_string())?;

    let page_count = (log_count as f64 / per_page as f64).ceil() as u32;

    let mut enemy_ids = Vec::new();
    let mut quest_ids = Vec::new();

    let mut query = conn
        .prepare("SELECT primary_target, quest_id from logs")
        .map_err(|e| e.to_string())?;

    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<usize, Option<u32>>(0)?,
                row.get::<usize, Option<u32>>(1)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (primary_target, quest_id) = row.map_err(|e| e.to_string())?;

        if let Some(primary_target) = primary_target {
            if !enemy_ids.contains(&primary_target) {
                enemy_ids.push(primary_target);
            }
        }

        if let Some(quest_id) = quest_id {
            if !quest_ids.contains(&quest_id) {
                quest_ids.push(quest_id);
            }
        }
    }

    Ok(SearchResult {
        logs,
        page,
        page_count,
        log_count,
        enemy_ids,
        quest_ids,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EncounterStateResponse {
    encounter_state: v1::DerivedEncounterState,
    players: [Option<PlayerData>; 4],
    quest_id: Option<u32>,
    quest_timer: Option<u32>,
    quest_completed: bool,
    targets: Vec<EnemyType>,
    dps_chart: HashMap<u32, Vec<i32>>,
    sba_chart: HashMap<u32, Vec<f32>>,
    sba_events: Vec<(i64, protocol::Message)>,
    chart_len: usize,
    sba_chart_len: usize,
}

#[derive(Debug, Deserialize)]
struct ParseOptions {
    targets: Vec<EnemyType>,
}

#[tauri::command]
fn fetch_encounter_state(id: u64, options: ParseOptions) -> Result<EncounterStateResponse, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data, version FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let (blob, version): (Vec<u8>, u8) = stmt
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    // @TODO(false): If we deserialize from an older version, we should save it back into the DB as the newer format.
    let mut parser = parser::deserialize_version(&blob, version).map_err(|e| e.to_string())?;

    parser.reparse_with_options(&options.targets);

    let duration = parser.derived_state.duration();

    let mut player_dps: HashMap<u32, Vec<i32>> = HashMap::new();

    const DPS_INTERVAL: i64 = 3 * 1_000;
    const SBA_INTERVAL: i64 = 1_000;

    for player in parser.derived_state.party.values() {
        player_dps.insert(
            player.index,
            vec![0; (duration / DPS_INTERVAL) as usize + 1],
        );
    }

    let mut targets = Vec::new();
    let start_time = parser.start_time();

    for (timestamp, event) in parser.encounter.event_log() {
        match event {
            Message::DamageEvent(damage_event) => {
                let index = ((timestamp - start_time) / DPS_INTERVAL) as usize;
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
            _ => continue,
        }
    }

    let sba_chart = parser.generate_sba_chart(SBA_INTERVAL);

    let sba_events = parser
        .encounter
        .event_log()
        .filter(|(_, e)| {
            matches!(
                e,
                Message::OnContinueSBAChain(_)
                    | Message::OnAttemptSBA(_)
                    | Message::OnPerformSBA(_)
            )
        })
        .map(|(ts, e)| (*ts - start_time, e.clone()))
        .collect();

    Ok(EncounterStateResponse {
        encounter_state: parser.derived_state,
        players: parser.encounter.player_data,
        quest_id: parser.encounter.quest_id,
        quest_timer: parser.encounter.quest_timer,
        quest_completed: parser.encounter.quest_completed,
        dps_chart: player_dps,
        chart_len: (duration / DPS_INTERVAL) as usize + 1,
        sba_chart_len: (duration / SBA_INTERVAL) as usize + 1,
        sba_chart,
        sba_events,
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

                info!("Found game process, injecting DLL: {:?}", dll_path);

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
    let logs_window = app.get_window("logs").expect("Logs window not found");

    let database = db::connect_to_db().expect("Could not connect to database");
    let mut state = v1::Parser::new(app.clone(), window.clone(), database);

    tauri::async_runtime::spawn(async move {
        loop {
            match RecvPipeStream::connect_by_path(protocol::PIPE_NAME).await {
                Ok(stream) => {
                    info!("Connected to game!");

                    let _ = app.emit_all("success-alert", "Connnected to game!");

                    let decoder = tokio_util::codec::LengthDelimitedCodec::new();
                    let mut reader = FramedRead::new(stream, decoder);

                    while let Some(Ok(msg)) = reader.next().await {
                        // Handle EOF when the game closes.
                        if msg.len() == 0 {
                            break;
                        }

                        let debug_mode = app.state::<DebugMode>().0.load(Ordering::Relaxed);

                        if let Ok(msg) = protocol::bincode::deserialize::<protocol::Message>(&msg) {
                            if debug_mode {
                                let _ = logs_window.emit("debug-event", &msg);
                            }

                            match msg {
                                protocol::Message::DamageEvent(event) => {
                                    state.on_damage_event(event);
                                }
                                protocol::Message::OnAreaEnter(event) => {
                                    state.on_area_enter_event(event);
                                }
                                protocol::Message::PlayerLoadEvent(event) => {
                                    state.on_player_load_event(event);
                                }
                                protocol::Message::OnQuestComplete(event) => {
                                    state.on_quest_complete_event(event);
                                }
                                protocol::Message::OnUpdateSBA(event) => {
                                    state.on_sba_update(event);
                                }
                                protocol::Message::OnAttemptSBA(event) => {
                                    state.on_sba_attempt(event);
                                }
                                protocol::Message::OnPerformSBA(event) => {
                                    state.on_sba_perform(event);
                                }
                                protocol::Message::OnContinueSBAChain(event) => {
                                    state.on_continue_sba_chain(event)
                                }
                            }
                        }
                    }

                    info!("Game has closed.");

                    // The game has closed, so we should go back to waiting for the game to reopen.
                    let _ = app.emit_all("error-alert", "Game has closed!");
                    break;
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }

        // Check for the game process again.
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        tauri::async_runtime::spawn(check_and_perform_hook(app));
    });
}

fn system_tray_with_menu() -> SystemTray {
    let meter = CustomMenuItem::new("open_meter", "Open Meter");
    let logs = CustomMenuItem::new("open_logs", "Open Logs");
    let always_on_top = CustomMenuItem::new("always_on_top", "Always on top ✓");
    let toggle_clickthrough = CustomMenuItem::new("toggle_clickthrough", "Clickthrough");
    let reset_windows = CustomMenuItem::new("reset_windows", "Reset Windows");
    let quit = CustomMenuItem::new("quit", "Quit");

    let menu = SystemTrayMenu::new()
        .add_item(meter)
        .add_item(logs)
        .add_item(always_on_top)
        .add_item(toggle_clickthrough)
        .add_item(reset_windows)
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

#[tauri::command]
fn toggle_always_on_top(window: tauri::Window, state: State<AlwaysOnTop>) {
    let always_on_top = &state.0;
    let new_state = !always_on_top.load(Ordering::Acquire);
    always_on_top.store(new_state, Ordering::Release);
    window.set_always_on_top(new_state).unwrap();
    let _ = window.emit("on-pinned", new_state);
    let _ = window
        .app_handle()
        .tray_handle()
        .get_item("always_on_top")
        .set_title(if new_state {
            "Always on top ✓"
        } else {
            "Always on top"
        });
}

#[tauri::command]
fn toggle_clickthrough(window: tauri::Window, state: State<ClickThrough>) {
    let click_through = &state.0;
    let new_state = !click_through.load(Ordering::Acquire);
    click_through.store(new_state, Ordering::Release);
    window.set_ignore_cursor_events(new_state).unwrap();
    let _ = window.emit("on-clickthrough", new_state);
    let _ = window
        .app_handle()
        .tray_handle()
        .get_item("toggle_clickthrough")
        .set_title(if new_state {
            "Clickthrough ✓"
        } else {
            "Clickthrough"
        });
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
            "reset_windows" => {
                if let Some(window) = handle.get_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_size(Size::Logical(LogicalSize {
                        width: 500.0,
                        height: 350.0,
                    }));
                }

                if let Some(window) = handle.get_window("logs") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_size(Size::Logical(LogicalSize {
                        width: 800.0,
                        height: 600.0,
                    }));
                }
            }
            "quit" => {
                let _ = handle.save_window_state(StateFlags::all());
                handle.exit(0)
            }
            _ => {}
        },
        _ => {} // Ignore rest of the events.
    }
}

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
        .system_tray(system_tray_with_menu())
        .on_system_tray_event(menu_tray_handler)
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            fetch_encounter_state,
            fetch_logs,
            delete_logs,
            delete_all_logs,
            toggle_always_on_top,
            export_damage_log_to_file,
            set_debug_mode,
        ])
        .setup(|app| {
            // Perform the game hook check in a separate thread.
            tauri::async_runtime::spawn(check_and_perform_hook(app.handle()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
