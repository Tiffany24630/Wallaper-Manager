use rusqlite::{Connection, Result};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf, sync::OnceLock};
use tauri::Manager;

static DATABASE_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/originals")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/thumbnails")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/enhanced")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/resources")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/sources")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/previews")).map_err(|e| e.to_string())?;
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
            original_path TEXT,
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

        CREATE TABLE IF NOT EXISTS monitor_playlists(
            monitor_id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL,
            current_wallpaper_id TEXT,
            last_error TEXT,
            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY(current_wallpaper_id) REFERENCES wallpapers(id) ON DELETE SET NULL
        );
        CREATE INDEX IF NOT EXISTS monitor_playlists_playlist ON monitor_playlists(playlist_id);

        CREATE TABLE IF NOT EXISTS settings(
            id INTEGER PRIMARY KEY,
            launch_at_startup INTEGER DEFAULT 0,
            minimize_to_tray INTEGER DEFAULT 1,
            hardware_acceleration INTEGER DEFAULT 1,
            pause_on_battery INTEGER DEFAULT 0,
            pause_when_maximized INTEGER DEFAULT 1,
            scaling_mode TEXT DEFAULT 'fill',
            accent_color TEXT DEFAULT 'violet',
            ui_scale INTEGER DEFAULT 100,
            dynamic_enabled INTEGER DEFAULT 0,
            dynamic_playlist_id TEXT,
            dynamic_interval_minutes INTEGER DEFAULT 5,
            dynamic_current_wallpaper_id TEXT,
            dynamic_last_error TEXT,
            cursor_enabled INTEGER DEFAULT 0,
            active_cursor_id TEXT
        );

        CREATE TABLE IF NOT EXISTS wallpaper_scheduler(
            id INTEGER PRIMARY KEY,
            enabled INTEGER,
            interval_minutes INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cursor_resources(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            preview TEXT,
            resource_type TEXT NOT NULL,
            source_path TEXT,
            hash TEXT,
            hotspot_x INTEGER NOT NULL DEFAULT 0,
            hotspot_y INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlist_wallpapers(
            playlist_id TEXT NOT NULL,
            wallpaper_id TEXT NOT NULL,
            PRIMARY KEY(playlist_id, wallpaper_id),
            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY(wallpaper_id) REFERENCES wallpapers(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS sync_queue(
            id TEXT PRIMARY KEY,
            operation_type TEXT NOT NULL,
            entity_id TEXT,
            revision INTEGER,
            state TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            next_retry_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_error TEXT
        );
        CREATE INDEX IF NOT EXISTS sync_queue_ready ON sync_queue(state, next_retry_at, created_at);

        CREATE TABLE IF NOT EXISTS sync_tombstones(
            entity_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sync_pending_playlist_items(
            playlist_id TEXT NOT NULL,
            wallpaper_id TEXT NOT NULL,
            PRIMARY KEY(playlist_id, wallpaper_id)
        );
        CREATE TABLE IF NOT EXISTS sync_playlist_state(
            playlist_id TEXT PRIMARY KEY,
            last_remote_revision INTEGER,
            base_payload TEXT,
            local_dirty INTEGER NOT NULL DEFAULT 0,
            deleted_pending INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sync_playlist_conflicts(
            id TEXT PRIMARY KEY,
            playlist_id TEXT NOT NULL,
            local_payload TEXT NOT NULL,
            remote_payload TEXT NOT NULL,
            base_revision INTEGER,
            remote_revision INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            resolved INTEGER NOT NULL DEFAULT 0
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
    // Keep the original source when a managed wallpaper is transformed.
    let _ = conn.execute("ALTER TABLE wallpapers ADD COLUMN original_path TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN dynamic_enabled INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN dynamic_playlist_id TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN dynamic_interval_minutes INTEGER DEFAULT 5",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN dynamic_current_wallpaper_id TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN dynamic_last_error TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN cursor_enabled INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE settings ADD COLUMN active_cursor_id TEXT", []);
    let _ = conn.execute("ALTER TABLE cursor_resources ADD COLUMN hash TEXT", []);
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS cursor_resources_hash_unique ON cursor_resources(hash) WHERE hash IS NOT NULL",
        [],
    );
    backfill_cursor_hashes(&conn)?;
    conn.execute("INSERT OR IGNORE INTO settings(id) VALUES (1)", [])?;
    Ok(conn)
}

fn backfill_cursor_hashes(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT id, COALESCE(NULLIF(source_path, ''), path) FROM cursor_resources WHERE hash IS NULL",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);
    for (id, path) in rows {
        let Ok(mut file) = File::open(&path) else {
            continue;
        };
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1024 * 1024];
        let mut complete = true;
        loop {
            let read = match file.read(&mut buffer) {
                Ok(read) => read,
                Err(_) => {
                    complete = false;
                    break;
                }
            };
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        if !complete {
            continue;
        }
        if let Err(_) = conn.execute(
            "UPDATE cursor_resources SET hash = ?1 WHERE id = ?2 AND hash IS NULL",
            rusqlite::params![format!("{:x}", hasher.finalize()), id],
        ) {
            // A duplicate legacy resource must not prevent Lumina from opening.
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::backfill_cursor_hashes;
    use rusqlite::Connection;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn backfills_legacy_cursor_hash_once_and_keeps_distinct_content() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(
            "CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, source_path TEXT, hash TEXT);",
        ).unwrap();
        let path = std::env::temp_dir().join(format!(
            "lumina-legacy-cursor-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bytes = b"legacy-cur-content";
        fs::write(&path, bytes).unwrap();
        connection.execute(
            "INSERT INTO cursor_resources(id,name,path,source_path) VALUES('old','Same name',?1,NULL)",
            [&path.to_string_lossy()],
        ).unwrap();
        backfill_cursor_hashes(&connection).unwrap();
        backfill_cursor_hashes(&connection).unwrap();
        let expected = format!("{:x}", Sha256::digest(bytes));
        let actual: String = connection
            .query_row(
                "SELECT hash FROM cursor_resources WHERE id='old'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actual, expected);
        connection.execute(
            "CREATE UNIQUE INDEX cursor_resources_hash_unique_test ON cursor_resources(hash) WHERE hash IS NOT NULL", [],
        ).unwrap();
        assert!(connection.execute(
            "INSERT INTO cursor_resources(id,name,path,hash) VALUES('duplicate','Same name','other',?1)", [&expected],
        ).is_err());
        let other_hash = format!("{:x}", Sha256::digest(b"different"));
        assert!(connection.execute(
            "INSERT INTO cursor_resources(id,name,path,hash) VALUES('different','Same name','other',?1)", [&other_hash],
        ).is_ok());
        let _ = fs::remove_file(path);
    }
}
