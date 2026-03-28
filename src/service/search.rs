use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use futures::stream::{self, StreamExt};
use tracing::{debug, info, instrument, warn};

use crate::models::BookEntry;
use crate::state::AppState;
use crate::{db, opds, scraper};

use super::inflight::{InflightRole, begin_inflight};
use super::metadata::enrich_book_metadata;
use super::retry::{get_json_with_retry, get_text_raced, log_sanitized_html};

struct ResolvedBook {
    entry: BookEntry,
}

pub async fn do_search(state: &AppState, query: &str, page: usize) -> anyhow::Result<String> {
    let normalized_query = normalize_query(query);
    let page = page.max(1);
    let cache_key = search_cache_key(&normalized_query, page);
    let search_cache_cutoff = db::unix_now() - state.search_cache_ttl_secs;
    if let Some(xml) = db::get_cached_search(&state.pool, &cache_key, search_cache_cutoff).await? {
        state
            .metrics
            .search_cache_hits
            .fetch_add(1, Ordering::Relaxed);
        info!(query = %normalized_query, page, "serving cached search feed");
        return Ok(xml);
    }

    let search_key = cache_key.clone();
    let inflight = begin_inflight(state.search_inflight.clone(), search_key.clone()).await;
    let _guard = match inflight {
        InflightRole::Leader(guard) => guard,
        InflightRole::Waiter(notify) => {
            info!(query = %search_key, "waiting for in-flight search");
            notify.notified().await;
            if let Some(xml) =
                db::get_cached_search(&state.pool, &search_key, search_cache_cutoff).await?
            {
                state
                    .metrics
                    .search_cache_hits
                    .fetch_add(1, Ordering::Relaxed);
                info!(query = %search_key, "serving cached search feed after wait");
                return Ok(xml);
            }
            begin_inflight(state.search_inflight.clone(), search_key.clone())
                .await
                .into_leader()?
        }
    };

    info!(query = %normalized_query, page, "starting archive search");
    let search_path = format!(
        "/search?q={}&page={}",
        urlencoding::encode(&normalized_query),
        page
    );
    let html = get_text_raced(state, &search_path, "archive search").await?;
    log_sanitized_html("search results", &html);

    if scraper::has_search_error(&html) {
        warn!(query = %normalized_query, page, "archive returned search error (page limit?)");
        let feed_xml = opds::search_feed(
            &normalized_query,
            &[],
            page,
            false,
            state.public_base_url.as_deref(),
            &state.app_name,
            &state.archive_name,
            &state.archive_base,
        );
        db::cache_search(&state.pool, &cache_key, &feed_xml).await?;
        return Ok(feed_xml);
    }

    let mut raw = scraper::parse_search_results(&html);
    let has_more = raw.len() > state.search_result_limit;
    raw.truncate(state.search_result_limit);
    let raw = dedup_raw_entries(raw);
    state
        .metrics
        .search_result_books_seen
        .fetch_add(raw.len() as u64, Ordering::Relaxed);

    info!(query = %normalized_query, page, results = raw.len(), has_more, "parsed search results");

    debug!(
        results = raw.len(),
        inline_info_concurrency = state.inline_info_concurrency,
        "fetching inline info with bounded concurrency"
    );
    let raw_md5s: Vec<_> = raw.iter().map(|entry| entry.md5.clone()).collect();
    let cached_books =
        db::get_cached_books(&state.pool, &raw_md5s, db::unix_now() - state.book_cache_ttl_secs)
            .await?;
    let cached_by_md5: HashMap<_, _> = cached_books
        .into_iter()
        .map(|cached| (cached.entry.md5.clone(), cached.entry))
        .collect();
    state.metrics.search_book_cache_hits.fetch_add(
        cached_by_md5.len().min(raw_md5s.len()) as u64,
        Ordering::Relaxed,
    );
    state.metrics.search_book_cache_misses.fetch_add(
        raw_md5s.len().saturating_sub(cached_by_md5.len()) as u64,
        Ordering::Relaxed,
    );

    let resolved: Vec<_> = stream::iter(raw.into_iter())
        .map(|entry| resolve_book_entry(state, entry, &cached_by_md5))
        .buffer_unordered(state.inline_info_concurrency.max(1))
        .collect()
        .await;
    let mut books: Vec<_> = resolved.into_iter().map(|book| book.entry).collect();

    sort_books_for_query(&normalized_query, &mut books);

    db::upsert_books(&state.pool, &books).await?;
    let feed_xml = opds::search_feed(
        &normalized_query,
        &books,
        page,
        has_more,
        state.public_base_url.as_deref(),
        &state.app_name,
        &state.archive_name,
        &state.archive_base,
    );
    db::cache_search(&state.pool, &cache_key, &feed_xml).await?;
    prewarm_related_assets(state.clone(), books.clone());
    info!(query = %normalized_query, page, books = books.len(), "search flow completed");

    Ok(feed_xml)
}

fn search_cache_key(query: &str, page: usize) -> String {
    if page <= 1 {
        query.to_owned()
    } else {
        format!("{query}|page:{page}")
    }
}

