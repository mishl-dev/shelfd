use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicU64, atomic::AtomicUsize},
    time::Duration,
};

use dashmap::DashMap;
use moka::future::Cache;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::sync::Notify;

use crate::config::AppConfig;
use crate::flaresolverr::FlareSolverrClient;

const HOT_COVER_RESOLUTION_TTL_SECS: u64 = 300;

#[derive(Clone)]
pub struct AppState {
    pub fs: Arc<FlareSolverrClient>,
    pub pool: Arc<SqlitePool>,
    pub http: Client,
    pub archive_base: String,
    pub archive_bases: Arc<Vec<String>>,
    pub archive_rr: Arc<AtomicUsize>,
    pub archive_name: String,
    pub app_name: String,
    pub metadata_base_url: String,
    pub public_base_url: Option<String>,
    pub search_cache_ttl_secs: i64,
    pub book_cache_ttl_secs: i64,
    pub link_cache_ttl_secs: i64,
    pub link_failure_ttl_secs: i64,
    pub explore_cache_ttl_secs: i64,
    pub cover_negative_ttl_secs: i64,
    pub search_result_limit: usize,
    pub explore_page_size: usize,
    #[allow(dead_code)]
    pub cover_lookup_limit: usize,
    pub inline_info_concurrency: usize,
    #[allow(dead_code)]
    pub cover_lookup_concurrency: usize,
    pub search_prewarm_count: usize,
    pub upstream_retry_attempts: usize,
    pub upstream_retry_backoff_ms: u64,
    pub explore_subjects: Arc<Vec<ExploreSubject>>,
    pub subject_name_by_slug: Arc<HashMap<String, String>>,
    pub metrics: Arc<AppMetrics>,
    pub search_inflight: Arc<DashMap<String, Arc<Notify>>>,
    pub download_inflight: Arc<DashMap<String, Arc<Notify>>>,
    pub cover_inflight: Arc<DashMap<String, Arc<Notify>>>,
    pub hot_cover_resolutions: Arc<Cache<String, Option<String>>>,
}

impl AppState {
    pub fn new(config: &AppConfig, pool: SqlitePool, http: Client) -> Self {
        let explore_subjects = Arc::new(config.explore_subjects());
        let subject_name_by_slug = Arc::new(
            explore_subjects
                .iter()
                .map(|s| (s.slug.clone(), s.name.clone()))
                .collect::<HashMap<_, _>>(),
        );
        Self {
            fs: Arc::new(FlareSolverrClient::new(
                http.clone(),
                config.flaresolverr_url.clone(),
                config.flaresolverr_session.clone(),
            )),
            pool: Arc::new(pool),
            http,
            archive_base: config.archive_base.clone(),
            archive_bases: Arc::new(config.archive_bases.clone()),
            archive_rr: Arc::new(AtomicUsize::new(0)),
            archive_name: config.archive_name.clone(),
            app_name: config.app_name.clone(),
            metadata_base_url: config.metadata_base_url.clone(),
            public_base_url: config.public_base_url.clone(),
            search_cache_ttl_secs: config.search_cache_ttl_secs,
            book_cache_ttl_secs: config.book_cache_ttl_secs,
            link_cache_ttl_secs: config.link_cache_ttl_secs,
            link_failure_ttl_secs: config.link_failure_ttl_secs,
            explore_cache_ttl_secs: config.explore_cache_ttl_secs,
            cover_negative_ttl_secs: config.cover_negative_ttl_secs,
            search_result_limit: config.search_result_limit,
            explore_page_size: config.explore_page_size,
            cover_lookup_limit: config.cover_lookup_limit,
            inline_info_concurrency: config.inline_info_concurrency,
            cover_lookup_concurrency: config.cover_lookup_concurrency,
            search_prewarm_count: config.search_prewarm_count,
            upstream_retry_attempts: config.upstream_retry_attempts,
            upstream_retry_backoff_ms: config.upstream_retry_backoff_ms,
            explore_subjects,
            subject_name_by_slug,
            metrics: Arc::new(AppMetrics::default()),
            search_inflight: Arc::new(DashMap::new()),
            download_inflight: Arc::new(DashMap::new()),
            cover_inflight: Arc::new(DashMap::new()),
            hot_cover_resolutions: Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(HOT_COVER_RESOLUTION_TTL_SECS))
                    .build(),
            ),
        }
    }

    pub fn next_archive_base(&self) -> &str {
        if self.archive_bases.len() <= 1 {
            return &self.archive_bases[0];
        }
        let idx = self
            .archive_rr
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        &self.archive_bases[idx % self.archive_bases.len()]
    }
}

#[derive(Default)]
pub struct AppMetrics {
    pub requests_total: AtomicU64,
    pub searches_total: AtomicU64,
    pub search_cache_hits: AtomicU64,
    pub search_result_books_seen: AtomicU64,
    pub search_book_cache_hits: AtomicU64,
    pub search_book_cache_misses: AtomicU64,
    pub search_inline_info_requests: AtomicU64,
    pub search_inline_info_failures: AtomicU64,
    pub explore_cache_hits: AtomicU64,
    pub downloads_total: AtomicU64,
    pub download_cache_hits: AtomicU64,
    pub download_failure_cache_hits: AtomicU64,
    pub flaresolverr_solves_started: AtomicU64,
    pub flaresolverr_solves_completed: AtomicU64,
    pub cover_prewarm_jobs_started: AtomicU64,
    pub cover_prewarm_jobs_completed: AtomicU64,
    pub cover_prewarm_attempts: AtomicU64,
    pub cover_prewarm_hits: AtomicU64,
    pub cover_resolution_hot_hits: AtomicU64,
    pub cover_resolution_hot_misses: AtomicU64,
    pub upstream_retries: AtomicU64,
    pub cover_jobs_started: AtomicU64,
    pub cover_jobs_completed: AtomicU64,
    pub last_cleanup_unix: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct ExploreSubject {
    pub slug: String,
    pub name: String,
}
