use crate::{
    db::get_connection,
    dynamic::DynamicScheduler,
    models::monitor::MonitorInfo,
    models::monitor_assignment::{
        resolve_monitor_identities, MonitorGeometry, MonitorIdentity, MonitorResolution,
        UiMonitorDescriptor, WallpaperBackend,
    },
    models::monitor_playlist::MonitorPlaylist,
    models::wallpaper_coordinator,
    utils::windows_wallpaper::{native_monitor_descriptors, set_monitor_wallpaper},
};
use tauri::Manager;
use tauri::State;

pub(crate) fn current_monitor_ids(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal")?;
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    resolve_monitor_ids(&monitors)
}

fn resolve_monitor_ids(monitors: &[tauri::Monitor]) -> Result<Vec<String>, String> {
    let ui = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            Ok(UiMonitorDescriptor {
                name: monitor.name().cloned(),
                geometry: MonitorGeometry::new(position.x, position.y, size.width, size.height)
                    .map_err(str::to_string)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    #[cfg(target_os = "windows")]
    {
        let native = native_monitor_descriptors().map_err(|error| error.to_string())?;
        let resolutions = resolve_monitor_identities(&ui, &native).map_err(str::to_string)?;
        return resolutions
            .into_iter()
            .enumerate()
            .map(|(index, resolution)| match resolution {
                MonitorResolution::Resolved(identity) => Ok(identity.as_str().to_string()),
                MonitorResolution::Ambiguous => {
                    Err(format!("La identidad del monitor {index} es ambigua"))
                }
                MonitorResolution::Unavailable => Err(format!(
                    "La identidad nativa del monitor {index} no está disponible"
                )),
            })
            .collect();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = ui;
        Err("La resolución nativa de monitores solo está disponible en Windows".into())
    }
}

fn restorable_assignments(
    current_ids: &[String],
    assignments: &[(String, String)],
) -> Vec<(String, String)> {
    assignments
        .iter()
        .filter(|(monitor_id, _)| current_ids.iter().any(|id| id == monitor_id))
        .cloned()
        .collect()
}

#[cfg(target_os = "windows")]
fn refresh_rate_for(display_name: &str) -> u32 {
    use std::mem::size_of;
    use windows::{
        core::PCWSTR,
        Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS},
    };

    let wide_name: Vec<u16> = display_name.encode_utf16().chain(Some(0)).collect();
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let found = unsafe {
        EnumDisplaySettingsW(PCWSTR(wide_name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode).as_bool()
    };
    if found && mode.dmDisplayFrequency > 1 {
        mode.dmDisplayFrequency
    } else {
        0
    }
}

#[cfg(not(target_os = "windows"))]
fn refresh_rate_for(_display_name: &str) -> u32 {
    0
}

#[tauri::command]
pub fn get_monitors(app: tauri::AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal")?;
    let available = window.available_monitors().map_err(|e| e.to_string())?;
    let primary = window.primary_monitor().map_err(|e| e.to_string())?;
    let ids = resolve_monitor_ids(&available)?;
    Ok(available
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let id = ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("monitor-{index}"));
            let name = monitor.name().cloned().unwrap_or_else(|| id.clone());
            let refresh_rate = refresh_rate_for(&name);
            MonitorInfo {
                id,
                name,
                width: monitor.size().width,
                height: monitor.size().height,
                refresh_rate,
                primary: primary
                    .as_ref()
                    .map(|item| item.name() == monitor.name())
                    .unwrap_or(false),
            }
        })
        .collect())
}

pub fn restore_monitor_wallpapers(app: &tauri::AppHandle) -> Result<(), String> {
    let ids = current_monitor_ids(app)?;
    if ids.is_empty() {
        return Ok(());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    let assignments: Vec<(String, String)> = {
        let mut statement = conn
            .prepare("SELECT monitor_id, wallpaper_id FROM monitor_wallpapers")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        rows
    };
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
    let assignments = restorable_assignments(&ids, &assignments);
    if assignments.is_empty() {
        return Ok(());
    }
    wallpaper::set_mode(mode)
        .map_err(|e| format!("Windows no pudo restaurar el modo de ajuste: {e}"))?;
    let mut restore_errors = Vec::new();
    for (monitor_id, wallpaper_id) in assignments {
        let Ok(path) = conn.query_row(
            "SELECT path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| row.get::<_, String>(0),
        ) else {
            restore_errors.push(format!(
                "{monitor_id}: no se encontró el wallpaper persistido {wallpaper_id}"
            ));
            continue;
        };
        if std::path::Path::new(&path).is_file() {
            let identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
            let operation = wallpaper_coordinator::global()
                .lock()
                .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?
                .reserve(identity, WallpaperBackend::Static)
                .map_err(|error| error.to_string())?;
            if let Err(error) = set_monitor_wallpaper(&monitor_id, &path) {
                if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
                    coordinator.abort(&operation);
                }
                restore_errors.push(format!(
                    "{monitor_id}: Windows no pudo restaurar el fondo: {error}"
                ));
                continue;
            }
            let mut coordinator = wallpaper_coordinator::global()
                .lock()
                .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
            let coordinator_result = (|| {
                coordinator.confirm_previous_deactivated(&operation)?;
                coordinator.confirm_activated(&operation)?;
                coordinator.commit(&operation)
            })();
            if let Err(error) = coordinator_result {
                coordinator.abort(&operation);
                restore_errors.push(format!(
                    "{monitor_id}: restaurado físicamente, pero no confirmado en el coordinador: {error}"
                ));
            }
        } else {
            restore_errors.push(format!(
                "{monitor_id}: el archivo persistido no está disponible"
            ));
        }
    }
    if restore_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "No se pudieron restaurar todos los monitores: {}",
            restore_errors.join("; ")
        ))
    }
}

