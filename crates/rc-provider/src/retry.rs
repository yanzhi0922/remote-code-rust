//! Retry logic for provider API requests.
//!
//! Implements [`with_retry`] — a generic retry loop with exponential back-off,
//! jitter, and structured error classification.  Modeled after upstream Claude
//! Code's `withRetry` generator in `services/api/withRetry.ts`.

use anyhow::{Result, anyhow};
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of retry attempts.
const DEFAULT_MAX_RETRIES: u32 = 10;

/// Base delay in milliseconds for the first retry back-off.
const BASE_DELAY_MS: u64 = 500;

/// Maximum delay cap for standard retries (5 minutes).
const MAX_BACKOFF_MS: u64 = 5 * 60 * 1000;

/// Maximum number of consecutive 529 (overloaded) errors before giving up.
const MAX_529_RETRIES: u32 = 3;

// ---------------------------------------------------------------------------
// RetryContext
// ---------------------------------------------------------------------------

/// Context carried across retry attempts.
///
/// Allows callers to adjust parameters (e.g. `max_tokens_override`) based on
/// what was learned from previous failed attempts.
#[derive(Debug, Clone)]
pub struct RetryContext {
    /// Override the `max_tokens` parameter for subsequent attempts.
    pub max_tokens_override: Option<u32>,
    /// The model being queried.
    pub model: String,
    /// Whether extended thinking is enabled.
    pub thinking_enabled: bool,
    /// Current attempt number (1-based).
    pub attempt: u32,
}

impl RetryContext {
    /// Create a new retry context for the given model.
    #[must_use]
    pub fn new(model: &str) -> Self {
        Self {
            max_tokens_override: None,
            model: model.to_owned(),
            thinking_enabled: false,
            attempt: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Retry configuration
// ---------------------------------------------------------------------------

/// Configuration for the retry loop.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential back-off.
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds.
    pub max_backoff_ms: u64,
    /// Maximum consecutive 529 errors before giving up.
    pub max_529_retries: u32,
    /// Whether to respect the `Retry-After` header.
    pub respect_retry_after: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: BASE_DELAY_MS,
            max_backoff_ms: MAX_BACKOFF_MS,
            max_529_retries: MAX_529_RETRIES,
            respect_retry_after: true,
        }
    }
}

impl RetryConfig {
    /// Create a retry config from provider settings.
    #[must_use]
    pub fn from_provider(max_retries: u32, initial_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms: initial_backoff_ms,
            max_backoff_ms,
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Error classification for retries
// ---------------------------------------------------------------------------

/// Classify an HTTP status code as retryable or not.
#[must_use]
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

/// Classify a transport error as retryable or not.
#[must_use]
pub fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

// ---------------------------------------------------------------------------
// Delay computation
// ---------------------------------------------------------------------------

/// Compute the delay before the next retry attempt.
///
/// Uses exponential back-off with ±25% jitter to avoid thundering herd.
/// If `retry_after` is provided and the config allows it, that value is used
/// directly.
#[must_use]
pub fn compute_retry_delay(
    config: &RetryConfig,
    attempt: u32,
    retry_after: Option<Duration>,
) -> Duration {
    if let Some(retry_after) = retry_after
        && config.respect_retry_after {
            return retry_after;
        }

    let multiplier = 2u64.saturating_pow(attempt.min(16));
    let base_ms = config
        .base_delay_ms
        .saturating_mul(multiplier)
        .min(config.max_backoff_ms)
        .max(1);

    // ±25% jitter using deterministic hash.
    let jitter_range = base_ms / 4;
    let jitter_offset = if jitter_range > 0 {
        let hash = (attempt as u64).wrapping_mul(2654435761) ^ base_ms;
        hash % (2 * jitter_range)
    } else {
        0
    };

    let delay_ms = base_ms
        .saturating_sub(jitter_range)
        .saturating_add(jitter_offset);
    Duration::from_millis(delay_ms.max(1))
}

// ---------------------------------------------------------------------------
// with_retry
// ---------------------------------------------------------------------------

/// Execute an async operation with automatic retries on transient failures.
///
/// The closure receives a [`RetryContext`] on each attempt so it can adjust
/// request parameters based on what was learned from previous failures.
///
/// # Errors
///
/// Returns an error if all retry attempts are exhausted or if the operation
/// fails with a non-retryable error.
pub async fn with_retry<T, F, Fut>(
    config: &RetryConfig,
    model: &str,
    operation: F,
) -> Result<T>
where
    F: Fn(RetryContext) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut context = RetryContext::new(model);
    let mut consecutive_529: u32 = 0;

    for attempt in 0..=config.max_retries {
        context.attempt = attempt + 1;

        match operation(context.clone()).await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let error_str = error.to_string().to_ascii_lowercase();

                // Check if this is a non-retryable error.
                let is_non_retryable =
                    error_str.contains("401") || error_str.contains("403") || error_str.contains("404");

                if is_non_retryable {
                    return Err(error);
                }

                // Track consecutive 529 errors.
                if error_str.contains("529") {
                    consecutive_529 += 1;
                    if consecutive_529 >= config.max_529_retries {
                        return Err(error.context(format!(
                            "giving up after {consecutive_529} consecutive 529 errors"
                        )));
                    }
                } else {
                    consecutive_529 = 0;
                }

                if attempt >= config.max_retries {
                    return Err(error.context(format!(
                        "all {} retry attempts exhausted",
                        config.max_retries
                    )));
                }

                let delay = compute_retry_delay(config, attempt, None);
                warn!(
                    attempt = attempt + 1,
                    max = config.max_retries,
                    delay_ms = delay.as_millis(),
                    "retrying after error: {error:#}"
                );
                sleep(delay).await;
            }
        }
    }

    Err(anyhow!("retry loop exited unexpectedly for model {model}"))
}

