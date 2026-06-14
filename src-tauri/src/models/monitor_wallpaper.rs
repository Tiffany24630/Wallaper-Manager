use serde::{
    Serialize,
    Deserialize
};

#[derive(
    Serialize,
    Deserialize
)]
pub struct MonitorWallpaper {
    pub monitor_id: String,
    pub wallpaper_id: String,
}