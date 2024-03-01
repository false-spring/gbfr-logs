// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

use dll_syringe::{process::OwnedProcess, Syringe};
use futures::io::AsyncReadExt;
use interprocess::os::windows::named_pipe::tokio::MsgReaderPipeStream;
use parser::Parser;
use tauri::{AppHandle, Manager};

mod parser;

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
    let mut state = Parser::new(Some(window.clone()));

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
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![load_parse_log_from_file])
        .setup(|app| {
            tauri::async_runtime::spawn(check_and_perform_hook(app.handle()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
