mod feeds;
mod links;
mod xml;

use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

pub use feeds::{PaginationPaths, explore_feed, explore_root_feed, search_feed};

pub async fn root_feed(State(state): State<AppState>) -> Response {
    let xml = feeds::root_navigation_feed(
        state.public_base_url.as_deref(),
        "/opds",
        state.explore_subjects.as_slice(),
        &state.app_name,
        &state.archive_name,
    );
    atom_response(xml)
}

pub async fn open_search(State(state): State<AppState>) -> Response {
    let xml = feeds::build_open_search_description(
        state.public_base_url.as_deref(),
        &state.app_name,
        &state.archive_name,
    );
    (
        [
            (
                header::CONTENT_TYPE,
                "application/opensearchdescription+xml; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        xml,
    )
        .into_response()
}

fn atom_response(xml: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
            (
                header::CACHE_CONTROL,
                "public, max-age=1800, stale-while-revalidate=21600",
            ),
        ],
        xml,
    )
        .into_response()
}
