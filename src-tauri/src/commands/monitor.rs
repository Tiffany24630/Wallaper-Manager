use serde::Serialize;

#[derive(Serialize)]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub primary: bool,
}

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
            refresh_rate: 60,
            primary: is_primary,
        });
    }

    Ok(monitors)
}