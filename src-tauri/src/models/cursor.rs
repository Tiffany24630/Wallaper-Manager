use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorResource {
    pub id: String,
    pub name: String,
    pub path: String,
    pub preview: Option<String>,
    pub resource_type: String,
    pub source_path: Option<String>,
    pub hash: Option<String>,
    pub hotspot_x: u16,
    pub hotspot_y: u16,
    pub created_at: Option<String>,
}
