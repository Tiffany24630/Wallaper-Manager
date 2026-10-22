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
                created_at
            FROM playlists
            ",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn add_wallpaper_to_playlist(playlist_id: String, wallpaper_id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_wallpapers WHERE playlist_id = ?1",
            [&playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "
        INSERT OR IGNORE INTO
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

    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn import_synced_playlist(
    playlist_id: String,
    name: String,
    wallpaper_ids: Vec<String>,
) -> Result<(), String> {
    let name = name.trim();
    if playlist_id.trim().is_empty() || name.is_empty() {
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
            (&playlist_id, name, Utc::now().to_rfc3339()),
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
    let name = name.trim();
    if playlist_id.trim().is_empty() || name.is_empty() {
        return Err("La lista remota no es válida".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO playlists(id,name,created_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name", (&playlist_id, name, Utc::now().to_rfc3339())).map_err(|e| e.to_string())?;
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
    let name = name.trim();
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
        (&id, name, Utc::now().to_rfc3339()),
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn delete_playlist(
    playlist_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "
        DELETE FROM playlists
        WHERE id = ?1
        ",
        [&playlist_id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE settings SET dynamic_enabled = 0, dynamic_playlist_id = NULL, dynamic_current_wallpaper_id = NULL, dynamic_last_error = ?1 WHERE dynamic_playlist_id = ?2",
        ("La lista del fondo dinámico fue eliminada", &playlist_id),
    )
    .map_err(|e| e.to_string())?;
    scheduler.restart();

    Ok(())
}

#[tauri::command]
pub fn remove_wallpaper_from_playlist(
    playlist_id: String,
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM playlist_wallpapers WHERE playlist_id = ?1 AND wallpaper_id = ?2",
        (&playlist_id, wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    normalize_playlist_positions(&conn, &playlist_id)?;
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
               AND prior.rowid <= playlist_wallpapers.rowid
         )
         WHERE playlist_id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{insert_playlist_wallpaper, normalize_playlist_positions};
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
}
