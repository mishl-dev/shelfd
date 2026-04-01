use anyhow::Result;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tracing::debug;

use crate::models::{BookEntry, CachedBook};

use super::time::now_unix;

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

pub async fn upsert_books(pool: &SqlitePool, books: &[BookEntry]) -> Result<()> {
    if books.is_empty() {
        return Ok(());
    }

    let now = now_unix();
    debug!(
        count = books.len(),
        timestamp = now,
        "upserting books into cache"
    );
    let mut tx = pool.begin().await?;

    let prepared: Vec<_> = books
        .iter()
        .map(|book| {
            let subjects_json = if book.subjects.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&book.subjects).ok())
            };
            (
                &book.md5,
                &book.title,
                &book.author,
                book.downloads,
                &book.cover_url,
                &book.download_media_type,
                book.cover_checked_at,
                now,
                book.first_publish_year,
                &book.language,
                subjects_json,
                &book.description,
            )
        })
        .collect();

    let mut builder = QueryBuilder::<Sqlite>::new(
        "INSERT INTO books (md5, title, author, downloads, cover_url, media_type, cover_checked_at, cached_at, first_publish_year, language, subjects_json, description) ",
    );

    builder.push_values(&prepared, |mut b, row| {
        b.push_bind(row.0)
            .push_bind(row.1)
            .push_bind(row.2)
            .push_bind(row.3)
            .push_bind(row.4)
            .push_bind(row.5)
            .push_bind(row.6)
            .push_bind(row.7)
            .push_bind(row.8)
            .push_bind(row.9)
            .push_bind(&row.10)
            .push_bind(row.11);
    });

    builder.push(
        " ON CONFLICT(md5) DO UPDATE SET
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
    );

    builder.build().execute(&mut *tx).await?;

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

    Ok(row.map(cached_book_from_row))
}

pub async fn get_cached_books(
    pool: &SqlitePool,
    md5s: &[String],
    min_cached_at: i64,
) -> Result<Vec<CachedBook>> {
    if md5s.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT md5, title, author, downloads, cover_url, media_type, cover_checked_at, cached_at, first_publish_year, language, subjects_json, description
         FROM books
         WHERE cached_at > ",
    );
    builder.push_bind(min_cached_at);
    builder.push(" AND md5 IN (");
    {
        let mut separated = builder.separated(", ");
        for md5 in md5s {
            separated.push_bind(md5);
        }
    }
    builder.push(")");

    let rows: Vec<CachedBookRow> = builder.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(cached_book_from_row).collect())
}

fn cached_book_from_row(
    (
        md5,
        title,
        author,
        downloads,
        cover_url,
        download_media_type,
        cover_checked_at,
        _cached_at,
        first_publish_year,
        language,
        subjects_json,
        description,
    ): CachedBookRow,
) -> CachedBook {
    CachedBook {
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
    }
}
