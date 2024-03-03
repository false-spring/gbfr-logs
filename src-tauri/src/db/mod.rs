use anyhow::Result;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

/// Setup database and run migrations.
pub fn setup_db() -> Result<()> {
    let mut conn = Connection::open("logs.db")?;

    conn.pragma_update(None, "journal_mode", &"WAL")?;

    let migrations = Migrations::new(vec![M::up(
        r#"CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            time INTEGER NOT NULL,
            duration INTEGER NOT NULL,
            data BLOB NOT NULL
        )"#,
    )]);

    migrations.to_latest(&mut conn)?;

    Ok(())
}

/// Connect to database.
pub fn connect_to_db() -> Result<Connection> {
    let conn = Connection::open("logs.db")?;
    conn.pragma_update(None, "journal_mode", &"WAL")?;

    Ok(conn)
}
