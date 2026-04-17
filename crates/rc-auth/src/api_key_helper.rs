//! API Key Helper with time-based cache (5-minute TTL).
//!
//! Executes an external command to obtain an API key, caches the result,
//! and serves stale values while refreshing in the background (SWR pattern).
//!
//! Mirrors `utils/auth.ts` — `getApiKeyFromApiKeyHelper` and related functions.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tracing::debug;

/// Default TTL for the API key helper cache (5 minutes).
pub const DEFAULT_API_KEY_HELPER_TTL: Duration = Duration::from_secs(5 * 60);

/// The source of an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    /// `ANTHROPIC_API_KEY` environment variable.
    EnvVar,
    /// apiKeyHelper command output.
    ApiKeyHelper,
    /// `/login` managed key (stored in config or keychain).
    ManagedKey,
    /// No key available.
    None,
}

/// Cached API key result.
#[derive(Debug, Clone)]
pub struct ApiKeyHelperResult {
    /// The API key value.
    pub key: String,
    /// Where the key came from.
    pub source: ApiKeySource,
    /// When the key was cached.
    pub cached_at: DateTime<Utc>,
}

/// Errors from the API key helper.
#[derive(Debug, thiserror::Error)]
pub enum ApiKeyHelperError {
    #[error("apiKeyHelper command failed: {0}")]
    CommandFailed(String),

    #[error("apiKeyHelper returned empty output")]
    EmptyOutput,

    #[error("apiKeyHelper not configured")]
    NotConfigured,

    #[error("command execution error: {0}")]
    ExecError(String),
}

/// Thread-safe API key helper cache with TTL.
pub struct ApiKeyHelperCache {
    inner: Mutex<Option<CachedEntry>>,
    ttl: Duration,
}

#[derive(Debug)]
struct CachedEntry {
    value: String,
    cached_at: Instant,
}

impl ApiKeyHelperCache {
    /// Create a new cache with the default 5-minute TTL.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            ttl: DEFAULT_API_KEY_HELPER_TTL,
        }
    }

    /// Create a new cache with a custom TTL.
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(None),
            ttl,
        }
    }

    /// Get a cached API key if it exists and is not expired.
    pub fn get_cached(&self) -> Option<String> {
        let inner = self.inner.lock().expect("cache lock");
        match inner.as_ref() {
            Some(entry) if entry.cached_at.elapsed() < self.ttl => Some(entry.value.clone()),
            Some(entry) => {
                // Stale — caller should refresh in background
                debug!("API key cache is stale, returning stale value");
                Some(entry.value.clone())
            }
            None => None,
        }
    }

    /// Get a cached API key only if it's fresh (within TTL).
    pub fn get_fresh(&self) -> Option<String> {
        let inner = self.inner.lock().expect("cache lock");
        match inner.as_ref() {
            Some(entry) if entry.cached_at.elapsed() < self.ttl => Some(entry.value.clone()),
            _ => None,
        }
    }

    /// Store a value in the cache.
    pub fn set(&self, value: String) {
        let mut inner = self.inner.lock().expect("cache lock");
        *inner = Some(CachedEntry {
            value,
            cached_at: Instant::now(),
        });
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("cache lock");
        *inner = None;
    }

    /// Check if the cache has a value (even if stale).
    pub fn is_populated(&self) -> bool {
        self.inner.lock().expect("cache lock").is_some()
    }
}

impl Default for ApiKeyHelperCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute an apiKeyHelper command and cache the result.
///
/// The command is executed via the system shell. Its trimmed stdout is
/// used as the API key.
pub async fn execute_api_key_helper(
    command: &str,
    cache: &ApiKeyHelperCache,
) -> Result<ApiKeyHelperResult, ApiKeyHelperError> {
    // Check cache first
    if let Some(key) = cache.get_fresh() {
        debug!("Returning cached API key");
        return Ok(ApiKeyHelperResult {
            key,
            source: ApiKeySource::ApiKeyHelper,
            cached_at: Utc::now(),
        });
    }

    // Execute the command
    debug!("Executing apiKeyHelper command");
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| ApiKeyHelperError::ExecError(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // On failure with stale cache, return stale value (SWR)
        if let Some(stale) = cache.get_cached() {
            debug!("apiKeyHelper failed, returning stale cached value");
            return Ok(ApiKeyHelperResult {
                key: stale,
                source: ApiKeySource::ApiKeyHelper,
                cached_at: Utc::now(),
            });
        }
        return Err(ApiKeyHelperError::CommandFailed(stderr.trim().to_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout.is_empty() {
        return Err(ApiKeyHelperError::EmptyOutput);
    }

    // Cache the result
    cache.set(stdout.clone());

    Ok(ApiKeyHelperResult {
        key: stdout,
        source: ApiKeySource::ApiKeyHelper,
        cached_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_fresh() {
        let cache = ApiKeyHelperCache::with_ttl(Duration::from_secs(60));
        cache.set("test-key".to_owned());
        assert_eq!(cache.get_fresh().as_deref(), Some("test-key"));
    }

    #[test]
    fn cache_expired_returns_none_fresh() {
        let cache = ApiKeyHelperCache::with_ttl(Duration::from_nanos(1));
        cache.set("test-key".to_owned());
        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get_fresh().is_none());
    }

    #[test]
    fn cache_expired_returns_stale() {
        let cache = ApiKeyHelperCache::with_ttl(Duration::from_nanos(1));
        cache.set("test-key".to_owned());
        std::thread::sleep(Duration::from_millis(1));
        // get_cached returns stale values
        assert_eq!(cache.get_cached().as_deref(), Some("test-key"));
    }

    #[test]
    fn cache_clear() {
        let cache = ApiKeyHelperCache::new();
        cache.set("test-key".to_owned());
        assert!(cache.is_populated());
        cache.clear();
        assert!(!cache.is_populated());
    }
}
