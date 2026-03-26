use anyhow::Result;
use sqlx::SqlitePool;

use crate::models::{CachedExploreEntries, ExploreEntry};

use super::time::now_unix;

pub async fn get_cached_explore_entries(
    pool: &SqlitePool,
    cache_key: &str,
    min_cached_at: i64,
) -> Result<Option<Vec<ExploreEntry>>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT entries_json
         FROM explore_sources
         WHERE cache_key = ?1 AND cached_at > ?2",
    )
    .bind(cache_key)
    .bind(min_cached_at)
    .fetch_optional(pool)
    .await?;

    row.map(|(json,)| {
        let cached: CachedExploreEntries = serde_json::from_str(&json)?;
        Ok(cached.entries)
    })
    .transpose()
}

pub async fn cache_explore_entries(
    pool: &SqlitePool,
    cache_key: &str,
    entries: &[ExploreEntry],
) -> Result<()> {
    let payload = serde_json::to_string(&CachedExploreEntries {
        entries: entries.to_vec(),
    })?;

    sqlx::query(
        "INSERT INTO explore_sources (cache_key, entries_json, cached_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(cache_key) DO UPDATE SET
           entries_json = excluded.entries_json,
           cached_at = excluded.cached_at",
    )
    .bind(cache_key)
    .bind(payload)
    .bind(now_unix())
    .execute(pool)
    .await?;
    Ok(())
}
