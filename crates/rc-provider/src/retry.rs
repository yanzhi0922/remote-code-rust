//! Retry logic for provider API requests.
//!
//! Implements [`with_retry`] — a generic retry loop with exponential back-off,
//! jitter, and structured error classification.  Modeled after upstream Claude
//! Code's `withRetry` generator in `services/api/withRetry.ts`.
//!
//! # Authentication recovery
//!
//! The retry loop distinguishes between **permanent** auth errors (401/403 with
//! invalid credentials) and **transient** auth errors that may be recoverable:
//!
//! - **401 Unauthorized**: May indicate an expired OAuth token. The closure can
//!   attempt a token refresh before the next retry.
//! - **403 Forbidden**: May indicate revoked OAuth tokens or Bedrock/Vertex
//!   credential expiry. Credential caches are cleared to force re-auth.
//! - **429 Rate Limited**: Always retryable with exponential back-off.
//! - **5xx Server Errors**: Always retryable.
//! - **529 Overloaded**: Retryable up to `max_529_retries` consecutive times.

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

/// Maximum number of auth recovery retries (401/403 with credential refresh).
const MAX_AUTH_RETRIES: u32 = 2;

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
    /// Whether the previous attempt failed with an auth error (401/403).
    pub auth_refresh_attempted: bool,
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
            auth_refresh_attempted: false,
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
    /// Maximum number of auth recovery retries (401/403 with credential refresh).
    pub max_auth_retries: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: BASE_DELAY_MS,
            max_backoff_ms: MAX_BACKOFF_MS,
            max_529_retries: MAX_529_RETRIES,
            respect_retry_after: true,
            max_auth_retries: MAX_AUTH_RETRIES,
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

/// Classify an error as a transient auth error that may be recoverable.
///
/// Returns `true` for:
/// - 401 Unauthorized (expired OAuth token)
/// - 403 Forbidden with OAuth token revoked message
/// - Bedrock credential provider errors
/// - Vertex credential refresh failures
#[must_use]
pub fn is_transient_auth_error(error_str: &str) -> bool {
    let lower = error_str.to_ascii_lowercase();

    // 401 Unauthorized — may be an expired token.
    if lower.contains("401") {
        return true;
    }

    // 403 Forbidden with specific recoverable messages.
    if lower.contains("403") {
        // OAuth token revoked — another process refreshed the token.
        if lower.contains("oauth token has been revoked")
            || lower.contains("token has been revoked")
        {
            return true;
        }
        // Bedrock credential errors.
        if lower.contains("credentialsprovidererror")
            || lower.contains("security token included in the request is invalid")
            || lower.contains("security token included in the request is expired")
        {
            return true;
        }
        // Vertex/GCP credential errors.
        if lower.contains("could not load the default credentials")
            || lower.contains("could not refresh access token")
            || lower.contains("invalid_grant")
        {
            return true;
        }
    }

    false
}

/// Classify an error as a permanent (non-retryable) auth error.
///
/// Returns `true` for:
/// - 403 Forbidden (without recoverable messages)
/// - 404 Not Found
#[must_use]
pub fn is_permanent_auth_error(error_str: &str) -> bool {
    let lower = error_str.to_ascii_lowercase();

    // 404 Not Found — permanent.
    if lower.contains("404") {
        return true;
    }

    // 403 Forbidden — permanent unless it's a known transient auth error.
    if lower.contains("403") && !is_transient_auth_error(error_str) {
        return true;
    }

    false
}

/// Classify an error as a stale connection error (ECONNRESET/EPIPE).
#[must_use]
pub fn is_stale_connection_error(error_str: &str) -> bool {
    let lower = error_str.to_ascii_lowercase();
    lower.contains("econnreset")
        || lower.contains("epipe")
        || lower.contains("broken pipe")
        || lower.contains("connection reset")
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
        && config.respect_retry_after
    {
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
/// # Authentication recovery
///
/// When a 401 or recoverable 403 error is encountered, the retry loop will:
/// 1. Mark `auth_refresh_attempted` in the context
/// 2. Allow the closure to refresh credentials before the next attempt
/// 3. Retry up to `max_auth_retries` times for auth errors
///
/// # Errors
///
/// Returns an error if all retry attempts are exhausted or if the operation
/// fails with a non-retryable error.
pub async fn with_retry<T, F, Fut>(config: &RetryConfig, model: &str, operation: F) -> Result<T>
where
    F: Fn(RetryContext) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut context = RetryContext::new(model);
    let mut consecutive_529: u32 = 0;
    let mut auth_retries: u32 = 0;

    for attempt in 0..=config.max_retries {
        context.attempt = attempt + 1;

        match operation(context.clone()).await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let error_str = format!("{error:#}").to_ascii_lowercase();

                // Check if this is a permanent non-retryable error.
                if is_permanent_auth_error(&error_str) {
                    return Err(error);
                }

                // Check if this is a transient auth error that may be recoverable.
                if is_transient_auth_error(&error_str) {
                    auth_retries += 1;
                    if auth_retries > config.max_auth_retries {
                        return Err(error.context(format!(
                            "giving up after {auth_retries} auth recovery attempts"
                        )));
                    }
                    // Signal to the closure that auth refresh is needed.
                    context.auth_refresh_attempted = true;
                    warn!(
                        attempt = attempt + 1,
                        auth_retry = auth_retries,
                        max_auth = config.max_auth_retries,
                        "auth error detected, will retry with credential refresh: {error:#}"
                    );
                    // Short delay for auth recovery (don't use long back-off).
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }

                // Check for stale connection errors (ECONNRESET/EPIPE).
                if is_stale_connection_error(&error_str) {
                    if attempt >= config.max_retries {
                        return Err(
                            error.context("all retry attempts exhausted (stale connection)")
                        );
                    }
                    let delay = compute_retry_delay(config, attempt, None);
                    warn!(
                        attempt = attempt + 1,
                        delay_ms = delay.as_millis(),
                        "stale connection, retrying: {error:#}"
                    );
                    sleep(delay).await;
                    continue;
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
    let mut auth_retries: u32 = 0;

    loop {
        match operation().await {
            Ok((status, body)) => {
                // Handle transient auth errors with retry.
                if (status == 401 || is_transient_auth_error(&format!("{status}")))
                    && auth_retries < config.max_auth_retries
                {
                    auth_retries += 1;
                    warn!(
                        attempt = attempt + 1,
                        status,
                        auth_retry = auth_retries,
                        "auth error, retrying with credential refresh"
                    );
                    sleep(Duration::from_millis(500)).await;
                    attempt += 1;
                    continue;
                }

                // Permanent auth errors — don't retry.
                if status == 403 || status == 404 {
                    return Ok((status, body));
                }

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
        // 404 is a permanent error — should not be retried.
        let result = with_retry(&config, "test-model", |_ctx| async {
            Err::<(), _>(anyhow!("404 model not found"))
        })
        .await;
        assert!(result.is_err());
        assert!(
            result
                .expect_err("non-retryable error should be returned")
                .to_string()
                .contains("404")
        );
    }
}
