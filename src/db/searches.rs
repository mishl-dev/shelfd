use anyhow::Result;
use sqlx::SqlitePool;
use tracing::debug;

use super::time::now_unix;

pub async fn get_cached_search(
    pool: &SqlitePool,
    query: &str,
    min_cached_at: i64,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT feed_xml
         FROM searches
         WHERE query = ?1 AND cached_at > ?2",
    )
    .bind(query)
    .bind(min_cached_at)
    .fetch_optional(pool)
    .await?;

    debug!(query, hit = row.is_some(), "checked cached search feed");
    Ok(row.map(|(feed_xml,)| feed_xml))
}

pub async fn cache_search(pool: &SqlitePool, query: &str, feed_xml: &str) -> Result<()> {
    let now = now_unix();
    debug!(query, "caching rendered search feed");
    sqlx::query(
        "INSERT INTO searches (query, feed_xml, cached_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(query) DO UPDATE SET
           feed_xml = excluded.feed_xml,
           cached_at = excluded.cached_at",
    )
    .bind(query)
    .bind(feed_xml)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}
