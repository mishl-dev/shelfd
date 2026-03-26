use axum::Router;
use tower_http::trace::TraceLayer;

use crate::http::cover::handle_cover;
use crate::http::download::handle_download;
use crate::http::explore::{handle_explore_root, handle_explore_subject, handle_explore_top};
use crate::http::health::{handle_health, handle_ready};
use crate::http::metrics::handle_metrics;
use crate::http::search::handle_search;
use crate::opds;
use crate::state::AppState;

use axum::routing::get;

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handle_health))
        .route("/readyz", get(handle_ready))
        .route("/metrics", get(handle_metrics))
        .route("/opds", get(opds::root_feed))
        .route("/opds/explore", get(handle_explore_root))
        .route("/opds/explore/top", get(handle_explore_top))
        .route(
            "/opds/explore/subject/{subject}",
            get(handle_explore_subject),
        )
        .route("/opds/opensearch.xml", get(opds::open_search))
        .route("/opds/search", get(handle_search))
        .route("/opds/cover/{md5}", get(handle_cover))
        .route("/opds/download/{md5}", get(handle_download))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
