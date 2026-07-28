use anyhow::Result;
use rusqlite::Connection;
use sea_query::{Expr, Condition, Iden, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::RusqliteBinder;
use serde::Serialize;

use crate::parser::constants::EnemyType;
use crate::style_catalog;

/// Rows recorded before the game version was captured store NULL; this
/// string selects that bucket.
pub const PRE_EXPANSION_SENTINEL: &str = "__pre_expansion__";

pub enum SortType {
    Time,
    Duration,
    QuestElapsedTime,
}

pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Iden)]
enum Logs {
    Table,
    Id,
    Name,
    Time,
    Duration,
    Version,
    PrimaryTarget,
    P1Name,
    P1Type,
    P2Name,
    P2Type,
    P3Name,
    P3Type,
    P4Name,
    P4Type,
    QuestId,
    QuestElapsedTime,
    QuestCompleted,
    GameVersion,
    P1NetworkId,
    P2NetworkId,
    P3NetworkId,
    P4NetworkId,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// The ID of the log entry.
    id: u64,
    /// The name of the log.
    name: String,
    /// Milliseconds since UNIX epoch.
    time: i64,
    /// Duration of the encounter in milliseconds.
    duration: i64,
    /// The version of the parser used
    version: u8,
    /// Primary enemy target
    primary_target: Option<EnemyType>,
    /// Player 1 display name
    p1_name: Option<String>,
    /// Player 1 character type
    p1_type: Option<String>,
    /// Player 2 display name
    p2_name: Option<String>,
    /// Player 2 character type
    p2_type: Option<String>,
    /// Player 3 display name
    p3_name: Option<String>,
    /// Player 3 character type
    p3_type: Option<String>,
    /// Player 4 display name
    p4_name: Option<String>,
    /// Player 4 character type
    p4_type: Option<String>,
    /// Quest ID
    quest_id: Option<u32>,
    /// Quest elapsed time
    quest_elapsed_time: Option<u32>,
    /// Was quest completed?
    quest_completed: Option<bool>,
    /// The game version this encounter was recorded on (NULL = "Pre-Expansion")
    game_version: Option<String>,
    /// Persistent PlayFab entity id; NULL for local/offline members.
    p1_network_id: Option<String>,
    p2_network_id: Option<String>,
    p3_network_id: Option<String>,
    p4_network_id: Option<String>,
}

/// Per-slot filters, AND'd within a slot then OR'd across P1..P4; all active
/// filters must match the same party slot.
fn player_slot_condition(
    filter_by_player_id: &Option<String>,
    filter_by_player_character: &Option<String>,
    style_mask: Option<u8>,
) -> Option<Condition> {
    if filter_by_player_id.is_none() && filter_by_player_character.is_none() && style_mask.is_none()
    {
        return None;
    }

    let slots = [
        (Logs::P1Name, Logs::P1Type, "p1_styles"),
        (Logs::P2Name, Logs::P2Type, "p2_styles"),
        (Logs::P3Name, Logs::P3Type, "p3_styles"),
        (Logs::P4Name, Logs::P4Type, "p4_styles"),
    ];

    let mut cond = Condition::any();
    for (name_col, type_col, styles_col) in slots {
        let mut slot = Condition::all();
        if let Some(player_id) = filter_by_player_id {
            slot = slot.add(Expr::col(name_col).eq(player_id.clone()));
        }
        if let Some(player_character) = filter_by_player_character {
            slot = slot.add(Expr::col(type_col).eq(player_character.clone()));
        }
        if let Some(mask) = style_mask {
            slot = slot.add(Expr::cust_with_values(
                format!("({styles_col} & ?) <> 0"),
                [mask as i32],
            ));
        }
        cond = cond.add(slot);
    }

    Some(cond)
}

fn game_version_condition(filter_by_game_versions: &[String]) -> Option<Condition> {
    if filter_by_game_versions.is_empty() {
        return None;
    }

    let mut real_versions: Vec<String> = Vec::new();
    let mut include_null = false;
    for v in filter_by_game_versions {
        if v == PRE_EXPANSION_SENTINEL {
            include_null = true;
        } else {
            real_versions.push(v.clone());
        }
    }

    let mut cond = Condition::any();
    if !real_versions.is_empty() {
        cond = cond.add(Expr::col(Logs::GameVersion).is_in(real_versions));
    }
    if include_null {
        cond = cond.add(Expr::col(Logs::GameVersion).is_null());
    }

    Some(cond)
}

