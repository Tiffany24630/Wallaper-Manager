use chrono::Utc;
use image::{imageops::FilterType, GenericImageView};
use rand::seq::SliceRandom;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::Manager;
use uuid::Uuid;

use crate::{
    db::{app_data_dir, get_connection},
    models::wallpaper::Wallpaper,
    utils::{hash::file_hash, thumbnail::generate_thumbnail},
};

#[tauri::command]
pub fn list_wallpapers() -> Result<Vec<Wallpaper>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, name, path, thumbnail, favorite, active, size_bytes, width, height, file_type, created_at
         FROM wallpapers ORDER BY active DESC, created_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Wallpaper {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                thumbnail: row.get(3)?,
                favorite: row.get::<_, i32>(4)? == 1,
                active: row.get::<_, i32>(5)? == 1,
                size_bytes: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                file_type: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn import_file(source_path: &Path, display_name: Option<String>) -> Result<String, String> {
    if !source_path.is_file() {
        return Err("No se encontró el archivo seleccionado".into());
    }
    let decoded = image::open(source_path)
        .map_err(|_| "El archivo no es una imagen compatible".to_string())?;
    let (width, height) = decoded.dimensions();
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .ok_or("La imagen no tiene una extensión válida")?;
    if !["png", "jpg", "jpeg", "webp", "bmp"].contains(&extension.as_str()) {
        return Err("Formato no compatible. Usa PNG, JPG, WEBP o BMP".into());
    }

    let hash = file_hash(&source_path.to_string_lossy())?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM wallpapers WHERE hash = ?1",
            [&hash],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists > 0 {
        return Err("Esta imagen ya está en la biblioteca".into());
    }

    let id = Uuid::new_v4().to_string();
    let root = app_data_dir().map_err(|e| e.to_string())?;
    let original_path = root
        .join("wallpapers/originals")
        .join(format!("{id}.{extension}"));
    let thumbnail_path = root.join("wallpapers/thumbnails").join(format!("{id}.jpg"));
    fs::copy(source_path, &original_path).map_err(|e| e.to_string())?;
    if let Err(error) = generate_thumbnail(
        &original_path.to_string_lossy(),
        &thumbnail_path.to_string_lossy(),
    ) {
        let _ = fs::remove_file(&original_path);
        return Err(error);
    }
    let size = fs::metadata(&original_path)
        .map_err(|e| e.to_string())?
        .len();
    let name = display_name.unwrap_or_else(|| {
        source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    if let Err(error) = conn.execute(
        "INSERT INTO wallpapers(id, name, path, thumbnail, size_bytes, width, height, file_type, hash, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![id, name, original_path.to_string_lossy(), thumbnail_path.to_string_lossy(),
            size, width, height, extension, hash, Utc::now().to_rfc3339()],
    ) {
        let _ = fs::remove_file(&original_path);
        let _ = fs::remove_file(&thumbnail_path);
        return Err(error.to_string());
    }
    Ok(id)
}

#[tauri::command]
pub fn import_wallpaper(source_path: String) -> Result<String, String> {
    import_file(Path::new(&source_path), None)
}

#[tauri::command]
pub fn set_active_wallpaper(wallpaper_id: String) -> Result<(), String> {
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|_| "El fondo ya no existe en la biblioteca".to_string())?;
    let scaling_mode: String = conn
        .query_row("SELECT scaling_mode FROM settings LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or_else(|_| "fill".into());
    let mode = match scaling_mode.as_str() {
        "fit" => wallpaper::Mode::Fit,
        "stretch" => wallpaper::Mode::Stretch,
        "center" => wallpaper::Mode::Center,
        _ => wallpaper::Mode::Crop,
    };
    wallpaper::set_mode(mode)
        .map_err(|e| format!("Windows no pudo cambiar el modo de ajuste: {e}"))?;
    wallpaper::set_from_path(&path)
        .map_err(|e| format!("Windows no pudo aplicar el fondo: {e}"))?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE wallpapers SET active = 0", [])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE wallpapers SET active = 1 WHERE id = ?1",
        [&wallpaper_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_favorite(wallpaper_id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE wallpapers SET favorite = CASE favorite WHEN 1 THEN 0 ELSE 1 END WHERE id = ?1",
        [wallpaper_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_wallpaper(wallpaper_id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (path, thumbnail): (String, Option<String>) = conn
        .query_row(
            "SELECT path, thumbnail FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "El fondo ya no existe".to_string())?;
    conn.execute("DELETE FROM wallpapers WHERE id = ?1", [&wallpaper_id])
        .map_err(|e| e.to_string())?;
    let _ = fs::remove_file(path);
    if let Some(thumbnail) = thumbnail {
        let _ = fs::remove_file(thumbnail);
    }
    Ok(())
}

#[tauri::command]
pub fn scan_wallpaper_folder() -> Result<(), String> {
    let import_dir = app_data_dir()
        .map_err(|e| e.to_string())?
        .join("wallpapers/import");
    fs::create_dir_all(&import_dir).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(import_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            let _ = import_file(entry.path(), None);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rotate_wallpaper() -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id FROM wallpapers")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    if let Some(random) = ids.choose(&mut rand::thread_rng()) {
        set_active_wallpaper(random.clone())?;
    }
    Ok(())
}

fn realesrgan_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources/realesrgan");
    if bundled.exists() {
        return Ok(bundled);
    }
    let development = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("resources/realesrgan");
    if development.exists() {
        Ok(development)
    } else {
        Err("No se encontró Real-ESRGAN en los recursos de la aplicación".into())
    }
}

fn target_dimensions(width: u32, height: u32, scale: u8) -> Result<(u32, u32), String> {
    let target_width = width
        .checked_mul(scale as u32)
        .ok_or("La resolución resultante es demasiado grande")?;
    let target_height = height
        .checked_mul(scale as u32)
        .ok_or("La resolución resultante es demasiado grande")?;
    if u64::from(target_width) * u64::from(target_height) > 120_000_000 {
        return Err(
            "La imagen resultante superaría 120 megapíxeles; elige una escala menor".into(),
        );
    }
    Ok((target_width, target_height))
}

#[tauri::command]
pub fn upscale_wallpaper(
    app: tauri::AppHandle,
    wallpaper_id: String,
    scale: Option<u8>,
) -> Result<String, String> {
    let scale = scale.unwrap_or(4);
    let model = match scale {
        3 => "realesr-animevideov3-x3",
        4 => "realesrgan-x4plus",
        _ => return Err("La escala debe ser 3x o 4x".into()),
    };
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (source_path, original_name): (String, String) = conn
        .query_row(
            "SELECT path, name FROM wallpapers WHERE id = ?1",
            [wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "No se encontró el fondo".to_string())?;
    let hardware_acceleration = conn
        .query_row(
            "SELECT hardware_acceleration FROM settings LIMIT 1",
            [],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(1)
        == 1;
    drop(conn);

    let output_path = app_data_dir()
        .map_err(|e| e.to_string())?
        .join("wallpapers/enhanced")
        .join(format!("{}-{scale}x.png", Uuid::new_v4()));
    let output_string = output_path.to_string_lossy().into_owned();
    let scale_string = scale.to_string();
    let used_ai = hardware_acceleration
        && realesrgan_dir(&app)
            .ok()
            .and_then(|tool_dir| {
                Command::new(tool_dir.join("realesrgan-ncnn-vulkan.exe"))
                    .current_dir(&tool_dir)
                    .args([
                        "-i",
                        &source_path,
                        "-o",
                        &output_string,
                        "-n",
                        model,
                        "-s",
                        &scale_string,
                    ])
                    .status()
                    .ok()
                    .filter(|status| status.success())
            })
            .is_some()
        && output_path.is_file();

    if !used_ai {
        let _ = fs::remove_file(&output_path);
        let source = image::open(&source_path)
            .map_err(|e| format!("No se pudo abrir la imagen para mejorarla: {e}"))?;
        let (width, height) = source.dimensions();
        let (target_width, target_height) = target_dimensions(width, height, scale)?;
        source
            .resize_exact(target_width, target_height, FilterType::Lanczos3)
            .save(&output_path)
            .map_err(|e| format!("No se pudo guardar la imagen mejorada: {e}"))?;
    }

    let stem = Path::new(&original_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let method = if used_ai { "IA" } else { "Lanczos" };
    let new_id = match import_file(
        &output_path,
        Some(format!("{stem} · mejorado {scale}x ({method})")),
    ) {
        Ok(id) => id,
        Err(error) => {
            let _ = fs::remove_file(&output_path);
            return Err(error);
        }
    };
    let _ = fs::remove_file(output_path);
    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::target_dimensions;

    #[test]
    fn calculates_upscaled_resolution() {
        assert_eq!(target_dimensions(640, 360, 4).unwrap(), (2560, 1440));
        assert_eq!(target_dimensions(1280, 720, 3).unwrap(), (3840, 2160));
    }

    #[test]
    fn rejects_unsafe_output_sizes() {
        assert!(target_dimensions(u32::MAX, 100, 4).is_err());
        assert!(target_dimensions(10_000, 10_000, 4).is_err());
    }
}
