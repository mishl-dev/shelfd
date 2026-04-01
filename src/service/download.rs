use std::sync::atomic::Ordering;

use tracing::info;

use crate::db;
use crate::scraper;
use crate::state::AppState;

use super::inflight::{InflightRole, begin_inflight};
use super::retry::{get_flaresolverr_html_with_retry, log_sanitized_html};

async fn check_cached_link(
    pool: &sqlx::SqlitePool,
    metrics: &crate::state::AppMetrics,
    md5: &str,
    success_min_cached_at: i64,
    failure_min_cached_at: i64,
) -> Result<Option<String>, anyhow::Error> {
    if let Some(cached) =
        db::get_cached_link(pool, md5, success_min_cached_at, failure_min_cached_at).await?
    {
        if cached.failed {
            metrics
                .download_failure_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            anyhow::bail!(
                "{}",
                cached
                    .failure_reason
                    .unwrap_or_else(|| "cached download resolution failure".to_owned())
            );
        }
        metrics.download_cache_hits.fetch_add(1, Ordering::Relaxed);
        info!(%md5, media_type = cached.media_type.as_deref().unwrap_or(""), cached_at = cached.cached_at, "download URL cache hit");
        return Ok(cached.download_url);
    }
    Ok(None)
}

pub async fn resolve_download(state: &AppState, md5: &str) -> anyhow::Result<String> {
    let success_min_cached_at = db::unix_now() - state.link_cache_ttl_secs;
    let failure_min_cached_at = db::unix_now() - state.link_failure_ttl_secs;
    if let Some(url) = check_cached_link(
        &state.pool,
        &state.metrics,
        md5,
        success_min_cached_at,
        failure_min_cached_at,
    )
    .await?
    {
        return Ok(url);
    }

    let inflight = begin_inflight(state.download_inflight.clone(), md5.to_owned()).await;
    let _guard = match inflight {
        InflightRole::Leader(guard) => guard,
        InflightRole::Waiter(notify) => {
            info!(%md5, "waiting for in-flight download resolution");
            notify.notified().await;
            if let Some(url) = check_cached_link(
                &state.pool,
                &state.metrics,
                md5,
                success_min_cached_at,
                failure_min_cached_at,
            )
            .await?
            {
                return Ok(url);
            }
            begin_inflight(state.download_inflight.clone(), md5.to_owned())
                .await
                .into_leader()?
        }
    };

    let slow_url = format!("{}/slow_download/{}/0/4", state.next_archive_base(), md5);
    state
        .metrics
        .flaresolverr_solves_started
        .fetch_add(1, Ordering::Relaxed);
    info!(%md5, %slow_url, "resolving download URL from archive");
    let html = match get_flaresolverr_html_with_retry(state, &slow_url).await {
        Ok(html) => html,
        Err(error) => {
            db::cache_link_failure(&state.pool, md5, &error.to_string()).await?;
            return Err(error);
        }
    };
    log_sanitized_html("download page", &html);
    let download_url = match scraper::parse_download_url(&html) {
        Ok(url) => url,
        Err(error) => {
            db::cache_link_failure(&state.pool, md5, &error.to_string()).await?;
            return Err(error);
        }
    };
    let media_type = infer_media_type_from_url(&download_url);

    db::cache_link_success(&state.pool, md5, &download_url, media_type.as_deref()).await?;
    state
        .metrics
        .flaresolverr_solves_completed
        .fetch_add(1, Ordering::Relaxed);
    info!(%md5, %download_url, "download URL resolved and cached");
    Ok(download_url)
}

pub fn infer_media_type_from_url(url: &str) -> Option<String> {
    let lowered = url.to_lowercase();
    let path = lowered.split('?').next().unwrap_or(&lowered);
    if path.ends_with(".epub") {
        Some("application/epub+zip".to_owned())
    } else if path.ends_with(".pdf") {
        Some("application/pdf".to_owned())
    } else if path.ends_with(".mobi") {
        Some("application/x-mobipocket-ebook".to_owned())
    } else if path.ends_with(".azw3") {
        Some("application/vnd.amazon.ebook".to_owned())
    } else if path.ends_with(".fb2") {
        Some("application/x-fictionbook+xml".to_owned())
    } else if path.ends_with(".djvu") || path.ends_with(".djv") {
        Some("image/vnd.djvu".to_owned())
    } else if path.ends_with(".txt") {
        Some("text/plain".to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_media_type_epub() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.epub"),
            Some("application/epub+zip".to_owned())
        );
    }

    #[test]
    fn infer_media_type_pdf() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.pdf"),
            Some("application/pdf".to_owned())
        );
    }

    #[test]
    fn infer_media_type_mobi() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.mobi"),
            Some("application/x-mobipocket-ebook".to_owned())
        );
    }

    #[test]
    fn infer_media_type_azw3() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.azw3"),
            Some("application/vnd.amazon.ebook".to_owned())
        );
    }

    #[test]
    fn infer_media_type_fb2() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.fb2"),
            Some("application/x-fictionbook+xml".to_owned())
        );
    }

    #[test]
    fn infer_media_type_djvu() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.djvu"),
            Some("image/vnd.djvu".to_owned())
        );
    }

    #[test]
    fn infer_media_type_djv() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.djv"),
            Some("image/vnd.djvu".to_owned())
        );
    }

    #[test]
    fn infer_media_type_txt() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.txt"),
            Some("text/plain".to_owned())
        );
    }

    #[test]
    fn infer_media_type_case_insensitive() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.EPUB"),
            Some("application/epub+zip".to_owned())
        );
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.Pdf"),
            Some("application/pdf".to_owned())
        );
    }

    #[test]
    fn infer_media_type_with_query_params() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.epub?token=abc"),
            Some("application/epub+zip".to_owned())
        );
    }

    #[test]
    fn infer_media_type_unknown_extension() {
        assert_eq!(
            infer_media_type_from_url("https://example.com/file.xyz"),
            None
        );
    }

    #[test]
    fn infer_media_type_no_extension() {
        assert_eq!(infer_media_type_from_url("https://example.com/file"), None);
    }

    #[test]
    fn infer_media_type_empty_url() {
        assert_eq!(infer_media_type_from_url(""), None);
    }
}
