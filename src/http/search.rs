use std::sync::atomic::Ordering;

use axum::{
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::instrument;

use crate::state::AppState;

use crate::service::search::do_search;

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub page: Option<usize>,
}

#[instrument(skip(state, params), fields(q = %params.q))]
pub async fn handle_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state.metrics.searches_total.fetch_add(1, Ordering::Relaxed);
    let page = params.page.unwrap_or(1).max(1);
    match do_search(&state, &params.q, page).await {
        Ok(xml) => (
            [
                (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=300, stale-while-revalidate=1800",
                ),
            ],
            xml,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("search failed: {e:#}");
            (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
        }
    }
}
