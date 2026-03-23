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

#[derive(Debug, Clone)]
pub struct CachedBook {
    pub entry: BookEntry,
    pub cached_at: i64,
}

#[derive(Debug, Clone)]
pub struct CachedLink {
    pub download_url: Option<String>,
    pub media_type: Option<String>,
    pub failed: bool,
    pub failure_reason: Option<String>,
    pub cached_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct InlineInfo {
    pub downloads_total: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OlResponse {
    pub docs: Vec<OlDoc>,
}

#[derive(Debug, Deserialize)]
pub struct OlDoc {
    pub cover_i: Option<i64>,
    pub subject: Option<Vec<String>>,
    pub first_publish_year: Option<i64>,
    pub language: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct OlSubjectResponse {
    pub works: Vec<OlSubjectWork>,
}

#[derive(Debug, Deserialize)]
pub struct OlSubjectWork {
    pub key: String,
    pub title: String,
    pub authors: Vec<OlAuthorRef>,
    pub cover_id: Option<i64>,
    pub edition_count: Option<i64>,
    pub subject: Option<Vec<String>>,
    pub first_publish_year: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OlAuthorRef {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CacheTtls {
    pub books_secs: i64,
    pub links_secs: i64,
    pub link_failures_secs: i64,
    pub searches_secs: i64,
    pub explore_secs: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedExploreEntries {
    pub entries: Vec<ExploreEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct OlEnrichment {
    pub cover_url: Option<String>,
    pub first_publish_year: Option<i64>,
    pub language: Option<String>,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CacheCounts {
    pub books: i64,
    pub links: i64,
    pub searches: i64,
    pub explore_sources: i64,
}
