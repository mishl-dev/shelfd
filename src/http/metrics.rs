use std::sync::atomic::Ordering;

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::db;
use crate::state::AppState;

#[derive(Serialize)]
struct MetricsPayload {
    requests_total: u64,
    searches_total: u64,
    search_cache_hits: u64,
    search_result_books_seen: u64,
    search_book_cache_hits: u64,
    search_book_cache_misses: u64,
    search_inline_info_requests: u64,
    search_inline_info_failures: u64,
    explore_cache_hits: u64,
    downloads_total: u64,
    download_cache_hits: u64,
    download_failure_cache_hits: u64,
    flaresolverr_solves_started: u64,
    flaresolverr_solves_completed: u64,
    cover_prewarm_jobs_started: u64,
    cover_prewarm_jobs_completed: u64,
    cover_prewarm_attempts: u64,
    cover_prewarm_hits: u64,
    cover_resolution_hot_hits: u64,
    cover_resolution_hot_misses: u64,
    upstream_retries: u64,
    cover_jobs_started: u64,
    cover_jobs_completed: u64,
    cached_books: i64,
    cached_links: i64,
    cached_searches: i64,
    cached_explore_sources: i64,
    last_cleanup_unix: u64,
}

pub async fn handle_metrics(State(state): State<AppState>) -> Response {
    let counts = match db::cache_counts(&state.pool).await {
        Ok(counts) => counts,
        Err(error) => {
            tracing::error!("metrics cache count query failed: {error:#}");
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
    };

    Json(MetricsPayload {
        requests_total: state.metrics.requests_total.load(Ordering::Relaxed),
        searches_total: state.metrics.searches_total.load(Ordering::Relaxed),
        search_cache_hits: state.metrics.search_cache_hits.load(Ordering::Relaxed),
        search_result_books_seen: state.metrics.search_result_books_seen.load(Ordering::Relaxed),
        search_book_cache_hits: state.metrics.search_book_cache_hits.load(Ordering::Relaxed),
        search_book_cache_misses: state
            .metrics
            .search_book_cache_misses
            .load(Ordering::Relaxed),
        search_inline_info_requests: state
            .metrics
            .search_inline_info_requests
            .load(Ordering::Relaxed),
        search_inline_info_failures: state
            .metrics
            .search_inline_info_failures
            .load(Ordering::Relaxed),
        explore_cache_hits: state.metrics.explore_cache_hits.load(Ordering::Relaxed),
        downloads_total: state.metrics.downloads_total.load(Ordering::Relaxed),
        download_cache_hits: state.metrics.download_cache_hits.load(Ordering::Relaxed),
        download_failure_cache_hits: state
            .metrics
            .download_failure_cache_hits
            .load(Ordering::Relaxed),
        flaresolverr_solves_started: state
            .metrics
            .flaresolverr_solves_started
            .load(Ordering::Relaxed),
        flaresolverr_solves_completed: state
            .metrics
            .flaresolverr_solves_completed
            .load(Ordering::Relaxed),
        cover_prewarm_jobs_started: state
            .metrics
            .cover_prewarm_jobs_started
            .load(Ordering::Relaxed),
        cover_prewarm_jobs_completed: state
            .metrics
            .cover_prewarm_jobs_completed
            .load(Ordering::Relaxed),
        cover_prewarm_attempts: state
            .metrics
            .cover_prewarm_attempts
            .load(Ordering::Relaxed),
        cover_prewarm_hits: state.metrics.cover_prewarm_hits.load(Ordering::Relaxed),
        cover_resolution_hot_hits: state
            .metrics
            .cover_resolution_hot_hits
            .load(Ordering::Relaxed),
        cover_resolution_hot_misses: state
            .metrics
            .cover_resolution_hot_misses
            .load(Ordering::Relaxed),
        upstream_retries: state.metrics.upstream_retries.load(Ordering::Relaxed),
        cover_jobs_started: state.metrics.cover_jobs_started.load(Ordering::Relaxed),
        cover_jobs_completed: state.metrics.cover_jobs_completed.load(Ordering::Relaxed),
        cached_books: counts.books,
        cached_links: counts.links,
        cached_searches: counts.searches,
        cached_explore_sources: counts.explore_sources,
        last_cleanup_unix: state.metrics.last_cleanup_unix.load(Ordering::Relaxed),
    })
    .into_response()
}
