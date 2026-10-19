use crate::dynamic::DynamicScheduler;
use chrono::Utc;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use rand::seq::SliceRandom;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::Manager;
use tauri::State;
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
        "SELECT id, name, path, thumbnail, favorite, active, size_bytes, width, height, file_type, hash, created_at
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
                hash: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub(crate) fn import_file(
    source_path: &Path,
    display_name: Option<String>,
) -> Result<String, String> {
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
pub fn import_synced_wallpaper(
    bytes: Vec<u8>,
    name: String,
    extension: String,
    expected_hash: String,
    remote_id: Option<String>,
) -> Result<String, String> {
    let extension = extension.to_ascii_lowercase();
    if bytes.is_empty() || !["png", "jpg", "jpeg", "webp", "bmp"].contains(&extension.as_str()) {
        return Err("El archivo remoto no tiene un formato válido".into());
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    if format!("{:x}", hasher.finalize()) != expected_hash.to_ascii_lowercase() {
        return Err("La imagen descargada no coincide con su hash".into());
    }
    let decoded = image::load_from_memory(&bytes)
        .map_err(|e| format!("La imagen descargada no se puede leer: {e}"))?;
    let (width, height) = decoded.dimensions();
    let id = remote_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let conn = get_connection().map_err(|e| e.to_string())?;
    if let Ok(existing) = conn.query_row(
        "SELECT id FROM wallpapers WHERE hash=?1",
        [&expected_hash],
        |row| row.get::<_, String>(0),
    ) {
        return Ok(existing);
    }
    let root = app_data_dir().map_err(|e| e.to_string())?;
    let path = root
        .join("wallpapers/originals")
        .join(format!("{id}.{extension}"));
    let thumbnail = root.join("wallpapers/thumbnails").join(format!("{id}.jpg"));
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    if let Err(error) = generate_thumbnail(&path.to_string_lossy(), &thumbnail.to_string_lossy()) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    conn.execute("INSERT INTO wallpapers(id,name,path,thumbnail,size_bytes,width,height,file_type,hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", rusqlite::params![id, name.trim(), path.to_string_lossy(), thumbnail.to_string_lossy(), bytes.len() as u64, width, height, extension, expected_hash.to_ascii_lowercase(), Utc::now().to_rfc3339()]).map_err(|e| { let _ = fs::remove_file(&path); let _ = fs::remove_file(&thumbnail); e.to_string() })?;
    conn.execute("INSERT OR IGNORE INTO playlist_wallpapers(playlist_id, wallpaper_id) SELECT playlist_id, ?1 FROM sync_pending_playlist_items WHERE wallpaper_id = ?1", [&id]).ok();
    conn.execute(
        "DELETE FROM sync_pending_playlist_items WHERE wallpaper_id=?1",
        [&id],
    )
    .ok();
    Ok(id)
}

#[tauri::command]
pub fn set_active_wallpaper(
    wallpaper_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    set_active_wallpaper_internal(wallpaper_id.clone())?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE settings SET dynamic_enabled = 0, dynamic_current_wallpaper_id = ?1, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        [&wallpaper_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM monitor_playlists", [])
        .map_err(|e| e.to_string())?;
    scheduler.stop();
    Ok(())
}

pub(crate) fn set_active_wallpaper_internal(wallpaper_id: String) -> Result<(), String> {
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
    let (path, thumbnail, original_path): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT path, thumbnail, original_path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "El fondo ya no existe".to_string())?;
    conn.execute("DELETE FROM wallpapers WHERE id = ?1", [&wallpaper_id])
        .map_err(|e| e.to_string())?;
    let _ = fs::remove_file(path);
    if let Some(thumbnail) = thumbnail {
        let _ = fs::remove_file(thumbnail);
    }
    if let Some(original_path) = original_path {
        let _ = fs::remove_file(original_path);
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
pub fn rotate_random_wallpaper() -> Result<(), String> {
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
        set_active_wallpaper_internal(random.clone())?;
    }
    Ok(())
}

fn validate_wallpaper_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("El nombre del fondo no puede estar vacío".into());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("El nombre del fondo contiene caracteres de control no válidos".into());
    }
    Ok(trimmed.to_owned())
}

#[tauri::command]
pub fn rename_wallpaper(wallpaper_id: String, name: String) -> Result<(), String> {
    let name = validate_wallpaper_name(&name)?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    rename_wallpaper_in_connection(&conn, &wallpaper_id, &name)
}

fn rename_wallpaper_in_connection(
    conn: &rusqlite::Connection,
    wallpaper_id: &str,
    name: &str,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE wallpapers SET name = ?1 WHERE id = ?2",
            (name, wallpaper_id),
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("El fondo ya no existe en la biblioteca".into());
    }
    Ok(())
}

