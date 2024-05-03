use anyhow::Result;
use log::info;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

pub mod logs;

/// Setup database and run migrations.
pub fn setup_db() -> Result<()> {
    info!("Setting up the database, opening logs.db..");

    let mut conn = Connection::open("logs.db")?;

    conn.pragma_update(None, "journal_mode", "WAL")?;

    let migrations = Migrations::new(vec![
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
    ]);

    info!("Database found, running migrations..");

    migrations.to_latest(&mut conn)?;

    Ok(())
}

/// Connect to database.
pub fn connect_to_db() -> Result<Connection> {
    let conn = Connection::open("logs.db")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;

    Ok(conn)
}
