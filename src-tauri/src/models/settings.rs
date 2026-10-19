use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub hardware_acceleration: bool,
    pub pause_on_battery: bool,
    pub pause_when_maximized: bool,
    pub scaling_mode: String,
    pub accent_color: String,
    pub ui_scale: u16,
    pub dynamic_enabled: bool,
    pub dynamic_playlist_id: Option<String>,
    pub dynamic_interval_minutes: u32,
    pub dynamic_current_wallpaper_id: Option<String>,
    pub dynamic_last_error: Option<String>,
    pub cursor_enabled: bool,
    pub active_cursor_id: Option<String>,
}
