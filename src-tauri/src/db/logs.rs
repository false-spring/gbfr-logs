use anyhow::Result;
use rusqlite::Connection;
use sea_query::{Expr, Iden, Order, Query, SqliteQueryBuilder};
use sea_query_rusqlite::RusqliteBinder;
use serde::Serialize;

use crate::parser::constants::EnemyType;

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
) -> anyhow::Result<Vec<LogEntry>> {
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
            })
        })
        .collect::<rusqlite::Result<Vec<LogEntry>>>();

    return Ok(rows.unwrap_or(vec![]));
}

pub fn get_logs_count(
    conn: &Connection,
    filter_by_enemy_id: Option<u32>,
    filter_by_quest_id: Option<u32>,
    cleared: Option<bool>,
) -> Result<i32> {
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
        .build_rusqlite(SqliteQueryBuilder);

    let mut stmt = conn.prepare(&sql).unwrap();
    let params = values.as_params();

    let row: i32 = stmt.query_row(&*params, |r| r.get(0))?;

    return Ok(row);
}
