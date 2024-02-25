// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

use dll_syringe::{process::OwnedProcess, Syringe};
use futures::io::AsyncReadExt;
use interprocess::os::windows::named_pipe::tokio::MsgReaderPipeStream;
use parser::EncounterState;
use tauri::Manager;

mod parser;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .setup(|app| {
            // @TODO(false): Let application continue to run even if the game is not found.
            // We can still show the window and let the user know that the game was not found.
            // We can also show a button to retry the injection or automatically detect the game again.
            let target = OwnedProcess::find_first_by_name("granblue_fantasy_relink.exe").expect("Process was not found for injection");
            let syringe = Syringe::for_process(target);
            let debug_dll_path = Path::new("hook-dbg.dll");
            let mut dll_path = Path::new("hook.dll");

            // If the debug DLL is present, use it instead.
            if debug_dll_path.exists() {
                dll_path = debug_dll_path;
            }

            let _ = syringe.inject(dll_path).unwrap();

            let window = app.get_window("main").expect("Window not found");
            let mut state = EncounterState::new(Some(window.clone()));

            // @TODO(false): Actually track the connection status and reflect back to the user if we were able to connect to the game or not.
            tauri::async_runtime::spawn(async move {
                loop  {
                    match MsgReaderPipeStream::connect(protocol::PIPE_NAME) {
                        Ok(mut stream) => {
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
                                            state.reset();
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
