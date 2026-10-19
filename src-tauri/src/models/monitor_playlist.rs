use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorPlaylist {
    pub monitor_id: String,
    pub playlist_id: String,
    pub current_wallpaper_id: Option<String>,
    pub last_error: Option<String>,
}
