use std::sync::Arc;
use std::sync::atomic::Ordering;

use sqlx::SqlitePool;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
use tracing::warn;

use crate::db;
use crate::models::CacheTtls;
use crate::state::AppMetrics;

pub fn spawn_cache_cleanup(
    pool: Arc<SqlitePool>,
    metrics: Arc<AppMetrics>,
    cache_ttls: CacheTtls,
    cleanup_interval_secs: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(cleanup_interval_secs)).await;
            if let Err(error) = db::prune_expired_cache(&pool, &cache_ttls).await {
                warn!(error = %error, "background cache cleanup failed");
            } else {
                metrics
                    .last_cleanup_unix
                    .store(db::unix_now() as u64, Ordering::Relaxed);
            }
        }
    })
}
