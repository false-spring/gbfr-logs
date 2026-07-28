use anyhow::Result;
use log::info;
use rusqlite::{Connection, Transaction};
use rusqlite_migration::{Migrations, M};

pub mod logs;

/// Adds a column to `logs`, tolerating one that is already there. Migrations
/// are positional and tester builds shipped the expansion columns out of order.
fn add_column(column: &'static str, decl: &'static str) -> M<'static> {
    M::up_with_hook("", move |tx: &Transaction| {
        let present = tx
            .prepare("SELECT 1 FROM pragma_table_info('logs') WHERE name = ?1")?
            .exists([column])?;

        if !present {
            tx.execute_batch(&format!("ALTER TABLE logs ADD COLUMN {column} {decl}"))?;
        }

        Ok(())
    })
}

/// The ordered migration list. Positional. Only ever append to it.
fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            r#"CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            time INTEGER NOT NULL,
            duration INTEGER NOT NULL,
            data BLOB NOT NULL
        )"#,
        ),
        M::up("ALTER TABLE logs ADD COLUMN version INTEGER NOT NULL DEFAULT 0"),
        M::up("ALTER TABLE logs ADD COLUMN primary_target INTEGER"),
        M::up("ALTER TABLE logs ADD COLUMN p1_name TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p1_type TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p2_name TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p2_type TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p3_name TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p3_type TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p4_name TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN p4_type TEXT"),
        M::up("ALTER TABLE logs ADD COLUMN quest_id INTEGER"),
        M::up("ALTER TABLE logs ADD COLUMN quest_elapsed_time INTEGER"),
        M::up("ALTER TABLE logs ADD COLUMN quest_completed BOOLEAN"),
        add_column("app_version", "TEXT"),
        add_column("game_version", "TEXT"),
        add_column("p1_network_id", "TEXT"),
        add_column("p2_network_id", "TEXT"),
        add_column("p3_network_id", "TEXT"),
        add_column("p4_network_id", "TEXT"),
        // Per-player completed-styles bitmask; NULL = not yet backfilled, 0 = none.
        add_column("p1_styles", "INTEGER"),
        add_column("p2_styles", "INTEGER"),
        add_column("p3_styles", "INTEGER"),
        add_column("p4_styles", "INTEGER"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_time ON logs(time)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_duration ON logs(duration)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_quest_elapsed ON logs(quest_elapsed_time)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_quest_id ON logs(quest_id)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_primary_target ON logs(primary_target)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_game_version ON logs(game_version)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_p1 ON logs(p1_name, p1_type)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_p2 ON logs(p2_name, p2_type)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_p3 ON logs(p3_name, p3_type)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_p4 ON logs(p4_name, p4_type)"),
        M::up("CREATE INDEX IF NOT EXISTS idx_logs_p1_styles ON logs(p1_styles)"),
        // Never shipped in a release, but a dev database may have run it, so
        // drop it here; the list is append-only.
        M::up("DROP INDEX IF EXISTS idx_logs_p1_styles"),
        M::up(
            "CREATE INDEX IF NOT EXISTS idx_logs_pending_styles \
             ON logs(id) WHERE p1_styles IS NULL",
        ),
        // A dev database can sit at a version inside the expansion block and
        // skip a column; re-landing the whole block is a no-op for everyone else.
        add_column("app_version", "TEXT"),
        add_column("game_version", "TEXT"),
        add_column("p1_network_id", "TEXT"),
        add_column("p2_network_id", "TEXT"),
        add_column("p3_network_id", "TEXT"),
        add_column("p4_network_id", "TEXT"),
        add_column("p1_styles", "INTEGER"),
        add_column("p2_styles", "INTEGER"),
        add_column("p3_styles", "INTEGER"),
        add_column("p4_styles", "INTEGER"),
    ])
}

/// Setup database and run migrations.
pub fn setup_db() -> Result<()> {
    info!("Setting up the database, opening logs.db..");

    let mut conn = Connection::open("logs.db")?;

    conn.pragma_update(None, "journal_mode", "WAL")?;

    info!("Database found, running migrations..");

    migrations().to_latest(&mut conn)?;

    Ok(())
}

/// Connect to database.
pub fn connect_to_db() -> Result<Connection> {
    let conn = Connection::open("logs.db")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    Ok(conn)
}

pub fn backfill_style_columns() {
    std::thread::spawn(|| {
        if let Err(e) = run_style_backfill() {
            log::warn!("Style backfill aborted: {e}");
        }
    });
}

fn run_style_backfill() -> Result<()> {
    let mut conn = connect_to_db()?;

    loop {
        let batch: Vec<(i64, Vec<u8>, u8)> = {
            let mut stmt = conn
                .prepare("SELECT id, data, version FROM logs WHERE p1_styles IS NULL LIMIT 100")?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        if batch.is_empty() {
            break;
        }

        // Undecodable blobs and v0 logs get 0s so they're computed once, not retried.
        let updates: Vec<(i64, [u8; 4])> = batch
            .iter()
            .map(|(id, blob, version)| {
                let masks = if *version == 1 {
                    match crate::parser::v1::Encounter::builds_from_blob(blob) {
                        Ok(player_data) => std::array::from_fn(|slot| {
                            player_data[slot].as_ref().map_or(0, |player| {
                                crate::style_catalog::completed_styles(player.master_trait_flags())
                            })
                        }),
                        Err(_) => [0; 4],
                    }
                } else {
                    [0; 4]
                };
                (*id, masks)
            })
            .collect();

        let tx = conn.transaction()?;
        for (id, masks) in updates {
            tx.execute(
                "UPDATE logs SET p1_styles = ?, p2_styles = ?, p3_styles = ?, p4_styles = ? WHERE id = ?",
                rusqlite::params![masks[0], masks[1], masks[2], masks[3], id],
            )?;
        }
        tx.commit()?;

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column_names(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT name FROM pragma_table_info('logs')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migrates_a_fresh_database() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();

        let columns = column_names(&conn);
        for expected in [
            "app_version",
            "game_version",
            "p1_network_id",
            "p4_network_id",
            "p1_styles",
            "p4_styles",
        ] {
            assert!(columns.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn migrates_a_database_that_already_has_a_later_column() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Stop one short of the expansion block, then bolt on the column an
        // early tester build shipped.
        migrations().to_version(&mut conn, 14).unwrap();
        conn.execute_batch("ALTER TABLE logs ADD COLUMN app_version TEXT")
            .unwrap();

        migrations()
            .to_latest(&mut conn)
            .expect("replaying over an existing column must converge");

        let columns = column_names(&conn);
        assert_eq!(
            columns.iter().filter(|c| *c == "app_version").count(),
            1,
            "app_version should exist exactly once"
        );
        assert!(columns.contains(&"game_version".to_string()));
        assert!(columns.contains(&"p4_styles".to_string()));
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();
        let first = column_names(&conn);

        migrations().to_latest(&mut conn).unwrap();
        assert_eq!(first, column_names(&conn));
    }
}
