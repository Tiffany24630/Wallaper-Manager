use rusqlite::{Connection, Result};

const CURRENT_SCHEMA_VERSION: i64 = 1;

pub fn run(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    ensure_column(conn, "settings", "accent_color", "TEXT DEFAULT 'violet'")?;
    ensure_column(conn, "settings", "ui_scale", "INTEGER DEFAULT 100")?;
    ensure_column(conn, "wallpapers", "hash", "TEXT")?;
    ensure_column(conn, "wallpapers", "width", "INTEGER")?;
    ensure_column(conn, "wallpapers", "height", "INTEGER")?;
    ensure_column(conn, "wallpapers", "size_bytes", "INTEGER")?;
    ensure_column(conn, "wallpapers", "file_type", "TEXT")?;
    ensure_column(conn, "wallpapers", "original_path", "TEXT")?;
    ensure_column(
        conn,
        "wallpapers",
        "resource_type",
        "TEXT NOT NULL DEFAULT 'static'",
    )?;
    ensure_column(conn, "wallpapers", "parent_wallpaper_id", "TEXT")?;
    ensure_column(conn, "settings", "dynamic_enabled", "INTEGER DEFAULT 0")?;
    ensure_column(conn, "settings", "dynamic_playlist_id", "TEXT")?;
    ensure_column(
        conn,
        "settings",
        "dynamic_interval_minutes",
        "INTEGER DEFAULT 5",
    )?;
    ensure_column(conn, "settings", "dynamic_current_wallpaper_id", "TEXT")?;
    ensure_column(conn, "settings", "dynamic_last_error", "TEXT")?;
    ensure_column(conn, "settings", "cursor_enabled", "INTEGER DEFAULT 0")?;
    ensure_column(conn, "settings", "active_cursor_id", "TEXT")?;
    ensure_column(conn, "cursor_resources", "hash", "TEXT")?;
    ensure_column(
        conn,
        "playlist_wallpapers",
        "position",
        "INTEGER NOT NULL DEFAULT 0",
    )?;

    conn.execute(
        "UPDATE wallpapers SET resource_type = 'static' WHERE resource_type IS NULL OR resource_type = ''",
        [],
    )?;
    conn.execute(
        "UPDATE playlist_wallpapers SET position = (SELECT COUNT(*) - 1 FROM playlist_wallpapers AS prior WHERE prior.playlist_id = playlist_wallpapers.playlist_id AND prior.rowid <= playlist_wallpapers.rowid)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS cursor_resources_hash_unique ON cursor_resources(hash) WHERE hash IS NOT NULL",
        [],
    )?;
    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use rusqlite::Connection;

    #[test]
    fn migrates_legacy_resources_and_playlist_order_once() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE settings(id INTEGER PRIMARY KEY);
            CREATE TABLE wallpapers(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, hash TEXT);
            CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL);
            CREATE TABLE playlist_wallpapers(playlist_id TEXT NOT NULL, wallpaper_id TEXT NOT NULL, PRIMARY KEY(playlist_id, wallpaper_id));
            INSERT INTO wallpapers(id,name,path) VALUES ('w1','Uno','one.png'),('w2','Dos','two.png');
            INSERT INTO playlist_wallpapers(playlist_id,wallpaper_id) VALUES ('p','w1'),('p','w2');
            ",
        )
        .unwrap();

        run(&conn).unwrap();
        run(&conn).unwrap();

        let resource_type: String = conn
            .query_row(
                "SELECT resource_type FROM wallpapers WHERE id='w1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(resource_type, "static");

        let positions: Vec<i64> = conn
            .prepare(
                "SELECT position FROM playlist_wallpapers WHERE playlist_id='p' ORDER BY position",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(positions, vec![0, 1]);

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }
}
