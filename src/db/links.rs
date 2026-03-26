use anyhow::Result;
use sqlx::SqlitePool;
use tracing::debug;

use crate::models::CachedLink;

use super::time::now_unix;

type CachedLinkRow = (Option<String>, Option<String>, i64, Option<String>, i64);

pub async fn get_cached_link(
    pool: &SqlitePool,
    md5: &str,
    success_min_cached_at: i64,
    failure_min_cached_at: i64,
) -> Result<Option<CachedLink>> {
    let row: Option<CachedLinkRow> = sqlx::query_as(
        "SELECT download_url, media_type, failed, failure_reason, cached_at
         FROM links
         WHERE md5 = ?1
           AND (
             (failed = 0 AND cached_at > ?2) OR
             (failed = 1 AND cached_at > ?3)
           )",
    )
    .bind(md5)
    .bind(success_min_cached_at)
    .bind(failure_min_cached_at)
    .fetch_optional(pool)
    .await?;

    debug!(%md5, hit = row.is_some(), "checked cached download link");
    Ok(row.map(
        |(download_url, media_type, failed, failure_reason, cached_at)| CachedLink {
            download_url,
            media_type,
            failed: failed != 0,
            failure_reason,
            cached_at,
        },
    ))
}

pub async fn cache_link_success(
    pool: &SqlitePool,
    md5: &str,
    url: &str,
    media_type: Option<&str>,
) -> Result<()> {
    debug!(%md5, %url, media_type, "caching resolved download link");
    sqlx::query(
        "INSERT INTO links (md5, download_url, media_type, failed, failure_reason, cached_at)
         VALUES (?1, ?2, ?3, 0, NULL, ?4)
         ON CONFLICT(md5) DO UPDATE SET
           download_url = excluded.download_url,
           media_type = excluded.media_type,
           failed = 0,
           failure_reason = NULL,
           cached_at    = excluded.cached_at",
    )
    .bind(md5)
    .bind(url)
    .bind(media_type)
    .bind(now_unix())
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE books
         SET media_type = COALESCE(?2, media_type)
         WHERE md5 = ?1",
    )
    .bind(md5)
    .bind(media_type)
    .execute(pool)
    .await?;

    debug!(%md5, "download link cached");
    Ok(())
}

pub async fn cache_link_failure(pool: &SqlitePool, md5: &str, reason: &str) -> Result<()> {
    debug!(%md5, %reason, "caching failed download resolution");
    sqlx::query(
        "INSERT INTO links (md5, download_url, media_type, failed, failure_reason, cached_at)
         VALUES (?1, NULL, NULL, 1, ?2, ?3)
         ON CONFLICT(md5) DO UPDATE SET
           download_url = NULL,
           media_type = NULL,
           failed = 1,
           failure_reason = excluded.failure_reason,
           cached_at = excluded.cached_at",
    )
    .bind(md5)
    .bind(reason)
    .bind(now_unix())
    .execute(pool)
    .await?;
    Ok(())
}
