use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_links_md5_failed ON links(md5, failed)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_books_md5_cached_at ON books(md5, cached_at)")
        .execute(pool)
        .await?;

    info!("sqlite migrations complete");
    Ok(())
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
