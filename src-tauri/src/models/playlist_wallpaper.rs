use serde::{Serialize, Deserialize,};

#[derive(
    Serialize,
    Deserialize,
)]
pub struct PlaylistWallpaper {
    pub playlist_id: String,
    pub wallpaper_id: String,
}