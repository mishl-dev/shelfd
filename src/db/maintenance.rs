use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

use crate::models::{CacheCounts, CacheTtls};

use super::time::now_unix;

pub async fn cache_counts(pool: &SqlitePool) -> Result<CacheCounts> {
    Ok(CacheCounts {
        books: query_count(pool, "books").await?,
        links: query_count(pool, "links").await?,
        searches: query_count(pool, "searches").await?,
        explore_sources: query_count(pool, "explore_sources").await?,
    })
}

pub async fn prune_expired_cache(pool: &SqlitePool, ttls: &CacheTtls) -> Result<()> {
    let now = now_unix();
    let mut tx = pool.begin().await?;

    let deleted_books = sqlx::query("DELETE FROM books WHERE cached_at <= ?1")
        .bind(now - ttls.books_secs)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    let deleted_links = sqlx::query(
        "DELETE FROM links
         WHERE (failed = 0 AND cached_at <= ?1)
            OR (failed = 1 AND cached_at <= ?2)",
    )
    .bind(now - ttls.links_secs)
    .bind(now - ttls.link_failures_secs)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let deleted_searches = sqlx::query("DELETE FROM searches WHERE cached_at <= ?1")
        .bind(now - ttls.searches_secs)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    let deleted_explore_sources = sqlx::query("DELETE FROM explore_sources WHERE cached_at <= ?1")
        .bind(now - ttls.explore_secs)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    tx.commit().await?;

    info!(
        deleted_books,
        deleted_links, deleted_searches, deleted_explore_sources, "pruned expired cache rows"
    );
    Ok(())
}

async fn query_count(pool: &SqlitePool, table: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let count: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
    Ok(count.0)
}