#[tauri::command]
pub fn assign_wallpaper_to_monitor(
    app: tauri::AppHandle,
    monitor_id: String,
    wallpaper_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    if !current_monitor_ids(&app)?
        .iter()
        .any(|id| id == &monitor_id)
    {
        return Err("La pantalla seleccionada ya no está disponible".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| row.get(0),
        )
        .map_err(|_| "No se encontró el fondo seleccionado".to_string())?;
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
    let identity = MonitorIdentity::new(monitor_id.clone()).map_err(str::to_string)?;
    let operation = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?
        .reserve(identity, WallpaperBackend::Static)
        .map_err(|error| error.to_string())?;
    if let Err(error) = set_monitor_wallpaper(&monitor_id, &path) {
        if let Ok(mut coordinator) = wallpaper_coordinator::global().lock() {
            coordinator.abort(&operation);
        }
        return Err(format!(
            "Windows no pudo aplicar el fondo al monitor: {error}"
        ));
    }
    let mut coordinator = wallpaper_coordinator::global()
        .lock()
        .map_err(|_| "No se pudo bloquear el coordinador de wallpapers".to_string())?;
    if let Err(error) = coordinator.confirm_previous_deactivated(&operation) {
        coordinator.abort(&operation);
        return Err(error.to_string());
    }
    coordinator
        .confirm_activated(&operation)
        .map_err(|error| error.to_string())?;
    coordinator
        .commit(&operation)
        .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES(?1, ?2)",
        (&monitor_id, &wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM monitor_playlists WHERE monitor_id = ?1",
        [&monitor_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE settings SET dynamic_enabled = 0, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        [],
    )
    .map_err(|e| e.to_string())?;
    scheduler.stop();
    Ok(())
}

#[tauri::command]
pub fn get_monitor_playlists() -> Result<Vec<MonitorPlaylist>, String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT monitor_id, playlist_id, current_wallpaper_id, last_error
             FROM monitor_playlists ORDER BY monitor_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(MonitorPlaylist {
                monitor_id: row.get(0)?,
                playlist_id: row.get(1)?,
                current_wallpaper_id: row.get(2)?,
                last_error: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn assign_playlist_to_monitor(
    app: tauri::AppHandle,
    monitor_id: String,
    playlist_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    if !current_monitor_ids(&app)?
        .iter()
        .any(|id| id == &monitor_id)
    {
        return Err("La pantalla seleccionada ya no está disponible".into());
    }
    let conn = get_connection().map_err(|e| e.to_string())?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
            [&playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err("La playlist seleccionada ya no existe".into());
    }
    conn.execute(
        "INSERT INTO monitor_playlists(monitor_id, playlist_id, current_wallpaper_id, last_error)
         VALUES(?1, ?2, NULL, NULL)
         ON CONFLICT(monitor_id) DO UPDATE SET
           playlist_id = excluded.playlist_id,
           current_wallpaper_id = CASE
             WHEN monitor_playlists.playlist_id = excluded.playlist_id
             THEN monitor_playlists.current_wallpaper_id ELSE NULL END,
           last_error = NULL",
        (&monitor_id, &playlist_id),
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE settings SET dynamic_enabled = 0, dynamic_last_error = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        [],
    )
    .map_err(|e| e.to_string())?;
    scheduler.restart();
    Ok(())
}

#[tauri::command]
pub fn remove_playlist_from_monitor(
    monitor_id: String,
    scheduler: State<'_, DynamicScheduler>,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM monitor_playlists WHERE monitor_id = ?1",
        [&monitor_id],
    )
    .map_err(|e| e.to_string())?;
    scheduler.restart();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::restorable_assignments;

    #[test]
    fn restores_only_assignments_for_current_monitors() {
        let current = vec!["\\\\.\\DISPLAY1".to_string(), "\\\\.\\DISPLAY2".to_string()];
        let assignments = vec![
            ("\\\\.\\DISPLAY1".to_string(), "wallpaper-a".to_string()),
            ("\\\\.\\DISPLAY2".to_string(), "wallpaper-b".to_string()),
            ("\\\\.\\DISPLAY9".to_string(), "stale".to_string()),
        ];
        assert_eq!(restorable_assignments(&current, &assignments).len(), 2);
    }

    #[test]
    fn does_not_merge_different_monitor_assignments() {
        let current = vec!["monitor-0".to_string(), "monitor-1".to_string()];
        let assignments = vec![
            ("monitor-0".to_string(), "same-name-a".to_string()),
            ("monitor-1".to_string(), "same-name-b".to_string()),
        ];
        assert_eq!(restorable_assignments(&current, &assignments), assignments);
    }
}
