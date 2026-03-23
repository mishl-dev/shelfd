use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tracing::warn;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthPayload {
    pub status: &'static str,
}

pub async fn handle_health() -> Json<HealthPayload> {
    Json(HealthPayload { status: "ok" })
}

pub async fn handle_ready(State(state): State<AppState>) -> Response {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(state.pool.as_ref())
        .await
    {
        Ok(_) => Json(HealthPayload { status: "ok" }).into_response(),
        Err(error) => {
            warn!(error = %error, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthPayload {
                    status: "db_unavailable",
                }),
            )
                .into_response()
        }
    }
}
