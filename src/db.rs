use anyhow::Result;
use sqlx::SqlitePool;
use tracing::{debug, info};

use crate::models::{
    BookEntry, CacheCounts, CacheTtls, CachedBook, CachedExploreEntries, CachedLink, ExploreEntry,
};

type CachedBookRow = (
    String,
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    i64,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type CachedLinkRow = (Option<String>, Option<String>, i64, Option<String>, i64);

/// Create tables if they don't exist.  Called once at startup.
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    info!("running sqlite migrations");
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS books (
            md5        TEXT    PRIMARY KEY,
            title      TEXT    NOT NULL,
            author     TEXT    NOT NULL,
            downloads  INTEGER NOT NULL DEFAULT 0,
            cover_url  TEXT,
            media_type TEXT,
            cover_checked_at INTEGER,
            cached_at  INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS links (
            md5           TEXT    PRIMARY KEY,
            download_url  TEXT,
            media_type    TEXT,
            failed        INTEGER NOT NULL DEFAULT 0,
            failure_reason TEXT,
            cached_at     INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    add_column_if_missing(pool, "ALTER TABLE books ADD COLUMN media_type TEXT").await?;
    add_column_if_missing(
        pool,
        "ALTER TABLE books ADD COLUMN cover_checked_at INTEGER",
    )
    .await?;
    add_column_if_missing(
        pool,
        "ALTER TABLE books ADD COLUMN first_publish_year INTEGER",
    )
    .await?;
    add_column_if_missing(pool, "ALTER TABLE books ADD COLUMN language TEXT").await?;
    add_column_if_missing(pool, "ALTER TABLE books ADD COLUMN subjects_json TEXT").await?;
    add_column_if_missing(pool, "ALTER TABLE books ADD COLUMN description TEXT").await?;
    add_column_if_missing(pool, "ALTER TABLE links ADD COLUMN media_type TEXT").await?;
    add_column_if_missing(
        pool,
        "ALTER TABLE links ADD COLUMN failed INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    add_column_if_missing(pool, "ALTER TABLE links ADD COLUMN failure_reason TEXT").await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS searches (
            query       TEXT    PRIMARY KEY,
            feed_xml    TEXT    NOT NULL,
            cached_at   INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS explore_sources (
            cache_key    TEXT    PRIMARY KEY,
            entries_json TEXT    NOT NULL,
            cached_at    INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_books_cached_at ON books(cached_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_links_cached_at ON links(cached_at)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_searches_cached_at ON searches(cached_at)")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_explore_sources_cached_at ON explore_sources(cached_at)",
    )
    .execute(pool)
    .await?;

    info!("sqlite migrations complete");
    Ok(())
}

/// Upsert a slice of books.  Uses a single transaction.
pub async fn upsert_books(pool: &SqlitePool, books: &[BookEntry]) -> Result<()> {
    let now = now_unix();
    debug!(
        count = books.len(),
        timestamp = now,
        "upserting books into cache"
    );
    let mut tx = pool.begin().await?;

    for b in books {
        let subjects_json = if b.subjects.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&b.subjects)?)
        };
        sqlx::query(
            "INSERT INTO books (md5, title, author, downloads, cover_url, media_type, cover_checked_at, cached_at, first_publish_year, language, subjects_json, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(md5) DO UPDATE SET
               title      = excluded.title,
               author     = excluded.author,
               downloads  = excluded.downloads,
               cover_url  = excluded.cover_url,
               media_type = COALESCE(excluded.media_type, books.media_type),
               cover_checked_at = COALESCE(excluded.cover_checked_at, books.cover_checked_at),
               cached_at  = excluded.cached_at,
               first_publish_year = COALESCE(excluded.first_publish_year, books.first_publish_year),
               language = COALESCE(excluded.language, books.language),
               subjects_json = COALESCE(excluded.subjects_json, books.subjects_json),
               description = COALESCE(excluded.description, books.description)",
        )
        .bind(&b.md5)
        .bind(&b.title)
        .bind(&b.author)
        .bind(b.downloads)
        .bind(&b.cover_url)
        .bind(&b.download_media_type)
        .bind(b.cover_checked_at)
        .bind(now)
        .bind(b.first_publish_year)
        .bind(&b.language)
        .bind(&subjects_json)
        .bind(&b.description)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    debug!(count = books.len(), "book upsert transaction committed");
    Ok(())
}

pub async fn get_cached_book(
    pool: &SqlitePool,
    md5: &str,
    min_cached_at: i64,
) -> Result<Option<CachedBook>> {
    let row: Option<CachedBookRow> = sqlx::query_as(
        "SELECT md5, title, author, downloads, cover_url, media_type, cover_checked_at, cached_at, first_publish_year, language, subjects_json, description
         FROM books
         WHERE md5 = ?1 AND cached_at > ?2",
    )
    .bind(md5)
    .bind(min_cached_at)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            md5,
            title,
            author,
            downloads,
            cover_url,
            download_media_type,
            cover_checked_at,
            cached_at,
            first_publish_year,
            language,
            subjects_json,
            description,
        )| CachedBook {
            entry: BookEntry {
                md5,
                title,
                author,
                downloads,
                cover_url,
                download_media_type,
                cover_checked_at,
                first_publish_year,
                language,
                subjects: subjects_json
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_default(),
                description,
            },
            cached_at,
        },
    ))
}

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

/// Returns a cached download URL for `md5`, if present and newer than `min_cached_at`.
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

/// Cache a resolved download URL.
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

async fn add_column_if_missing(pool: &SqlitePool, sql: &str) -> Result<()> {
    match sqlx::query(sql).execute(pool).await {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(error)) if error.message().contains("duplicate column name") => {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs() as i64
}

pub fn unix_now() -> i64 {
    now_unix()
}
