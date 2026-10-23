use rusqlite::{Connection, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tauri::Manager;

mod migrations;

static DATABASE_PATH: OnceLock<PathBuf> = OnceLock::new();
static SMOKE_DIAGNOSTIC_LOCK: Mutex<()> = Mutex::new(());
const SMOKE_MARKER: &str = "lumina-smoke-root-v1\n";

pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let production_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let smoke_token = smoke_test_token_from_args(std::env::args())?;
    let app_dir = resolve_data_dir(&production_dir, smoke_token.as_deref())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/originals")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/thumbnails")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("wallpapers/enhanced")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/resources")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/sources")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(app_dir.join("cursors/previews")).map_err(|e| e.to_string())?;
    let _ = DATABASE_PATH.set(app_dir.join("lumina.db"));
    eprintln!(
        "LUMINA_DATA_ROOT mode={} path={}",
        if smoke_token.is_some() {
            "smoke"
        } else {
            "production"
        },
        app_dir.display()
    );
    get_connection().map(|_| ()).map_err(|e| e.to_string())
}

fn smoke_test_token_from_args<I>(args: I) -> Result<Option<String>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut token = None;
    for argument in args {
        if let Some(value) = argument.strip_prefix("--lumina-smoke-test=") {
            if token.is_some() {
                return Err("El modo smoke test fue solicitado más de una vez".into());
            }
            if value.is_empty()
                || value.len() > 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err("El identificador smoke test no es válido".into());
            }
            token = Some(value.to_string());
        }
    }
    Ok(token)
}

pub fn smoke_mode_requested() -> Result<bool, String> {
    Ok(smoke_test_token_from_args(std::env::args())?.is_some())
}

