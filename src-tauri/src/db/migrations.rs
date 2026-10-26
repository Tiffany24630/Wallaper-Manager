use rusqlite::{Connection, Result};

const CURRENT_SCHEMA_VERSION: i64 = 3;

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
    if version < 2 {
        migrate_typed_sync_tombstones(conn)?;
    }
    if version < 3 {
        migrate_playlist_conflicts_to_typed_conflicts(conn)?;
    }
    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    Ok(())
}

fn migrate_playlist_conflicts_to_typed_conflicts(conn: &Connection) -> Result<()> {
    let has_legacy: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_playlist_conflicts')",
        [],
        |row| row.get(0),
    )?;
    let has_typed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_conflicts')",
        [],
        |row| row.get(0),
    )?;
    if !has_legacy || !has_typed {
        return Ok(());
    }
    conn.execute(
        "INSERT OR IGNORE INTO sync_conflicts(id,kind,record_id,local_payload,remote_payload,base_revision,remote_revision,created_at,resolved)
         SELECT id,'playlist',playlist_id,local_payload,remote_payload,base_revision,remote_revision,created_at,resolved
         FROM sync_playlist_conflicts",
        [],
    )?;
    Ok(())
}

fn migrate_typed_sync_tombstones(conn: &Connection) -> Result<()> {
    let columns = conn
        .prepare("PRAGMA table_info(sync_tombstones)")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let primary_key = columns
        .iter()
        .filter(|(_, position)| *position > 0)
        .map(|(name, position)| (*position, name.as_str()))
        .collect::<Vec<_>>();
    if primary_key == vec![(1, "entity_id")] {
        conn.execute_batch(
            "
            CREATE TABLE sync_tombstones_typed(
                entity_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY(entity_id, kind)
            );
            INSERT INTO sync_tombstones_typed(entity_id, kind, created_at)
                SELECT entity_id, kind, created_at FROM sync_tombstones;
            DROP TABLE sync_tombstones;
            ALTER TABLE sync_tombstones_typed RENAME TO sync_tombstones;
            ",
        )?;
    }
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
        assert_eq!(version, 3);
    }

    #[test]
    fn migrates_tombstones_to_a_typed_identity_and_is_repeatable() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE settings(id INTEGER PRIMARY KEY);
            CREATE TABLE wallpapers(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, hash TEXT);
            CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL);
            CREATE TABLE playlist_wallpapers(playlist_id TEXT NOT NULL, wallpaper_id TEXT NOT NULL, PRIMARY KEY(playlist_id, wallpaper_id));
            CREATE TABLE sync_tombstones(entity_id TEXT PRIMARY KEY, kind TEXT NOT NULL, created_at INTEGER NOT NULL);
            INSERT INTO sync_tombstones VALUES ('same-id', 'wallpaper', 1);
            ",
        )
        .unwrap();

        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO sync_tombstones(entity_id,kind,created_at) VALUES (?1,?2,?3)",
            ("same-id", "cursor", 2),
        )
        .unwrap();
        run(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE entity_id='same-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn migrates_playlist_conflicts_to_typed_conflicts_without_duplicates() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE settings(id INTEGER PRIMARY KEY);
            CREATE TABLE wallpapers(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, hash TEXT);
            CREATE TABLE cursor_resources(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL);
            CREATE TABLE playlist_wallpapers(playlist_id TEXT NOT NULL, wallpaper_id TEXT NOT NULL, PRIMARY KEY(playlist_id, wallpaper_id));
            CREATE TABLE sync_tombstones(entity_id TEXT PRIMARY KEY, kind TEXT NOT NULL, created_at INTEGER NOT NULL);
            CREATE TABLE sync_playlist_conflicts(id TEXT PRIMARY KEY, playlist_id TEXT NOT NULL, local_payload TEXT NOT NULL, remote_payload TEXT NOT NULL, base_revision INTEGER, remote_revision INTEGER NOT NULL, created_at INTEGER NOT NULL, resolved INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE sync_conflicts(id TEXT PRIMARY KEY, kind TEXT NOT NULL, record_id TEXT NOT NULL, local_payload TEXT NOT NULL, remote_payload TEXT NOT NULL, base_revision INTEGER, remote_revision INTEGER NOT NULL, created_at INTEGER NOT NULL, last_error TEXT, resolved INTEGER NOT NULL DEFAULT 0);
            INSERT INTO sync_playlist_conflicts VALUES ('c1','same-id','{}','{}',1,2,3,0);
            ",
        )
        .unwrap();

        run(&conn).unwrap();
        run(&conn).unwrap();

        let conflict: (String, String) = conn
            .query_row(
                "SELECT kind,record_id FROM sync_conflicts WHERE id='c1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(conflict, ("playlist".into(), "same-id".into()));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_conflicts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
