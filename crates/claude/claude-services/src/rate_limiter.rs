//! Token-bucket rate limiter with `Retry-After` header parsing.
//!
//! Tracks per-provider rate limits by counting tokens consumed and applying
//! backoff when limits are exceeded. Integrates with `claude-provider`'s
//! circuit breaker for automatic failover.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Configuration for rate limit tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Default requests per minute limit when no provider header is seen.
    pub default_rpm: u64,
    /// Default tokens per minute limit.
    pub default_tpm: u64,
    /// How long to wait before retrying a rate-limited request.
    pub base_retry_secs: u64,
    /// Maximum exponential backoff delay in seconds.
    pub max_retry_secs: u64,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            default_rpm: 60,
            default_tpm: 200_000,
            base_retry_secs: 5,
            max_retry_secs: 120,
        }
    }
}

/// Per-provider rate limit state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitState {
    /// Provider name (e.g. "anthropic", "openai").
    pub provider: String,
    /// Requests remaining in current window.
    pub requests_remaining: u64,
    /// Tokens remaining in current window.
    pub tokens_remaining: u64,
    /// Unix timestamp when the current window resets.
    pub resets_at: u64,
}

/// Token-bucket rate limiter.
///
/// Tracks two dimensions per provider:
/// - **Requests per minute (RPM)** — raw API call count.
/// - **Tokens per minute (TPM)** — total input+output token throughput.
pub struct RateLimiter {
    config: RateLimiterConfig,
    // provider → (requests_used, tokens_used, window_start)
    state: Mutex<HashMap<String, (u64, u64, Instant)>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given config.
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            config,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Create with default configuration.
    pub fn new_default() -> Self {
        Self::new(RateLimiterConfig::default())
    }

    /// Check whether a request with the given token count is allowed.
    ///
    /// Returns `Ok(())` if allowed, or `Err(retry_after_secs)` with the
    /// suggested backoff duration when rate-limited.
    pub fn check(&self, provider: &str, estimated_tokens: u64) -> std::result::Result<(), u64> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (used_req, used_tok, window_start) = state
            .entry(provider.to_owned())
            .or_insert_with(|| (0, 0, Instant::now()));

        let elapsed = window_start.elapsed().as_secs();
        if elapsed >= 60 {
            // Reset window every 60 seconds.
            *used_req = 0;
            *used_tok = 0;
            *window_start = Instant::now();
        }

        if *used_req >= self.config.default_rpm {
            let retry_after = self
                .config
                .base_retry_secs
                .saturating_mul(2u64.pow(std::cmp::min(*used_req as u32 / 10, 5)))
                .min(self.config.max_retry_secs);
            warn!(%provider, used_req = *used_req, retry_after, "Rate limited (RPM)");
            return Err(retry_after);
        }

        if *used_tok + estimated_tokens > self.config.default_tpm {
            let retry_after = self.config.base_retry_secs;
            warn!(%provider, used_tok = *used_tok, estimated_tokens, "Rate limited (TPM)");
            return Err(retry_after);
        }

        Ok(())
    }

    /// Record a successful API call, consuming tokens.
    pub fn record_success(&self, provider: &str, tokens_consumed: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (used_req, used_tok, _) = state
            .entry(provider.to_owned())
            .or_insert_with(|| (0, 0, Instant::now()));
        *used_req += 1;
        *used_tok += tokens_consumed;
    }

    /// Apply a `Retry-After` header value (in seconds).
    ///
    /// Resets the current window and advances it by `retry_after_secs`.
    pub fn record_retry_after(&self, provider: &str, retry_after_secs: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.insert(
            provider.to_owned(),
            (
                self.config.default_rpm,
                self.config.default_tpm,
                Instant::now() + Duration::from_secs(retry_after_secs),
            ),
        );
        warn!(%provider, retry_after_secs, "Applied Retry-After backoff");
    }

    /// Return current rate limit state for a provider.
    pub fn state(&self, provider: &str) -> Option<RateLimitState> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state
            .get(provider)
            .map(|(used_req, used_tok, window_start)| {
                let _elapsed = window_start.elapsed().as_secs();
                RateLimitState {
                    provider: provider.to_owned(),
                    requests_remaining: self.config.default_rpm.saturating_sub(*used_req),
                    tokens_remaining: self.config.default_tpm.saturating_sub(*used_tok),
                    resets_at: (window_start.elapsed().as_secs_f64() + 60.0) as u64,
                }
            })
    }

    /// Reset all rate limit tracking.
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_requests_under_limit() {
        let limiter = RateLimiter::new_default();
        assert!(limiter.check("anthropic", 1000).is_ok());

        limiter.record_success("anthropic", 1000);
        assert!(limiter.check("anthropic", 1000).is_ok());
    }

    #[test]
    fn blocks_above_rpm() {
        let limiter = RateLimiter::new(RateLimiterConfig {
            default_rpm: 2,
            ..Default::default()
        });

        limiter.record_success("anthropic", 0);
        limiter.record_success("anthropic", 0);
        assert!(limiter.check("anthropic", 0).is_err());
    }

    #[test]
    fn blocks_above_tpm() {
        let limiter = RateLimiter::new(RateLimiterConfig {
            default_tpm: 5000,
            ..Default::default()
        });

        limiter.record_success("openai", 4000);
        assert!(limiter.check("openai", 2000).is_err());
    }

    #[test]
    fn retry_after_resets_window() {
        let limiter = RateLimiter::new_default();
        limiter.record_retry_after("anthropic", 30);
        let state = limiter.state("anthropic");
        assert!(state.is_some());
        assert_eq!(state.unwrap().requests_remaining, 0);
    }

    #[test]
    fn independent_per_provider() {
        let limiter = RateLimiter::new(RateLimiterConfig {
            default_rpm: 1,
            ..Default::default()
        });

        limiter.record_success("anthropic", 0);
        assert!(limiter.check("anthropic", 0).is_err());
        assert!(limiter.check("openai", 0).is_ok());
    }

    #[test]
    fn reset_clears_all_state() {
        let limiter = RateLimiter::new_default();
        limiter.record_success("anthropic", 1000);
        limiter.reset();
        assert!(limiter.state("anthropic").is_none());
    }
}
