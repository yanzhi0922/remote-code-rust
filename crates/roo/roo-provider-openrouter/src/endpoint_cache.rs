//! Model endpoint cache for OpenRouter.
//!
//! Ported from `.research/Roo-Code/src/api/providers/fetchers/modelEndpointCache.ts`.
//!
//! Provides an in-memory cache with a 5-minute TTL for OpenRouter model
//! endpoint data. A file-based persistence layer can be layered on top
//! by the caller (the TS version writes JSON to the VS Code global storage
//! directory; in Rust this is handled at a higher level).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default TTL for cache entries (5 minutes), matching the TS source.
const DEFAULT_TTL: Duration = Duration::from_secs(5 * 60);

/// Cache entry with TTL.
struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Model endpoint cache with in-memory TTL.
///
/// Thread-safe via an internal `Mutex`. Each entry is stored with an
/// expiration time; `get` returns `None` for expired entries.
pub struct ModelEndpointCache {
    cache: Mutex<HashMap<String, CacheEntry<serde_json::Value>>>,
    ttl: Duration,
}

impl ModelEndpointCache {
    /// Create a new cache with the default 5-minute TTL.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            ttl: DEFAULT_TTL,
        }
    }

    /// Create a new cache with a custom TTL (useful for testing).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Retrieve a cached value by key.
    ///
    /// Returns `Some(value)` if the key exists and has not expired,
    /// `None` otherwise. Expired entries are lazily removed on access.
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        match cache.get(key) {
            Some(entry) if !entry.is_expired() => Some(entry.data.clone()),
            Some(_) => {
                // Entry is expired — remove it.
                cache.remove(key);
                None
            }
            None => None,
        }
    }

    /// Store a value in the cache with the configured TTL.
    pub fn set(&self, key: &str, data: serde_json::Value) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(key.to_string(), CacheEntry::new(data, self.ttl));
    }

    /// Remove a specific entry from the cache.
    ///
    /// Corresponds to `flushModelProviders` in the TS source.
    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.remove(key);
    }
}

impl Default for ModelEndpointCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_set_and_get() {
        let cache = ModelEndpointCache::new();
        cache.set(
            "openrouter_gpt-4o",
            json!({"endpoint": "https://api.example.com"}),
        );

        let value = cache.get("openrouter_gpt-4o").unwrap();
        assert_eq!(value["endpoint"], "https://api.example.com");
    }

    #[test]
    fn test_cache_miss() {
        let cache = ModelEndpointCache::new();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = ModelEndpointCache::new();
        cache.set("key", json!({"data": 42}));
        assert!(cache.get("key").is_some());

        cache.invalidate("key");
        assert!(cache.get("key").is_none());
    }

    #[test]
    fn test_cache_ttl_expiration() {
        // Use a very short TTL so the entry expires immediately.
        let cache = ModelEndpointCache::with_ttl(Duration::from_millis(1));
        cache.set("short_lived", json!({"temp": true}));

        // Wait for the entry to expire.
        std::thread::sleep(Duration::from_millis(5));

        assert!(cache.get("short_lived").is_none());
    }

    #[test]
    fn test_cache_overwrite() {
        let cache = ModelEndpointCache::new();
        cache.set("key", json!({"version": 1}));
        cache.set("key", json!({"version": 2}));

        let value = cache.get("key").unwrap();
        assert_eq!(value["version"], 2);
    }

    #[test]
    fn test_cache_default() {
        let cache = ModelEndpointCache::default();
        cache.set("test", json!(null));
        assert!(cache.get("test").is_some());
    }
}
