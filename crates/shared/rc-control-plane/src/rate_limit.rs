//! Simple per-IP rate limiter using a sliding window counter.
//!
//! Protects authentication-related endpoints (bootstrap claim, pairing accept,
//! token refresh) from brute-force attacks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;

/// Maximum number of requests allowed within the window per IP.
const DEFAULT_MAX_REQUESTS: usize = 20;

/// Window duration in seconds.
const WINDOW_SECS: u64 = 60;

/// Maximum number of distinct IPs tracked before pruning.
const MAX_TRACKED_IPS: usize = 10_000;

#[derive(Debug, Clone)]
struct WindowEntry {
    count: usize,
    started_at: Instant,
}

/// A per-IP sliding window rate limiter.
#[derive(Debug, Clone)]
pub(crate) struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, WindowEntry>>>,
    max_requests: usize,
    window_secs: u64,
}

impl RateLimiter {
    pub(crate) fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    /// Check whether the given key (typically an IP address) is within rate
    /// limits. Returns `true` if the request should be allowed.
    pub(crate) async fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut map = self.inner.lock().await;

        // Prune expired entries periodically to prevent unbounded growth.
        if map.len() > MAX_TRACKED_IPS {
            map.retain(|_, entry| now.duration_since(entry.started_at).as_secs() < self.window_secs);
        }

        let entry = map.entry(key.to_owned()).or_insert(WindowEntry {
            count: 0,
            started_at: now,
        });

        // Reset the window if it has expired.
        if now.duration_since(entry.started_at).as_secs() >= self.window_secs {
            entry.count = 0;
            entry.started_at = now;
        }

        entry.count += 1;
        entry.count <= self.max_requests
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_REQUESTS, WINDOW_SECS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_up_to_max_requests() {
        let limiter = RateLimiter::new(3, 60);
        assert!(limiter.allow("1.2.3.4").await);
        assert!(limiter.allow("1.2.3.4").await);
        assert!(limiter.allow("1.2.3.4").await);
        assert!(!limiter.allow("1.2.3.4").await);
    }

    #[tokio::test]
    async fn different_ips_are_independent() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.allow("1.1.1.1").await);
        assert!(!limiter.allow("1.1.1.1").await);
        assert!(limiter.allow("2.2.2.2").await);
    }

    #[tokio::test]
    async fn window_resets_after_expiry() {
        let limiter = RateLimiter::new(1, 0); // 0-second window expires immediately
        assert!(limiter.allow("1.1.1.1").await);
        // With a 0-second window, the next call should reset and allow.
        assert!(limiter.allow("1.1.1.1").await);
    }
}
