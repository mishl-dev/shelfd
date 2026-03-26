use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use serde::Deserialize;
use tracing::instrument;

use crate::service::cover::resolve_cover_response;
use crate::state::AppState;

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

    resolve_cover_response(&state, &param, size_suffix).await
}
