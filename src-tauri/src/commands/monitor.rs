use crate::{
    db::get_connection, models::monitor::MonitorInfo,
    utils::windows_wallpaper::set_monitor_wallpaper,
};
use tauri::Manager;

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
    Ok(available
        .into_iter()
        .map(|monitor| {
            let name = monitor.name().cloned().unwrap_or_else(|| "Pantalla".into());
            let refresh_rate = refresh_rate_for(&name);
            MonitorInfo {
                id: name.clone(),
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

#[tauri::command]
pub fn assign_wallpaper_to_monitor(monitor_id: String, wallpaper_id: String) -> Result<(), String> {
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
    Ok(())
}
