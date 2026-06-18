use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;
use crate::{db::get_connection, models::playlist::Playlist,};

#[tauri::command]
pub fn get_playlists()
-> Result<Vec<Playlist>, String> {
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
pub fn add_wallpaper_to_playlist(
    playlist_id: String,
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "
        INSERT INTO
        playlist_wallpapers(
            playlist_id,
            wallpaper_id
        )
        VALUES(
            ?1,
            ?2
        )
        ",
        (
            playlist_id,
            wallpaper_id,
        ),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_playlist_wallpapers(
    playlist_id: String,
) -> Result<Vec<String>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    let mut stmt =
        conn.prepare(
            "
            SELECT wallpaper_id
            FROM playlist_wallpapers
            WHERE playlist_id = ?1
            "
        ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([playlist_id], |row| row.get(0)).map_err(|e| e.to_string())?;

    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn create_playlist(
    name: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

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
        (
            Uuid::new_v4().to_string(),
            name,
            Utc::now().to_rfc3339(),
        ),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_playlist(
    playlist_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "
        DELETE FROM playlists
        WHERE id = ?1
        ",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}