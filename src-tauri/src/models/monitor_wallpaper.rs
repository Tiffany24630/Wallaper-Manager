use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MonitorWallpaper {
    pub monitor_id: String,
    pub wallpaper_id: String,
}
