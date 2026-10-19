use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncQueueItem {
    pub id: String,
    pub operation_type: String,
    pub entity_id: Option<String>,
    pub revision: Option<i64>,
    pub state: String,
    pub attempts: i64,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlaylistState {
    pub playlist_id: String,
    pub last_remote_revision: Option<i64>,
    pub base_payload: Option<String>,
    pub local_dirty: bool,
    pub deleted_pending: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlaylistConflict {
    pub id: String,
    pub playlist_id: String,
    pub local_payload: String,
    pub remote_payload: String,
    pub base_revision: Option<i64>,
    pub remote_revision: i64,
}
