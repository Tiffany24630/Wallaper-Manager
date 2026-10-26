use crate::{
    dynamic::DynamicScheduler,
    models::monitor_assignment::{MonitorIdentity, WallpaperBackend},
    models::wallpaper_coordinator,
    utils::windows_wallpaper::{available_monitor_ids, set_monitor_wallpaper},
};
use chrono::Utc;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use rand::seq::SliceRandom;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;
use tauri::State;
use uuid::Uuid;

use crate::{
    animated_wallpaper,
    db::{app_data_dir, ensure_physical_operations_allowed, get_connection},
    models::wallpaper::{Wallpaper, WallpaperAsset, WallpaperResourceType, WallpaperThumbnail},
    utils::{hash::file_hash, thumbnail::generate_thumbnail},
};

#[tauri::command]
pub fn list_wallpapers() -> Result<Vec<Wallpaper>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, name, path, thumbnail, favorite, active, size_bytes, width, height, file_type, hash, created_at, resource_type, parent_wallpaper_id, original_path
         FROM wallpapers ORDER BY active DESC, created_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let path: String = row.get(2)?;
            let thumbnail: Option<String> = row.get(3)?;
            let original_path: Option<String> = row.get(14)?;
            let resource_type = WallpaperResourceType::from_db(
                &row.get::<_, Option<String>>(12)?
                    .unwrap_or_else(|| "static".into()),
            )
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?;
            Ok(Wallpaper {
                id: row.get(0)?,
                name: row.get(1)?,
                path: path.clone(),
                thumbnail: thumbnail.clone(),
                favorite: row.get::<_, i32>(4)? == 1,
                active: row.get::<_, i32>(5)? == 1,
                size_bytes: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                file_type: row.get(9)?,
                hash: row.get(10)?,
                created_at: row.get(11)?,
                resource_type,
                parent_wallpaper_id: row.get(13)?,
                asset: WallpaperAsset {
                    path: path.clone(),
                    original_path: original_path.clone(),
                    hash: row.get(10)?,
                    size_bytes: row.get(6)?,
                    file_type: row.get(9)?,
                    available: Path::new(&path).is_file(),
                },
                thumbnail_asset: WallpaperThumbnail {
                    path: thumbnail.clone(),
                    available: thumbnail
                        .as_deref()
                        .is_some_and(|value| Path::new(value).is_file()),
                },
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("No se pudo leer la biblioteca: {error}"))
}

#[tauri::command]
pub fn list_wallpaper_derivative_ids(parent_wallpaper_id: String) -> Result<Vec<String>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT id FROM wallpapers WHERE parent_wallpaper_id = ?1 ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let result = statement
        .query_map([parent_wallpaper_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub(crate) fn import_file(
    source_path: &Path,
    display_name: Option<String>,
) -> Result<String, String> {
    import_file_with_parent(source_path, display_name, None)
}

pub(crate) fn import_file_with_parent(
    source_path: &Path,
    display_name: Option<String>,
    parent_wallpaper_id: Option<&str>,
) -> Result<String, String> {
    if !source_path.is_file() {
        return Err("No se encontró el archivo seleccionado".into());
    }
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .ok_or("La imagen no tiene una extensión válida")?;
    let (width, height, resource_type) = if extension == "gif" {
        let resource = animated_wallpaper::validate_animated_gif(source_path, 0, 0)?;
        (
            resource.width,
            resource.height,
            WallpaperResourceType::Animated,
        )
    } else {
        if !["png", "jpg", "jpeg", "webp", "bmp"].contains(&extension.as_str()) {
            return Err("Formato no compatible. Usa PNG, JPG, WEBP o BMP".into());
        }
        let decoded = image::open(source_path)
            .map_err(|_| "El archivo no es una imagen compatible".to_string())?;
        let (width, height) = decoded.dimensions();
        (width, height, WallpaperResourceType::Static)
    };

    let hash = file_hash(&source_path.to_string_lossy())?;
    let conn = get_connection().map_err(|e| e.to_string())?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM wallpapers WHERE hash = ?1",
            [&hash],
            |row| row.get(0),
        )
        .map_err(|error| format!("No se pudo comprobar si la imagen ya existe: {error}"))?;
    if exists > 0 {
        return Err("Esta imagen ya está en la biblioteca".into());
    }

    let name = validate_wallpaper_name(&display_name.unwrap_or_else(|| {
        source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }))?;
    let id = Uuid::new_v4().to_string();
    let root = app_data_dir().map_err(|e| e.to_string())?;
    let original_path = root
        .join("wallpapers/originals")
        .join(format!("{id}.{extension}"));
    let thumbnail_path = root.join("wallpapers/thumbnails").join(format!("{id}.jpg"));
    fs::copy(source_path, &original_path).map_err(|e| e.to_string())?;
    if let Err(error) = validate_imported_copy(&original_path, &hash, (width, height)) {
        return Err(compensate_import_failure(
            error,
            [&original_path, &thumbnail_path],
        ));
    }
    if let Err(error) = generate_thumbnail(
        &original_path.to_string_lossy(),
        &thumbnail_path.to_string_lossy(),
    ) {
        return Err(compensate_import_failure(
            error,
            [&original_path, &thumbnail_path],
        ));
    }
    let size = fs::metadata(&original_path)
        .map_err(|e| e.to_string())?
        .len();
    if let Err(error) = conn.execute(
        "INSERT INTO wallpapers(id, name, path, thumbnail, size_bytes, width, height, file_type, hash, resource_type, parent_wallpaper_id, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![id, name, original_path.to_string_lossy(), thumbnail_path.to_string_lossy(),
            size, width, height, extension, hash, resource_type.as_str(), parent_wallpaper_id, Utc::now().to_rfc3339()],
    ) {
        return Err(compensate_import_failure(
            error.to_string(),
            [&original_path, &thumbnail_path],
        ));
    }
    Ok(id)
}

fn validate_imported_copy(
    path: &Path,
    expected_hash: &str,
    expected_dimensions: (u32, u32),
) -> Result<(), String> {
    if !path.is_file() {
        return Err("La copia administrada no está disponible".into());
    }
    let image = image::open(path)
        .map_err(|error| format!("La copia administrada no se puede abrir: {error}"))?;
    if image.dimensions() != expected_dimensions {
        return Err("La copia administrada no coincide con las dimensiones originales".into());
    }
    let actual_hash = file_hash(&path.to_string_lossy())?;
    if actual_hash != expected_hash {
        return Err("La copia administrada no coincide con el hash original".into());
    }
    Ok(())
}

