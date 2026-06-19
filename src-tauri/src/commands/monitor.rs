use serde::Serialize;
use crate::db::get_connection;
use crate::models::monitor::MonitorInfo;
use tauri::Manager;

#[tauri::command]
pub fn get_monitors(
    app: tauri::AppHandle,
) -> Result<Vec<MonitorInfo>, String> {
    let mut monitors = vec![];
    let window = app.get_webview_window("main").ok_or("Window not found")?;
    let available = window.available_monitors().map_err(|e| e.to_string())?;
    let primary_monitor = window.primary_monitor().map_err(|e| e.to_string())?;

    for monitor in available {
        let is_primary = primary_monitor.as_ref().map(|m| {m.name() == monitor.name()}).unwrap_or(false);

        monitors.push(MonitorInfo {
            id: monitor.name().unwrap_or_default(),
            name: monitor.name().unwrap_or_default(),
            width: monitor.size().width,
            height: monitor.size().height,
            refresh_rate:
                monitor.refresh_rate_millihertz()
                    .unwrap_or(60000)
                    / 1000,
            primary: is_primary,
        });
    }
    Ok(monitors)
}

#[tauri::command]
pub fn assign_wallpaper_to_monitor(
    monitor_id: String,
    wallpaper_id: String,
) -> Result<(), String> {
    let conn = get_connection().map_err(|e| e.to_string())?;

    conn.execute(
        "
        INSERT OR REPLACE
        INTO monitor_wallpapers(
            monitor_id,
            wallpaper_id
        )
        VALUES(?1,?2)
        ",
        (
            monitor_id,
            wallpaper_id
        )
    ).map_err(|e| e.to_string())?;
    Ok(())
}