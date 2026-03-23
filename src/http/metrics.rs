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
    explore_cache_hits: u64,
    downloads_total: u64,
    download_cache_hits: u64,
    download_failure_cache_hits: u64,
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
        explore_cache_hits: state.metrics.explore_cache_hits.load(Ordering::Relaxed),
        downloads_total: state.metrics.downloads_total.load(Ordering::Relaxed),
        download_cache_hits: state.metrics.download_cache_hits.load(Ordering::Relaxed),
        download_failure_cache_hits: state
            .metrics
            .download_failure_cache_hits
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
