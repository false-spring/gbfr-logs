// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dll_syringe::{process::OwnedProcess, Syringe};
use futures::io::AsyncReadExt;
use interprocess::os::windows::named_pipe::tokio::MsgReaderPipeStream;
use tauri::{CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu};

fn main() {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = SystemTrayMenu::new().add_item(quit);
    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .system_tray(system_tray)
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_window("main").unwrap();
                window.show().unwrap();
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .setup(|_app| {
            // @TODO(false): Let application continue to run even if the game is not found.
            // We can still show the window and let the user know that the game was not found.
            // We can also show a button to retry the injection or automatically detect the game again.
            let target = OwnedProcess::find_first_by_name("granblue_fantasy_relink.exe").expect("Process was not found for injection");
            let syringe = Syringe::for_process(target);
            let dll_path = "hook.dll";

            let _ = syringe.inject(dll_path).unwrap();

            // @TODO(false): Actually track the connection status and reflect back to the user if we were able to connect to the game or not.
            tauri::async_runtime::spawn(async {
                loop  {
                    match MsgReaderPipeStream::connect(protocol::PIPE_NAME) {
                        Ok(mut stream) => {
                            let mut buffer = [0; 1024];
                            while let Ok(msg) = stream.read(&mut buffer).await {
                                if let Ok(msg) =
                                    protocol::bincode::deserialize::<protocol::Message>(&buffer[..msg])
                                {
                                    println!("Received message: {:?}", msg);
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