fn compensate_import_failure<E: ToString, const N: usize>(error: E, paths: [&Path; N]) -> String {
    let mut cleanup_errors = Vec::new();
    for path in paths {
        if let Err(cleanup_error) = remove_owned_file(&path.to_string_lossy()) {
            cleanup_errors.push(format!("{}: {cleanup_error}", path.display()));
        }
    }
    if cleanup_errors.is_empty() {
        error.to_string()
    } else {
        format!(
            "{}; la compensación física quedó incompleta: {}",
            error.to_string(),
            cleanup_errors.join("; ")
        )
    }
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
    conn.execute(
        "INSERT OR IGNORE INTO playlist_wallpapers(playlist_id, wallpaper_id, position)
         SELECT pending.playlist_id, ?1,
           COALESCE((SELECT MAX(existing.position) + 1 FROM playlist_wallpapers AS existing WHERE existing.playlist_id = pending.playlist_id), 0)
         FROM sync_pending_playlist_items AS pending WHERE pending.wallpaper_id = ?1",
        [&id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM sync_pending_playlist_items WHERE wallpaper_id=?1",
        [&id],
    )
    .ok();
    Ok(id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperApplyResult {
    pub state: String,
    pub wallpaper_id: String,
    pub monitor_id: Option<String>,
    pub generation: Option<u64>,
    pub activation_id: Option<String>,
}

#[tauri::command]
pub async fn set_active_wallpaper(
    app: tauri::AppHandle,
    wallpaper_id: String,
    monitor_id: Option<String>,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<WallpaperApplyResult, String> {
    ensure_physical_operations_allowed()?;
    let (resource_type, path): (String, String) = get_connection()
        .map_err(|e| e.to_string())?
        .query_row(
            "SELECT COALESCE(resource_type, 'static'), path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "El fondo ya no existe en la biblioteca".to_string())?;
    if WallpaperResourceType::from_db(&resource_type)? == WallpaperResourceType::Animated {
        let monitor_id = monitor_id
            .ok_or("Animated requiere seleccionar explícitamente un monitor de destino")?;
        let known_monitors = available_monitor_ids().map_err(|error| error.to_string())?;
        if !known_monitors.iter().any(|known| known == &monitor_id) {
            return Err("El monitor de destino ya no está disponible".into());
        }
        let activation =
            animated_wallpaper::open_library_animated(app, monitor_id, wallpaper_id.clone(), path)
                .await?;
        return Ok(WallpaperApplyResult {
            state: "pending".into(),
            wallpaper_id,
            monitor_id: Some(activation.monitor_id.as_str().to_string()),
            generation: Some(activation.generation),
            activation_id: Some(activation.activation_id),
        });
    }
    let mut manual_operation = scheduler.begin_manual_operation();
    if let Some(monitor_id) = monitor_id.as_deref() {
        set_active_wallpaper_on_monitor(&wallpaper_id, monitor_id, &path)?;
    } else {
        set_active_wallpaper_internal(wallpaper_id.clone())?;
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    if monitor_id.is_some() {
        // `active` is the legacy/global selection. A monitor-specific
        // assignment is authoritative only in monitor_wallpapers and must
        // not clear the global flag for another monitor's wallpaper.
        conn.execute(
            "UPDATE settings SET dynamic_enabled = 0, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE settings SET dynamic_enabled = 0, dynamic_current_wallpaper_id = ?1, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
            [&wallpaper_id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(monitor_id) = monitor_id.as_deref() {
        conn.execute(
            "DELETE FROM monitor_playlists WHERE monitor_id = ?1",
            [monitor_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute("DELETE FROM monitor_playlists", [])
            .map_err(|e| e.to_string())?;
    }
    if monitor_id.is_some() {
        manual_operation.complete_and_resume();
    } else {
        manual_operation.complete();
    }
    Ok(WallpaperApplyResult {
        state: "confirmed".into(),
        wallpaper_id,
        monitor_id,
        generation: None,
        activation_id: None,
    })
}

pub(crate) fn read_scaling_mode(conn: &rusqlite::Connection) -> Result<wallpaper::Mode, String> {
    let value: String = conn
        .query_row("SELECT scaling_mode FROM settings LIMIT 1", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("No se pudo leer scaling_mode desde SQLite: {error}"))?;
    Ok(match value.as_str() {
        "fit" => wallpaper::Mode::Fit,
        "stretch" => wallpaper::Mode::Stretch,
        "center" => wallpaper::Mode::Center,
        _ => wallpaper::Mode::Crop,
    })
}

fn set_active_wallpaper_on_monitor(
    wallpaper_id: &str,
    monitor_id: &str,
    path: &str,
) -> Result<(), String> {
    if !available_monitor_ids()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|known| known == monitor_id)
    {
        return Err("La pantalla seleccionada ya no estÃ¡ disponible".into());
    }
    ensure_asset_available(path)?;

    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let mode = read_scaling_mode(&conn)?;
    wallpaper::set_mode(mode)
        .map_err(|e| format!("Windows no pudo cambiar el modo de ajuste: {e}"))?;

    let identity = MonitorIdentity::new(monitor_id.to_owned()).map_err(str::to_string)?;
    let operation = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?
        .reserve(identity, WallpaperBackend::Static)
        .map_err(|error| error.to_string())?;

    if let Err(error) = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())
        .and_then(|coordinator| {
            coordinator
                .validate_previous_deactivated(&operation)
                .map_err(|error| error.to_string())
        })
    {
        abort_operations(std::slice::from_ref(&operation));
        return Err(error);
    }

    if let Err(error) = set_monitor_wallpaper(monitor_id, path) {
        abort_operations(std::slice::from_ref(&operation));
        return Err(format!(
            "Windows no pudo aplicar el fondo al monitor: {error}"
        ));
    }

    let mut coordinator = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
    let result = (|| {
        coordinator
            .confirm_previous_deactivated(&operation)
            .map_err(|error| error.to_string())?;
        coordinator
            .confirm_activated(&operation)
            .map_err(|error| error.to_string())?;
        coordinator
            .commit(&operation)
            .map_err(|error| error.to_string())?;
        Ok::<(), String>(())
    })();
    if let Err(error) = result {
        coordinator.abort(&operation);
        return Err(error);
    }

    persist_monitor_wallpaper_assignment(&mut conn, monitor_id, wallpaper_id).map_err(|error| {
        format!(
            "El fondo se aplicÃ³ fÃ­sicamente, pero no se pudo persistir la asignaciÃ³n del monitor: {error}"
        )
    })?;
    Ok(())
}

pub(crate) fn set_active_wallpaper_internal(wallpaper_id: String) -> Result<(), String> {
    ensure_physical_operations_allowed()?;
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|_| "El fondo ya no existe en la biblioteca".to_string())?;
    ensure_asset_available(&path)?;
    let mode = read_scaling_mode(&conn)?;
    let operations = reserve_static_for_all_monitors()?;
    if let Err(error) = wallpaper::set_mode(mode)
        .map_err(|e| format!("Windows no pudo cambiar el modo de ajuste: {e}"))
        .and_then(|_| {
            wallpaper::set_from_path(&path)
                .map_err(|e| format!("Windows no pudo aplicar el fondo: {e}"))
        })
    {
        abort_operations(&operations);
        return Err(error);
    }
    {
        let mut coordinator = wallpaper_coordinator::global()
            .lock()
            .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
        let coordination_result = (|| {
            for operation in &operations {
                coordinator
                    .confirm_previous_deactivated(operation)
                    .map_err(|error| error.to_string())?;
            }
            for operation in &operations {
                coordinator
                    .confirm_activated(operation)
                    .map_err(|error| error.to_string())?;
                coordinator
                    .commit(operation)
                    .map_err(|error| error.to_string())?;
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = coordination_result {
            for operation in &operations {
                coordinator.abort(operation);
            }
            return Err(error);
        }
    }
    persist_active_wallpaper(&mut conn, &wallpaper_id).map_err(|error| {
        format!(
            "El fondo se aplicó físicamente, pero no se pudo persistir el estado activo: {error}"
        )
    })
}

pub(crate) fn persist_active_wallpaper(
    conn: &mut rusqlite::Connection,
    wallpaper_id: &str,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE wallpapers SET active = 0", [])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE wallpapers SET active = 1 WHERE id = ?1",
        [wallpaper_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

pub(crate) fn persist_monitor_wallpaper_assignment(
    conn: &mut rusqlite::Connection,
    monitor_id: &str,
    wallpaper_id: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES(?1, ?2)",
        (monitor_id, wallpaper_id),
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn reserve_static_for_all_monitors(
) -> Result<Vec<wallpaper_coordinator::AssignmentOperation>, String> {
    let monitor_ids = available_monitor_ids().map_err(|error| error.to_string())?;
    let mut operations = Vec::with_capacity(monitor_ids.len());
    let mut coordinator = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
    for monitor_id in monitor_ids {
        let identity = MonitorIdentity::new(monitor_id).map_err(str::to_string)?;
        match coordinator.reserve(identity, WallpaperBackend::Static) {
            Ok(operation) => {
                if let Err(error) = coordinator.validate_previous_deactivated(&operation) {
                    for previous in &operations {
                        coordinator.abort(previous);
                    }
                    coordinator.abort(&operation);
                    return Err(error.to_string());
                }
                operations.push(operation)
            }
            Err(error) => {
                for operation in &operations {
                    coordinator.abort(operation);
                }
                return Err(error.to_string());
            }
        }
    }
    Ok(operations)
}

fn abort_operations(operations: &[wallpaper_coordinator::AssignmentOperation]) {
    if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
        for operation in operations {
            coordinator.abort(operation);
        }
    }
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
pub fn delete_wallpaper(
    wallpaper_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let mut manual_operation = scheduler.begin_manual_operation();
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (path, thumbnail, original_path, active): (String, Option<String>, Option<String>, i32) =
        conn.query_row(
            "SELECT path, thumbnail, original_path, active FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| "El fondo ya no existe".to_string())?;
    if active == 1 {
        return Err(
            "No se puede eliminar el wallpaper activo sin coordinar primero su reemplazo".into(),
        );
    }
    let assigned_monitors = wallpaper_assigned_monitors(&conn, &wallpaper_id)?;
    if !assigned_monitors.is_empty() {
        return Err(format!(
            "No se puede eliminar el wallpaper mientras estÃ¡ asignado a los monitores: {}",
            assigned_monitors.join(", ")
        ));
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    clear_wallpaper_references(&tx, &wallpaper_id)?;
    tx.execute("DELETE FROM wallpapers WHERE id = ?1", [&wallpaper_id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    let mut cleanup_errors = Vec::new();
    for candidate in [Some(path), thumbnail, original_path].into_iter().flatten() {
        if let Err(error) = remove_owned_file(&candidate) {
            cleanup_errors.push(format!("{candidate}: {error}"));
        }
    }
    let result = if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "El registro del wallpaper fue eliminado, pero la limpieza física quedó incompleta: {}",
            cleanup_errors.join("; ")
        ))
    };
    manual_operation.complete_and_resume();
    result
}

fn remove_owned_file(path: &str) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => fs::remove_file(path).map_err(|e| e.to_string()),
        Ok(_) => Err("la ruta no es un archivo".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn wallpaper_assigned_monitors(
    conn: &rusqlite::Connection,
    wallpaper_id: &str,
) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare(
            "SELECT monitor_id FROM monitor_wallpapers WHERE wallpaper_id = ?1 ORDER BY monitor_id",
        )
        .map_err(|error| error.to_string())?;
    let result = statement
        .query_map([wallpaper_id], |row| row.get(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|error| error.to_string());
    result
}

fn clear_wallpaper_references(
    tx: &rusqlite::Transaction<'_>,
    wallpaper_id: &str,
) -> Result<(), String> {
    tx.execute(
        "DELETE FROM playlist_wallpapers WHERE wallpaper_id = ?1",
        [wallpaper_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE monitor_playlists
         SET current_wallpaper_id = NULL,
             last_error = 'El wallpaper actual fue eliminado de la biblioteca'
         WHERE current_wallpaper_id = ?1",
        [wallpaper_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE settings
         SET dynamic_current_wallpaper_id = NULL,
             dynamic_last_error = 'El wallpaper actual fue eliminado de la biblioteca'
         WHERE dynamic_current_wallpaper_id = ?1",
        [wallpaper_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn ensure_asset_available(path: &str) -> Result<(), String> {
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err("El recurso local del wallpaper no está disponible".into())
    }
}

#[tauri::command]
pub fn regenerate_wallpaper_thumbnail(wallpaper_id: String) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let (path, thumbnail): (String, Option<String>) = conn
        .query_row(
            "SELECT path, thumbnail FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "El wallpaper ya no existe".to_string())?;
    ensure_asset_available(&path)?;
    let thumbnail_was_missing = thumbnail.is_none();
    let thumbnail = thumbnail.unwrap_or_else(|| {
        app_data_dir()
            .map(|root| {
                root.join("wallpapers/thumbnails")
                    .join(format!("{wallpaper_id}.jpg"))
            })
            .unwrap_or_else(|_| PathBuf::from(format!("{wallpaper_id}.jpg")))
            .to_string_lossy()
            .into_owned()
    });
    generate_thumbnail(&path, &thumbnail)?;
    if let Err(error) = conn.execute(
        "UPDATE wallpapers SET thumbnail = ?1 WHERE id = ?2",
        [&thumbnail, &wallpaper_id],
    ) {
        if thumbnail_was_missing {
            let _ = remove_owned_file(&thumbnail);
        }
        return Err(format!(
            "La miniatura fue regenerada pero no se pudo registrar: {error}"
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn scan_wallpaper_folder() -> Result<(), String> {
    let import_dir = app_data_dir()
        .map_err(|e| e.to_string())?
        .join("wallpapers/import");
    fs::create_dir_all(&import_dir).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    for entry in walkdir::WalkDir::new(import_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            if let Err(error) = import_file(entry.path(), None) {
                failures.push(format!("{}: {error}", entry.path().display()));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "No se pudieron importar algunos archivos: {}",
            failures.join("; ")
        ))
    }
}

#[tauri::command]
pub fn rotate_random_wallpaper(scheduler: State<'_, DynamicScheduler>) -> Result<(), String> {
    ensure_physical_operations_allowed()?;
    let mut manual_operation = scheduler.begin_manual_operation();
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
    manual_operation.complete_and_resume();
    Ok(())
}

fn validate_wallpaper_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("El nombre del fondo no puede estar vacío".into());
    }
    if trimmed.chars().count() > 200 {
        return Err("El nombre del fondo no puede superar los 200 caracteres".into());
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

fn ensure_rotatable_resource_type(resource_type: &str) -> Result<(), String> {
    if resource_type == "static" {
        Ok(())
    } else {
        Err("La rotación local solo admite wallpapers Static; Animated no se transforma como imagen fija".into())
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
pub fn rotate_wallpaper_version(
    wallpaper_id: String,
    degrees: u16,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let mut manual_operation = scheduler.begin_manual_operation();
    if !matches!(degrees, 0 | 90 | 180 | 270) {
        return Err("La rotación debe ser 0°, 90°, 180° o 270°".into());
    }
    let mut conn = get_connection().map_err(|e| e.to_string())?;
    let (path, thumbnail, original_path, active, resource_type):
        (String, Option<String>, Option<String>, i32, String) =
        conn.query_row(
            "SELECT path, thumbnail, original_path, active, COALESCE(resource_type, 'static') FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|_| "El fondo ya no existe en la biblioteca".to_string())?;
    ensure_rotatable_resource_type(&resource_type)?;
    let path = PathBuf::from(path);
    ensure_asset_available(&path.to_string_lossy())?;
    let assigned_monitors = wallpaper_assigned_monitors(&conn, &wallpaper_id)?;
    let source_path = if let Some(ref original_path) = original_path {
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
        backup
    };
    ensure_asset_available(&source_path.to_string_lossy())?;
    let created_original = original_path.is_none();
    let rollback_path = path.with_file_name(format!(
        ".{}.lumina-rotation-rollback{}",
        wallpaper_id,
        path.extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()))
            .unwrap_or_default()
    ));
    fs::copy(&path, &rollback_path)
        .map_err(|e| format!("No se pudo preparar el rollback de la rotación: {e}"))?;
    let thumbnail_path = thumbnail.unwrap_or_else(|| {
        app_data_dir()
            .map(|root| {
                root.join("wallpapers/thumbnails")
                    .join(format!("{wallpaper_id}.jpg"))
            })
            .unwrap_or_else(|_| PathBuf::from(format!("{wallpaper_id}.jpg")))
            .to_string_lossy()
            .into_owned()
    });
    let thumbnail_path_ref = Path::new(&thumbnail_path);
    let thumbnail_rollback = thumbnail_path_ref
        .with_file_name(format!(".{}.lumina-thumbnail-rollback.jpg", wallpaper_id));
    let had_thumbnail = thumbnail_path_ref.is_file();
    if had_thumbnail {
        fs::copy(thumbnail_path_ref, &thumbnail_rollback)
            .map_err(|e| format!("No se pudo preparar el rollback de la miniatura: {e}"))?;
    }
    let operation = (|| {
        let (width, height) = replace_validated_image(&source_path, &path, degrees)?;
        generate_thumbnail(&path.to_string_lossy(), &thumbnail_path)?;
        let hash = file_hash(&path.to_string_lossy())?;
        let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE wallpapers SET original_path = ?1, thumbnail = ?2, width = ?3, height = ?4, size_bytes = ?5, hash = ?6 WHERE id = ?7",
            rusqlite::params![
                Some(source_path.to_string_lossy().to_string()),
                thumbnail_path,
                width,
                height,
                size,
                hash,
                wallpaper_id
            ],
        )
        .map_err(|e| format!("No se pudo actualizar los datos del fondo rotado: {e}"))?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })();
    if let Err(error) = operation {
        let mut rollback_errors = Vec::new();
        if let Err(restore_error) = fs::copy(&rollback_path, &path) {
            rollback_errors.push(format!("no se pudo restaurar el asset: {restore_error}"));
        }
        if had_thumbnail {
            if let Err(restore_error) = fs::copy(&thumbnail_rollback, thumbnail_path_ref) {
                rollback_errors.push(format!(
                    "no se pudo restaurar la miniatura: {restore_error}"
                ));
            }
        } else if let Err(remove_error) = remove_owned_file(&thumbnail_path) {
            rollback_errors.push(format!(
                "no se pudo retirar la miniatura nueva: {remove_error}"
            ));
        }
        if let Err(cleanup_error) = remove_owned_file(&rollback_path.to_string_lossy()) {
            rollback_errors.push(format!(
                "no se pudo limpiar el rollback del asset: {cleanup_error}"
            ));
        }
        if let Err(cleanup_error) = remove_owned_file(&thumbnail_rollback.to_string_lossy()) {
            rollback_errors.push(format!(
                "no se pudo limpiar el rollback de la miniatura: {cleanup_error}"
            ));
        }
        if created_original {
            if let Err(cleanup_error) = remove_owned_file(&source_path.to_string_lossy()) {
                rollback_errors.push(format!(
                    "no se pudo retirar el backup original creado: {cleanup_error}"
                ));
            }
        }
        return Err(if rollback_errors.is_empty() {
            format!("La rotación no se completó: {error}")
        } else {
            format!(
                "La rotación falló y el rollback quedó incompleto: {error}; {}",
                rollback_errors.join("; ")
            )
        });
    }
    let mut post_transform_errors = Vec::new();
    if let Err(cleanup_error) = remove_owned_file(&rollback_path.to_string_lossy()) {
        post_transform_errors.push(format!(
            "no se pudo limpiar el rollback del asset: {cleanup_error}"
        ));
    }
    if let Err(cleanup_error) = remove_owned_file(&thumbnail_rollback.to_string_lossy()) {
        post_transform_errors.push(format!(
            "no se pudo limpiar el rollback de la miniatura: {cleanup_error}"
        ));
    }

    if assigned_monitors.is_empty() && active == 1 {
        if let Err(error) = set_active_wallpaper_internal(wallpaper_id.clone()) {
            post_transform_errors.push(format!(
                "el wallpaper fue transformado y persistido, pero no se pudo reaplicar globalmente: {error}"
            ));
        }
    } else {
        for monitor_id in &assigned_monitors {
            if let Err(error) =
                set_active_wallpaper_on_monitor(&wallpaper_id, monitor_id, &path.to_string_lossy())
            {
                post_transform_errors.push(format!(
                    "el wallpaper fue transformado y persistido, pero no se pudo reaplicar en {monitor_id}: {error}"
                ));
            }
        }
    }
    if !post_transform_errors.is_empty() {
        return Err(format!(
            "La transformación se completó, pero quedaron resultados parciales: {}",
            post_transform_errors.join("; ")
        ));
    }
    manual_operation.complete_and_resume();
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

#[derive(Debug)]
struct RealEsrganResources {
    directory: PathBuf,
    executable: PathBuf,
}

fn resolve_realesrgan_resources(
    app: &tauri::AppHandle,
    model: &str,
) -> Result<RealEsrganResources, String> {
    let directory = realesrgan_dir(app)?;
    validate_realesrgan_resources(&directory, model)?;
    let executable = directory.join("realesrgan-ncnn-vulkan.exe");
    Ok(RealEsrganResources {
        directory,
        executable,
    })
}

fn validate_realesrgan_resources(directory: &Path, model: &str) -> Result<(), String> {
    let executable = directory.join("realesrgan-ncnn-vulkan.exe");
    if !executable.is_file() {
        return Err(format!(
            "No se encontrÃ³ el ejecutable Real-ESRGAN en {}",
            executable.display()
        ));
    }
    let dll = directory.join("vcomp140.dll");
    if !dll.is_file() {
        return Err(format!(
            "Falta la DLL requerida de Real-ESRGAN: {}",
            dll.display()
        ));
    }
    let model_directory = directory.join("models");
    for extension in ["bin", "param"] {
        let path = model_directory.join(format!("{model}.{extension}"));
        if !path.is_file() {
            return Err(format!(
                "Falta el modelo Real-ESRGAN requerido: {}",
                path.display()
            ));
        }
    }
    Ok(())
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
    if size > 512 * 1024 * 1024 {
        return Err("La salida mejorada supera el límite local de 512 MB".into());
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
    pub job_id: String,
    pub state: EnhancementJobState,
    pub id: String,
    pub method: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnhancementJobState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

static ACTIVE_ENHANCEMENTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn active_enhancements() -> &'static Mutex<HashSet<String>> {
    ACTIVE_ENHANCEMENTS.get_or_init(|| Mutex::new(HashSet::new()))
}

struct EnhancementJobGuard {
    source_id: String,
    job_id: String,
    state: EnhancementJobState,
}

impl EnhancementJobGuard {
    fn begin(source_id: &str) -> Result<Self, String> {
        let jobs = active_enhancements();
        let mut active = jobs
            .lock()
            .map_err(|_| "No se pudo registrar el trabajo de mejora".to_string())?;
        if !active.insert(source_id.to_owned()) {
            return Err("Ya hay una mejora en curso para este wallpaper".into());
        }
        Ok(Self {
            source_id: source_id.to_owned(),
            job_id: Uuid::new_v4().to_string(),
            state: EnhancementJobState::Pending,
        })
    }

    fn running(&mut self) {
        self.state = EnhancementJobState::Running;
    }

    fn complete(&mut self) {
        self.state = EnhancementJobState::Completed;
    }
}

impl Drop for EnhancementJobGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = active_enhancements().lock() {
            active.remove(&self.source_id);
        }
    }
}

struct TemporaryOutput {
    path: PathBuf,
    cleaned: bool,
}

impl TemporaryOutput {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            cleaned: false,
        }
    }

    fn cleanup(&mut self) -> Result<(), String> {
        self.cleanup_with(|path| fs::remove_file(path))
    }

    fn cleanup_with<F>(&mut self, remove: F) -> Result<(), String>
    where
        F: FnOnce(&Path) -> std::io::Result<()>,
    {
        if self.cleaned {
            return Ok(());
        }
        match remove(&self.path) {
            Ok(()) => {
                self.cleaned = true;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.cleaned = true;
                Ok(())
            }
            Err(error) => Err(format!(
                "No se pudo limpiar el temporal de mejora {}: {error}",
                self.path.display()
            )),
        }
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.cleaned {
            if let Err(error) = self.cleanup() {
                eprintln!("{error}");
            }
        }
    }
}

fn combine_operation_and_cleanup<T>(
    operation: Result<T, String>,
    cleanup: Result<(), String>,
) -> Result<T, String> {
    match (operation, cleanup) {
        (Ok(result), Ok(())) => Ok(result),
        (Ok(_), Err(cleanup_error)) => Err(format!(
            "La mejora se completÃ³ y se persistiÃ³, pero la limpieza del temporal fallÃ³: {cleanup_error}"
        )),
        (Err(operation_error), Ok(())) => Err(operation_error),
        (Err(operation_error), Err(cleanup_error)) => Err(format!(
            "{operation_error}; ademÃ¡s, la limpieza del temporal fallÃ³: {cleanup_error}"
        )),
    }
}

const REAL_ESRGAN_TIMEOUT: Duration = Duration::from_secs(30 * 60);

fn run_realesrgan(
    executable: &Path,
    tool_dir: &Path,
    source_path: &str,
    output_path: &str,
    model: &str,
    scale: &str,
) -> Result<Output, String> {
    let mut child = Command::new(executable)
        .current_dir(tool_dir)
        .args([
            "-i",
            source_path,
            "-o",
            output_path,
            "-n",
            model,
            "-s",
            scale,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No se pudo iniciar Real-ESRGAN: {e}"))?;
    let deadline = Instant::now() + REAL_ESRGAN_TIMEOUT;
    loop {
        if child
            .try_wait()
            .map_err(|e| format!("No se pudo consultar Real-ESRGAN: {e}"))?
            .is_some()
        {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Real-ESRGAN superó el tiempo máximo de 30 minutos".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    child
        .wait_with_output()
        .map_err(|e| format!("No se pudo recoger la salida de Real-ESRGAN: {e}"))
}

fn realesrgan_process_failure(output: &Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        "Real-ESRGAN terminÃ³ con error de ejecuciÃ³n".into()
    } else {
        format!("Real-ESRGAN terminÃ³ con error de ejecuciÃ³n: {detail}")
    }
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
    let (source_path, original_name, source_id, resource_type): (String, String, String, String) = conn
        .query_row(
            "SELECT path, name, id, COALESCE(resource_type, 'static') FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| "No se encontró el fondo".to_string())?;
    if resource_type != "static" {
        return Err("La mejora local solo admite wallpapers Static; Animated no se convierte en una imagen fija".into());
    }
    let mut job = EnhancementJobGuard::begin(&source_id)?;
    let hardware_acceleration = conn
        .query_row(
            "SELECT hardware_acceleration FROM settings LIMIT 1",
            [],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(1)
        == 1;
    drop(conn);

    ensure_asset_available(&source_path)?;
    let source_hash = file_hash(&source_path)?;

    let output_path = app_data_dir()
        .map_err(|e| e.to_string())?
        .join("wallpapers/enhanced")
        .join(format!("{}-{scale}x.png", Uuid::new_v4()));
    let mut temporary_output = TemporaryOutput::new(output_path.clone());
    let operation = (|| -> Result<UpscaleResult, String> {
        let output_string = output_path.to_string_lossy().into_owned();
        let scale_string = scale.to_string();
        let source = image::open(&source_path)
            .map_err(|e| format!("No se pudo abrir la imagen para mejorarla: {e}"))?;
        let expected_dimensions = target_dimensions(source.width(), source.height(), scale)?;
        job.running();
        let mut used_ai = false;
        let mut warning = None;

        if hardware_acceleration {
            if let Ok(resources) = resolve_realesrgan_resources(&app, model) {
                let tool_dir = resources.directory;
                let executable = resources.executable;
                if executable.is_file() {
                    let result = run_realesrgan(
                        &executable,
                        &tool_dir,
                        &source_path,
                        &output_string,
                        model,
                        &scale_string,
                    )?;
                    if !result.status.success() {
                        return Err(realesrgan_process_failure(&result));
                    }
                    let valid_output = validate_generated_image(&output_path, expected_dimensions)
                        .is_ok()
                        && validate_ai_pixels(&output_path, expected_dimensions, &source).is_ok();
                    if valid_output {
                        used_ai = true;
                    } else {
                        warning = Some("Real-ESRGAN generÃ³ una salida invÃ¡lida; se usÃ³ el respaldo Lanczos.".into());
                    }
                } else {
                    warning = Some(
                        "No se encontró el ejecutable Real-ESRGAN; se usó el respaldo Lanczos."
                            .into(),
                    );
                }
            } else {
                warning = Some(
                    "No se encontraron los recursos de Real-ESRGAN; se usó el respaldo Lanczos."
                        .into(),
                );
            }
        } else {
            warning = Some(
                "La aceleración por hardware está desactivada; se usó el respaldo Lanczos.".into(),
            );
        }

        if hardware_acceleration && !used_ai {
            if let Err(reason) = resolve_realesrgan_resources(&app, model) {
                warning = Some(format!("{reason}; se usÃ³ el respaldo Lanczos."));
            }
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

        let current_hash = file_hash(&source_path)?;
        if current_hash != source_hash {
            return Err(
                "El wallpaper de origen cambió durante la mejora; no se registró el resultado"
                    .into(),
            );
        }

        let stem = Path::new(&original_name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let method = if used_ai { "IA" } else { "Lanczos" };
        let new_id = match import_file_with_parent(
            &output_path,
            Some(format!("{stem} · mejorado {scale}x ({method})")),
            Some(&source_id),
        ) {
            Ok(id) => id,
            Err(error) => return Err(error),
        };
        let _ = fs::remove_file(&output_path);
        job.complete();
        Ok(UpscaleResult {
            job_id: job.job_id.clone(),
            state: job.state.clone(),
            id: new_id,
            method: method.into(),
            warning,
        })
    })();
    let cleanup = temporary_output.cleanup();
    combine_operation_and_cleanup(operation, cleanup)
}

#[cfg(test)]
mod tests {
    use super::{
        clear_wallpaper_references, combine_operation_and_cleanup, compensate_import_failure,
        ensure_asset_available, ensure_rotatable_resource_type, persist_active_wallpaper,
        persist_monitor_wallpaper_assignment, read_scaling_mode, realesrgan_process_failure,
        remove_owned_file, rename_wallpaper_in_connection, replace_validated_image, rotate_image,
        target_dimensions, validate_ai_pixels, validate_generated_image, validate_imported_copy,
        validate_realesrgan_resources, validate_wallpaper_name, wallpaper_assigned_monitors,
        EnhancementJobGuard, EnhancementJobState, TemporaryOutput,
    };
    use crate::models::wallpaper::WallpaperResourceType;
    use crate::utils::hash::file_hash;
    use crate::utils::thumbnail::generate_thumbnail;
    use image::{GenericImageView, RgbImage};
    use rusqlite::Connection;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    use uuid::Uuid;

    #[test]
    fn scaling_mode_reads_valid_configuration() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE settings(scaling_mode TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO settings(scaling_mode) VALUES ('fit')", [])
            .unwrap();

        assert!(matches!(
            read_scaling_mode(&conn).unwrap(),
            wallpaper::Mode::Fit
        ));
    }

    #[test]
    fn scaling_mode_keeps_existing_policy_for_invalid_values() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE settings(scaling_mode TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO settings(scaling_mode) VALUES ('invalid')", [])
            .unwrap();

        assert!(matches!(
            read_scaling_mode(&conn).unwrap(),
            wallpaper::Mode::Crop
        ));
    }

    #[test]
    fn scaling_mode_does_not_turn_a_read_failure_into_fill() {
        let conn = Connection::open_in_memory().unwrap();
        let error = read_scaling_mode(&conn).unwrap_err();
        assert!(error.contains("scaling_mode"));
    }

    #[test]
    fn persists_active_wallpaper_atomically_in_sqlite() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE wallpapers(id TEXT PRIMARY KEY, active INTEGER NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO wallpapers(id, active) VALUES ('old', 1), ('new', 0)",
            [],
        )
        .unwrap();

        persist_active_wallpaper(&mut conn, "new").unwrap();

        let active: Vec<String> = conn
            .prepare("SELECT id FROM wallpapers WHERE active = 1 ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(active, vec!["new"]);
    }

    #[test]
    fn persistence_failure_is_returned_without_faking_success() {
        let mut conn = Connection::open_in_memory().unwrap();
        let error = persist_active_wallpaper(&mut conn, "new").unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn monitor_assignment_persistence_does_not_change_global_active_state() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE wallpapers(id TEXT PRIMARY KEY, active INTEGER NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE monitor_wallpapers(monitor_id TEXT PRIMARY KEY, wallpaper_id TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO wallpapers(id, active) VALUES ('global', 1), ('animated', 0)",
            [],
        )
        .unwrap();

        persist_monitor_wallpaper_assignment(&mut conn, "DISPLAY-A", "animated").unwrap();

        let active: String = conn
            .query_row("SELECT id FROM wallpapers WHERE active = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let assigned: String = conn
            .query_row(
                "SELECT wallpaper_id FROM monitor_wallpapers WHERE monitor_id = 'DISPLAY-A'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, "global");
        assert_eq!(assigned, "animated");
    }
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
        image::RgbaImage::from_pixel(8, 6, image::Rgba([0, 0, 0, 255]))
            .save(&output_path)
            .unwrap();

        assert!(
            validate_ai_pixels(&output_path, (8, 6), &image::open(&source_path).unwrap()).is_err()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ai_pixel_validation_distinguishes_transparency_and_valid_low_contrast_output() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-ai-pixel-variants-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        let visible_source = RgbImage::from_fn(4, 3, |x, _| {
            if x < 2 {
                image::Rgb([220, 80, 40])
            } else {
                image::Rgb([20, 140, 220])
            }
        });
        let visible_source_path = dir.join("visible-source.png");
        visible_source.save(&visible_source_path).unwrap();
        let visible_source = image::open(&visible_source_path).unwrap();

        let transparent_output_path = dir.join("transparent-output.png");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([0, 0, 0, 0]))
            .save(&transparent_output_path)
            .unwrap();
        assert!(validate_ai_pixels(&transparent_output_path, (8, 6), &visible_source).is_err());

        let alpha_black_output_path = dir.join("alpha-black-output.png");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([0, 0, 0, 128]))
            .save(&alpha_black_output_path)
            .unwrap();
        assert!(validate_ai_pixels(&alpha_black_output_path, (8, 6), &visible_source).is_err());

        let low_contrast_output_path = dir.join("low-contrast-output.png");
        image::RgbaImage::from_fn(8, 6, |x, _| {
            let red = if x % 2 == 0 { 10 } else { 11 };
            image::Rgba([red, 10, 10, 255])
        })
        .save(&low_contrast_output_path)
        .unwrap();
        assert!(validate_ai_pixels(&low_contrast_output_path, (8, 6), &visible_source).is_ok());

        let transparent_source_path = dir.join("transparent-source.png");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([0, 0, 0, 0]))
            .save(&transparent_source_path)
            .unwrap();
        let transparent_source = image::open(&transparent_source_path).unwrap();
        let transparent_result_path = dir.join("transparent-result.png");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([0, 0, 0, 255]))
            .save(&transparent_result_path)
            .unwrap();
        assert!(validate_ai_pixels(&transparent_result_path, (8, 6), &transparent_source).is_ok());

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
        assert!(validate_wallpaper_name(&"x".repeat(201)).is_err());
    }

    #[test]
    fn rotation_rejects_animated_resources_explicitly() {
        assert!(ensure_rotatable_resource_type("static").is_ok());
        let error = ensure_rotatable_resource_type("animated").unwrap_err();
        assert!(error.contains("Animated"));
    }

    #[test]
    fn reads_all_monitor_assignments_by_stable_monitor_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE monitor_wallpapers(monitor_id TEXT PRIMARY KEY, wallpaper_id TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES
             ('DISPLAY\\\\B', 'wallpaper-1'), ('DISPLAY\\\\A', 'wallpaper-1'), ('DISPLAY\\\\C', 'other')",
            [],
        )
        .unwrap();
        assert_eq!(
            wallpaper_assigned_monitors(&conn, "wallpaper-1").unwrap(),
            vec!["DISPLAY\\\\A", "DISPLAY\\\\B"]
        );
    }

    #[test]
    fn clearing_wallpaper_references_is_atomic_and_keeps_playlists() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE playlist_wallpapers(playlist_id TEXT, wallpaper_id TEXT);
             CREATE TABLE monitor_playlists(monitor_id TEXT PRIMARY KEY, current_wallpaper_id TEXT, last_error TEXT);
             CREATE TABLE settings(dynamic_current_wallpaper_id TEXT, dynamic_last_error TEXT);
             INSERT INTO playlist_wallpapers VALUES ('playlist-a', 'wallpaper-1'), ('playlist-b', 'wallpaper-1');
             INSERT INTO monitor_playlists VALUES ('monitor-a', 'wallpaper-1', NULL);
             INSERT INTO settings VALUES ('wallpaper-1', NULL);",
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        clear_wallpaper_references(&tx, "wallpaper-1").unwrap();
        tx.commit().unwrap();

        let playlist_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM playlist_wallpapers WHERE wallpaper_id = 'wallpaper-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let current: Option<String> = conn
            .query_row(
                "SELECT current_wallpaper_id FROM monitor_playlists WHERE monitor_id = 'monitor-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let dynamic_current: Option<String> = conn
            .query_row(
                "SELECT dynamic_current_wallpaper_id FROM settings",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(playlist_count, 0);
        assert_eq!(current, None);
        assert_eq!(dynamic_current, None);
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
        let mut fixture = RgbImage::new(2, 1);
        fixture.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        fixture.put_pixel(1, 0, image::Rgb([0, 255, 0]));
        fixture.save(&source).unwrap();
        fs::copy(&source, &destination).unwrap();

        let original_hash = file_hash(&source.to_string_lossy()).unwrap();
        let original_bytes = fs::read(&source).unwrap();

        let rotated_90 = rotate_image(image::open(&source).unwrap(), 90).unwrap();
        assert_eq!(rotated_90.dimensions(), (1, 2));
        assert_eq!(rotated_90.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(rotated_90.get_pixel(0, 1).0, [0, 255, 0, 255]);

        let rotated_180 = rotate_image(image::open(&source).unwrap(), 180).unwrap();
        assert_eq!(rotated_180.dimensions(), (2, 1));
        assert_eq!(rotated_180.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(rotated_180.get_pixel(1, 0).0, [255, 0, 0, 255]);

        let rotated_270 = rotate_image(image::open(&source).unwrap(), 270).unwrap();
        assert_eq!(rotated_270.dimensions(), (1, 2));
        assert_eq!(rotated_270.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(rotated_270.get_pixel(0, 1).0, [255, 0, 0, 255]);

        assert_eq!(
            rotate_image(image::DynamicImage::ImageRgb8(RgbImage::new(4, 4)), 90)
                .unwrap()
                .dimensions(),
            (4, 4)
        );

        assert_eq!(
            replace_validated_image(&source, &destination, 90).unwrap(),
            (1, 2)
        );
        let rotated_file = image::open(&destination).unwrap();
        assert_eq!(rotated_file.dimensions(), (1, 2));
        assert_eq!(rotated_file.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(rotated_file.get_pixel(0, 1).0, [0, 255, 0, 255]);
        assert_ne!(
            file_hash(&destination.to_string_lossy()).unwrap(),
            original_hash
        );
        generate_thumbnail(&destination.to_string_lossy(), &thumbnail.to_string_lossy()).unwrap();
        assert!(image::open(&thumbnail).is_ok());
        assert_eq!(
            replace_validated_image(&source, &destination, 0).unwrap(),
            (2, 1)
        );
        assert_eq!(fs::read(&destination).unwrap(), original_bytes);
        assert_eq!(
            file_hash(&destination.to_string_lossy()).unwrap(),
            original_hash
        );
        assert!(replace_validated_image(&dir.join("missing.png"), &destination, 90).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_owned_file_is_already_clean_but_cleanup_errors_are_reported() {
        let missing = std::env::temp_dir().join("lumina-missing-owned-file.png");
        assert!(remove_owned_file(&missing.to_string_lossy()).is_ok());

        let directory = std::env::temp_dir().join(format!(
            "lumina-owned-directory-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        assert!(remove_owned_file(&directory.to_string_lossy()).is_err());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn missing_asset_has_a_distinguishable_availability_error() {
        let missing = std::env::temp_dir().join(format!(
            "lumina-missing-asset-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let error = ensure_asset_available(&missing.to_string_lossy()).unwrap_err();
        assert!(error.contains("no está disponible"));
    }

    #[test]
    fn imported_copy_is_validated_by_hash_and_dimensions() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-import-copy-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("copy.png");
        RgbImage::new(8, 6).save(&path).unwrap();
        let hash = crate::utils::hash::file_hash(&path.to_string_lossy()).unwrap();
        assert!(validate_imported_copy(&path, &hash, (8, 6)).is_ok());
        assert!(validate_imported_copy(&path, &hash, (6, 8)).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn import_compensation_reports_cleanup_failures() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-import-compensation-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let directory = dir.join("not-a-file");
        fs::create_dir_all(&directory).unwrap();
        let error = compensate_import_failure("metadata failed", [&directory]);
        assert!(error.contains("metadata failed"));
        assert!(error.contains("compensación física"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resource_type_contract_uses_stable_serializable_values() {
        assert_eq!(WallpaperResourceType::Static.as_str(), "static");
        assert_eq!(
            WallpaperResourceType::from_db("animated").unwrap(),
            WallpaperResourceType::Animated
        );
        assert!(WallpaperResourceType::from_db("video").is_err());
    }

    #[test]
    fn enhancement_jobs_reject_duplicate_sources_and_release_after_drop() {
        let first = EnhancementJobGuard::begin("source").unwrap();
        assert!(EnhancementJobGuard::begin("source").is_err());
        drop(first);
        let mut next = EnhancementJobGuard::begin("source").unwrap();
        assert!(matches!(next.state, EnhancementJobState::Pending));
        next.running();
        assert!(matches!(next.state, EnhancementJobState::Running));
    }

    #[test]
    fn enhancement_temporary_is_removed_when_scope_ends() {
        let path = std::env::temp_dir().join(format!(
            "lumina-enhancement-temp-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"temporary").unwrap();
        {
            let _temporary = TemporaryOutput::new(path.clone());
        }
        assert!(!path.exists());
    }

    #[test]
    fn validates_all_realesrgan_resources_before_starting_process() {
        let directory =
            std::env::temp_dir().join(format!("lumina-realesrgan-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(directory.join("models")).unwrap();

        let missing_executable =
            validate_realesrgan_resources(&directory, "realesrgan-x4plus").unwrap_err();
        assert!(missing_executable.contains("ejecutable"));

        fs::write(directory.join("realesrgan-ncnn-vulkan.exe"), b"test").unwrap();
        let missing_dll =
            validate_realesrgan_resources(&directory, "realesrgan-x4plus").unwrap_err();
        assert!(missing_dll.contains("DLL"));

        fs::write(directory.join("vcomp140.dll"), b"test").unwrap();
        let missing_model =
            validate_realesrgan_resources(&directory, "realesrgan-x4plus").unwrap_err();
        assert!(missing_model.contains("modelo"));

        fs::write(directory.join("models/realesrgan-x4plus.bin"), b"test").unwrap();
        fs::write(directory.join("models/realesrgan-x4plus.param"), b"test").unwrap();
        assert!(validate_realesrgan_resources(&directory, "realesrgan-x4plus").is_ok());
        assert!(validate_realesrgan_resources(&directory, "realesr-animevideov3-x3").is_err());

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn explicit_temporary_cleanup_reports_filesystem_errors() {
        let path = std::env::temp_dir().join(format!("lumina-cleanup-test-{}", Uuid::new_v4()));
        let mut temporary = TemporaryOutput::new(path);
        let error = temporary
            .cleanup_with(|_| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)))
            .unwrap_err();
        assert!(error.contains("limpiar el temporal"));
    }

    #[test]
    fn operation_and_cleanup_errors_are_both_preserved() {
        let error = combine_operation_and_cleanup::<()>(
            Err("fallo de procesamiento".into()),
            Err("fallo de limpieza".into()),
        )
        .unwrap_err();
        assert!(error.contains("fallo de procesamiento"));
        assert!(error.contains("fallo de limpieza"));

        let error = combine_operation_and_cleanup::<()>(Ok(()), Err("fallo de limpieza".into()))
            .unwrap_err();
        assert!(error.contains("persistiÃ³"));
    }

    #[cfg(windows)]
    #[test]
    fn failed_external_process_is_not_converted_to_lanczos_success() {
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "exit", "7"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(realesrgan_process_failure(&output).contains("error de ejecuciÃ³n"));
    }
}
