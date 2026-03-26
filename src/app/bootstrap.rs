use anyhow::{Context, Result};
use reqwest::Client;
use sqlx::Executor;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::time::Duration;

pub fn build_http_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("shelfd/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(300))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(16)
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .context("failed to build shared reqwest client")
}

pub async fn build_sqlite_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .connect(database_url)
        .await?;

    pool.execute("PRAGMA journal_mode = WAL").await?;
    pool.execute("PRAGMA synchronous = NORMAL").await?;
    pool.execute("PRAGMA busy_timeout = 5000").await?;
    pool.execute("PRAGMA temp_store = MEMORY").await?;

    Ok(pool)
}
