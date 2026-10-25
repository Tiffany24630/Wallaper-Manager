use crate::{db::get_connection, dynamic::DynamicScheduler, models::playlist::Playlist};
use chrono::Utc;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn get_playlists() -> Result<Vec<Playlist>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "
            SELECT
                id,
                name,
                created_at,
                (SELECT COUNT(*) FROM playlist_wallpapers pw WHERE pw.playlist_id = playlists.id)
            FROM playlists
            ORDER BY created_at ASC, id ASC
            ",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                wallpaper_count: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("No se pudieron leer las playlists: {e}"))
}

fn validate_playlist_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("El nombre de la lista no puede estar vacío".into());
    }
    if name.chars().any(char::is_control) {
        return Err("El nombre de la lista contiene caracteres de control no válidos".into());
    }
    if name.chars().count() > 80 {
        return Err("El nombre de la lista no puede superar 80 caracteres".into());
    }
    Ok(name.to_owned())
}

fn ensure_playlist_exists(conn: &rusqlite::Connection, playlist_id: &str) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
            [playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists {
        Ok(())
    } else {
        Err("La playlist no existe".into())
    }
}

fn clear_deleted_playlist_references(
    tx: &rusqlite::Transaction<'_>,
    playlist_id: &str,
) -> Result<(), String> {
    for table in [
        "playlist_wallpapers",
        "monitor_playlists",
        "sync_pending_playlist_items",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE playlist_id = ?1"),
            [playlist_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE settings
         SET dynamic_enabled = 0,
             dynamic_playlist_id = NULL,
             dynamic_current_wallpaper_id = NULL,
             dynamic_last_error = ?1
         WHERE dynamic_playlist_id = ?2",
        ("La lista del fondo dinámico fue eliminada", playlist_id),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn add_wallpaper_to_playlist(playlist_id: String, wallpaper_id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    ensure_playlist_exists(&conn, &playlist_id)?;
    let wallpaper_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM wallpapers WHERE id = ?1)",
            [&wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !wallpaper_exists {
        return Err("El wallpaper no existe en la biblioteca".into());
    }
    let already_member: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlist_wallpapers WHERE playlist_id = ?1 AND wallpaper_id = ?2)",
            (&playlist_id, &wallpaper_id),
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if already_member {
        return Err("El wallpaper ya pertenece a esta playlist".into());
    }
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_wallpapers WHERE playlist_id = ?1",
            [&playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "
        INSERT INTO
        playlist_wallpapers(
            playlist_id,
            wallpaper_id,
            position
        )
        VALUES(
            ?1,
            ?2,
            ?3
        )
        ",
        (playlist_id, wallpaper_id, position),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_playlist_wallpapers(playlist_id: String) -> Result<Vec<String>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    ensure_playlist_exists(&conn, &playlist_id)?;

    let mut stmt = conn
        .prepare(
            "
            SELECT wallpaper_id
            FROM playlist_wallpapers
            WHERE playlist_id = ?1
            ORDER BY position ASC, wallpaper_id ASC
            ",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([playlist_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("No se pudieron leer los elementos de la playlist: {e}"))
}

#[tauri::command]
pub fn rename_playlist(playlist_id: String, name: String) -> Result<(), String> {
    let name = validate_playlist_name(&name)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            "UPDATE playlists SET name = ?1 WHERE id = ?2",
            (&name, &playlist_id),
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("La playlist no existe".into());
    }
    Ok(())
}

#[tauri::command]
pub fn reorder_playlist(playlist_id: String, wallpaper_ids: Vec<String>) -> Result<(), String> {
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    reorder_playlist_in_connection(&mut conn, &playlist_id, &wallpaper_ids)
}

fn reorder_playlist_in_connection(
    conn: &mut rusqlite::Connection,
    playlist_id: &str,
    wallpaper_ids: &[String],
) -> Result<(), String> {
    ensure_playlist_exists(conn, playlist_id)?;
    let current = playlist_wallpaper_ids(&conn, playlist_id)?;
    if current.len() != wallpaper_ids.len() {
        return Err("El reordenamiento debe incluir todos los elementos de la playlist".into());
    }
    let mut expected = current;
    expected.sort();
    let mut received = wallpaper_ids.to_vec();
    received.sort();
    if expected != received {
        return Err("El reordenamiento contiene elementos ajenos o faltantes".into());
    }
    let mut seen = std::collections::HashSet::new();
    if wallpaper_ids.iter().any(|id| !seen.insert(id)) {
        return Err("El reordenamiento contiene wallpapers duplicados".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (position, wallpaper_id) in wallpaper_ids.iter().enumerate() {
        tx.execute(
            "UPDATE playlist_wallpapers SET position = ?1 WHERE playlist_id = ?2 AND wallpaper_id = ?3",
            rusqlite::params![position as i64, playlist_id, wallpaper_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

fn playlist_wallpaper_ids(
    conn: &rusqlite::Connection,
    playlist_id: &str,
) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare("SELECT wallpaper_id FROM playlist_wallpapers WHERE playlist_id = ?1 ORDER BY position ASC, wallpaper_id ASC")
        .map_err(|e| e.to_string())?;
    let result = statement
        .query_map([playlist_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string());
    result
}

#[tauri::command]
pub fn import_synced_playlist(
    playlist_id: String,
    name: String,
    wallpaper_ids: Vec<String>,
) -> Result<(), String> {
    let name = validate_playlist_name(&name)?;
    if playlist_id.trim().is_empty() {
        return Err("La lista remota no es válida".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id=?1)",
            [&playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        conn.execute(
            "INSERT INTO playlists(id,name,created_at) VALUES(?1,?2,?3)",
            (&playlist_id, &name, Utc::now().to_rfc3339()),
        )
        .map_err(|e| e.to_string())?;
    }
    for wallpaper_id in wallpaper_ids {
        let present: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM wallpapers WHERE id=?1)",
                [&wallpaper_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if present {
            insert_playlist_wallpaper(&conn, &playlist_id, &wallpaper_id)?;
        } else {
            conn.execute("INSERT OR IGNORE INTO sync_pending_playlist_items(playlist_id,wallpaper_id) VALUES(?1,?2)", (&playlist_id, &wallpaper_id)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn apply_synced_playlist(
    playlist_id: String,
    name: String,
    wallpaper_ids: Vec<String>,
) -> Result<(), String> {
    let name = validate_playlist_name(&name)?;
    if playlist_id.trim().is_empty() {
        return Err("La lista remota no es válida".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO playlists(id,name,created_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name", (&playlist_id, &name, Utc::now().to_rfc3339())).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM playlist_wallpapers WHERE playlist_id=?1",
        [&playlist_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM sync_pending_playlist_items WHERE playlist_id=?1",
        [&playlist_id],
    )
    .map_err(|e| e.to_string())?;
    for wallpaper_id in wallpaper_ids {
        let present: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM wallpapers WHERE id=?1)",
                [&wallpaper_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if present {
            insert_playlist_wallpaper(&conn, &playlist_id, &wallpaper_id)?;
        } else {
            conn.execute("INSERT OR IGNORE INTO sync_pending_playlist_items(playlist_id,wallpaper_id) VALUES(?1,?2)", (&playlist_id, &wallpaper_id)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn create_playlist(name: String) -> Result<String, String> {
    let name = validate_playlist_name(&name)?;
    if name.is_empty() {
        return Err("El nombre de la lista no puede estar vacío".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "
        INSERT INTO playlists(
            id,
            name,
            created_at
        )
        VALUES(
            ?1,
            ?2,
            ?3
        )
        ",
        (&id, &name, Utc::now().to_rfc3339()),
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn delete_playlist(
    playlist_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    ensure_playlist_exists(&conn, &playlist_id)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    clear_deleted_playlist_references(&tx, &playlist_id)?;

    tx.execute(
        "
        DELETE FROM playlists
        WHERE id = ?1
        ",
        [&playlist_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    scheduler.restart();

    Ok(())
}

#[tauri::command]
pub fn remove_wallpaper_from_playlist(
    playlist_id: String,
    wallpaper_id: String,
) -> Result<(), String> {
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    ensure_playlist_exists(&conn, &playlist_id)?;
    let membership: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlist_wallpapers WHERE playlist_id = ?1 AND wallpaper_id = ?2)",
            (&playlist_id, &wallpaper_id),
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !membership {
        return Err("El wallpaper no pertenece a esta playlist".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM playlist_wallpapers WHERE playlist_id = ?1 AND wallpaper_id = ?2",
        (&playlist_id, &wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    normalize_playlist_positions(&tx, &playlist_id)?;
    tx.execute(
        "UPDATE settings SET dynamic_current_wallpaper_id = NULL, dynamic_last_error = ?1 WHERE dynamic_playlist_id = ?2 AND dynamic_current_wallpaper_id = ?3",
        ("El wallpaper actual fue retirado de la playlist", &playlist_id, &wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn insert_playlist_wallpaper(
    conn: &rusqlite::Connection,
    playlist_id: &str,
    wallpaper_id: &str,
) -> Result<(), String> {
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_wallpapers WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO playlist_wallpapers(playlist_id, wallpaper_id, position) VALUES(?1, ?2, ?3)",
        (playlist_id, wallpaper_id, position),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn normalize_playlist_positions(
    conn: &rusqlite::Connection,
    playlist_id: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE playlist_wallpapers
         SET position = (
             SELECT COUNT(*) - 1
             FROM playlist_wallpapers AS prior
             WHERE prior.playlist_id = playlist_wallpapers.playlist_id
              AND (prior.position < playlist_wallpapers.position
                   OR (prior.position = playlist_wallpapers.position
                       AND prior.wallpaper_id <= playlist_wallpapers.wallpaper_id))
         )
         WHERE playlist_id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        clear_deleted_playlist_references, insert_playlist_wallpaper, normalize_playlist_positions,
        reorder_playlist_in_connection, validate_playlist_name,
    };
    use rusqlite::Connection;

    #[test]
    fn playlist_positions_are_explicit_and_stable_after_removal() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE playlist_wallpapers(
                playlist_id TEXT NOT NULL,
                wallpaper_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY(playlist_id, wallpaper_id)
            );",
        )
        .unwrap();

        insert_playlist_wallpaper(&conn, "p", "one").unwrap();
        insert_playlist_wallpaper(&conn, "p", "two").unwrap();
        insert_playlist_wallpaper(&conn, "p", "three").unwrap();
        conn.execute(
            "DELETE FROM playlist_wallpapers WHERE playlist_id='p' AND wallpaper_id='two'",
            [],
        )
        .unwrap();
        normalize_playlist_positions(&conn, "p").unwrap();

        let rows: Vec<(String, i64)> = conn
            .prepare("SELECT wallpaper_id, position FROM playlist_wallpapers WHERE playlist_id='p' ORDER BY position")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![("one".into(), 0), ("three".into(), 1)]);
    }

    #[test]
    fn playlist_names_validate_unicode_controls_and_length() {
        assert_eq!(
            validate_playlist_name("  Música 夜  ").unwrap(),
            "Música 夜"
        );
        assert!(validate_playlist_name("   ").is_err());
        assert!(validate_playlist_name("lista\ninválida").is_err());
        assert!(validate_playlist_name(&"x".repeat(81)).is_err());
    }

    #[test]
    fn reorder_is_explicit_and_rejects_incomplete_or_duplicate_requests() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE playlists(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL);
             CREATE TABLE playlist_wallpapers(playlist_id TEXT NOT NULL, wallpaper_id TEXT NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(playlist_id, wallpaper_id));
             INSERT INTO playlists VALUES('p','Lista','now');
             INSERT INTO playlist_wallpapers VALUES('p','one',0),('p','two',1),('p','three',2);",
        ).unwrap();
        let reversed = vec!["three".into(), "two".into(), "one".into()];
        reorder_playlist_in_connection(&mut conn, "p", &reversed).unwrap();
        let order: Vec<String> = conn.prepare("SELECT wallpaper_id FROM playlist_wallpapers WHERE playlist_id='p' ORDER BY position").unwrap().query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(order, reversed);
        assert!(reorder_playlist_in_connection(
            &mut conn,
            "p",
            &["one".into(), "one".into(), "two".into()]
        )
        .is_err());
        assert!(
            reorder_playlist_in_connection(&mut conn, "p", &["one".into(), "two".into()]).is_err()
        );
        let after_error: Vec<(String, i64)> = conn.prepare("SELECT wallpaper_id, position FROM playlist_wallpapers WHERE playlist_id='p' ORDER BY position").unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(
            after_error,
            vec![("three".into(), 0), ("two".into(), 1), ("one".into(), 2)]
        );
    }

    #[test]
    fn deleting_playlist_clears_all_persistent_references_atomically() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE playlist_wallpapers(playlist_id TEXT, wallpaper_id TEXT, position INTEGER);
             CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, playlist_id TEXT, current_wallpaper_id TEXT, last_error TEXT);
             CREATE TABLE sync_pending_playlist_items(playlist_id TEXT, wallpaper_id TEXT);
             CREATE TABLE settings(dynamic_enabled INTEGER, dynamic_playlist_id TEXT, dynamic_current_wallpaper_id TEXT, dynamic_last_error TEXT);
             INSERT INTO playlist_wallpapers VALUES ('playlist-a', 'wallpaper-a', 0);
             INSERT INTO monitor_playlists VALUES ('monitor-a', 'playlist-a', 'wallpaper-a', NULL);
             INSERT INTO sync_pending_playlist_items VALUES ('playlist-a', 'wallpaper-missing');
             INSERT INTO settings VALUES (1, 'playlist-a', 'wallpaper-a', NULL);",
        )
        .unwrap();

        let tx = conn.unchecked_transaction().unwrap();
        clear_deleted_playlist_references(&tx, "playlist-a").unwrap();
        tx.commit().unwrap();

        for table in [
            "playlist_wallpapers",
            "monitor_playlists",
            "sync_pending_playlist_items",
        ] {
            let count: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE playlist_id = 'playlist-a'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "references remain in {table}");
        }
        let settings: (i64, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT dynamic_enabled, dynamic_playlist_id, dynamic_current_wallpaper_id, dynamic_last_error FROM settings",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(settings.0, 0);
        assert_eq!(settings.1, None);
        assert_eq!(settings.2, None);
        assert_eq!(
            settings.3.as_deref(),
            Some("La lista del fondo dinámico fue eliminada")
        );
    }

    #[test]
    fn deleting_monitor_playlist_preserves_assignments_and_other_monitors() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE playlist_wallpapers(playlist_id TEXT, wallpaper_id TEXT, position INTEGER);
             CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, playlist_id TEXT, current_wallpaper_id TEXT, last_error TEXT);
             CREATE TABLE sync_pending_playlist_items(playlist_id TEXT, wallpaper_id TEXT);
             CREATE TABLE monitor_wallpapers(monitor_id TEXT PRIMARY KEY, wallpaper_id TEXT NOT NULL);
             CREATE TABLE settings(dynamic_enabled INTEGER, dynamic_playlist_id TEXT, dynamic_current_wallpaper_id TEXT, dynamic_last_error TEXT);
             INSERT INTO monitor_playlists VALUES ('monitor-a', 'playlist-a', 'wallpaper-a', NULL), ('monitor-b', 'playlist-b', 'wallpaper-b', NULL);
             INSERT INTO monitor_wallpapers VALUES ('monitor-a', 'wallpaper-a'), ('monitor-b', 'wallpaper-b');
             INSERT INTO settings VALUES (0, NULL, NULL, NULL);",
        )
        .unwrap();

        let tx = conn.unchecked_transaction().unwrap();
        clear_deleted_playlist_references(&tx, "playlist-a").unwrap();
        tx.commit().unwrap();

        let monitor_playlists: Vec<(String, String)> = conn
            .prepare("SELECT monitor_id, playlist_id FROM monitor_playlists ORDER BY monitor_id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            monitor_playlists,
            vec![("monitor-b".into(), "playlist-b".into())]
        );

        let assignments: Vec<(String, String)> = conn
            .prepare("SELECT monitor_id, wallpaper_id FROM monitor_wallpapers ORDER BY monitor_id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            assignments,
            vec![
                ("monitor-a".into(), "wallpaper-a".into()),
                ("monitor-b".into(), "wallpaper-b".into())
            ]
        );
    }
}