fn rotation_format(path: &Path) -> Result<ImageFormat, String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok(ImageFormat::Png),
        Some("jpg") | Some("jpeg") => Ok(ImageFormat::Jpeg),
        Some("webp") => Ok(ImageFormat::WebP),
        Some("bmp") => Ok(ImageFormat::Bmp),
        _ => Err("El formato del fondo no se puede rotar".into()),
    }
}

fn rotate_image(source: DynamicImage, degrees: u16) -> Result<DynamicImage, String> {
    match degrees {
        0 => Ok(source),
        90 => Ok(source.rotate90()),
        180 => Ok(source.rotate180()),
        270 => Ok(source.rotate270()),
        _ => Err("La rotación debe ser 0°, 90°, 180° o 270°".into()),
    }
}

fn replace_validated_image(
    source: &Path,
    destination: &Path,
    degrees: u16,
) -> Result<(u32, u32), String> {
    let image =
        image::open(source).map_err(|e| format!("No se pudo abrir el fondo para rotarlo: {e}"))?;
    let rotated = rotate_image(image, degrees)?;
    let format = rotation_format(destination)?;
    let extension = destination
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("img");
    let temp = destination.with_file_name(format!(
        ".{}.lumina-rotate.{}",
        destination
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
        extension
    ));
    let _ = fs::remove_file(&temp);
    rotated
        .save_with_format(&temp, format)
        .map_err(|e| format!("No se pudo guardar la imagen rotada: {e}"))?;
    validate_generated_image(&temp, rotated.dimensions())?;
    fs::copy(&temp, destination)
        .map_err(|e| format!("No se pudo reemplazar el fondo rotado: {e}"))?;
    let _ = fs::remove_file(&temp);
    validate_generated_image(destination, rotated.dimensions())?;
    Ok(rotated.dimensions())
}

