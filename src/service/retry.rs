use anyhow::Context;
use backoff::ExponentialBackoffBuilder;
use backoff::backoff::Backoff;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep};
use tracing::{debug, trace, warn};

use crate::state::AppState;

fn make_backoff(state: &AppState) -> backoff::ExponentialBackoff {
    ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(state.upstream_retry_backoff_ms))
        .with_multiplier(2.0)
        .with_max_elapsed_time(None)
        .build()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn retry_backoff(base_ms: u64, attempt: usize) -> Duration {
    let multiplier = 1_u64 << attempt.saturating_sub(1).min(5);
    Duration::from_millis(base_ms.saturating_mul(multiplier))
}

pub async fn get_text_raced(
    state: &AppState,
    path: &str,
    label: &'static str,
) -> anyhow::Result<String> {
    if state.archive_bases.len() <= 1 {
        let url = format!("{}{}", state.archive_bases[0], path);
        return get_text_with_retry(state, &url, label).await;
    }

    trace!(
        instances = state.archive_bases.len(),
        "racing archive text request"
    );

    let mut handles: Vec<_> = state
        .archive_bases
        .iter()
        .map(|base| {
            let url = format!("{}{}", base, path);
            let http = state.http.clone();
            tokio::spawn(async move {
                let resp = http.get(&url).send().await?;
                resp.error_for_status()?
                    .text()
                    .await
                    .with_context(|| format!("{label} body read failed"))
            })
        })
        .collect();

    let mut last_error = None;
    while !handles.is_empty() {
        let (result, _index, remaining) = futures::future::select_all(handles).await;
        handles = remaining;
        match result {
            Ok(Ok(text)) => return Ok(text),
            Ok(Err(e)) => last_error = Some(e),
            Err(e) => last_error = Some(anyhow::anyhow!("{label} task failed: {e}")),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("{label} failed without an error")))
}

pub async fn get_text_with_retry(
    state: &AppState,
    url: &str,
    label: &'static str,
) -> anyhow::Result<String> {
    let attempts = state.upstream_retry_attempts.max(1);
    let mut backoff = make_backoff(state);
    let mut last_error = None;

    trace!(attempts, "starting upstream text retries");

    for attempt in 1..=attempts {
        debug!(attempt, "issuing upstream text request");
        match state.http.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => {
                    return resp
                        .text()
                        .await
                        .with_context(|| format!("{label} body read failed"));
                }
                Err(error) => {
                    last_error =
                        Some(anyhow::Error::new(error).context(format!("{label} returned non-2xx")))
                }
            },
            Err(error) => {
                last_error =
                    Some(anyhow::Error::new(error).context(format!("{label} request failed")))
            }
        }

        if let Some(duration) = backoff.next_backoff() {
            state
                .metrics
                .upstream_retries
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                attempt,
                backoff_ms = duration.as_millis(),
                label,
                "retrying upstream text request"
            );
            sleep(duration).await;
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("{label} failed without an error")))
}

pub async fn get_json_with_retry<T>(
    state: &AppState,
    url: &str,
    label: &'static str,
) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let attempts = state.upstream_retry_attempts.max(1);
    let mut backoff = make_backoff(state);
    let mut last_error = None;

    trace!(attempts, "starting upstream json retries");

    for attempt in 1..=attempts {
        debug!(attempt, "issuing upstream json request");
        match state.http.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => match resp.json::<T>().await {
                    Ok(body) => return Ok(body),
                    Err(error) => {
                        last_error = Some(
                            anyhow::Error::new(error).context(format!("{label} JSON parse failed")),
                        )
                    }
                },
                Err(error) => {
                    last_error =
                        Some(anyhow::Error::new(error).context(format!("{label} returned non-2xx")))
                }
            },
            Err(error) => {
                last_error =
                    Some(anyhow::Error::new(error).context(format!("{label} request failed")))
            }
        }

        if let Some(duration) = backoff.next_backoff() {
            state
                .metrics
                .upstream_retries
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                attempt,
                backoff_ms = duration.as_millis(),
                label,
                "retrying upstream JSON request"
            );
            sleep(duration).await;
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("{label} failed without an error")))
}

pub async fn get_flaresolverr_html_with_retry(
    state: &AppState,
    url: &str,
) -> anyhow::Result<String> {
    let attempts = state.upstream_retry_attempts.max(1);
    let mut backoff = make_backoff(state);
    let mut last_error = None;

    for attempt in 1..=attempts {
        match state.fs.get(url).await {
            Ok(html) => return Ok(html),
            Err(error) => last_error = Some(error),
        }

        if let Some(duration) = backoff.next_backoff() {
            state
                .metrics
                .upstream_retries
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                attempt,
                backoff_ms = duration.as_millis(),
                "retrying FlareSolverr request"
            );
            sleep(duration).await;
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow::anyhow!("FlareSolverr request failed without an error")))
}

pub fn log_sanitized_html(label: &str, html: &str) {
    let snippet: String = html.chars().take(2048).collect();
    debug!(
        %label,
        sanitized_html_len = html.len(),
        sanitized_html_snippet = %snippet,
        "sanitized HTML for debugging"
    );
}
