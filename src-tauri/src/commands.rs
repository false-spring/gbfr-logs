use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Context;
use protocol::Message;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use tauri::{api::dialog::blocking::FileDialogBuilder, AppHandle, Manager, State};

use crate::db::{
    self,
    logs::{distinct_party_text, distinct_text, distinct_u32, LogEntry},
};
use crate::parser::{
    self,
    constants::{CharacterType, EnemyType},
    v1::{self, PlayerData, StunReconstructor},
};

pub struct AlwaysOnTop(pub AtomicBool);
pub struct ClickThrough(pub AtomicBool);
pub struct DebugMode(pub AtomicBool);

#[tauri::command]
pub fn set_debug_mode(app: AppHandle, state: State<DebugMode>, enabled: bool) {
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
pub async fn delete_all_logs() -> Result<(), String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM logs", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn export_damage_log_to_file(id: u32, options: ParseOptions) -> Result<(), String> {
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

            // Check to see if the target is in the list of targets to filter by.
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
pub struct SearchResult {
    logs: Vec<LogEntry>,
    page: u32,
    page_count: u32,
    log_count: i32,
    /// IDs of the enemies that can be filtered by.
    enemy_ids: Vec<u32>,
    /// IDs of the quests that can be filtered by.
    quest_ids: Vec<u32>,
    /// Names of the Players that can be filtered by.
    player_ids: Vec<String>,
    /// Names of the Characters that can be filtered by.
    player_types: Vec<String>,
    game_versions: Vec<String>,
}

#[tauri::command]
pub fn fetch_logs(
    page: Option<u32>,
    filter_by_enemy_id: Option<u32>,
    filter_by_quest_id: Option<u32>,
    sort_direction: Option<String>,
    sort_type: Option<String>,
    quest_completed: Option<bool>,
    filter_by_player_id: Option<String>,
    filter_by_player_character: Option<String>,
    filter_by_style: Option<String>,
    filter_by_game_versions: Option<Vec<String>>,
) -> Result<SearchResult, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let page = page.unwrap_or(1);
    let per_page = 10;
    let offset = page.saturating_sub(1) * per_page;
    let game_versions_filter = filter_by_game_versions.unwrap_or_default();

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
        &filter_by_player_id,
        &filter_by_player_character,
        &filter_by_style,
        &game_versions_filter,
    )
    .map_err(|e| e.to_string())?;

    let log_count = db::logs::get_logs_count(
        &conn,
        filter_by_enemy_id,
        filter_by_quest_id,
        quest_completed,
        &filter_by_player_id,
        &filter_by_player_character,
        &filter_by_style,
        &game_versions_filter,
    )
    .map_err(|e| e.to_string())?;

    let page_count = (log_count as f64 / per_page as f64).ceil() as u32;

    let enemy_ids = distinct_u32(&conn, "primary_target")?;
    let quest_ids = distinct_u32(&conn, "quest_id")?;
    let game_versions = distinct_text(&conn, "game_version")?;
    let player_ids = distinct_party_text(&conn, "name")?;
    let player_types = distinct_party_text(&conn, "type")?;

    Ok(SearchResult {
        logs,
        page,
        page_count,
        log_count,
        enemy_ids,
        quest_ids,
        player_ids,
        player_types,
        game_versions,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterStateResponse {
    encounter_state: v1::DerivedEncounterState,
    players: [Option<PlayerData>; 4],
    quest_id: Option<u32>,
    quest_timer: Option<u32>,
    quest_completed: bool,
    /// DPS chart buckets: enemy parent_index -> player parent_index -> per-interval.
    dps_chart_by_enemy: HashMap<u32, HashMap<u32, Vec<i32>>>,
    stun_chart_by_enemy: HashMap<u32, HashMap<u32, Vec<f32>>>,
    stun_reset_by_enemy: HashMap<u32, Vec<usize>>,
    sba_chart: HashMap<u32, Vec<f32>>,
    sba_events: Vec<(i64, protocol::Message)>,
    /// Link Time windows as `(start_ms, end_ms)` from the encounter start.
    link_time_windows: Vec<(i64, i64)>,
    conflux_area_clears: Vec<i64>,
    conflux_boss_clears: Vec<i64>,
    sba_windows: Vec<(i64, i64)>,
    break_windows: Vec<(i64, i64)>,
    /// Actor grouping id -> status kind id -> uptime windows.
    status_intervals: HashMap<u32, HashMap<u32, Vec<(i64, i64)>>>,
    status_peak_stacks: HashMap<u32, HashMap<u32, u32>>,
    status_stack_series: HashMap<u32, HashMap<u32, Vec<(i64, u32)>>>,
    status_value_series: HashMap<u32, HashMap<u32, Vec<(i64, f32)>>>,
    status_value_is_fraction: HashMap<u32, bool>,
    status_sources: HashMap<u32, HashMap<u32, Vec<StatusSourceWindows>>>,
    chart_len: usize,
    sba_chart_len: usize,
    heal_provided_chart: HashMap<u32, Vec<u64>>,
    heal_received_chart: HashMap<u32, Vec<u64>>,
    damage_taken_chart: HashMap<u32, Vec<u64>>,
    deaths: HashMap<u32, Vec<i64>>,
    misc_chart_len: usize,
}

/// Who applied it plus the game's own three-`u32` source tuple; together
/// they identify one `Status` object.
type ObjectKey = (Option<u32>, Option<[u32; 3]>);

const DPS_INTERVAL: i64 = 3 * 1_000;

/// The frontend plots `chart_len + 1` points, so the axis runs up to one
/// bucket past the last damage event. Overlays are trimmed to this, not the duration.
fn chart_extent(duration: i64) -> i64 {
    (duration / DPS_INTERVAL + 1) * DPS_INTERVAL
}

/// The game tears a fight's statuses down ~5.5s after the last damage event,
/// so removal-closed windows routinely overrun the chart.
fn clamp_windows(windows: &mut Vec<(i64, i64)>, duration: i64) {
    windows.retain(|(open, _)| *open < duration);
    for (_, close) in windows.iter_mut() {
        *close = (*close).min(duration);
    }
    windows.retain(|(open, close)| close > open);
}

fn clamp_series<T: Copy>(points: &mut Vec<(i64, T)>, duration: i64) {
    points.retain(|(at, _)| *at <= duration);
}

/// The game refreshes a buff by removing and re-applying it within ~1ms;
/// shorter holds are churn, not real changes.
const MIN_HOLD_MS: i64 = 50;

fn despike(points: &mut Vec<(i64, f32)>) {
    if points.len() < 3 {
        return;
    }
    let mut kept: Vec<(i64, f32)> = Vec::with_capacity(points.len());
    kept.push(points[0]);
    for i in 1..points.len() - 1 {
        let (at, value) = points[i];
        let (next_at, next_value) = points[i + 1];
        let previous = kept.last().map(|(_, v)| *v);
        let brief = next_at - at < MIN_HOLD_MS;
        let returns = previous.map(|p| p.to_bits() == next_value.to_bits()) == Some(true);
        if brief && returns && previous.map(|p| p.to_bits() != value.to_bits()) == Some(true) {
            continue;
        }
        kept.push((at, value));
    }
    kept.push(points[points.len() - 1]);
    *points = kept;
}

fn sum_object_series(objects: &[Vec<(i64, f32)>]) -> Vec<(i64, f32)> {
    let mut moments: Vec<i64> = objects
        .iter()
        .flat_map(|points| points.iter().map(|(at, _)| *at))
        .collect();
    moments.sort_unstable();
    moments.dedup();

    let mut out: Vec<(i64, f32)> = Vec::new();
    for at in moments {
        let mut total = 0.0f32;
        for points in objects {
            if let Some((_, v)) = points.iter().rev().find(|(t, _)| *t <= at) {
                total += *v;
            }
        }
        if out.last().map(|(_, v)| v.to_bits()) == Some(total.to_bits()) {
            continue;
        }
        out.push((at, total));
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSourceWindows {
    pub applier_index: Option<u32>,
    /// The game's own source-identity tuple. Opaque; only used to tell one
    /// source from another.
    pub source_ids: Option<[u32; 3]>,
    pub windows: Vec<(i64, i64)>,
    pub values: Vec<(i64, f32)>,
}

#[derive(Debug, Deserialize)]
pub struct ParseOptions {
    targets: Vec<EnemyType>,
}

#[tauri::command]
pub fn fetch_encounter_state(id: u64) -> Result<EncounterStateResponse, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data, version FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let (blob, version): (Vec<u8>, u8) = stmt
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    // @TODO(false): If we deserialize from an older version, we should save it back into the DB as the newer format.
    let mut parser = parser::deserialize_version(&blob, version).map_err(|e| e.to_string())?;

    parser.reparse();

    const SBA_INTERVAL: i64 = 1_000;
    // Minimum fill-fraction drop between hits that counts as a stun reset.
    const STUN_RESET_DROP: f32 = 0.3;

    let duration = parser.derived_state.duration();
    let chart_len = (duration / DPS_INTERVAL) as usize + 1;

    let mut dps_by_enemy: HashMap<u32, HashMap<u32, Vec<i32>>> = HashMap::new();
    let mut stun_by_enemy: HashMap<u32, HashMap<u32, Vec<f32>>> = HashMap::new();
    let mut last_stun_fill: HashMap<u32, f32> = HashMap::new();
    let mut stun_reset_by_enemy: HashMap<u32, std::collections::BTreeSet<usize>> = HashMap::new();
    let mut last_hit_by_target: HashMap<u32, i64> = HashMap::new();
    let mut stun_recon = StunReconstructor::default();

    let party_players: std::collections::HashSet<u32> =
        parser.derived_state.party.keys().copied().collect();

    let start_time = parser.start_time();

    for (timestamp, event) in parser.encounter.event_log() {
        if let Message::DamageEvent(damage_event) = event {
            let index = ((timestamp - start_time) / DPS_INTERVAL) as usize;

            let enemy = damage_event.target.parent_index;
            let player = damage_event.source.parent_index;
            last_hit_by_target.insert(enemy, timestamp - start_time);
            let counted = stun_recon.counted_stun(damage_event);
            if party_players.contains(&player) {
                let dps_chart = dps_by_enemy
                    .entry(enemy)
                    .or_default()
                    .entry(player)
                    .or_insert_with(|| vec![0; chart_len]);
                dps_chart[index] += damage_event.damage;

                let stun_chart = stun_by_enemy
                    .entry(enemy)
                    .or_default()
                    .entry(player)
                    .or_insert_with(|| vec![0.0; chart_len]);
                stun_chart[index] += counted as f32;
            }

            if let Some(fill) = damage_event.stun_fill {
                let last = last_stun_fill.insert(enemy, fill).unwrap_or(0.0);
                if last - fill >= STUN_RESET_DROP {
                    stun_reset_by_enemy.entry(enemy).or_default().insert(index);
                }
            }
        }
    }

    let sba_chart = parser.generate_sba_chart(SBA_INTERVAL);
    let misc_charts = parser.generate_misc_charts(SBA_INTERVAL);

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

    let mut link_time_windows: Vec<(i64, i64)> = Vec::new();
    let mut open_link_time: Option<i64> = None;

    for (ts, event) in parser.encounter.event_log() {
        match event {
            Message::OnLinkTimeStart(_) => {
                if open_link_time.is_none() {
                    open_link_time = Some(*ts - start_time);
                }
            }
            Message::OnLinkTimeEnd(_) => {
                if let Some(opened) = open_link_time.take() {
                    let closed = *ts - start_time;
                    if closed > opened {
                        link_time_windows.push((opened, closed));
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(opened) = open_link_time {
        if duration > opened {
            link_time_windows.push((opened, duration));
        }
    }

    let conflux_area_clears: Vec<i64> = parser
        .encounter
        .event_log()
        .filter(|(_, e)| matches!(e, Message::OnConfluxAreaClear(_)))
        .map(|(ts, _)| *ts - start_time)
        .filter(|offset| *offset > 0 && *offset < duration)
        .collect();

    let conflux_boss_clears: Vec<i64> = parser
        .encounter
        .event_log()
        .filter(|(_, e)| matches!(e, Message::OnConfluxBossClear(_)))
        .map(|(ts, _)| *ts - start_time)
        .filter(|offset| *offset > 0 && *offset < duration)
        .collect();

    let sba_marks: Vec<(i64, bool)> = parser
        .encounter
        .event_log()
        .filter_map(|(ts, e)| match e {
            Message::OnAttemptSBA(_) | Message::OnPerformSBA(_) => Some((*ts - start_time, false)),
            Message::DamageEvent(d) if d.action_id == protocol::ActionType::SBA => {
                Some((*ts - start_time, true))
            }
            _ => None,
        })
        .collect();

    let mut fight_ends: Vec<i64> = conflux_boss_clears.clone();
    fight_ends.extend(conflux_area_clears.iter().copied());
    fight_ends.sort_unstable();

    let sba_transitions: Vec<(i64, bool)> = parser
        .encounter
        .event_log()
        .filter_map(|(ts, e)| match e {
            Message::OnSbaWindowChange(w) => Some((*ts - start_time, w.active)),
            _ => None,
        })
        .collect();

    let sba_windows: Vec<(i64, i64)> = if sba_transitions.is_empty() {
        sba_lockdown_windows(&sba_marks)
    } else {
        transition_windows(&sba_transitions, &fight_ends, duration)
    }
    .into_iter()
    .filter(|(open, close)| *open >= 0 && *close <= duration)
    .collect();

    let mut break_by_target: HashMap<u32, Vec<(i64, bool)>> = HashMap::new();
    let mut deaths_by_target: HashMap<u32, Vec<i64>> = HashMap::new();
    for (ts, event) in parser.encounter.event_log() {
        match event {
            Message::OnEnemyModeChange(m) => break_by_target
                .entry(m.target_index)
                .or_default()
                .push((*ts - start_time, m.in_break)),
            Message::OnEnemyDeath(d) => deaths_by_target
                .entry(d.target_index)
                .or_default()
                .push(*ts - start_time),
            _ => {}
        }
    }

    let owner_of = child_actor_owners(parser.encounter.event_log());
    let (
        status_intervals,
        status_peak_stacks,
        status_stack_series,
        status_value_series,
        status_value_is_fraction,
        status_sources,
    ) =
        pair_status_intervals(parser.encounter.event_log(), start_time, duration, &owner_of);

    let break_windows: Vec<(i64, i64)> = {
        let mut windows: Vec<(i64, i64)> = break_by_target
            .iter()
            .flat_map(|(target, transitions)| {
                let mut closers = fight_ends.clone();
                closers.extend(deaths_by_target.get(target).into_iter().flatten().copied());
                closers.extend(last_hit_by_target.get(target).copied());
                closers.sort_unstable();
                transition_windows(transitions, &closers, duration)
            })
            .collect();
        windows.sort_unstable();
        merge_windows(windows)
    }
    .into_iter()
    .filter(|(open, close)| *open >= 0 && *close <= duration)
    .collect();

    Ok(EncounterStateResponse {
        encounter_state: parser.derived_state,
        players: parser.encounter.player_data,
        quest_id: parser.encounter.quest_id,
        quest_timer: parser.encounter.quest_timer,
        quest_completed: parser.encounter.quest_completed,
        dps_chart_by_enemy: dps_by_enemy,
        stun_chart_by_enemy: stun_by_enemy,
        stun_reset_by_enemy: stun_reset_by_enemy
            .into_iter()
            .map(|(enemy, buckets)| (enemy, buckets.into_iter().collect()))
            .collect(),
        chart_len,
        sba_chart_len: (duration / SBA_INTERVAL) as usize + 1,
        sba_chart,
        heal_provided_chart: misc_charts.heal_provided,
        heal_received_chart: misc_charts.heal_received,
        damage_taken_chart: misc_charts.damage_taken,
        deaths: misc_charts.deaths,
        misc_chart_len: (duration / SBA_INTERVAL) as usize + 1,
        sba_events,
        link_time_windows,
        conflux_area_clears,
        conflux_boss_clears,
        sba_windows,
        break_windows,
        status_intervals,
        status_peak_stacks,
        status_stack_series,
        status_value_series,
        status_value_is_fraction,
        status_sources,
    })
}

#[tauri::command]
pub fn fetch_enemy_encounter_state(
    id: u64,
    enemy_index: u32,
) -> Result<v1::DerivedEncounterState, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data, version FROM logs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    let (blob, version): (Vec<u8>, u8) = stmt
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    let mut parser = parser::deserialize_version(&blob, version).map_err(|e| e.to_string())?;
    parser.reparse();

    Ok(parser.derived_state_for_enemy(enemy_index))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[tauri::command]
pub fn bug_report_payload(id: u64) -> Result<serde_json::Value, String> {
    let conn = db::connect_to_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT name, time, duration, data, version, primary_target, \
             p1_name, p1_type, p2_name, p2_type, p3_name, p3_type, p4_name, p4_type, \
             quest_id, quest_elapsed_time, quest_completed, game_version, app_version \
             FROM logs WHERE id = ?",
        )
        .map_err(|e| e.to_string())?;

    stmt.query_row([id], |row| {
        let data: Vec<u8> = row.get(3)?;
        Ok(serde_json::json!({
            "id": id,
            "name": row.get::<_, Option<String>>(0)?,
            "time": row.get::<_, Option<i64>>(1)?,
            "duration": row.get::<_, Option<i64>>(2)?,
            "data_base64": base64_encode(&data),
            "version": row.get::<_, Option<i64>>(4)?,
            "primary_target": row.get::<_, Option<i64>>(5)?,
            "p1_name": row.get::<_, Option<String>>(6)?,
            "p1_type": row.get::<_, Option<String>>(7)?,
            "p2_name": row.get::<_, Option<String>>(8)?,
            "p2_type": row.get::<_, Option<String>>(9)?,
            "p3_name": row.get::<_, Option<String>>(10)?,
            "p3_type": row.get::<_, Option<String>>(11)?,
            "p4_name": row.get::<_, Option<String>>(12)?,
            "p4_type": row.get::<_, Option<String>>(13)?,
            "quest_id": row.get::<_, Option<i64>>(14)?,
            "quest_elapsed_time": row.get::<_, Option<i64>>(15)?,
            "quest_completed": row.get::<_, Option<i64>>(16)?,
            "game_version": row.get::<_, Option<String>>(17)?,
            "app_version": row.get::<_, Option<String>>(18)?,
        }))
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_logs(ids: Vec<u64>) -> Result<(), String> {
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
pub fn toggle_always_on_top(window: tauri::Window, state: State<AlwaysOnTop>) {
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
pub fn toggle_clickthrough(window: tauri::Window, state: State<ClickThrough>) {
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

/// A full 4-player chain keeps its internal gaps under about seven seconds;
/// separate chains sit tens of seconds apart.
const SBA_CHAIN_GAP_MS: i64 = 10_000;

/// `marks` is `(offset, is_hit)` in log order; a chain runs from its cast to
/// the last SBA-tagged hit it produced.
fn sba_lockdown_windows(marks: &[(i64, bool)]) -> Vec<(i64, i64)> {
    let mut windows = Vec::new();
    let mut open: Option<i64> = None;
    let mut last_hit: Option<i64> = None;
    let mut previous: Option<i64> = None;

    for (at, is_hit) in marks {
        if previous.is_some_and(|p| at - p > SBA_CHAIN_GAP_MS) {
            if let (Some(start), Some(end)) = (open, last_hit) {
                if end > start {
                    windows.push((start, end));
                }
            }
            open = None;
            last_hit = None;
        }

        open.get_or_insert(*at);
        if *is_hit {
            last_hit = Some(*at);
        }
        previous = Some(*at);
    }

    if let (Some(start), Some(end)) = (open, last_hit) {
        if end > start {
            windows.push((start, end));
        }
    }

    windows
}

/// `(offset, active)` transitions -> windows, force-closed at `closers`.
/// A window still open after every closer closes at `duration`.
fn transition_windows(
    transitions: &[(i64, bool)],
    closers: &[i64],
    duration: i64,
) -> Vec<(i64, i64)> {
    let mut windows = Vec::new();
    let mut open: Option<i64> = None;

    let close_at = |start: i64| -> i64 {
        closers
            .iter()
            .copied()
            .find(|at| *at > start)
            .unwrap_or(duration)
            .min(duration)
    };

    for (at, active) in transitions {
        match (open, active) {
            (None, true) => open = Some(*at),
            (Some(start), false) => {
                let end = close_at(start).min(*at);
                if end > start {
                    windows.push((start, end));
                }
                open = None;
            }
            _ => {}
        }
    }

    if let Some(start) = open {
        let end = close_at(start);
        if end > start {
            windows.push((start, end));
        }
    }

    windows
}

/// Child actor id -> the player who owns it, learned from the damage stream.
/// The hook resolves this at the source now, so only a log recorded before that
/// fix still carries the split ids.
fn child_actor_owners<'a>(events: impl Iterator<Item = &'a (i64, Message)>) -> HashMap<u32, u32> {
    let mut owners = HashMap::new();
    for (_, event) in events {
        if let Message::DamageEvent(e) = event {
            if e.source.index != e.source.parent_index {
                owners.insert(e.source.index, e.source.parent_index);
            }
        }
    }
    owners
}

#[allow(clippy::type_complexity)]
fn note_object(
    objects: &mut HashMap<(u32, u32), HashMap<ObjectKey, Vec<(i64, f32)>>>,
    actor: u32,
    status: u32,
    key: ObjectKey,
    at: i64,
    value: f32,
) {
    let points = objects
        .entry((actor, status))
        .or_default()
        .entry(key)
        .or_default();
    if points.last().map(|(_, v)| v.to_bits()) == Some(value.to_bits()) {
        return;
    }
    points.push((at, value));
}

/// Status-effect uptime intervals paired per `(actor, status)`, plus the peak
/// stack depth each pair reached. This is the only implementation of the
/// removal-depth rule, and every status surface the app draws comes from it.
///
/// A removal is not automatically the end of a status. `stacks` on the removal
/// is the depth REMAINING, so peeling one off a stack of 3 leaves the interval
/// open. Only `Some(0)`, or `None` when the depth is unreadable, ends it.
///
/// The stream is not guaranteed balanced, so both edges are defensive. A second
/// apply while one is open renews that interval rather than nesting. A removal
/// with nothing open is discarded, unless it is the first thing seen for the
/// pair. An interval still open at the end closes at the encounter duration.
#[allow(clippy::type_complexity)]
fn pair_status_intervals<'a>(
    events: impl Iterator<Item = &'a (i64, Message)>,
    start_time: i64,
    duration: i64,
    owner_of: &HashMap<u32, u32>,
) -> (
    HashMap<u32, HashMap<u32, Vec<(i64, i64)>>>,
    HashMap<u32, HashMap<u32, u32>>,
    HashMap<u32, HashMap<u32, Vec<(i64, u32)>>>,
    HashMap<u32, HashMap<u32, Vec<(i64, f32)>>>,
    HashMap<u32, bool>,
    HashMap<u32, HashMap<u32, Vec<StatusSourceWindows>>>,
) {
    let extent = chart_extent(duration);

    let mut open: HashMap<(u32, u32), i64> = HashMap::new();
    // Per-object magnitude; summing these reproduces the engine's additive fold.
    let mut objects: HashMap<(u32, u32), HashMap<ObjectKey, Vec<(i64, f32)>>> = HashMap::new();
    // Removed objects, moved aside so a reused key starts a fresh life.
    let mut retired: HashMap<(u32, u32), Vec<(ObjectKey, Vec<(i64, f32)>)>> = HashMap::new();
    let mut seen_objects: std::collections::HashSet<(u32, u32, ObjectKey)> =
        std::collections::HashSet::new();

    let mut open_by_source: HashMap<(u32, u32, ObjectKey), i64> = HashMap::new();
    let mut seen_sources: std::collections::HashSet<(u32, u32, ObjectKey)> =
        std::collections::HashSet::new();
    let mut sources: HashMap<u32, HashMap<u32, HashMap<ObjectKey, Vec<(i64, i64)>>>> =
        HashMap::new();
    let mut seen: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    let mut out: HashMap<u32, HashMap<u32, Vec<(i64, i64)>>> = HashMap::new();
    let mut peak: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    let mut series: HashMap<u32, HashMap<u32, Vec<(i64, u32)>>> = HashMap::new();
    let mut values: HashMap<u32, HashMap<u32, Vec<(i64, f32)>>> = HashMap::new();
    let mut fractional: HashMap<u32, bool> = HashMap::new();

    let mut note_peak = |actor: u32, status: u32, depth: u32| {
        let slot = peak.entry(actor).or_default().entry(status).or_default();
        *slot = (*slot).max(depth);
    };

    let mut note_value = |actor: u32, status: u32, at: i64, total: f32| {
        let points: &mut Vec<(i64, f32)> = values.entry(actor).or_default().entry(status).or_default();
        if points.last().map(|(_, v)| v.to_bits()) == Some(total.to_bits()) {
            return;
        }
        points.push((at, total));
    };

    let mut note_depth = |actor: u32, status: u32, at: i64, depth: u32| {
        let points: &mut Vec<(i64, u32)> = series.entry(actor).or_default().entry(status).or_default();
        if points.last().map(|(_, d)| *d) == Some(depth) {
            return;
        }
        points.push((at, depth));
    };

    // File every status against the owning player, not a transform or summon body.
    let owner = |actor: u32| *owner_of.get(&actor).unwrap_or(&actor);

    for (ts, event) in events {
        let at = *ts - start_time;
        match event {
            Message::OnStatusApplied(e) => {
                seen.insert((owner(e.actor_index), e.status_id));
                note_peak(owner(e.actor_index), e.status_id, e.stacks.unwrap_or(1));
                note_depth(owner(e.actor_index), e.status_id, at, e.stacks.unwrap_or(1));
                if let Some(total) = e.value_total {
                    note_value(owner(e.actor_index), e.status_id, at, total);
                }
                if let Some(is_fraction) = e.value_is_fraction {
                    fractional.insert(e.status_id, is_fraction);
                }
                seen_objects.insert((
                    owner(e.actor_index),
                    e.status_id,
                    (e.applier_index, e.source_ids),
                ));
                if let Some(value) = e.value {
                    note_object(
                        &mut objects,
                        owner(e.actor_index),
                        e.status_id,
                        (e.applier_index, e.source_ids),
                        at,
                        value,
                    );
                }
                open.entry((owner(e.actor_index), e.status_id)).or_insert(at);
                // The applier is not run through `owner`: a summon applying
                // a buff is named as itself.
                let source_key = (
                    owner(e.actor_index),
                    e.status_id,
                    (e.applier_index, e.source_ids),
                );
                seen_sources.insert(source_key);
                open_by_source.entry(source_key).or_insert(at);
            }
            // Corrects a levelled status's depth: the apply event reports the
            // depth standing before the grant. Not always present, though --
            // a re-application already at the ceiling writes no new level.
            Message::OnStatusStacksChanged(e) => {
                note_peak(owner(e.actor_index), e.status_id, e.stacks);
                note_depth(owner(e.actor_index), e.status_id, at, e.stacks);
                if let Some(total) = e.value_total {
                    note_value(owner(e.actor_index), e.status_id, at, total);
                }
                if let Some(is_fraction) = e.value_is_fraction {
                    fractional.insert(e.status_id, is_fraction);
                }
                if let Some(value) = e.value {
                    note_object(
                        &mut objects,
                        owner(e.actor_index),
                        e.status_id,
                        (e.applier_index, e.source_ids),
                        at,
                        value,
                    );
                }
            }
            Message::OnStatusRemoved(e) => {
                let remaining = e.stacks.unwrap_or(0);
                note_peak(
                    owner(e.actor_index),
                    e.status_id,
                    e.stacks_before.unwrap_or_else(|| remaining.saturating_add(1)),
                );
                note_depth(owner(e.actor_index), e.status_id, at, remaining);
                if let Some(total) = e.value_total {
                    note_value(owner(e.actor_index), e.status_id, at, total);
                }
                if let Some(is_fraction) = e.value_is_fraction {
                    fractional.insert(e.status_id, is_fraction);
                }
                // Nothing open means the status was already held when the fight
                // began. The log only starts at the first damage event, so a
                // pre-fight buff is never seen arriving, only leaving. Credit it
                // from the encounter start, but only when this removal is the
                // first event ever seen for the pair. Once the pair has been
                // observed at all, a later orphan removal is a second source's
                // copy, not a pre-encounter application.
                if seen.insert((owner(e.actor_index), e.status_id)) {
                    open.entry((owner(e.actor_index), e.status_id)).or_insert(0);
                }

                // A removal sees the final value; an apply can be read before
                // the game writes it, so a lone premature reading is corrected in place.
                {
                    let object_key = (e.applier_index, e.source_ids);
                    let slot = (owner(e.actor_index), e.status_id);
                    let mut existing = objects.get_mut(&slot).and_then(|by_key| by_key.remove(&object_key));

                    // The applier can be gone by removal time; fall back to the
                    // source tuple when it matches exactly one open object.
                    if existing.is_none() && e.applier_index.is_none() {
                        if let Some(by_key) = objects.get_mut(&slot) {
                            let matches: Vec<ObjectKey> = by_key
                                .keys()
                                .filter(|(_, ids)| *ids == e.source_ids)
                                .copied()
                                .collect();
                            if let [only] = matches[..] {
                                existing = by_key.remove(&only);
                            }
                        }
                    }
                    match existing {
                        Some(mut points) => {
                            if let Some(value) = e.value {
                                if points.len() == 1 && points[0].1.to_bits() != value.to_bits() {
                                    points[0].1 = value;
                                } else if points.last().map(|(_, v)| v.to_bits())
                                    != Some(value.to_bits())
                                {
                                    points.push((at, value));
                                }
                            }
                            points.push((at, 0.0));
                            retired.entry(slot).or_default().push((object_key, points));
                        }
                        None if seen_objects.insert((slot.0, slot.1, object_key)) => {
                            if let Some(value) = e.value {
                                retired
                                    .entry(slot)
                                    .or_default()
                                    .push((object_key, vec![(0, value), (at, 0.0)]));
                            }
                        }
                        None => {}
                    }
                }

                let source_key = (
                    owner(e.actor_index),
                    e.status_id,
                    (e.applier_index, e.source_ids),
                );
                let source_key = if open_by_source.contains_key(&source_key) || e.applier_index.is_some() {
                    source_key
                } else {
                    let matches: Vec<_> = open_by_source
                        .keys()
                        .filter(|(a, s, (_, ids))| {
                            *a == owner(e.actor_index) && *s == e.status_id && *ids == e.source_ids
                        })
                        .copied()
                        .collect();
                    match matches[..] {
                        [only] => only,
                        _ => source_key,
                    }
                };

                match open_by_source.remove(&source_key) {
                    Some(opened) if at > opened => {
                        sources
                            .entry(owner(e.actor_index))
                            .or_default()
                            .entry(e.status_id)
                            .or_default()
                            .entry((e.applier_index, e.source_ids))
                            .or_default()
                            .push((opened, at));
                    }
                    Some(_) => {}
                    None if seen_sources.insert(source_key) => {
                        if at > 0 {
                            sources
                                .entry(owner(e.actor_index))
                                .or_default()
                                .entry(e.status_id)
                                .or_default()
                                .entry((e.applier_index, e.source_ids))
                                .or_default()
                                .push((0, at));
                        }
                    }
                    None => {}
                }
                if remaining > 0 {
                    continue;
                }
                if let Some(opened) = open.remove(&(owner(e.actor_index), e.status_id)) {
                    if at > opened {
                        out.entry(owner(e.actor_index))
                            .or_default()
                            .entry(e.status_id)
                            .or_default()
                            .push((opened, at));
                    }
                }
            }
            _ => {}
        }
    }

    for ((actor, status), opened) in open {
        if extent > opened {
            out.entry(actor)
                .or_default()
                .entry(status)
                .or_default()
                .push((opened, extent));
        }
    }

    for ((actor, status, object_key), opened) in open_by_source {
        if extent > opened {
            sources
                .entry(actor)
                .or_default()
                .entry(status)
                .or_default()
                .entry(object_key)
                .or_default()
                .push((opened, extent));
        }
    }

    for statuses in out.values_mut() {
        for intervals in statuses.values_mut() {
            intervals.sort_unstable();
            clamp_windows(intervals, extent);
        }
    }

    // Only kinds that ever exceeded depth 1 keep a series; the UI keys the
    // depth chart off its presence.
    for statuses in series.values_mut() {
        statuses.retain(|_, points| {
            points.sort_unstable();
            clamp_series(points, extent);
            points.iter().any(|(_, depth)| *depth > 1)
        });
    }
    series.retain(|_, statuses| !statuses.is_empty());

    for statuses in values.values_mut() {
        statuses.retain(|_, points| {
            points.sort_by(|a, b| a.0.cmp(&b.0));
            clamp_series(points, extent);
            points.iter().any(|(_, v)| *v != 0.0)
        });
    }
    values.retain(|_, statuses| !statuses.is_empty());

    // Where the per-object fold produces anything it replaces the hook's
    // `value_total` series.
    let mut per_source_values: HashMap<(u32, u32), HashMap<ObjectKey, Vec<(i64, f32)>>> =
        HashMap::new();
    {
        let mut all: HashMap<(u32, u32), Vec<(ObjectKey, Vec<(i64, f32)>)>> = HashMap::new();
        for (slot, list) in retired {
            all.entry(slot).or_default().extend(list);
        }
        for (slot, by_key) in objects {
            for (key, points) in by_key {
                if !points.is_empty() {
                    all.entry(slot).or_default().push((key, points));
                }
            }
        }

        for (slot, list) in all {
            let series: Vec<Vec<(i64, f32)>> = list.iter().map(|(_, p)| p.clone()).collect();
            let mut summed = sum_object_series(&series);
            despike(&mut summed);
            clamp_series(&mut summed, extent);
            if !summed.is_empty() {
                values.entry(slot.0).or_default().insert(slot.1, summed);
            }
            let mut by_source: HashMap<ObjectKey, Vec<Vec<(i64, f32)>>> = HashMap::new();
            for (key, points) in list {
                by_source.entry(key).or_default().push(points);
            }
            let folded = per_source_values.entry(slot).or_default();
            for (key, group) in by_source {
                let mut points = sum_object_series(&group);
                despike(&mut points);
                clamp_series(&mut points, extent);
                folded.insert(key, points);
            }
        }
    }

    // Flattened to a list per (actor, kind); JSON cannot key on a pair.
    let sources = sources
        .into_iter()
        .map(|(actor, statuses)| {
            let statuses = statuses
                .into_iter()
                .map(|(status, by_source)| {
                    let mut list: Vec<StatusSourceWindows> = by_source
                        .into_iter()
                        .map(|(object_key, mut windows)| {
                            let (applier_index, source_ids) = object_key;
                            windows.sort_unstable();
                            clamp_windows(&mut windows, extent);
                            let mut vals: Vec<(i64, f32)> = per_source_values
                                .get(&(actor, status))
                                .and_then(|by_key| by_key.get(&object_key))
                                .cloned()
                                .unwrap_or_default();
                            vals.sort_by_key(|(at, _)| *at);
                            clamp_series(&mut vals, extent);
                            StatusSourceWindows {
                                applier_index,
                                source_ids,
                                windows,
                                values: vals,
                            }
                        })
                        .collect();
                    list.retain(|s| !s.windows.is_empty());
                    list.sort_by_key(|s| (s.applier_index, s.source_ids));
                    (status, list)
                })
                .collect();
            (actor, statuses)
        })
        .collect();

    (out, peak, series, values, fractional, sources)
}

fn merge_windows(sorted: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    sorted.into_iter().fold(Vec::new(), |mut merged, (open, close)| {
        match merged.last_mut() {
            Some(last) if open <= last.1 => last.1 = last.1.max(close),
            _ => merged.push((open, close)),
        }
        merged
    })
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn a_chain_runs_from_its_cast_to_its_last_hit() {
        let mut marks = vec![(19_321, false), (20_171, true)];
        let mut at = 26_364;
        while at < 47_423 {
            marks.push((at, true));
            at += 430;
        }
        marks.push((47_423, true));

        assert_eq!(sba_lockdown_windows(&marks), vec![(19_321, 47_423)]);
    }

    #[test]
    fn a_long_silence_inside_a_run_splits_it() {
        let marks = [(1_000, false), (2_000, true), (17_000, false), (18_000, true)];

        assert_eq!(
            sba_lockdown_windows(&marks),
            vec![(1_000, 2_000), (17_000, 18_000)]
        );
    }

    #[test]
    fn chains_far_apart_do_not_merge() {
        let marks = [(1_000, false), (2_000, true), (600_000, false), (601_000, true)];

        assert_eq!(
            sba_lockdown_windows(&marks),
            vec![(1_000, 2_000), (600_000, 601_000)]
        );
    }

    #[test]
    fn a_cast_with_no_hits_draws_nothing() {
        assert!(sba_lockdown_windows(&[(1_000, false), (1_500, false)]).is_empty());
    }

    #[test]
    fn a_log_with_no_sba_at_all_draws_nothing() {
        assert!(sba_lockdown_windows(&[]).is_empty());
    }

    #[test]
    fn transitions_pair_into_windows() {
        let t = [(10_000, true), (25_000, false), (60_000, true), (70_000, false)];

        assert_eq!(
            transition_windows(&t, &[], 120_000),
            vec![(10_000, 25_000), (60_000, 70_000)]
        );
    }

    #[test]
    fn a_window_still_open_at_the_end_is_closed_at_the_duration() {
        assert_eq!(
            transition_windows(&[(10_000, true)], &[], 30_000),
            vec![(10_000, 30_000)]
        );
    }

    #[test]
    fn windows_from_different_enemies_merge_for_one_chart() {
        let merged = merge_windows(vec![(0, 10), (5, 20), (40, 50)]);

        assert_eq!(merged, vec![(0, 20), (40, 50)]);
    }

    #[test]
    fn a_window_open_when_a_fight_ended_closes_there_not_at_the_next_hit() {
        let t = [(10_000, true), (95_000, false)];
        let fight_ends = [30_000];

        assert_eq!(
            transition_windows(&t, &fight_ends, 200_000),
            vec![(10_000, 30_000)]
        );
    }

    #[test]
    fn an_earlier_fight_end_does_not_close_a_later_window() {
        let t = [(50_000, true), (60_000, false)];

        assert_eq!(
            transition_windows(&t, &[30_000], 200_000),
            vec![(50_000, 60_000)]
        );
    }

    #[test]
    fn an_enemys_last_hit_closes_a_window_nothing_else_can() {
        let t = [(116_690, true)];

        assert_eq!(
            transition_windows(&t, &[120_800], 478_727),
            vec![(116_690, 120_800)]
        );
    }

    #[test]
    fn a_leave_break_transition_beats_the_last_hit_backstop() {
        let t = [(453_030, true), (460_110, false)];

        assert_eq!(
            transition_windows(&t, &[478_727], 478_727),
            vec![(453_030, 460_110)]
        );
    }

    #[test]
    fn repeated_same_value_transitions_are_ignored() {
        let t = [(1_000, false), (10_000, true), (12_000, true), (20_000, false)];

        assert_eq!(transition_windows(&t, &[], 60_000), vec![(10_000, 20_000)]);
    }

    // ── Status intervals ──────────────────────────────────────────────
    //
    // The stack rule, and the only coverage of it. Keep this block whole.

    /// Party slot 0, and `StatusGutsBuff`.
    const ACTOR: u32 = protocol::PLAYER_ID_BASE;
    const GUTS: u32 = 0x14;

    fn applied(ts: i64, stacks: Option<u32>) -> (i64, Message) {
        (
            ts,
            Message::OnStatusApplied(protocol::StatusAppliedEvent {
                actor_index: ACTOR,
                status_id: GUTS,
                remaining_secs: 30.0,
                is_refresh: false,
                stacks,
                value_total: None,
                value_is_fraction: None,
                applier_index: None,
                source_ids: None,
                    value: None,
                full_duration_secs: None,
                is_permanent: None,
            }),
        )
    }

    fn removed(ts: i64, stacks: Option<u32>) -> (i64, Message) {
        (
            ts,
            Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                actor_index: ACTOR,
                status_id: GUTS,
                stacks,
                stacks_before: None,
                value_total: None,
                value_is_fraction: None,
                applier_index: None,
                source_ids: None,
                    value: None,
            }),
        )
    }

    fn intervals_for(events: &[(i64, Message)], duration: i64) -> Vec<(i64, i64)> {
        pair_status_intervals(events.iter(), 0, duration, &HashMap::new())
            .0
            .get(&ACTOR)
            .and_then(|statuses| statuses.get(&GUTS))
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn peeling_a_stack_off_leaves_the_interval_open() {
        let events = [
            applied(1_000, Some(1)),
            applied(2_000, Some(2)),
            applied(3_000, Some(3)),
            removed(4_000, Some(2)),
            removed(5_000, Some(1)),
        ];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, chart_extent(9_000))]);
    }

    #[test]
    fn the_last_stack_leaving_closes_the_interval() {
        let events = [
            applied(1_000, Some(2)),
            removed(4_000, Some(1)),
            removed(6_000, Some(0)),
        ];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, 6_000)]);
    }

    #[test]
    fn a_removal_with_nothing_open_is_credited_from_the_encounter_start() {
        let events = [removed(5_000, Some(0))];

        assert_eq!(intervals_for(&events, 9_000), vec![(0, 5_000)]);
    }

    #[test]
    fn a_credited_interval_stays_inside_the_chart() {
        let events = [removed(4_000, Some(2))];

        assert_eq!(intervals_for(&events, 9_000), vec![(0, chart_extent(9_000))]);
    }

    #[test]
    fn a_transform_bodys_status_files_under_the_player() {
        const DRAGON: u32 = 0x0BAD_F00D;
        let owner_of: HashMap<u32, u32> = [(DRAGON, ACTOR)].into_iter().collect();

        let events = [
            (
                1_000,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: DRAGON,
                    status_id: GUTS,
                    remaining_secs: 30.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: None,
                    source_ids: None,
                    value: None,
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                4_000,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: DRAGON,
                    status_id: GUTS,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: None,
                    source_ids: None,
                    value: None,
                }),
            ),
        ];

        let (intervals, _, _, _, _, _) = pair_status_intervals(events.iter(), 0, 9_000, &owner_of);

        assert_eq!(intervals[&ACTOR][&GUTS], vec![(1_000, 4_000)]);
        assert!(!intervals.contains_key(&DRAGON));
    }

    #[test]
    fn per_source_windows_separate_two_appliers() {
        const BOSS: u32 = 0x0BAD_F00D;
        const POISON: u32 = 0x0B;
        const FERRY: u32 = protocol::PLAYER_ID_BASE;
        const IO: u32 = protocol::PLAYER_ID_BASE + 1;

        let apply = |ts: i64, by: u32| {
            (
                ts,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: BOSS,
                    status_id: POISON,
                    remaining_secs: 10.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: Some(by),
                    source_ids: Some([0, 7, 0]),
                    value: None,
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            )
        };
        let remove = |ts: i64, by: u32, remaining: u32| {
            (
                ts,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: BOSS,
                    status_id: POISON,
                    stacks: Some(remaining),
                    stacks_before: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: Some(by),
                    source_ids: Some([0, 7, 0]),
                    value: None,
                }),
            )
        };

        let events = [
            apply(1_000, FERRY),
            apply(4_000, IO),
            remove(6_000, FERRY, 1),
            remove(9_000, IO, 0),
        ];

        let (intervals, _, _, _, _, sources) =
            pair_status_intervals(events.iter(), 0, 12_000, &HashMap::new());

        assert_eq!(intervals[&BOSS][&POISON], vec![(1_000, 9_000)]);

        let by_source = &sources[&BOSS][&POISON];
        assert_eq!(by_source.len(), 2, "one entry per applier");

        let ferry = by_source
            .iter()
            .find(|s| s.applier_index == Some(FERRY))
            .expect("Ferry's window");
        let io = by_source
            .iter()
            .find(|s| s.applier_index == Some(IO))
            .expect("Io's window");
        assert_eq!(ferry.windows, vec![(1_000, 6_000)]);
        assert_eq!(io.windows, vec![(4_000, 9_000)]);

        let covered: i64 = ferry.windows[0].1 - ferry.windows[0].0 + io.windows[0].1
            - io.windows[0].0;
        assert_eq!(covered, 10_000);
    }

    #[test]
    fn per_object_values_sum_without_lagging() {
        const BOSS: u32 = 0x0BAD_F00D;
        const DEF_DOWN: u32 = 0x2A;
        const FERRY: u32 = protocol::PLAYER_ID_BASE;
        const IO: u32 = protocol::PLAYER_ID_BASE + 1;

        let apply = |ts: i64, by: u32, ids: [u32; 3], value: f32| {
            (
                ts,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: BOSS,
                    status_id: DEF_DOWN,
                    remaining_secs: 30.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: Some(0.0),
                    value_is_fraction: Some(true),
                    applier_index: Some(by),
                    source_ids: Some(ids),
                    value: Some(value),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            )
        };

        let events = [
            apply(1_000, FERRY, [0, 0, 0], 0.10),
            apply(4_000, IO, [0, 1200, 0], 0.20),
        ];

        let (_, _, _, values, _, sources) =
            pair_status_intervals(events.iter(), 0, 10_000, &HashMap::new());

        assert_eq!(values[&BOSS][&DEF_DOWN], vec![(1_000, 0.10), (4_000, 0.30)]);

        let by_source = &sources[&BOSS][&DEF_DOWN];
        let ferry = by_source
            .iter()
            .find(|s| s.applier_index == Some(FERRY))
            .expect("Ferry");
        assert_eq!(ferry.values, vec![(1_000, 0.10)]);
    }

    #[test]
    fn a_removal_backfills_a_premature_apply_reading() {
        const BOSS: u32 = 0x0BAD_F00D;
        const BURN: u32 = 0x3E9;
        const FERRY: u32 = protocol::PLAYER_ID_BASE;
        const IDS: [u32; 3] = [0, 1200, 0];

        let events = [
            (
                1_000i64,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: BOSS,
                    status_id: BURN,
                    remaining_secs: 20.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: Some(1000.0),
                    value_is_fraction: Some(false),
                    applier_index: Some(FERRY),
                    source_ids: Some(IDS),
                    value: Some(1000.0),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                9_000i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: BOSS,
                    status_id: BURN,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: Some(4499.0),
                    value_is_fraction: Some(false),
                    applier_index: Some(FERRY),
                    source_ids: Some(IDS),
                    value: Some(4499.0),
                }),
            ),
        ];

        let (_, _, _, values, _, _) =
            pair_status_intervals(events.iter(), 0, 12_000, &HashMap::new());

        assert_eq!(values[&BOSS][&BURN], vec![(1_000, 4499.0), (9_000, 0.0)]);
    }

    #[test]
    fn nothing_is_drawn_past_the_chart() {
        const BOSS: u32 = 0x0BAD_F00D;
        const POISON: u32 = 0x0B;
        const FERRY: u32 = protocol::PLAYER_ID_BASE;
        const IDS: [u32; 3] = [0, 1200, 0];
        const DURATION: i64 = 13_237;

        let events = [
            (
                1_000i64,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: BOSS,
                    status_id: POISON,
                    remaining_secs: 60.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: Some(0.0),
                    value_is_fraction: Some(true),
                    applier_index: Some(FERRY),
                    source_ids: Some(IDS),
                    value: Some(0.20),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                18_760i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: BOSS,
                    status_id: POISON,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: Some(0.20),
                    value_is_fraction: Some(true),
                    applier_index: Some(FERRY),
                    source_ids: Some(IDS),
                    value: Some(0.20),
                }),
            ),
        ];

        let (intervals, _, _, values, _, sources) =
            pair_status_intervals(events.iter(), 0, DURATION, &HashMap::new());

        let edge = chart_extent(DURATION);
        assert_eq!(edge, 15_000, "the bucket containing the last damage event");
        assert_eq!(intervals[&BOSS][&POISON], vec![(1_000, edge)]);
        assert_eq!(sources[&BOSS][&POISON][0].windows, vec![(1_000, edge)]);

        assert_eq!(values[&BOSS][&POISON], vec![(1_000, 0.20)]);
        assert_eq!(sources[&BOSS][&POISON][0].values, vec![(1_000, 0.20)]);
    }

    #[test]
    fn a_one_millisecond_refresh_does_not_dip_to_zero() {
        const PLAYER: u32 = protocol::PLAYER_ID_BASE;
        const DMG_CUT: u32 = 0x4;
        const IDS: [u32; 3] = [75264, 10000, 0];

        let apply = |ts: i64, value: f32| {
            (
                ts,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: PLAYER,
                    status_id: DMG_CUT,
                    remaining_secs: 30.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: None,
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(IDS),
                    value: Some(value),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            )
        };
        let remove = |ts: i64, value: f32| {
            (
                ts,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: PLAYER,
                    status_id: DMG_CUT,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: None,
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(IDS),
                    value: Some(value),
                }),
            )
        };

        let events = [apply(2_796, 0.10), remove(13_323, 0.10), apply(13_324, 0.10)];

        let (_, _, _, values, _, _) =
            pair_status_intervals(events.iter(), 0, 20_000, &HashMap::new());

        let series = &values[&PLAYER][&DMG_CUT];
        assert!(
            series.iter().all(|(at, v)| *v != 0.0 || *at == 0),
            "no zero in the middle of a continuous buff: {series:?}"
        );
    }

    #[test]
    fn a_removal_that_lost_its_applier_still_closes_the_object() {
        const PLAYER: u32 = protocol::PLAYER_ID_BASE + 1;
        const FROSTBITE: u32 = 1005;
        const ENEMY: u32 = 1_822_830_607;
        const IDS: [u32; 3] = [76032, 0, 0];

        let events = [
            (
                93_171i64,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: PLAYER,
                    status_id: FROSTBITE,
                    remaining_secs: 40.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: Some(ENEMY),
                    source_ids: Some(IDS),
                    value: None,
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                129_491i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: PLAYER,
                    status_id: FROSTBITE,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: None,
                    value_is_fraction: None,
                    applier_index: None,
                    source_ids: Some(IDS),
                    value: None,
                }),
            ),
        ];

        let (_, _, _, _, _, sources) =
            pair_status_intervals(events.iter(), 0, 200_000, &HashMap::new());

        let list = &sources[&PLAYER][&FROSTBITE];
        assert_eq!(list.len(), 1, "one source, not one open and one orphan");
        assert_eq!(
            list[0].windows,
            vec![(93_171, 129_491)],
            "closes when it was removed, not at the end of the encounter"
        );
    }

    #[test]
    fn two_valueless_sources_keep_distinct_identities() {
        const PLAYER: u32 = protocol::PLAYER_ID_BASE + 3;
        const STOUT_HEART: u32 = 0x08;

        let ev = |ts: i64, applied: bool, ids: [u32; 3]| {
            (
                ts,
                if applied {
                    Message::OnStatusApplied(protocol::StatusAppliedEvent {
                        actor_index: PLAYER,
                        status_id: STOUT_HEART,
                        remaining_secs: 10.0,
                        is_refresh: false,
                        stacks: Some(1),
                        value_total: None,
                        value_is_fraction: None,
                        applier_index: Some(PLAYER),
                        source_ids: Some(ids),
                        value: None,
                        full_duration_secs: None,
                        is_permanent: None,
                    })
                } else {
                    Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                        actor_index: PLAYER,
                        status_id: STOUT_HEART,
                        stacks: Some(0),
                        stacks_before: Some(1),
                        value_total: None,
                        value_is_fraction: None,
                        applier_index: Some(PLAYER),
                        source_ids: Some(ids),
                        value: None,
                    })
                },
            )
        };

        let a = [69890, 170, 0];
        let b = [69890, 0, 0];
        let events = [
            ev(1_000, true, a),
            ev(2_000, true, b),
            ev(5_000, false, a),
            ev(6_000, false, b),
        ];

        let (_, _, _, _, _, sources) =
            pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new());

        let list = &sources[&PLAYER][&STOUT_HEART];
        assert_eq!(list.len(), 2, "one entry per source, not one merged entry");
        let ids: Vec<_> = list.iter().map(|s| s.source_ids).collect();
        assert!(ids.contains(&Some(a)), "got {ids:?}");
        assert!(ids.contains(&Some(b)), "got {ids:?}");
        assert!(list.iter().all(|s| s.source_ids.is_some()));
    }

    #[test]
    fn a_status_predating_the_log_still_counts_toward_the_total() {
        const PLAYER: u32 = protocol::PLAYER_ID_BASE;
        const DMG_CUT: u32 = 0x4;

        let seen_arriving_ids: [u32; 3] = [0, 10000, 0];
        let predating_ids: [u32; 3] = [0, 1000, 1];

        let events = [
            (
                2_000i64,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: PLAYER,
                    status_id: DMG_CUT,
                    remaining_secs: 60.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: Some(0.0),
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(seen_arriving_ids),
                    value: Some(0.10),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                9_000i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: PLAYER,
                    status_id: DMG_CUT,
                    stacks: Some(1),
                    stacks_before: Some(1),
                    value_total: Some(0.70),
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER + 3),
                    source_ids: Some(predating_ids),
                    value: Some(0.60),
                }),
            ),
            (
                9_500i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: PLAYER,
                    status_id: DMG_CUT,
                    stacks: Some(0),
                    stacks_before: Some(1),
                    value_total: Some(0.10),
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(seen_arriving_ids),
                    value: Some(0.10),
                }),
            ),
        ];

        let (_, _, _, values, _, _) =
            pair_status_intervals(events.iter(), 0, 12_000, &HashMap::new());

        // 0.6 + 0.1 lands on 0.70000005, hence the tolerance.
        let got = &values[&PLAYER][&DMG_CUT];
        let want = [(0i64, 0.60f32), (2_000, 0.70), (9_000, 0.10), (9_500, 0.0)];
        assert_eq!(got.len(), want.len(), "got {got:?}");
        for ((at, value), (want_at, want_value)) in got.iter().zip(want.iter()) {
            assert_eq!(at, want_at, "got {got:?}");
            assert!(
                (value - want_value).abs() < 1e-5,
                "at {at}ms: {value} != {want_value} (got {got:?})"
            );
        }
    }

    #[test]
    fn a_depth_change_keeps_its_history_through_removal() {
        const PLAYER: u32 = protocol::PLAYER_ID_BASE;
        const STACKED: u32 = 0x39;
        const IDS: [u32; 3] = [0, 1700, 0];

        let events = [
            (
                1_000i64,
                Message::OnStatusApplied(protocol::StatusAppliedEvent {
                    actor_index: PLAYER,
                    status_id: STACKED,
                    remaining_secs: 60.0,
                    is_refresh: false,
                    stacks: Some(1),
                    value_total: Some(0.0),
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(IDS),
                    value: Some(0.10),
                    full_duration_secs: None,
                    is_permanent: None,
                }),
            ),
            (
                5_000i64,
                Message::OnStatusStacksChanged(protocol::StatusStacksChangedEvent {
                    actor_index: PLAYER,
                    status_id: STACKED,
                    stacks: 3,
                    value_total: None,
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(IDS),
                    value: Some(0.30),
                }),
            ),
            (
                9_000i64,
                Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                    actor_index: PLAYER,
                    status_id: STACKED,
                    stacks: Some(0),
                    stacks_before: Some(3),
                    value_total: Some(0.30),
                    value_is_fraction: Some(true),
                    applier_index: Some(PLAYER),
                    source_ids: Some(IDS),
                    value: Some(0.30),
                }),
            ),
        ];

        let (_, _, _, values, _, _) =
            pair_status_intervals(events.iter(), 0, 12_000, &HashMap::new());

        assert_eq!(
            values[&PLAYER][&STACKED],
            vec![(1_000, 0.10), (5_000, 0.30), (9_000, 0.0)]
        );
    }

    #[test]
    fn a_recycled_source_is_not_credited_from_zero_each_time() {
        const BOSS: u32 = 0x0BAD_F00D;
        const POISON: u32 = 0x0B;
        const FERRY: u32 = protocol::PLAYER_ID_BASE;

        let ev = |ts: i64, applied: bool, remaining: u32| {
            (
                ts,
                if applied {
                    Message::OnStatusApplied(protocol::StatusAppliedEvent {
                        actor_index: BOSS,
                        status_id: POISON,
                        remaining_secs: 2.0,
                        is_refresh: false,
                        stacks: Some(1),
                        value_total: None,
                        value_is_fraction: None,
                        applier_index: Some(FERRY),
                        source_ids: Some([0, 7, 0]),
                    value: None,
                        full_duration_secs: None,
                        is_permanent: None,
                    })
                } else {
                    Message::OnStatusRemoved(protocol::StatusRemovedEvent {
                        actor_index: BOSS,
                        status_id: POISON,
                        stacks: Some(remaining),
                        stacks_before: Some(1),
                        value_total: None,
                        value_is_fraction: None,
                        applier_index: Some(FERRY),
                        source_ids: Some([0, 7, 0]),
                    value: None,
                    })
                },
            )
        };

        let events = [
            ev(1_000, true, 0),
            ev(2_000, false, 0),
            ev(4_000, true, 0),
            ev(5_000, false, 0),
            ev(7_000, true, 0),
            ev(8_000, false, 0),
        ];

        let (_, _, _, _, _, sources) =
            pair_status_intervals(events.iter(), 0, 10_000, &HashMap::new());

        let windows = &sources[&BOSS][&POISON][0].windows;
        assert_eq!(windows, &vec![(1_000, 2_000), (4_000, 5_000), (7_000, 8_000)]);

        let total: i64 = windows.iter().map(|(a, b)| b - a).sum();
        assert_eq!(total, 3_000, "three one-second windows, not three from zero");
        assert!(total <= 10_000, "uptime cannot exceed the encounter");
    }

    #[test]
    fn child_owners_come_from_the_damage_stream() {
        const DRAGON: u32 = 0x0BAD_F00D;
        let hit = |index: u32, parent: u32| {
            (
                1_000i64,
                Message::DamageEvent(protocol::DamageEvent {
                    source: protocol::Actor {
                        index,
                        actor_type: 0,
                        parent_index: parent,
                        parent_actor_type: 0,
                    },
                    target: protocol::Actor {
                        index: 9,
                        actor_type: 0,
                        parent_index: 9,
                        parent_actor_type: 0,
                    },
                    damage: 1,
                    flags: 0,
                    action_id: protocol::ActionType::Normal(0),
                    attack_rate: None,
                    stun_value: None,
                    damage_cap: None,
                    stun_fill: None,
                    target_base_type: None,
                    stun_max: None,
                }),
            )
        };

        let events = [hit(DRAGON, ACTOR), hit(ACTOR, ACTOR)];
        let owners = child_actor_owners(events.iter());

        assert_eq!(owners.get(&DRAGON), Some(&ACTOR));
        assert!(!owners.contains_key(&ACTOR), "a plain player is not a child");
    }

    #[test]
    fn only_the_first_sighting_of_a_status_is_credited_to_the_start() {
        let events = [
            applied(1_000, Some(1)),
            removed(3_000, Some(0)),
            removed(6_000, Some(0)),
        ];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, 3_000)]);
    }

    #[test]
    fn credited_intervals_never_overlap_observed_ones() {
        let events = [
            applied(1_000, Some(1)),
            removed(3_000, Some(0)),
            applied(4_000, Some(1)),
            removed(5_000, Some(0)),
            removed(8_000, Some(0)),
        ];

        let intervals = intervals_for(&events, 9_000);
        let total: i64 = intervals.iter().map(|(a, b)| b - a).sum();
        assert_eq!(intervals, vec![(1_000, 3_000), (4_000, 5_000)]);
        assert!(total <= 9_000, "uptime cannot exceed the encounter");
    }

    #[test]
    fn an_unknown_depth_closes_the_interval() {
        let events = [applied(1_000, None), removed(4_000, None)];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, 4_000)]);
    }

    #[test]
    fn peak_depth_is_the_deepest_stack_ever_held() {
        let events = [
            applied(1_000, Some(1)),
            applied(2_000, Some(3)),
            removed(4_000, Some(2)),
            removed(5_000, Some(0)),
        ];

        let peaks = pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1;
        assert_eq!(peaks[&ACTOR][&GUTS], 3);
    }

    #[test]
    fn peak_depth_survives_a_stack_first_seen_on_its_way_down() {
        let events = [removed(4_000, Some(2)), removed(5_000, Some(0))];

        let peaks = pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1;
        assert_eq!(peaks[&ACTOR][&GUTS], 3);
    }

    #[test]
    fn a_status_that_never_stacked_peaks_at_one() {
        let events = [applied(1_000, Some(1)), removed(4_000, Some(0))];

        let peaks = pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1;
        assert_eq!(peaks[&ACTOR][&GUTS], 1);
    }

    fn stacks_changed(ts: i64, stacks: u32) -> (i64, Message) {
        (
            ts,
            Message::OnStatusStacksChanged(protocol::StatusStacksChangedEvent {
                actor_index: ACTOR,
                status_id: GUTS,
                stacks,
                value_total: None,
                value_is_fraction: None,
                applier_index: None,
                source_ids: None,
                value: None,
            }),
        )
    }

    #[test]
    fn the_true_depth_arrives_after_the_apply() {
        let events = [applied(1_000, Some(1)), stacks_changed(1_001, 3)];

        let peaks = pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1;
        assert_eq!(peaks[&ACTOR][&GUTS], 3);
    }

    #[test]
    fn spending_charges_does_not_end_the_interval() {
        let events = [
            applied(1_000, Some(1)),
            stacks_changed(1_001, 3),
            stacks_changed(4_000, 2),
            stacks_changed(6_000, 1),
        ];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, chart_extent(9_000))]);
        assert_eq!(pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1[&ACTOR][&GUTS], 3);
    }

    #[test]
    fn the_last_charge_closes_the_interval_via_the_removal() {
        let events = [
            applied(1_000, Some(1)),
            stacks_changed(1_001, 2),
            stacks_changed(4_000, 1),
            removed(6_000, Some(0)),
        ];

        assert_eq!(intervals_for(&events, 9_000), vec![(1_000, 6_000)]);
    }

    fn series_for(events: &[(i64, Message)]) -> Vec<(i64, u32)> {
        pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new())
            .2
            .get(&ACTOR)
            .and_then(|statuses| statuses.get(&GUTS))
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn the_depth_series_records_each_change() {
        let events = [
            applied(1_000, Some(1)),
            stacks_changed(1_001, 3),
            stacks_changed(4_000, 2),
            removed(6_000, Some(0)),
        ];

        assert_eq!(series_for(&events), vec![(1_000, 1), (1_001, 3), (4_000, 2), (6_000, 0)]);
    }

    #[test]
    fn a_status_that_never_stacked_has_no_series() {
        let events = [applied(1_000, Some(1)), removed(4_000, Some(0))];

        assert!(pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).2.is_empty());
    }

    #[test]
    fn the_depth_series_skips_unchanged_repeats() {
        let events = [
            applied(1_000, Some(1)),
            stacks_changed(1_001, 2),
            stacks_changed(2_000, 2),
            applied(3_000, Some(2)),
            stacks_changed(5_000, 1),
        ];

        assert_eq!(series_for(&events), vec![(1_000, 1), (1_001, 2), (5_000, 1)]);
    }

    #[test]
    fn a_depth_change_alone_opens_no_interval() {
        let events = [stacks_changed(4_000, 2)];

        assert_eq!(intervals_for(&events, 9_000), Vec::<(i64, i64)>::new());
        assert_eq!(pair_status_intervals(events.iter(), 0, 9_000, &HashMap::new()).1[&ACTOR][&GUTS], 2);
    }
}
