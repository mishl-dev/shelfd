use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use tokio::net::TcpListener;
use tracing::info;

mod app;
mod config;
mod cover_gen;
mod db;
mod flaresolverr;
mod http;
mod models;
mod opds;
mod scraper;
mod service;
mod state;

use clap::Parser;
use app::{build_app, build_http_client, build_sqlite_pool};
use config::{
    Cli, Command, ServeArgs, init_tracing, load_config, parse_explore_subjects,
    print_startup_summary,
};
use flaresolverr::FlareSolverrClient;
use models::CacheTtls;
use state::{AppMetrics, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve(ServeArgs::default()));
    let serve_args = match &command {
        Command::Serve(args) | Command::PrintConfig(args) => args,
    };
    let config = load_config(serve_args)?;
    init_tracing(&config)?;

    if matches!(command, Command::PrintConfig(_)) {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    print_startup_summary(&config);

    info!(database_url = %config.database_url, "connecting to sqlite");
    let pool = build_sqlite_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    let explore_subjects = Arc::new(parse_explore_subjects(&config.explore_subjects_raw));
    let subject_name_by_slug = Arc::new(
        explore_subjects
            .iter()
            .map(|s| (s.slug.clone(), s.name.clone()))
            .collect::<std::collections::HashMap<_, _>>(),
    );
    let http = build_http_client()?;

    let state = AppState {
        fs: Arc::new(FlareSolverrClient::new(
            http.clone(),
            config.flaresolverr_url.clone(),
            config.flaresolverr_session.clone(),
        )),
        pool: Arc::new(pool),
        http,
        archive_base: config.archive_base.clone(),
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
        hot_cover_resolutions: Arc::new(DashMap::new()),
    };

    let cache_ttls = CacheTtls {
        books_secs: config.book_cache_ttl_secs,
        links_secs: config.link_cache_ttl_secs,
        link_failures_secs: config.link_failure_ttl_secs,
        searches_secs: config.search_cache_ttl_secs,
        explore_secs: config.explore_cache_ttl_secs,
    };
    db::prune_expired_cache(&state.pool, &cache_ttls).await?;
    state
        .metrics
        .last_cleanup_unix
        .store(db::unix_now() as u64, std::sync::atomic::Ordering::Relaxed);
    let _cleanup_task = service::cleanup::spawn_cache_cleanup(
        state.pool.clone(),
        state.metrics.clone(),
        cache_ttls.clone(),
        config.cleanup_interval_secs,
    );

    info!(
        flaresolverr_url = %config.flaresolverr_url,
        flaresolverr_session = %config.flaresolverr_session,
        archive_base = %config.archive_base,
        archive_name = %config.archive_name,
        app_name = %config.app_name,
        metadata_base_url = %config.metadata_base_url,
        public_base_url = config.public_base_url.as_deref().unwrap_or(""),
        search_cache_ttl_secs = config.search_cache_ttl_secs,
        book_cache_ttl_secs = config.book_cache_ttl_secs,
        link_cache_ttl_secs = config.link_cache_ttl_secs,
        link_failure_ttl_secs = config.link_failure_ttl_secs,
        explore_cache_ttl_secs = config.explore_cache_ttl_secs,
        cover_negative_ttl_secs = config.cover_negative_ttl_secs,
        search_result_limit = config.search_result_limit,
        explore_page_size = config.explore_page_size,
        cover_lookup_limit = config.cover_lookup_limit,
        inline_info_concurrency = config.inline_info_concurrency,
        cover_lookup_concurrency = config.cover_lookup_concurrency,
        search_prewarm_count = config.search_prewarm_count,
        upstream_retry_attempts = config.upstream_retry_attempts,
        upstream_retry_backoff_ms = config.upstream_retry_backoff_ms,
        explore_subject_count = state.explore_subjects.len(),
        cleanup_interval_secs = config.cleanup_interval_secs,
        "application state initialized"
    );

    let app = build_app(state);

    let listener = TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        Router,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::util::ServiceExt;

    use crate::config::parse_explore_subjects;
    use crate::models::BookEntry;
    use crate::service::retry::retry_backoff;
    use crate::service::search::sort_books_for_query;
    use crate::state::{AppMetrics, AppState};

    async fn test_app() -> Router {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        db::run_migrations(&pool).await.unwrap();
        let http = build_http_client().unwrap();

        let state = AppState {
            fs: Arc::new(FlareSolverrClient::new(
                http.clone(),
                "http://127.0.0.1:8191".to_owned(),
                "test-session".to_owned(),
            )),
            pool: Arc::new(pool),
            http,
            archive_base: "https://example.com".to_owned(),
            archive_name: "Archive".to_owned(),
            app_name: "shelfd".to_owned(),
            metadata_base_url: "https://openlibrary.org".to_owned(),
            public_base_url: Some("http://localhost:7451".to_owned()),
            search_cache_ttl_secs: 1800,
            book_cache_ttl_secs: 86400,
            link_cache_ttl_secs: 86400,
            link_failure_ttl_secs: 900,
            explore_cache_ttl_secs: 21_600,
            cover_negative_ttl_secs: 86_400,
            search_result_limit: 12,
            explore_page_size: 50,
            cover_lookup_limit: 8,
            inline_info_concurrency: 6,
            cover_lookup_concurrency: 4,
            search_prewarm_count: 3,
            upstream_retry_attempts: 2,
            upstream_retry_backoff_ms: 150,
            explore_subjects: Arc::new(parse_explore_subjects("science_fiction,fantasy")),
            subject_name_by_slug: Arc::new(
                [
                    ("science_fiction".to_owned(), "Science Fiction".to_owned()),
                    ("fantasy".to_owned(), "Fantasy".to_owned()),
                ]
                .into_iter()
                .collect(),
            ),
            metrics: Arc::new(AppMetrics::default()),
            search_inflight: Arc::new(DashMap::new()),
            download_inflight: Arc::new(DashMap::new()),
            cover_inflight: Arc::new(DashMap::new()),
            hot_cover_resolutions: Arc::new(DashMap::new()),
        };

        build_app(state)
    }

    #[tokio::test]
    async fn healthz_returns_ok_json() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"ok\""));
    }

    #[tokio::test]
    async fn readyz_returns_ok_when_db_is_available() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn opensearch_endpoint_is_served() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/opds/opensearch.xml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(
            content_type.to_str().unwrap(),
            "application/opensearchdescription+xml; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn metrics_endpoint_is_served() {
        let app = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"requests_total\""));
        assert!(body.contains("\"searches_total\""));
    }

    #[test]
    fn ranking_prefers_downloads_over_text_relevance() {
        let mut books = vec![
            BookEntry {
                md5: "1".to_owned(),
                title: "Completely Different Book".to_owned(),
                author: "Someone".to_owned(),
                downloads: 50_000,
                cover_url: None,
                download_media_type: None,
                cover_checked_at: None,
                first_publish_year: None,
                language: None,
                subjects: Vec::new(),
                description: None,
            },
            BookEntry {
                md5: "2".to_owned(),
                title: "Dune".to_owned(),
                author: "Frank Herbert".to_owned(),
                downloads: 500,
                cover_url: None,
                download_media_type: None,
                cover_checked_at: None,
                first_publish_year: None,
                language: None,
                subjects: Vec::new(),
                description: None,
            },
        ];

        sort_books_for_query("dune", &mut books);

        assert_eq!(books[0].md5, "1");
    }

    #[test]
    fn ranking_uses_downloads_as_tiebreaker_for_similar_matches() {
        let mut books = vec![
            BookEntry {
                md5: "1".to_owned(),
                title: "Dune Messiah".to_owned(),
                author: "Frank Herbert".to_owned(),
                downloads: 1_000,
                cover_url: None,
                download_media_type: None,
                cover_checked_at: None,
                first_publish_year: None,
                language: None,
                subjects: Vec::new(),
                description: None,
            },
            BookEntry {
                md5: "2".to_owned(),
                title: "Dune Encyclopedia".to_owned(),
                author: "Willis McNelly".to_owned(),
                downloads: 5_000,
                cover_url: None,
                download_media_type: None,
                cover_checked_at: None,
                first_publish_year: None,
                language: None,
                subjects: Vec::new(),
                description: None,
            },
        ];

        sort_books_for_query("dune", &mut books);

        assert_eq!(books[0].md5, "2");
    }

    #[test]
    fn retry_backoff_grows_exponentially() {
        assert_eq!(
            retry_backoff(250, 1),
            tokio::time::Duration::from_millis(250)
        );
        assert_eq!(
            retry_backoff(250, 2),
            tokio::time::Duration::from_millis(500)
        );
        assert_eq!(
            retry_backoff(250, 3),
            tokio::time::Duration::from_millis(1000)
        );
    }
}
