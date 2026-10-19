use crate::{
    db::get_connection,
    dynamic::DynamicScheduler,
    models::monitor::MonitorInfo,
    utils::windows_wallpaper::{available_monitor_ids, set_monitor_wallpaper},
};
use tauri::Manager;
use tauri::State;

fn current_monitor_ids(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("No se encontró la ventana principal")?;
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    let native_ids = available_monitor_ids()
        .ok()
        .filter(|ids| ids.len() == monitors.len());
    Ok(monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            native_ids
                .as_ref()
                .and_then(|ids| ids.get(index).cloned())
                .or_else(|| {
                    monitor
                        .name()
                        .cloned()
                        .filter(|name| !name.trim().is_empty())
                })
                .unwrap_or_else(|| format!("monitor-{index}"))
        })
        .collect())
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
    let native_ids = available_monitor_ids()
        .ok()
        .filter(|ids| ids.len() == available.len());
    let ids: Vec<String> = available
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            native_ids
                .as_ref()
                .and_then(|ids| ids.get(index).cloned())
                .or_else(|| {
                    monitor
                        .name()
                        .cloned()
                        .filter(|name| !name.trim().is_empty())
                })
                .unwrap_or_else(|| format!("monitor-{index}"))
        })
        .collect();
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
    for (monitor_id, wallpaper_id) in assignments {
        let Ok(path) = conn.query_row(
            "SELECT path FROM wallpapers WHERE id = ?1",
            [&wallpaper_id],
            |row| row.get::<_, String>(0),
        ) else {
            continue;
        };
        if std::path::Path::new(&path).is_file() {
            set_monitor_wallpaper(&monitor_id, &path)
                .map_err(|e| format!("Windows no pudo restaurar el fondo del monitor: {e}"))?;
        }
    }
    Ok(())
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
    set_monitor_wallpaper(&monitor_id, &path)
        .map_err(|e| format!("Windows no pudo aplicar el fondo al monitor: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES(?1, ?2)",
        (&monitor_id, &wallpaper_id),
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
