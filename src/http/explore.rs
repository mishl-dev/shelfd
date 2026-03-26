use axum::{
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::opds;
use crate::state::AppState;

use crate::service::explore::{
    explore_pagination_paths, explore_subject_name, fetch_subject_entries,
    fetch_top_explore_entries, paginate_entries,
};

#[derive(Debug, Deserialize)]
pub struct ExploreParams {
    pub page: Option<usize>,
}

pub async fn handle_explore_root(State(state): State<AppState>) -> Response {
    use std::sync::atomic::Ordering;

    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    (
        [
            (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
            (
                header::CACHE_CONTROL,
                "public, max-age=1800, stale-while-revalidate=21600",
            ),
        ],
        opds::explore_root_feed(
            state.public_base_url.as_deref(),
            "/opds/explore",
            state.explore_subjects.as_slice(),
            &state.app_name,
            &state.archive_name,
        ),
    )
        .into_response()
}

pub async fn handle_explore_top(
    State(state): State<AppState>,
    Query(params): Query<ExploreParams>,
) -> Response {
    use std::sync::atomic::Ordering;

    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    let page = params.page.unwrap_or(1).max(1);
    match fetch_top_explore_entries(&state).await {
        Ok(entries) => {
            let page_entries = paginate_entries(&entries, page, state.explore_page_size);
            (
                [
                    (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=1800, stale-while-revalidate=21600",
                    ),
                ],
                opds::explore_feed(
                    "Popular Top 250",
                    &format!("urn:{}:explore:top", state.app_name),
                    &page_entries,
                    &explore_pagination_paths(
                        "/opds/explore/top",
                        page,
                        state.explore_page_size,
                        entries.len(),
                    ),
                    state.public_base_url.as_deref(),
                    &state.app_name,
                ),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!("explore top failed: {error:#}");
            (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
        }
    }
}

pub async fn handle_explore_subject(
    State(state): State<AppState>,
    axum::extract::Path(subject): axum::extract::Path<String>,
    Query(params): Query<ExploreParams>,
) -> Response {
    use std::sync::atomic::Ordering;

    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    let Some(subject_name) = explore_subject_name(&state, &subject) else {
        return (StatusCode::NOT_FOUND, "unknown subject").into_response();
    };
    let page = params.page.unwrap_or(1).max(1);

    let fetch_limit = (page * state.explore_page_size).clamp(state.explore_page_size, 250);
    match fetch_subject_entries(&state, &subject, fetch_limit).await {
        Ok(entries) => {
            let page_entries = paginate_entries(&entries, page, state.explore_page_size);
            (
                [
                    (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=1800, stale-while-revalidate=21600",
                    ),
                ],
                opds::explore_feed(
                    &format!("Explore: {subject_name}"),
                    &format!("urn:{}:explore:subject:{subject}", state.app_name),
                    &page_entries,
                    &explore_pagination_paths(
                        &format!("/opds/explore/subject/{subject}"),
                        page,
                        state.explore_page_size,
                        entries.len(),
                    ),
                    state.public_base_url.as_deref(),
                    &state.app_name,
                ),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!("explore subject failed: {error:#}");
            (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
        }
    }
}
