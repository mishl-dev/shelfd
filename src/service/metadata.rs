use tracing::{debug, instrument, warn};

use crate::db;
use crate::models::{BookEntry, OlDoc, OlEnrichment, OlResponse};
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
    if enrichment.cover_url.is_some() {
        updated.cover_checked_at = Some(db::unix_now());
    }
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
    let limit = state.cover_lookup_limit.max(1);
    let fields = "cover_i,cover_edition_key,subject,first_publish_year,language,editions,editions.key,editions.cover_i";

    let first_author = book.author.split(';').next().unwrap_or(&book.author).trim();

    let attempts: Vec<(String, String)> = vec![
        (
            book.author.clone(),
            format!(
                "{}/search.json?title={}&author={}&limit={}&fields={fields}",
                state.metadata_base_url,
                urlencoding::encode(&book.title),
                urlencoding::encode(&book.author),
                limit,
            ),
        ),
        (
            first_author.to_owned(),
            format!(
                "{}/search.json?title={}&author={}&limit={}&fields={fields}",
                state.metadata_base_url,
                urlencoding::encode(&book.title),
                urlencoding::encode(first_author),
                limit,
            ),
        ),
        (
            String::new(),
            format!(
                "{}/search.json?title={}&limit={}&fields={fields}",
                state.metadata_base_url,
                urlencoding::encode(&book.title),
                limit,
            ),
        ),
    ];

    for (author_label, url) in &attempts {
        debug!(title = %book.title, author = %author_label, "querying metadata provider");
        match get_json_with_retry::<OlResponse>(state, url, "metadata lookup").await {
            Ok(body) => {
                if body.docs.is_empty() {
                    debug!(title = %book.title, author = %author_label, "no results, trying next fallback");
                    continue;
                }
                let doc = &body.docs[0];
                let cover_url = select_cover_url(&body.docs);
                if let Some(url) = cover_url.as_deref() {
                    debug!(title = %book.title, %url, "cover found");
                }
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
                return OlEnrichment {
                    cover_url,
                    first_publish_year: doc.first_publish_year,
                    language,
                    subjects,
                };
            }
            Err(error) => {
                warn!(title = %book.title, author = %author_label, error = %error, "failed to fetch Open Library response");
            }
        }
    }

    OlEnrichment::default()
}

fn select_cover_url(docs: &[OlDoc]) -> Option<String> {
    docs.iter().find_map(cover_url_for_doc)
}

fn cover_url_for_doc(doc: &OlDoc) -> Option<String> {
    if let Some(cover_i) = doc.cover_i {
        return Some(format!("https://covers.openlibrary.org/b/id/{cover_i}-M.jpg"));
    }

    if let Some(cover_edition_key) = doc.cover_edition_key.as_deref() {
        return Some(format!(
            "https://covers.openlibrary.org/b/olid/{cover_edition_key}-M.jpg"
        ));
    }

    doc.editions.as_ref().and_then(|editions| {
        editions.docs.iter().find_map(|edition| {
            edition
                .cover_i
                .map(|cover_i| format!("https://covers.openlibrary.org/b/id/{cover_i}-M.jpg"))
                .or_else(|| {
                    edition.key.as_deref().map(|key| {
                        let olid = key.trim_start_matches("/books/");
                        format!("https://covers.openlibrary.org/b/olid/{olid}-M.jpg")
                    })
                })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_first_work_level_cover() {
        let body: OlResponse = serde_json::from_str(
            r#"{
                "docs": [
                    { "cover_i": 12345, "first_publish_year": 1965 },
                    { "cover_i": 99999 }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(
            select_cover_url(&body.docs).as_deref(),
            Some("https://covers.openlibrary.org/b/id/12345-M.jpg")
        );
    }

    #[test]
    fn falls_back_to_later_doc_with_cover() {
        let body: OlResponse = serde_json::from_str(
            r#"{
                "docs": [
                    { "first_publish_year": 1965 },
                    { "cover_i": 67890 }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(
            select_cover_url(&body.docs).as_deref(),
            Some("https://covers.openlibrary.org/b/id/67890-M.jpg")
        );
    }

    #[test]
    fn falls_back_to_edition_level_cover_data() {
        let body: OlResponse = serde_json::from_str(
            r#"{
                "docs": [
                    {
                        "editions": {
                            "docs": [
                                { "key": "/books/OL123M", "cover_i": 24680 }
                            ]
                        }
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(
            select_cover_url(&body.docs).as_deref(),
            Some("https://covers.openlibrary.org/b/id/24680-M.jpg")
        );
    }

    #[test]
    fn falls_back_to_cover_edition_key() {
        let body: OlResponse = serde_json::from_str(
            r#"{
                "docs": [
                    { "cover_edition_key": "OL999M" }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(
            select_cover_url(&body.docs).as_deref(),
            Some("https://covers.openlibrary.org/b/olid/OL999M-M.jpg")
        );
    }
}