#[instrument(skip(state, entry, cached_by_md5), fields(md5 = %entry.md5, title = %entry.title))]
async fn resolve_book_entry(
    state: &AppState,
    entry: scraper::RawEntry,
    cached_by_md5: &HashMap<String, BookEntry>,
) -> ResolvedBook {
    if let Some(cached) = cached_by_md5.get(&entry.md5) {
        debug!(md5 = %entry.md5, "using cached book metadata");
        return ResolvedBook {
            entry: cached.clone(),
        };
    }

    let url = format!("{}/dyn/md5/inline_info/{}", state.next_archive_base(), entry.md5);
    let downloads = fetch_downloads(state, &url).await;
    debug!(downloads, "inline info resolved");

    ResolvedBook {
        entry: BookEntry {
            md5: entry.md5,
            title: entry.title,
            author: entry.author,
            downloads,
            cover_url: None,
            download_media_type: None,
            cover_checked_at: None,
            first_publish_year: None,
            language: None,
            subjects: Vec::new(),
            description: None,
        },
    }
}

fn prewarm_related_assets(state: AppState, books: Vec<BookEntry>) {
    let prewarm_count = state.search_prewarm_count;
    if prewarm_count == 0 {
        return;
    }
    let prewarm_books: Vec<_> = books.into_iter().take(prewarm_count).collect();
    if prewarm_books.is_empty() {
        return;
    }

    tokio::spawn(async move {
        state
            .metrics
            .cover_prewarm_jobs_started
            .fetch_add(1, Ordering::Relaxed);
        let mut seen = HashSet::new();
        for book in prewarm_books {
            if !seen.insert(book.md5.clone()) {
                continue;
            }

            if book.cover_url.is_none() {
                state
                    .metrics
                    .cover_prewarm_attempts
                    .fetch_add(1, Ordering::Relaxed);
                if enrich_book_metadata(&state, &book.md5).await.is_some() {
                    state
                        .metrics
                        .cover_prewarm_hits
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        state
            .metrics
            .cover_prewarm_jobs_completed
            .fetch_add(1, Ordering::Relaxed);
    });
}

fn dedup_raw_entries(entries: Vec<scraper::RawEntry>) -> Vec<scraper::RawEntry> {
    let mut seen = HashSet::new();
    let mut unique = Vec::with_capacity(entries.len());
    for entry in entries {
        if seen.insert(entry.md5.clone()) {
            unique.push(entry);
        }
    }
    unique
}

#[instrument(skip(state), fields(url))]
async fn fetch_downloads(state: &AppState, url: &str) -> i64 {
    use crate::models::InlineInfo;
    state
        .metrics
        .search_inline_info_requests
        .fetch_add(1, Ordering::Relaxed);

    match get_json_with_retry::<InlineInfo>(state, url, "inline info").await {
        Ok(body) => {
            let downloads = body.downloads_total.unwrap_or(0);
            debug!(downloads, "parsed inline downloads");
            downloads
        }
        Err(error) => {
            state
                .metrics
                .search_inline_info_failures
                .fetch_add(1, Ordering::Relaxed);
            warn!(error = %error, "failed to fetch inline info");
            0
        }
    }
}

pub fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn sort_books_for_query(query: &str, books: &mut [BookEntry]) {
    let normalized_query = fold_text(query);
    let terms = query_terms(&normalized_query);
    books.sort_unstable_by(|a, b| {
        b.downloads
            .cmp(&a.downloads)
            .then_with(|| {
                book_rank_key(&normalized_query, &terms, b).cmp(&book_rank_key(
                    &normalized_query,
                    &terms,
                    a,
                ))
            })
            .then_with(|| a.title.cmp(&b.title))
    });
}

fn book_rank_key(
    normalized_query: &str,
    query_terms: &[String],
    book: &BookEntry,
) -> (i32, i32, i32, i32) {
    let normalized_title = fold_text(&book.title);
    let normalized_author = fold_text(&book.author);

    let exact_title =
        i32::from(normalized_title == normalized_query && !normalized_query.is_empty());
    let title_contains_full_query =
        i32::from(!normalized_query.is_empty() && normalized_title.contains(normalized_query));
    let title_term_hits = query_terms
        .iter()
        .filter(|term| normalized_title.contains(term.as_str()))
        .count() as i32;
    let author_term_hits = query_terms
        .iter()
        .filter(|term| normalized_author.contains(term.as_str()))
        .count() as i32;

    (
        exact_title,
        title_contains_full_query,
        title_term_hits,
        author_term_hits,
    )
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|term| term.len() >= 2)
        .map(str::to_owned)
        .collect()
}

fn fold_text(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::dedup_raw_entries;
    use crate::scraper::RawEntry;

    #[test]
    fn dedup_raw_entries_retains_first_entry_for_each_md5() {
        let entries = vec![
            RawEntry {
                md5: "a".into(),
                title: "First".into(),
                author: "Author".into(),
            },
            RawEntry {
                md5: "b".into(),
                title: "Second".into(),
                author: "Writer".into(),
            },
            RawEntry {
                md5: "a".into(),
                title: "Duplicate".into(),
                author: "Other".into(),
            },
            RawEntry {
                md5: "c".into(),
                title: "Third".into(),
                author: "Author".into(),
            },
        ];

        let deduped = dedup_raw_entries(entries);

        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].md5, "a");
        assert_eq!(deduped[1].md5, "b");
        assert_eq!(deduped[2].md5, "c");
    }
}
