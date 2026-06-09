use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub hardware_acceleration: bool,
    pub pause_on_battery: bool,
    pub pause_when_maximized: bool,
    pub scaling_mode: String,
}