#[tauri::command]
pub fn rotate_wallpaper_version(wallpaper_id: String, degrees: u16) -> Result<(), String> {
    if !matches!(degrees, 0 | 90 | 180 | 270) {
        return Err("La rotación debe ser 0°, 90°, 180° o 270°".into());
    }
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let (path, thumbnail, original_path, active): (String, Option<String>, Option<String>, i32) =
        conn.query_row(
            "SELECT path, thumbnail, original_path, active FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| "El fondo ya no existe en la biblioteca".to_string())?;
    let path = PathBuf::from(path);
    let source_path = if let Some(original_path) = original_path {
        PathBuf::from(original_path)
    } else {
        let backup = path.with_file_name(format!(
            "{}.original{}",
            path.file_stem().unwrap_or_default().to_string_lossy(),
            path.extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default()
        ));
        if !backup.is_file() {
            fs::copy(&path, &backup)
                .map_err(|e| format!("No se pudo conservar el original antes de rotar: {e}"))?;
        }
        conn.execute(
            "UPDATE wallpapers SET original_path = ?1 WHERE id = ?2",
            (&backup.to_string_lossy(), &wallpaper_id),
        )
        .map_err(|e| e.to_string())?;
        backup
    };
    let (width, height) = replace_validated_image(&source_path, &path, degrees)?;
    let thumbnail_path =
        thumbnail.ok_or("El fondo no tiene una miniatura registrada".to_string())?;
    generate_thumbnail(&path.to_string_lossy(), &thumbnail_path)?;
    let hash = file_hash(&path.to_string_lossy())?;
    let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE wallpapers SET width = ?1, height = ?2, size_bytes = ?3, hash = ?4 WHERE id = ?5",
        rusqlite::params![width, height, size, hash, wallpaper_id],
    )
    .map_err(|e| format!("No se pudo actualizar los datos del fondo rotado: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;
    if active == 1 {
        set_active_wallpaper_internal(wallpaper_id)?;
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

fn validate_generated_image(path: &Path, expected: (u32, u32)) -> Result<(), String> {
    if !path.is_file() {
        return Err("Real-ESRGAN no generó el archivo de salida esperado".into());
    }
    let size = fs::metadata(path)
        .map_err(|e| format!("No se pudo inspeccionar la salida mejorada: {e}"))?
        .len();
    if size == 0 {
        return Err("Real-ESRGAN generó un archivo vacío".into());
    }
    let image = image::open(path)
        .map_err(|e| format!("La salida de Real-ESRGAN no es una imagen válida: {e}"))?;
    if image.dimensions() != expected {
        return Err(format!(
            "La salida mejorada tiene dimensiones {} × {}; se esperaban {} × {}",
            image.width(),
            image.height(),
            expected.0,
            expected.1
        ));
    }
    Ok(())
}

fn validate_ai_pixels(
    path: &Path,
    expected: (u32, u32),
    source: &DynamicImage,
) -> Result<(), String> {
    let output = image::open(path)
        .map_err(|e| format!("La salida de Real-ESRGAN no se puede decodificar: {e}"))?;
    if output.dimensions() != expected {
        return Err(format!(
            "La salida mejorada tiene dimensiones {} × {}; se esperaban {} × {}",
            output.width(),
            output.height(),
            expected.0,
            expected.1
        ));
    }
    let source_has_visible_content = source
        .to_rgba8()
        .pixels()
        .any(|pixel| pixel[3] != 0 && (pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0));
    let output_has_visible_content = output
        .to_rgba8()
        .pixels()
        .any(|pixel| pixel[3] != 0 && (pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0));
    if source_has_visible_content && !output_has_visible_content {
        return Err("Real-ESRGAN produjo una imagen transparente o completamente negra".into());
    }
    let color_variation = |image: &DynamicImage| {
        let pixels = image.to_rgba8();
        let mut first = None;
        pixels.pixels().filter(|pixel| pixel[3] != 0).any(|pixel| {
            let color = [pixel[0], pixel[1], pixel[2]];
            match first {
                Some(previous) => previous != color,
                None => {
                    first = Some(color);
                    false
                }
            }
        })
    };
    if color_variation(source) && !color_variation(&output) {
        return Err("Real-ESRGAN produjo una imagen uniforme inválida".into());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct UpscaleResult {
    pub id: String,
    pub method: String,
    pub warning: Option<String>,
}

#[tauri::command]
pub fn upscale_wallpaper(
    app: tauri::AppHandle,
    wallpaper_id: String,
    scale: Option<u8>,
) -> Result<UpscaleResult, String> {
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
    let source = image::open(&source_path)
        .map_err(|e| format!("No se pudo abrir la imagen para mejorarla: {e}"))?;
    let expected_dimensions = target_dimensions(source.width(), source.height(), scale)?;
    let mut used_ai = false;
    let mut warning = None;

    if hardware_acceleration {
        if let Ok(tool_dir) = realesrgan_dir(&app) {
            let executable = tool_dir.join("realesrgan-ncnn-vulkan.exe");
            if executable.is_file() {
                let result = Command::new(&executable)
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
                    .output()
                    .map_err(|e| format!("No se pudo iniciar Real-ESRGAN: {e}"))?;
                if result.status.success() {
                    let valid_output = validate_generated_image(&output_path, expected_dimensions)
                        .is_ok()
                        && validate_ai_pixels(&output_path, expected_dimensions, &source).is_ok();
                    if valid_output {
                        used_ai = true;
                    } else {
                        warning = Some("Real-ESRGAN generÃ³ una salida invÃ¡lida; se usÃ³ el respaldo Lanczos.".into());
                    }
                } else {
                    let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
                    warning = Some(if detail.is_empty() {
                        "Real-ESRGAN no pudo ejecutarse; se usó el respaldo Lanczos.".into()
                    } else {
                        format!("Real-ESRGAN no pudo ejecutarse ({detail}); se usó el respaldo Lanczos.")
                    });
                }
            } else {
                warning = Some(
                    "No se encontró el ejecutable Real-ESRGAN; se usó el respaldo Lanczos.".into(),
                );
            }
        } else {
            warning = Some(
                "No se encontraron los recursos de Real-ESRGAN; se usó el respaldo Lanczos.".into(),
            );
        }
    } else {
        warning = Some(
            "La aceleración por hardware está desactivada; se usó el respaldo Lanczos.".into(),
        );
    }

    if !used_ai {
        let _ = fs::remove_file(&output_path);
        source
            .resize_exact(
                expected_dimensions.0,
                expected_dimensions.1,
                FilterType::Lanczos3,
            )
            .save(&output_path)
            .map_err(|e| format!("No se pudo guardar la imagen mejorada: {e}"))?;
        validate_generated_image(&output_path, expected_dimensions)?;
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
    Ok(UpscaleResult {
        id: new_id,
        method: method.into(),
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        rename_wallpaper_in_connection, replace_validated_image, rotate_image, target_dimensions,
        validate_ai_pixels, validate_generated_image, validate_wallpaper_name,
    };
    use crate::utils::thumbnail::generate_thumbnail;
    use image::{GenericImageView, RgbImage};
    use rusqlite::Connection;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn validates_generated_image_dimensions_and_readability() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-upscale-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("output.png");
        RgbImage::new(24, 16).save(&path).unwrap();

        assert!(validate_generated_image(&path, (24, 16)).is_ok());
        assert!(validate_generated_image(&path, (48, 32)).is_err());
        fs::write(&path, b"not an image").unwrap();
        assert!(validate_generated_image(&path, (24, 16)).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_black_ai_output_when_source_has_visible_pixels() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-ai-pixel-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let source = RgbImage::from_fn(4, 3, |x, _| {
            if x < 2 {
                image::Rgb([220, 80, 40])
            } else {
                image::Rgb([20, 140, 220])
            }
        });
        let source_path = dir.join("source.png");
        let output_path = dir.join("output.png");
        source.save(&source_path).unwrap();
        image::RgbaImage::from_pixel(8, 6, image::Rgba([255, 255, 255, 255]))
            .save(&output_path)
            .unwrap();

        assert!(
            validate_ai_pixels(&output_path, (8, 6), &image::open(&source_path).unwrap()).is_err()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validates_names_without_restricting_unicode() {
        assert_eq!(
            validate_wallpaper_name("  Ártico 夜景  ").unwrap(),
            "Ártico 夜景"
        );
        assert!(validate_wallpaper_name("   ").is_err());
        assert!(validate_wallpaper_name("nombre\ninválido").is_err());
    }

    #[test]
    fn persists_renamed_wallpaper_without_changing_its_path() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE wallpapers(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO wallpapers(id, name, path) VALUES ('id-1', 'Antes', 'managed.png')",
            [],
        )
        .unwrap();
        rename_wallpaper_in_connection(&conn, "id-1", "Nuevo 名称").unwrap();
        let row: (String, String) = conn
            .query_row(
                "SELECT name, path FROM wallpapers WHERE id = 'id-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row, ("Nuevo 名称".into(), "managed.png".into()));
    }

    #[test]
    fn rotates_expected_dimensions_and_updates_thumbnail() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-rotate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.png");
        let destination = dir.join("wallpaper.png");
        let thumbnail = dir.join("thumbnail.jpg");
        RgbImage::new(3, 2).save(&source).unwrap();
        fs::copy(&source, &destination).unwrap();

        assert_eq!(
            rotate_image(image::open(&source).unwrap(), 90)
                .unwrap()
                .dimensions(),
            (2, 3)
        );
        assert_eq!(
            rotate_image(image::open(&source).unwrap(), 180)
                .unwrap()
                .dimensions(),
            (3, 2)
        );
        assert_eq!(
            rotate_image(image::open(&source).unwrap(), 270)
                .unwrap()
                .dimensions(),
            (2, 3)
        );
        assert_eq!(
            rotate_image(image::DynamicImage::ImageRgb8(RgbImage::new(4, 4)), 90)
                .unwrap()
                .dimensions(),
            (4, 4)
        );

        assert_eq!(
            replace_validated_image(&source, &destination, 90).unwrap(),
            (2, 3)
        );
        generate_thumbnail(&destination.to_string_lossy(), &thumbnail.to_string_lossy()).unwrap();
        assert!(image::open(&thumbnail).is_ok());
        assert!(replace_validated_image(&dir.join("missing.png"), &destination, 90).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
