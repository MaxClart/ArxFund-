use serde::{Deserialize, Serialize};

/// Identifiant de plateforme d'origine d'un morceau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Provider {
    #[default]
    Local,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Local => "local",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "local" => Provider::Local,
            _ => Provider::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub cover_path: Option<String>,
    pub provider: Provider,
    pub provider_track_id: Option<String>,
    pub source_url: Option<String>,
    pub local_path: Option<String>,
    pub audio_available: bool,
    pub is_favorite: bool,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub track_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Title,
    Artist,
    Album,
    Duration,
    AddedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}
