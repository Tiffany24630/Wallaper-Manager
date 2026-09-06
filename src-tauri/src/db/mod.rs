use rusqlite::{Connection, Result};
use std::{path::PathBuf, sync::OnceLock};
use tauri::Manager;

static DATABASE_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/originals")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/thumbnails")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/enhanced")).map_err(|e| e.to_string())?;
    let _ = DATABASE_PATH.set(app_dir.join("lumina.db"));
    get_connection().map(|_| ()).map_err(|e| e.to_string())
}

pub fn app_data_dir() -> Result<PathBuf> {
    DATABASE_PATH
        .get()
        .and_then(|path| path.parent().map(PathBuf::from))
        .ok_or(rusqlite::Error::InvalidPath(PathBuf::from(
            "app data no inicializado",
        )))
}

pub fn get_connection() -> Result<Connection> {
    let path = DATABASE_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("lumina.db"));
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

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
            wallpaper_id TEXT NOT NULL,
            FOREIGN KEY(wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS settings(
            id INTEGER PRIMARY KEY,
            launch_at_startup INTEGER DEFAULT 0,
            minimize_to_tray INTEGER DEFAULT 1,
            hardware_acceleration INTEGER DEFAULT 1,
            pause_on_battery INTEGER DEFAULT 0,
            pause_when_maximized INTEGER DEFAULT 1,
            scaling_mode TEXT DEFAULT 'fill',
            accent_color TEXT DEFAULT 'violet',
            ui_scale INTEGER DEFAULT 100
        );

        CREATE TABLE IF NOT EXISTS wallpaper_scheduler(
            id INTEGER PRIMARY KEY,
            enabled INTEGER,
            interval_minutes INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlist_wallpapers(
            playlist_id TEXT NOT NULL,
            wallpaper_id TEXT NOT NULL,
            PRIMARY KEY(playlist_id, wallpaper_id),
            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY(wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE
        );
        ",
    )?;
    // Keep existing installations compatible when new preference columns are added.
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN accent_color TEXT DEFAULT 'violet'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN ui_scale INTEGER DEFAULT 100",
        [],
    );
    Ok(conn)
}
