use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookEntry {
    pub md5: String,
    pub title: String,
    pub author: String,
    pub downloads: i64,
    pub cover_url: Option<String>,
    pub download_media_type: Option<String>,
    pub cover_checked_at: Option<i64>,
    pub first_publish_year: Option<i64>,
    pub language: Option<String>,
    pub subjects: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub cover_url: Option<String>,
    pub search_query: String,
    pub alternate_url: String,
    pub popularity: i64,
    pub first_publish_year: Option<i64>,
    pub subjects: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InlineInfo {
    pub downloads_total: Option<i64>,
}
