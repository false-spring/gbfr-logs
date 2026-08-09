use std::{os::windows::ffi::OsStrExt, path::Path, sync::atomic::Ordering};

use dll_syringe::{
    process::{OwnedProcess, Process},
    Syringe,
};
use interprocess::os::windows::named_pipe::tokio::RecvPipeStream;
use log::info;
use tauri::{AppHandle, Manager};
use tokio_stream::StreamExt;
use tokio_util::codec::FramedRead;

use crate::commands::DebugMode;
use crate::db;
use crate::parser::v1;

fn read_game_file_version(exe_path: &Path) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    let wide: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let path_pcwstr = PCWSTR(wide.as_ptr());

    unsafe {
        let size = GetFileVersionInfoSizeW(path_pcwstr, None);
        if size == 0 {
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        GetFileVersionInfoW(path_pcwstr, 0, size, buffer.as_mut_ptr() as *mut _).ok()?;

        let mut value_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len: u32 = 0;
        let sub_block: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
        let ok = VerQueryValueW(
            buffer.as_ptr() as *const _,
            PCWSTR(sub_block.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        );
        if !ok.as_bool()
            || value_ptr.is_null()
            || (value_len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }

        let info = &*(value_ptr as *const VS_FIXEDFILEINFO);
        let ms = info.dwFileVersionMS;
        let ls = info.dwFileVersionLS;
        Some(format!(
            "{}.{}.{}.{}",
            ms >> 16,
            ms & 0xFFFF,
            ls >> 16,
            ls & 0xFFFF
        ))
    }
}

// Continuously check for the game process and inject the DLL when found.
pub async fn check_and_perform_hook(app: AppHandle) {
    loop {
        match OwnedProcess::find_first_by_name("granblue_fantasy_relink.exe") {
            Some(target) => {
                let game_version = target.path().ok().as_deref().and_then(read_game_file_version);

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

                connect_and_run_parser(app, game_version);

                break;
            }
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    }
}

// Connect to the game hook event channel and listen for damage events.
fn connect_and_run_parser(app: AppHandle, game_version: Option<String>) {
    let window = app.get_window("main").expect("Window not found");
    let logs_window = app.get_window("logs").expect("Logs window not found");

    let database = db::connect_to_db().expect("Could not connect to database");
    let mut state = v1::Parser::new(app.clone(), window.clone(), database, game_version);

    tauri::async_runtime::spawn(async move {
        loop {
            match RecvPipeStream::connect_by_path(protocol::PIPE_NAME).await {
                Ok(stream) => {
                    info!("Connected to game!");

                    let _ = app.emit_all("success-alert", "Connnected to game!");

                    let decoder = tokio_util::codec::LengthDelimitedCodec::new();
                    let mut reader = FramedRead::new(stream, decoder);

                    loop {
                        let next = tokio::time::timeout(
                            std::time::Duration::from_millis(v1::LIVE_EMIT_INTERVAL_MILLIS as u64),
                            reader.next(),
                        )
                        .await;

                        let msg = match next {
                            Err(_idle) => {
                                state.flush_live_update();
                                continue;
                            }
                            Ok(Some(Ok(msg))) => msg,
                            Ok(_) => break,
                        };

                        // Handle EOF when the game closes.
                        if msg.is_empty() {
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
                                // Upstream's pre-2.0 player load hook is dead on
                                // ER, so this never arrives. The ER path uses
                                // PlayerIdentityEvent.
                                protocol::Message::PlayerLoadEvent(_) => {}
                                protocol::Message::PlayerIdentityEvent(event) => {
                                    state.on_player_identity_event(event);
                                }
                                protocol::Message::PartyRoster(event) => {
                                    state.on_party_roster_event(event);
                                }
                                protocol::Message::OnQuestComplete(event) => {
                                    state.on_quest_complete_event(event);
                                }
                                protocol::Message::OnQuestAbandon(event) => {
                                    state.on_quest_abandon_event(event);
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
                                    state.on_continue_sba_chain(event);
                                }
                                protocol::Message::OnDeathEvent(event) => {
                                    state.on_death_event(event);
                                }
                                // Debug-Mode-only visibility; nothing to count here.
                                protocol::Message::SuppressedDuplicateDamage(_) => {}
                                protocol::Message::OnPeerCounter(event) => {
                                    state.on_peer_counter(event);
                                }
                                protocol::Message::OnLinkTimeStart(event) => {
                                    state.on_link_time_start(event);
                                }
                                protocol::Message::OnLinkTimeEnd(event) => {
                                    state.on_link_time_end(event);
                                }
                                protocol::Message::OnLinkAttackChance(event) => {
                                    state.on_link_attack_chance(event);
                                }
                                protocol::Message::OnConfluxAreaClear(event) => {
                                    state.on_conflux_area_clear(event);
                                }
                                protocol::Message::OnConfluxAdvance(event) => {
                                    state.on_conflux_advance(event);
                                }
                                protocol::Message::OnConfluxBossClear(event) => {
                                    state.on_conflux_boss_clear(event);
                                }
                                protocol::Message::OnEnemyModeChange(event) => {
                                    state.on_enemy_mode_change(event);
                                }
                                protocol::Message::OnSbaWindowChange(event) => {
                                    state.on_sba_window_change(event);
                                }
                                protocol::Message::OnEnemyDeath(event) => {
                                    state.on_enemy_death(event);
                                }
                                protocol::Message::OnStatusApplied(event) => {
                                    state.on_status_applied(event);
                                }
                                protocol::Message::OnStatusRemoved(event) => {
                                    state.on_status_removed(event);
                                }
                                protocol::Message::OnStatusStacksChanged(event) => {
                                    state.on_status_stacks_changed(event);
                                }
                                protocol::Message::OnHeal(event) => {
                                    state.on_heal_event(event);
                                }
                            }
                        }
                    }

                    info!("Game has closed.");

                    // The game can close mid-encounter with no quest-over hook
                    // firing. Flush it as a failed run. quest_id and elapsed 0
                    // keep the log's own values.
                    state.on_quest_abandon_event(protocol::QuestAbandonEvent {
                        quest_id: 0,
                        elapsed_time_in_secs: 0,
                    });

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
