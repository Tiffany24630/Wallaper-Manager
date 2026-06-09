use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallpaper {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumbnail: Option<String>,
    pub favorite: bool,
    pub active: bool,
    pub size_bytes: Option<u64>,
    pub created_at: Option<String>,
}