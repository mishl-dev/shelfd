use serde::{Deserialize, Serialize};

use super::{BookEntry, ExploreEntry};

#[derive(Debug, Clone)]
pub struct CachedBook {
    pub entry: BookEntry,
}

#[derive(Debug, Clone)]
pub struct CachedLink {
    pub download_url: Option<String>,
    pub media_type: Option<String>,
    pub failed: bool,
    pub failure_reason: Option<String>,
    pub cached_at: i64,
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

#[derive(Debug, Clone)]
pub struct CacheCounts {
    pub books: i64,
    pub links: i64,
    pub searches: i64,
    pub explore_sources: i64,
}