pub fn prepare_smoke_webview_data_dir() -> Result<Option<PathBuf>, String> {
    let Some(token) = smoke_test_token_from_args(std::env::args())? else {
        return Ok(None);
    };
    let smoke_dir = std::env::temp_dir().join("lumina-smoke-tests").join(token);
    std::fs::create_dir_all(smoke_dir.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|error| error.to_string())?;
    if smoke_dir.exists() {
        let marker = smoke_dir.join(".lumina-smoke-owned");
        let contents = std::fs::read_to_string(&marker)
            .map_err(|_| "El directorio smoke test existente no está verificado".to_string())?;
        if contents != SMOKE_MARKER {
            return Err("El directorio smoke test existente no pertenece a Lumina".into());
        }
    } else {
        std::fs::create_dir(&smoke_dir).map_err(|error| error.to_string())?;
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(smoke_dir.join(".lumina-smoke-owned"))
            .map_err(|error| error.to_string())?;
        marker
            .write_all(SMOKE_MARKER.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let webview_dir = smoke_dir.join("webview");
    std::fs::create_dir_all(&webview_dir).map_err(|error| error.to_string())?;
    Ok(Some(webview_dir))
}

fn resolve_data_dir(production_dir: &Path, smoke_token: Option<&str>) -> Result<PathBuf, String> {
    let Some(token) = smoke_token else {
        return Ok(production_dir.to_path_buf());
    };

    let smoke_parent = std::env::temp_dir().join("lumina-smoke-tests");
    let smoke_dir = smoke_parent.join(token);
    let production_absolute = absolute_for_comparison(production_dir)?;
    let smoke_absolute = absolute_for_comparison(&smoke_dir)?;
    if smoke_absolute == production_absolute
        || smoke_absolute.starts_with(&production_absolute)
        || production_absolute.starts_with(&smoke_absolute)
    {
        return Err("La ruta smoke test coincide con el directorio real de datos".into());
    }

    std::fs::create_dir_all(&smoke_parent).map_err(|e| e.to_string())?;
    if smoke_dir.exists() {
        let marker = smoke_dir.join(".lumina-smoke-owned");
        let contents = std::fs::read_to_string(&marker)
            .map_err(|_| "El directorio smoke test existente no está verificado".to_string())?;
        if contents != SMOKE_MARKER {
            return Err("El directorio smoke test existente no pertenece a Lumina".into());
        }
    } else {
        std::fs::create_dir(&smoke_dir).map_err(|e| e.to_string())?;
        let marker = smoke_dir.join(".lumina-smoke-owned");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)
            .map_err(|e| e.to_string())?;
        file.write_all(SMOKE_MARKER.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(smoke_dir)
}

fn absolute_for_comparison(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        std::fs::canonicalize(path).map_err(|e| e.to_string())
    } else {
        std::path::absolute(path).map_err(|e| e.to_string())
    }
}

pub fn app_data_dir() -> Result<PathBuf> {
    DATABASE_PATH
        .get()
        .and_then(|path| path.parent().map(PathBuf::from))
        .ok_or(rusqlite::Error::InvalidPath(PathBuf::from(
            "app data no inicializado",
        )))
}

/// Writes diagnostics only for an explicitly requested, validated smoke run.
/// Production never creates or appends to this file.
pub fn record_smoke_diagnostic(stage: &str, detail: Option<&str>) -> Result<bool, String> {
    if smoke_test_token_from_args(std::env::args())?.is_none() {
        return Ok(false);
    }
    if stage.is_empty() || stage.len() > 96 || !stage.is_ascii() {
        return Err("Etapa de diagnóstico smoke inválida".into());
    }
    if detail.is_some_and(|value| value.len() > 1024) {
        return Err("Detalle de diagnóstico smoke demasiado largo".into());
    }
    let directory = app_data_dir().map_err(|error| error.to_string())?;
    let _guard = SMOKE_DIAGNOSTIC_LOCK
        .lock()
        .map_err(|_| "No se pudo bloquear el diagnóstico smoke".to_string())?;
    let record = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "stage": stage,
        "detail": detail,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("lumina-smoke-diagnostics.jsonl"))
        .map_err(|error| error.to_string())?;
    writeln!(file, "{record}").map_err(|error| error.to_string())?;
    Ok(true)
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
            resource_type TEXT NOT NULL DEFAULT 'static',
            parent_wallpaper_id TEXT,
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
            position INTEGER NOT NULL DEFAULT 0,
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
    migrations::run(&conn)?;
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
    use super::{backfill_cursor_hashes, resolve_data_dir, smoke_test_token_from_args};
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

    #[test]
    fn production_mode_keeps_the_requested_data_directory() {
        let production = std::env::temp_dir().join("lumina-production-contract");
        assert_eq!(resolve_data_dir(&production, None).unwrap(), production);
    }

    #[test]
    fn smoke_mode_uses_owned_isolated_directory() {
        let production = std::env::temp_dir().join("lumina-production-contract-real");
        let token = format!("test-{}", std::process::id());
        let isolated = resolve_data_dir(&production, Some(&token)).unwrap();
        assert_ne!(isolated, production);
        assert!(isolated.join(".lumina-smoke-owned").is_file());
        assert_eq!(
            resolve_data_dir(&production, Some(&token)).unwrap(),
            isolated
        );
        let _ = fs::remove_dir_all(isolated);
    }

    #[test]
    fn smoke_mode_rejects_paths_and_is_opt_in() {
        assert_eq!(
            smoke_test_token_from_args(vec!["lumina.exe".into()]).unwrap(),
            None
        );
        assert!(smoke_test_token_from_args(vec![
            "lumina.exe".into(),
            "--lumina-smoke-test=../real".into(),
        ])
        .is_err());
        assert!(smoke_test_token_from_args(vec![
            "lumina.exe".into(),
            "--lumina-smoke-test=C:\\real".into(),
        ])
        .is_err());
    }

    #[test]
    fn smoke_mode_rejects_unowned_existing_directory() {
        let token = format!("unowned-{}", std::process::id());
        let directory = std::env::temp_dir().join("lumina-smoke-tests").join(&token);
        fs::create_dir_all(&directory).unwrap();
        assert!(resolve_data_dir(
            &std::env::temp_dir().join("lumina-production-contract-real"),
            Some(&token)
        )
        .is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
