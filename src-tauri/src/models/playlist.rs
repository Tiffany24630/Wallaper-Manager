use serde::{Deserialize, Serialize,};

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub created_at: String,
}