/// Execute an async HTTP request with retries, returning `(status, body)`.
///
/// This is a convenience wrapper around [`with_retry`] specifically for HTTP
/// requests that return a status code and response body.
pub async fn retry_http_request<F, Fut>(
    config: &RetryConfig,
    _model: &str,
    operation: F,
) -> Result<(u16, String)>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<(u16, String)>>,
{
    let mut attempt: u32 = 0;

    loop {
        match operation().await {
            Ok((status, body)) => {
                if is_retryable_status(status) && attempt < config.max_retries {
                    let delay = compute_retry_delay(config, attempt, None);
                    warn!(
                        attempt = attempt + 1,
                        status,
                        delay_ms = delay.as_millis(),
                        "retrying HTTP request"
                    );
                    sleep(delay).await;
                    attempt += 1;
                    continue;
                }
                return Ok((status, body));
            }
            Err(error) => {
                if attempt >= config.max_retries {
                    return Err(error);
                }
                let delay = compute_retry_delay(config, attempt, None);
                warn!(
                    attempt = attempt + 1,
                    delay_ms = delay.as_millis(),
                    "retrying after transport error: {error:#}"
                );
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_increases_with_attempts() {
        let config = RetryConfig::default();
        let d0 = compute_retry_delay(&config, 0, None);
        let d1 = compute_retry_delay(&config, 1, None);
        let d2 = compute_retry_delay(&config, 2, None);
        assert!(d0 < d1);
        assert!(d1 < d2);
    }

    #[test]
    fn retry_after_overrides_delay() {
        let config = RetryConfig {
            respect_retry_after: true,
            ..RetryConfig::default()
        };
        let custom = Duration::from_secs(10);
        let delay = compute_retry_delay(&config, 5, Some(custom));
        assert_eq!(delay, custom);
    }

    #[test]
    fn retry_after_ignored_when_disabled() {
        let config = RetryConfig {
            respect_retry_after: false,
            ..RetryConfig::default()
        };
        let custom = Duration::from_secs(10);
        let delay = compute_retry_delay(&config, 0, Some(custom));
        assert_ne!(delay, custom);
    }

    #[test]
    fn is_retryable_status_classifies_correctly() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(529));
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(403));
        assert!(!is_retryable_status(404));
    }

    #[tokio::test]
    async fn with_retry_succeeds_on_first_try() {
        let config = RetryConfig {
            max_retries: 3,
            ..RetryConfig::default()
        };
        let result = with_retry(&config, "test-model", |_ctx| async { Ok(42) }).await;
        assert_eq!(result.expect("should succeed"), 42);
    }

    #[tokio::test]
    async fn with_retry_retries_on_transient_failure() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay_ms: 1,
            max_backoff_ms: 2,
            ..RetryConfig::default()
        };
        let attempt = std::sync::atomic::AtomicU32::new(0);
        let result = with_retry(&config, "test-model", |_ctx| {
            let current = attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                if current == 0 {
                    Err(anyhow!("server error 503"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;
        assert_eq!(result.expect("should succeed after retry"), "success");
    }

    #[tokio::test]
    async fn with_retry_fails_on_non_retryable() {
        let config = RetryConfig {
            max_retries: 3,
            ..RetryConfig::default()
        };
        let result = with_retry(&config, "test-model", |_ctx| async {
            Err::<(), _>(anyhow!("401 unauthorized"))
        })
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }
}
