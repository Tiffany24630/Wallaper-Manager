use crate::{
    db::get_connection, models::monitor::MonitorInfo,
    utils::windows_wallpaper::set_monitor_wallpaper,
};
use tauri::Manager;

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
            let name = monitor.name().unwrap_or_else(|| "Pantalla".into());
            MonitorInfo {
                id: name.clone(),
                name,
                width: monitor.size().width,
                height: monitor.size().height,
                refresh_rate: monitor.refresh_rate_millihertz().unwrap_or(60_000) / 1_000,
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
    set_monitor_wallpaper(&monitor_id, &path)
        .map_err(|e| format!("Windows no pudo aplicar el fondo al monitor: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO monitor_wallpapers(monitor_id, wallpaper_id) VALUES(?1, ?2)",
        (&monitor_id, &wallpaper_id),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
