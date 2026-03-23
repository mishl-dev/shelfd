use std::sync::atomic::Ordering;

use futures::stream::{self, StreamExt};
use tracing::{debug, info, instrument, warn};

use crate::models::BookEntry;
use crate::state::AppState;
use crate::{db, opds, scraper};

use super::inflight::{InflightRole, begin_inflight};
use super::retry::{get_json_with_retry, get_text_with_retry, log_sanitized_html};

struct ResolvedBook {
    entry: BookEntry,
}

pub async fn do_search(state: &AppState, query: &str, page: usize) -> anyhow::Result<String> {
    let normalized_query = normalize_query(query);
    let page = page.max(1);
    let cache_key = search_cache_key(&normalized_query, page);
    let search_cache_cutoff = db::unix_now() - state.search_cache_ttl_secs;
    if let Some(xml) =
        db::get_cached_search(&state.pool, &cache_key, search_cache_cutoff).await?
    {
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

    let search_url = format!(
        "{}/search?q={}&page={}",
        state.archive_base,
        urlencoding::encode(&normalized_query),
        page
    );

    info!(query = %normalized_query, page, %search_url, "starting archive search");
    let html = get_text_with_retry(state, &search_url, "archive search").await?;
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

    info!(query = %normalized_query, page, results = raw.len(), has_more, "parsed search results");

    debug!(
        results = raw.len(),
        inline_info_concurrency = state.inline_info_concurrency,
        "fetching inline info with bounded concurrency"
    );
    let resolved: Vec<_> = stream::iter(raw.into_iter())
        .map(|entry| resolve_book_entry(state, entry))
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

#[instrument(skip(state, entry), fields(md5 = %entry.md5, title = %entry.title))]
async fn resolve_book_entry(state: &AppState, entry: scraper::RawEntry) -> ResolvedBook {
    let min_cached_at = db::unix_now() - state.book_cache_ttl_secs;
    if let Ok(Some(cached)) = db::get_cached_book(&state.pool, &entry.md5, min_cached_at).await {
        debug!(md5 = %entry.md5, cached_at = cached.cached_at, "using cached book metadata");
        return ResolvedBook {
            entry: cached.entry,
        };
    }

    let url = format!("{}/dyn/md5/inline_info/{}", state.archive_base, entry.md5);
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

#[instrument(skip(state), fields(url))]
async fn fetch_downloads(state: &AppState, url: &str) -> i64 {
    use crate::models::InlineInfo;

    match get_json_with_retry::<InlineInfo>(state, url, "inline info").await {
        Ok(body) => {
            let downloads = body.downloads_total.unwrap_or(0);
            debug!(downloads, "parsed inline downloads");
            downloads
        }
        Err(error) => {
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
            .then_with(|| book_rank_key(&normalized_query, &terms, b).cmp(&book_rank_key(&normalized_query, &terms, a)))
            .then_with(|| a.title.cmp(&b.title))
    });
}

fn book_rank_key(normalized_query: &str, query_terms: &[String], book: &BookEntry) -> (i32, i32, i32, i32) {
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
