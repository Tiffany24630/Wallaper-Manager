use std::fs;
use std::path::Path;
use chrono::Utc;
use uuid::Uuid;
use crate::db::get_connection;
use crate::models::wallpaper::Wallpaper;

#[tauri::command]
pub fn list_wallpapers() -> Result<Vec<Wallpaper>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "
            SELECT
                id,
                name,
                path,
                thumbnail,
                favorite,
                active,
                created_at
            FROM wallpapers
        ",
        )
        .map_err(|e| e.to_string())?;

    let wallpapers = stmt
        .query_map([], |row| {
            Ok(Wallpaper {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                thumbnail: row.get(3)?,
                favorite: row.get::<_, i32>(4)? == 1,
                active: row.get::<_, i32>(5)? == 1,
                size_bytes: None,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let result: Vec<Wallpaper> = wallpapers.filter_map(Result::ok).collect();

    Ok(result)
}

#[tauri::command]
pub fn import_wallpaper(
    source_path: String,
) -> Result<(), String> {
    let source = Path::new(&source_path);

    if !source.exists() {
        return Err("File not found".into());
    }

    fs::create_dir_all("../wallpapers/originals").map_err(|e| e.to_string())?;
    fs::create_dir_all("../wallpapers/thumbnails").map_err(|e| e.to_string())?;

    let hash = file_hash(&source_path)?;
    let conn = get_connection().map_err(|e| e.to_string())?;

    let exists: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM wallpapers
            WHERE hash=?1
            ",
            [hash.clone()],
            |row| row.get(0)
        )
        .unwrap_or(0);

    if exists > 0 {
        return Err("Wallpaper already imported".into());
    }

    let id = Uuid::new_v4().to_string();
    let extension = source.extension().unwrap().to_string_lossy();
    let original_path = format!("../wallpapers/originals/{}.{}", id, extension);

    fs::copy(source, &original_path).map_err(|e| e.to_string())?;

    let thumbnail_path = format!("../wallpapers/thumbnails/{}.jpg", id);

    generate_thumbnail(&original_path, &thumbnail_path)?;

    let metadata = fs::metadata(&original_path).map_err(|e| e.to_string())?;

    conn.execute(
        "
        INSERT INTO wallpapers(
            id,
            name,
            path,
            thumbnail,
            size_bytes,
            hash,
            created_at
        )
        VALUES(
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7
        )
        ",
        (
            id,
            source.file_name().unwrap().to_string_lossy().to_string(),
            original_path,
            thumbnail_path,
            metadata.len(),
            hash,
            Utc::now().to_rfc3339(),
        )
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn set_active_wallpaper(
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE wallpapers SET active = 0",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "
        UPDATE wallpapers
        SET active = 1
        WHERE id = ?1
        ",
        [wallpaper_id.clone()],
    )
    .map_err(|e| e.to_string())?;

    let path: String = conn
        .query_row(
            "
            SELECT path
            FROM wallpapers
            WHERE id = ?1
            ",
            [wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    wallpaper::set_from_path(&path).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "
        UPDATE wallpapers
        SET favorite =
            CASE
                WHEN favorite = 1 THEN 0
                ELSE 1
            END
        WHERE id = ?1
        ",
        [wallpaper_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_wallpaper(
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    let path: String = conn
        .query_row(
            "
            SELECT path
            FROM wallpapers
            WHERE id = ?1
            ",
            [wallpaper_id.clone()],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let _ = fs::remove_file(path);

    conn.execute(
        "
        DELETE FROM wallpapers
        WHERE id = ?1
        ",
        [wallpaper_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn scan_wallpaper_folder()
-> Result<(), String> {
    use walkdir::WalkDir;

    for entry in WalkDir::new(
        "../wallpapers/import"
    ) {
        let entry = entry.map_err(|e| e.to_string())?;

        if entry.path().is_file() {
            let path = entry.path().to_string_lossy().to_string();
            let _ = import_wallpaper(path);
        }
    }
    Ok(())
}