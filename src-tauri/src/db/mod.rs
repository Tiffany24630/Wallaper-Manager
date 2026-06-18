use rusqlite::{Connection, Result};

pub mod migrations;
pub mod schema;

pub fn get_connection() -> Result<Connection> {
    let conn = Connection::open("lumina.db")?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS wallpapers(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            thumbnail TEXT,
            favorite INTEGER DEFAULT 0,
            active INTEGER DEFAULT 0,
            width INTEGER,
            height INTEGER,
            size_bytes INTEGER,
            file_type TEXT,
            hash TEXT UNIQUE,
            created_at TEXT
        );

        CREATE TABLE IF NOT EXISTS playlists(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS monitor_wallpapers(
            monitor_id TEXT PRIMARY KEY,
            wallpaper_id TEXT NOT NULL
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

        CREATE TABLE wallpaper_scheduler(
            id INTEGER PRIMARY KEY,
            enabled INTEGER,
            interval_minutes INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlist_wallpapers(
            playlist_id TEXT NOT NULL,
            wallpaper_id TEXT NOT NULL,
            PRIMARY KEY(playlist_id, wallpaper_id)
        );
        "
    )?;
    Ok(conn)
}