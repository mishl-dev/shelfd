use tracing::{debug, instrument, warn};

use crate::db;
use crate::models::{BookEntry, OlEnrichment, OlResponse};
use crate::state::AppState;

use super::retry::get_json_with_retry;

pub async fn enrich_book_metadata(state: &AppState, md5: &str) -> Option<String> {
    let min_cached_at = db::unix_now() - state.book_cache_ttl_secs;
    let book = match db::get_cached_book(&state.pool, md5, min_cached_at).await {
        Ok(Some(cached)) => cached.entry,
        _ => return None,
    };

    let enrichment = fetch_ol_metadata(state, &book).await;
    let mut updated = book;
    updated.cover_url = enrichment.cover_url.clone();
    updated.cover_checked_at = Some(db::unix_now());
    if updated.first_publish_year.is_none() {
        updated.first_publish_year = enrichment.first_publish_year;
    }
    if updated.language.is_none() {
        updated.language = enrichment.language;
    }
    if updated.subjects.is_empty() {
        updated.subjects = enrichment.subjects;
    }
    let _ = db::upsert_books(&state.pool, &[updated]).await;
    enrichment.cover_url
}

#[instrument(skip(state, book), fields(title = %book.title, author = %book.author))]
async fn fetch_ol_metadata(state: &AppState, book: &BookEntry) -> OlEnrichment {
    let url = format!(
        "{}/search.json?title={}&author={}&limit=1",
        state.metadata_base_url,
        urlencoding::encode(&book.title),
        urlencoding::encode(&book.author),
    );
    debug!(title = %book.title, author = %book.author, "querying metadata provider");
    match get_json_with_retry::<OlResponse>(state, &url, "metadata lookup").await {
        Ok(body) => {
            let Some(doc) = body.docs.first() else {
                return OlEnrichment::default();
            };
            let cover_url = doc.cover_i.map(|cover_i| {
                let url = format!("https://covers.openlibrary.org/b/id/{cover_i}-M.jpg");
                debug!(title = %book.title, %url, "cover found");
                url
            });
            let language = doc
                .language
                .as_ref()
                .and_then(|langs| langs.first().cloned());
            let subjects: Vec<String> = doc
                .subject
                .as_ref()
                .map(|s| s.iter().take(5).cloned().collect())
                .unwrap_or_default();
            debug!(
                title = %book.title,
                year = doc.first_publish_year,
                lang = language.as_deref().unwrap_or(""),
                subject_count = subjects.len(),
                "metadata found"
            );
            OlEnrichment {
                cover_url,
                first_publish_year: doc.first_publish_year,
                language,
                subjects,
            }
        }
        Err(error) => {
            warn!(title = %book.title, error = %error, "failed to fetch Open Library response");
            OlEnrichment::default()
        }
    }
}
