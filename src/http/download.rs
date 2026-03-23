use std::sync::atomic::Ordering;

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::instrument;

use crate::state::AppState;

use crate::service::download::resolve_download;

#[instrument(skip(state), fields(md5 = %md5))]
pub async fn handle_download(State(state): State<AppState>, Path(md5): Path<String>) -> Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .downloads_total
        .fetch_add(1, Ordering::Relaxed);
    match resolve_download(&state, &md5).await {
        Ok(url) => (StatusCode::FOUND, [(header::LOCATION, url)]).into_response(),
        Err(e) => {
            tracing::error!("download resolve failed: {e:#}");
            (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
        }
    }
}
