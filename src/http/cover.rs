use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::instrument;

use crate::db;
use crate::opds;
use crate::state::AppState;

use crate::service::metadata::enrich_book_metadata;

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
        Some("large") => "L",
        _ => "M",
    };

    let cover_id = if param.chars().all(|c| c.is_ascii_digit()) {
        param
    } else {
        let min_cached_at = db::unix_now() - state.book_cache_ttl_secs;

        let mut cover_url = match db::get_cached_book(&state.pool, &param, min_cached_at).await {
            Ok(Some(cached)) => cached.entry.cover_url,
            _ => None,
        };

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
                cover_url = enrich_book_metadata(&state, &param).await;
            }
        }

        let Some(url) = cover_url else {
            return (StatusCode::NOT_FOUND, "no cover available").into_response();
        };

        let Some(id) = opds::extract_cover_id(&url) else {
            return (StatusCode::NOT_FOUND, "no cover available").into_response();
        };

        id.to_string()
    };

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
