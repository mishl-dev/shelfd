use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::{debug, info};

use crate::cover_gen::render_cover;
use crate::db;
use crate::service::inflight::{InflightRole, begin_inflight};
use crate::service::metadata::enrich_book_metadata;
use crate::state::AppState;

pub async fn resolve_cover_response(state: &AppState, param: &str, size_suffix: &str) -> Response {
    if param.chars().all(|c| c.is_ascii_digit()) {
        return fetch_cover_url(
            state,
            &format!(
                "https://covers.openlibrary.org/b/id/{}-{}.jpg?default=false",
                param, size_suffix
            ),
        )
        .await;
    }

    let min_cached_at = db::unix_now() - state.book_cache_ttl_secs;
    let book = match db::get_cached_book(&state.pool, param, min_cached_at).await {
        Ok(Some(cached)) => {
            debug!(md5 = %param, title = %cached.entry.title, author = %cached.entry.author, "cover: book found in cache");
            Some(cached.entry)
        }
        Ok(None) => {
            debug!(md5 = %param, "cover: book NOT found in cache");
            None
        }
        Err(e) => {
            debug!(md5 = %param, error = %e, "cover: cache lookup error");
            None
        }
    };

    let title = book
        .as_ref()
        .map(|b| b.title.as_str())
        .unwrap_or("Unknown")
        .to_owned();
    let author = book
        .as_ref()
        .map(|b| b.author.as_str())
        .unwrap_or("Unknown")
        .to_owned();

    let mut cover_url = book.as_ref().and_then(|b| b.cover_url.clone());
    let hot_resolution = if cover_url.is_none() {
        get_hot_cover_resolution(state, param).await
    } else {
        cache_hot_cover_resolution(state, param, cover_url.clone()).await;
        None
    };
    if cover_url.is_none() && let Some(hot_cover_url) = hot_resolution.clone() {
        cover_url = hot_cover_url;
    }

    if cover_url.is_none() {
        if hot_resolution.is_some() {
            return generated_cover(&title, &author).await;
        }

        let min_cover_checked_at = db::unix_now() - state.cover_negative_ttl_secs;
        let needs_enrichment = match db::get_cached_book(&state.pool, param, 0).await {
            Ok(Some(cached)) => cached
                .entry
                .cover_checked_at
                .map(|t| t <= min_cover_checked_at)
                .unwrap_or(true),
            _ => true,
        };

        if needs_enrichment {
            let inflight = begin_inflight(state.cover_inflight.clone(), param.to_owned()).await;
            match inflight {
                InflightRole::Leader(guard) => {
                    cover_url = enrich_book_metadata(state, param).await;
                    cache_hot_cover_resolution(state, param, cover_url.clone()).await;
                    drop(guard);
                }
                InflightRole::Waiter(notify) => {
                    info!(md5 = %param, "waiting for in-flight cover enrichment");
                    notify.notified().await;
                    let book = match db::get_cached_book(&state.pool, param, 0).await {
                        Ok(Some(cached)) => Some(cached.entry),
                        _ => None,
                    };
                    cover_url = book.as_ref().and_then(|b| b.cover_url.clone());
                    cache_hot_cover_resolution(state, param, cover_url.clone()).await;
                }
            }
        }
    }

    let upstream_cover_url = cover_url.and_then(|url| openlibrary_cover_url_for_size(&url, size_suffix));

    match upstream_cover_url {
        Some(url) => match fetch_cover_url(state, &url).await {
            resp if resp.status().is_success() => resp,
            _ => generated_cover(&title, &author).await,
        },
        None => generated_cover(&title, &author).await,
    }
}

pub fn openlibrary_cover_url_for_size(url: &str, size_suffix: &str) -> Option<String> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let path = url.strip_prefix("covers.openlibrary.org/")?;
    let path = path.split('?').next()?;
    let (prefix, _) = path.rsplit_once('-')?;
    Some(format!("https://covers.openlibrary.org/{prefix}-{size_suffix}.jpg?default=false"))
}

async fn fetch_cover_url(state: &AppState, cover_url: &str) -> Response {
    match state.http.get(cover_url).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => {
                let content_type = resp
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("image/jpeg")
                    .to_owned();
                match resp.bytes().await {
                    Ok(body) => (
                        [
                            (header::CONTENT_TYPE, content_type),
                            (
                                header::CACHE_CONTROL,
                                "public, max-age=86400, stale-while-revalidate=604800"
                                    .to_owned(),
                            ),
                        ],
                        body,
                    )
                        .into_response(),
                    Err(e) => {
                        tracing::error!("cover body read failed: {e:#}");
                        (StatusCode::BAD_GATEWAY, "failed to read cover").into_response()
                    }
                }
            }
            Err(e) => {
                tracing::error!("cover upstream error: {e:#}");
                (StatusCode::BAD_GATEWAY, "cover not found").into_response()
            }
        },
        Err(e) => {
            tracing::error!("cover request failed: {e:#}");
            (StatusCode::BAD_GATEWAY, "cover request failed").into_response()
        }
    }
}

async fn get_hot_cover_resolution(state: &AppState, md5: &str) -> Option<Option<String>> {
    state.hot_cover_resolutions.get(md5).await
}

async fn cache_hot_cover_resolution(state: &AppState, md5: &str, cover_url: Option<String>) {
    state
        .hot_cover_resolutions
        .insert(md5.to_owned(), cover_url)
        .await;
}

async fn generated_cover(title: &str, author: &str) -> Response {
    debug!(%title, %author, "generating cover");
    match tokio::task::spawn_blocking({
        let title = title.to_owned();
        let author = author.to_owned();
        move || render_cover(&title, &author)
    })
    .await
    {
        Ok(Ok(png)) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (
                    header::CACHE_CONTROL,
                    "no-cache, no-store, must-revalidate",
                ),
            ],
            png,
        )
            .into_response(),
        Ok(Err(e)) => {
            tracing::error!("cover generation failed: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, "cover generation failed").into_response()
        }
        Err(e) => {
            tracing::error!("cover generation task failed: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, "cover generation failed").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_cover_id_urls_to_requested_size() {
        assert_eq!(
            openlibrary_cover_url_for_size(
                "https://covers.openlibrary.org/b/id/12345-M.jpg",
                "L"
            )
            .as_deref(),
            Some("https://covers.openlibrary.org/b/id/12345-L.jpg?default=false")
        );
    }

    #[test]
    fn rewrites_cover_olid_urls_to_requested_size() {
        assert_eq!(
            openlibrary_cover_url_for_size(
                "https://covers.openlibrary.org/b/olid/OL123M-M.jpg",
                "S"
            )
            .as_deref(),
            Some("https://covers.openlibrary.org/b/olid/OL123M-S.jpg?default=false")
        );
    }
}
