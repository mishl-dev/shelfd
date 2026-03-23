use std::collections::HashMap;
use std::sync::atomic::Ordering;

use futures::stream::{self, StreamExt};

use crate::db;
use crate::models::{ExploreEntry, OlSubjectResponse};
use crate::opds;
use crate::state::AppState;

use super::retry::get_json_with_retry;

pub fn explore_subject_name<'a>(state: &'a AppState, subject: &str) -> Option<&'a str> {
    state
        .subject_name_by_slug
        .get(subject)
        .map(|s| s.as_str())
}

pub async fn fetch_top_explore_entries(state: &AppState) -> anyhow::Result<Vec<ExploreEntry>> {
    let subjects: Vec<String> = state
        .explore_subjects
        .iter()
        .map(|subject| subject.slug.clone())
        .collect();
    let subject_results: Vec<_> = stream::iter(subjects.into_iter())
        .map(|subject| async move { fetch_subject_entries(state, &subject, 50).await })
        .buffer_unordered(3)
        .collect()
        .await;

    let mut by_id = HashMap::<String, ExploreEntry>::new();
    for result in subject_results {
        for entry in result? {
            by_id.entry(entry.id.clone()).or_insert(entry);
        }
    }

    let mut entries: Vec<_> = by_id.into_values().collect();
    entries.sort_unstable_by(|a, b| {
        b.popularity
            .cmp(&a.popularity)
            .then_with(|| a.title.cmp(&b.title))
    });
    entries.truncate(250);
    Ok(entries)
}

pub async fn fetch_subject_entries(
    state: &AppState,
    subject: &str,
    limit: usize,
) -> anyhow::Result<Vec<ExploreEntry>> {
    let cache_key = format!("subject:{subject}:limit:{limit}");
    let min_cached_at = db::unix_now() - state.explore_cache_ttl_secs;
    if let Some(entries) =
        db::get_cached_explore_entries(&state.pool, &cache_key, min_cached_at).await?
    {
        state
            .metrics
            .explore_cache_hits
            .fetch_add(1, Ordering::Relaxed);
        return Ok(entries);
    }

    let url = format!(
        "{}/subjects/{subject}.json?limit={limit}&details=false",
        state.metadata_base_url
    );
    let body =
        get_json_with_retry::<OlSubjectResponse>(state, &url, "metadata subjects").await?;
    let entries: Vec<_> = body
        .works
        .into_iter()
        .map(|work| subject_work_to_explore_entry(work, &state.metadata_base_url))
        .collect();
    db::cache_explore_entries(&state.pool, &cache_key, &entries).await?;
    Ok(entries)
}

fn subject_work_to_explore_entry(
    work: crate::models::OlSubjectWork,
    metadata_base_url: &str,
) -> ExploreEntry {
    let author = work
        .authors
        .first()
        .map(|author| author.name.clone())
        .unwrap_or_else(|| "Unknown Author".to_owned());
    let cover_url = work
        .cover_id
        .map(|cover_id| format!("https://covers.openlibrary.org/b/id/{cover_id}-M.jpg"));
    let edition_count = work.edition_count.unwrap_or(0);
    let search_query = format!("{} {}", work.title, author);
    let alternate_url = format!("{metadata_base_url}{}", work.key);
    let subjects = work.subject.unwrap_or_default();

    ExploreEntry {
        id: format!("urn:openlibrary:{}", work.key.trim_start_matches('/')),
        title: work.title,
        author,
        summary: format!("Open Library editions: {edition_count}"),
        cover_url,
        search_query,
        alternate_url,
        popularity: edition_count,
        first_publish_year: work.first_publish_year,
        subjects,
    }
}

pub fn explore_pagination_paths(
    base_path: &str,
    page: usize,
    page_size: usize,
    total_items: usize,
) -> opds::PaginationPaths {
    let page = page.max(1);
    let total_pages = total_items.div_ceil(page_size.max(1)).max(1);
    let current_page = page.min(total_pages);

    let path_for = |target_page: usize| {
        if target_page <= 1 {
            base_path.to_owned()
        } else {
            format!("{base_path}?page={target_page}")
        }
    };

    opds::PaginationPaths {
        self_href: path_for(current_page),
        next_href: (current_page < total_pages).then(|| path_for(current_page + 1)),
        previous_href: (current_page > 1).then(|| path_for(current_page - 1)),
        page: current_page,
        page_size,
        total_items,
    }
}

pub fn paginate_entries(
    entries: &[ExploreEntry],
    page: usize,
    page_size: usize,
) -> Vec<ExploreEntry> {
    let page_size = page_size.max(1);
    let start = page.saturating_sub(1).saturating_mul(page_size);
    entries
        .iter()
        .skip(start)
        .take(page_size)
        .cloned()
        .collect()
}
