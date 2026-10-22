use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperResourceType {
    Static,
    Animated,
}

impl WallpaperResourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Animated => "animated",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "static" => Ok(Self::Static),
            "animated" => Ok(Self::Animated),
            _ => Err(format!("Tipo de recurso de wallpaper no válido: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperAsset {
    pub path: String,
    pub original_path: Option<String>,
    pub hash: Option<String>,
    pub size_bytes: Option<u64>,
    pub file_type: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperThumbnail {
    pub path: Option<String>,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wallpaper {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumbnail: Option<String>,
    pub favorite: bool,
    pub active: bool,
    pub size_bytes: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub file_type: Option<String>,
    pub hash: Option<String>,
    pub created_at: Option<String>,
    pub resource_type: WallpaperResourceType,
    pub parent_wallpaper_id: Option<String>,
    pub asset: WallpaperAsset,
    pub thumbnail_asset: WallpaperThumbnail,
}
