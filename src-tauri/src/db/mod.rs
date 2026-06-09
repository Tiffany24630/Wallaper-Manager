use rusqlite::{Connection, Result};

pub fn get_connection() -> Result<Connection> {
    let conn = Connection::open("wallpaper_manager.db")?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS wallpapers(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            thumbnail TEXT,
            favorite INTEGER DEFAULT 0,
            active INTEGER DEFAULT 0,
            created_at TEXT
        );

        CREATE TABLE IF NOT EXISTS settings(
            id INTEGER PRIMARY KEY,
            launch_at_startup INTEGER DEFAULT 0,
            minimize_to_tray INTEGER DEFAULT 1,
            hardware_acceleration INTEGER DEFAULT 1,
            pause_on_battery INTEGER DEFAULT 0,
            pause_when_maximized INTEGER DEFAULT 1,
            scaling_mode TEXT DEFAULT 'fill'
        );

        CREATE TABLE IF NOT EXISTS playlists(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        ",
    )?;

    Ok(conn)
}

pub mod migrations;
pub mod schema;