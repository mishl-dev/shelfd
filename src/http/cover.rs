use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{instrument, info};

use crate::cover_gen::render_cover;
use crate::db;
use crate::opds;
use crate::state::AppState;

use crate::service::metadata::enrich_book_metadata;
use crate::service::inflight::{InflightRole, begin_inflight};

#[derive(Debug, Deserialize)]
pub struct CoverParams {
    #[serde(default)]
    pub size: Option<String>,
}

#[instrument(skip(state), fields(param = %param))]
pub async fn handle_cover(
    State(state): State<AppState>,
    Path(param): Path<String>,
    Query(params): Query<CoverParams>,
) -> Response {
    let size_suffix = match params.size.as_deref() {
        Some("small") => "S",
        Some("medium") => "M",
        Some("large") => "L",
        _ => "M",
    };

    // Direct Open Library cover ID — no fallback, just proxy.
    if param.chars().all(|c| c.is_ascii_digit()) {
        return fetch_ol_cover(&state, &param, size_suffix).await;
    }

    // md5 lookup — try to resolve an upstream cover, fall back to generated.
    let min_cached_at = db::unix_now() - state.book_cache_ttl_secs;
    let book = match db::get_cached_book(&state.pool, &param, min_cached_at).await {
        Ok(Some(cached)) => Some(cached.entry),
        _ => None,
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

    if cover_url.is_none() {
        let min_cover_checked_at = db::unix_now() - state.cover_negative_ttl_secs;
        let needs_enrichment = match db::get_cached_book(&state.pool, &param, 0).await {
            Ok(Some(cached)) => cached
                .entry
                .cover_checked_at
                .map(|t| t <= min_cover_checked_at)
                .unwrap_or(true),
            _ => true,
        };

        if needs_enrichment {
            let inflight = begin_inflight(state.cover_inflight.clone(), param.clone()).await;
            match inflight {
                InflightRole::Leader(guard) => {
                    cover_url = enrich_book_metadata(&state, &param).await;
                    drop(guard);
                }
                InflightRole::Waiter(notify) => {
                    info!(md5 = %param, "waiting for in-flight cover enrichment");
                    notify.notified().await;
                    // After enrichment, re-read book from DB to get updated cover_url
                    let book = match db::get_cached_book(&state.pool, &param, 0).await {
                        Ok(Some(cached)) => Some(cached.entry),
                        _ => None,
                    };
                    cover_url = book.as_ref().and_then(|b| b.cover_url.clone());
                }
            }
        }
    }

    let cover_id = cover_url.and_then(|url| opds::extract_cover_id(&url).map(|s| s.to_owned()));

    match cover_id {
        Some(id) => match fetch_ol_cover(&state, &id, size_suffix).await {
            resp if resp.status().is_success() => resp,
            _ => generated_cover(&title, &author),
        },
        None => generated_cover(&title, &author),
    }
}

async fn fetch_ol_cover(state: &AppState, cover_id: &str, size_suffix: &str) -> Response {
    let ol_url = format!("https://covers.openlibrary.org/b/id/{cover_id}-{size_suffix}.jpg");
    match state.http.get(&ol_url).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => {
                let content_type = resp
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("image/jpeg")
                    .to_owned();
                match resp.bytes().await {
                    Ok(body) => ([(header::CONTENT_TYPE, content_type)], body).into_response(),
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

fn generated_cover(title: &str, author: &str) -> Response {
    match tokio::task::block_in_place(|| render_cover(title, author)) {
        Ok(png) => ([(header::CONTENT_TYPE, "image/png")], png).into_response(),
        Err(e) => {
            tracing::error!("cover generation failed: {e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, "cover generation failed").into_response()
        }
    }
}