pub fn get_logs(
    conn: &Connection,
    filter_by_enemy_id: Option<u32>,
    filter_by_quest_id: Option<u32>,
    per_page: u32,
    offset: u32,
    sort_by: &SortType,
    sort_direction: &SortDirection,
    cleared: Option<bool>,
    filter_by_player_id: &Option<String>,
    filter_by_player_character: &Option<String>,
    filter_by_style: &Option<String>,
    filter_by_game_versions: &[String]
) -> anyhow::Result<Vec<LogEntry>> {
    let style_mask = filter_by_style
        .as_deref()
        .and_then(style_catalog::style_filter_mask);
    let slot_condition =
        player_slot_condition(filter_by_player_id, filter_by_player_character, style_mask);
    let version_condition = game_version_condition(filter_by_game_versions);
    let sort_column = match sort_by {
        SortType::Time => Logs::Time,
        SortType::Duration => Logs::Duration,
        SortType::QuestElapsedTime => Logs::QuestElapsedTime,
    };

    let order = match sort_direction {
        SortDirection::Ascending => Order::Asc,
        SortDirection::Descending => Order::Desc,
    };

    let (sql, values) = Query::select()
        .from(Logs::Table)
        .columns([
            Logs::Id,
            Logs::Name,
            Logs::Time,
            Logs::Duration,
            Logs::Version,
            Logs::PrimaryTarget,
            Logs::P1Name,
            Logs::P1Type,
            Logs::P2Name,
            Logs::P2Type,
            Logs::P3Name,
            Logs::P3Type,
            Logs::P4Name,
            Logs::P4Type,
            Logs::QuestId,
            Logs::QuestElapsedTime,
            Logs::QuestCompleted,
            Logs::GameVersion,
            Logs::P1NetworkId,
            Logs::P2NetworkId,
            Logs::P3NetworkId,
            Logs::P4NetworkId,
        ])
        .conditions(
            filter_by_enemy_id.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::PrimaryTarget).eq(filter_by_enemy_id.unwrap()));
            },
            |_| {},
        )
        .conditions(
            filter_by_quest_id.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::QuestId).eq(filter_by_quest_id.unwrap()));
            },
            |_| {},
        )
        .conditions(
            cleared.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::QuestCompleted).eq(cleared.unwrap()));
            },
            |_| {},
        )
        .conditions(
            slot_condition.is_some(),
            |q| {
                q.cond_where(slot_condition.clone().unwrap());
            },
            |_| {},
        )
        .conditions(
            version_condition.is_some(),
            |q| {
                q.cond_where(version_condition.clone().unwrap());
            },
            |_| {},
        )
        .order_by_with_nulls(sort_column, order, sea_query::NullOrdering::Last)
        .limit(per_page.into())
        .offset(offset.into())
        .build_rusqlite(SqliteQueryBuilder);

    let mut stmt = conn.prepare(&sql).unwrap();
    let params = values.as_params();

    let rows = stmt
        .query(&*params)?
        .mapped(|row| {
            Ok(LogEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                time: row.get(2)?,
                duration: row.get(3)?,
                version: row.get(4)?,
                primary_target: row.get::<usize, Option<u32>>(5)?.map(EnemyType::from_hash),
                p1_name: row.get(6)?,
                p1_type: row.get(7)?,
                p2_name: row.get(8)?,
                p2_type: row.get(9)?,
                p3_name: row.get(10)?,
                p3_type: row.get(11)?,
                p4_name: row.get(12)?,
                p4_type: row.get(13)?,
                quest_id: row.get(14)?,
                quest_elapsed_time: row.get(15)?,
                quest_completed: row.get(16)?,
                game_version: row.get(17)?,
                p1_network_id: row.get(18)?,
                p2_network_id: row.get(19)?,
                p3_network_id: row.get(20)?,
                p4_network_id: row.get(21)?,
            })
        })
        .collect::<rusqlite::Result<Vec<LogEntry>>>();

    Ok(rows.unwrap_or(vec![]))
}

pub fn get_logs_count(
    conn: &Connection,
    filter_by_enemy_id: Option<u32>,
    filter_by_quest_id: Option<u32>,
    cleared: Option<bool>,
    filter_by_player_id: &Option<String>,
    filter_by_player_character: &Option<String>,
    filter_by_style: &Option<String>,
    filter_by_game_versions: &[String]
) -> Result<i32> {
    let style_mask = filter_by_style
        .as_deref()
        .and_then(style_catalog::style_filter_mask);
    let slot_condition =
        player_slot_condition(filter_by_player_id, filter_by_player_character, style_mask);
    let version_condition = game_version_condition(filter_by_game_versions);

    let (sql, values) = Query::select()
        .expr(Expr::col(Logs::Id).count())
        .from(Logs::Table)
        .conditions(
            filter_by_enemy_id.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::PrimaryTarget).eq(filter_by_enemy_id.unwrap()));
            },
            |_| {},
        )
        .conditions(
            filter_by_quest_id.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::QuestId).eq(filter_by_quest_id.unwrap()));
            },
            |_| {},
        )
        .conditions(
            cleared.is_some(),
            |q| {
                q.and_where(Expr::col(Logs::QuestCompleted).eq(cleared.unwrap()));
            },
            |_| {},
        )
        .conditions(
            slot_condition.is_some(),
            |q| {
                q.cond_where(slot_condition.clone().unwrap());
            },
            |_| {},
        )
        .conditions(
            version_condition.is_some(),
            |q| {
                q.cond_where(version_condition.clone().unwrap());
            },
            |_| {},
        )
        .build_rusqlite(SqliteQueryBuilder);

    let mut stmt = conn.prepare(&sql).unwrap();
    let params = values.as_params();

    let row: i32 = stmt.query_row(&*params, |r| r.get(0))?;

    Ok(row)
}

pub fn distinct_u32(conn: &rusqlite::Connection, column: &str) -> Result<Vec<u32>, String> {
    let sql = format!("SELECT DISTINCT {column} FROM logs WHERE {column} IS NOT NULL ORDER BY {column}");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<usize, u32>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

pub fn distinct_text(conn: &rusqlite::Connection, column: &str) -> Result<Vec<String>, String> {
    let sql = format!("SELECT DISTINCT {column} FROM logs WHERE {column} IS NOT NULL ORDER BY {column}");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<usize, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

pub fn distinct_party_text(conn: &rusqlite::Connection, suffix: &str) -> Result<Vec<String>, String> {
    let sql = (1..=4)
        .map(|slot| {
            format!("SELECT DISTINCT p{slot}_{suffix} FROM logs WHERE p{slot}_{suffix} IS NOT NULL")
        })
        .collect::<Vec<_>>()
        .join(" UNION ")
        + " ORDER BY 1";
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<usize, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}
