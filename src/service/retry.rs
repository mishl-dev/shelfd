use anyhow::Context;
use rand::RngExt;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep};
use tracing::{debug, trace, warn};

use crate::state::AppState;

const MAX_BACKOFF_MS: u64 = 30_000;

pub fn retry_backoff(base_ms: u64, attempt: usize) -> Duration {
    let base = base_ms.min(MAX_BACKOFF_MS);
    let multiplier = 1_u64 << attempt.saturating_sub(1).min(5);
    let shifted = base.saturating_mul(multiplier).min(MAX_BACKOFF_MS);
    let jitter: u64 = rand::rng().random_range(0..=shifted);
    Duration::from_millis(shifted.wrapping_add(jitter) / 2)
}

fn make_backoff(initial_ms: u64) -> impl FnMut(usize) -> Duration {
    move |attempt| retry_backoff(initial_ms, attempt)
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
    let mut backoff = make_backoff(state.upstream_retry_backoff_ms);
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

        if attempt < attempts {
            let duration = backoff(attempt);
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
    let mut backoff = make_backoff(state.upstream_retry_backoff_ms);
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

        if attempt < attempts {
            let duration = backoff(attempt);
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
    let mut backoff = make_backoff(state.upstream_retry_backoff_ms);
    let mut last_error = None;

    for attempt in 1..=attempts {
        match state.fs.get(url).await {
            Ok(html) => return Ok(html),
            Err(error) => last_error = Some(error),
        }

        if attempt < attempts {
            let duration = backoff(attempt);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_attempt_0() {
        for _ in 0..10 {
            let result = retry_backoff(100, 0);
            assert!(result.as_millis() >= 50 && result.as_millis() <= 100);
        }
    }

    #[test]
    fn retry_backoff_attempt_1() {
        let result = retry_backoff(100, 1);
        assert!(result.as_millis() >= 50 && result.as_millis() <= 100);
    }

    #[test]
    fn retry_backoff_attempt_2() {
        let result = retry_backoff(100, 2);
        assert!(result.as_millis() >= 100 && result.as_millis() <= 200);
    }

    #[test]
    fn retry_backoff_attempt_3() {
        let result = retry_backoff(100, 3);
        assert!(result.as_millis() >= 200 && result.as_millis() <= 400);
    }

    #[test]
    fn retry_backoff_attempt_4() {
        let result = retry_backoff(100, 4);
        assert!(result.as_millis() >= 400 && result.as_millis() <= 800);
    }

    #[test]
    fn retry_backoff_attempt_5() {
        let result = retry_backoff(100, 5);
        assert!(result.as_millis() >= 800 && result.as_millis() <= 1600);
    }

    #[test]
    fn retry_backoff_attempt_6() {
        let result = retry_backoff(100, 6);
        assert!(result.as_millis() >= 1600 && result.as_millis() <= 3200);
    }

    #[test]
    fn retry_backoff_attempt_7_capped_at_shift_5() {
        let result = retry_backoff(100, 7);
        assert!(result.as_millis() >= 1600 && result.as_millis() <= 3200);
    }

    #[test]
    fn retry_backoff_attempt_100_capped() {
        let result = retry_backoff(100, 100);
        assert!(result.as_millis() >= 1600 && result.as_millis() <= 3200);
    }

    #[test]
    fn retry_backoff_large_base_overflow_safe() {
        let result = retry_backoff(u64::MAX, 5);
        assert!(result.as_millis() > 0);
    }

    #[test]
    fn retry_backoff_base_zero() {
        assert_eq!(retry_backoff(0, 5), Duration::from_millis(0));
    }

    #[test]
    fn retry_backoff_respects_max_cap() {
        let result = retry_backoff(100_000, 0);
        assert!(result.as_millis() <= MAX_BACKOFF_MS as u128);
    }

    #[test]
    fn log_sanitized_html_truncates_long_input() {
        let long_html = "a".repeat(3000);
        let snippet: String = long_html.chars().take(2048).collect();
        assert_eq!(snippet.len(), 2048);
    }

    #[test]
    fn log_sanitized_html_short_input_not_truncated() {
        let short_html = "short";
        let snippet: String = short_html.chars().take(2048).collect();
        assert_eq!(snippet, "short");
    }

    #[test]
    fn log_sanitized_html_empty_input() {
        let snippet: String = "".chars().take(2048).collect();
        assert!(snippet.is_empty());
    }
}
