//! Settings sync cache with TTL-based expiration.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::types::ManagedSetting;

/// State snapshot of the sync cache for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCacheState {
    /// Number of entries currently cached.
    pub entries: usize,
    /// When the cache was last updated.
    pub last_updated: Option<DateTime<Utc>>,
    /// Seconds remaining until the next TTL expiry sweep.
    pub ttl_remaining: i64,
}

/// A cached setting with its insertion time.
#[derive(Debug, Clone)]
struct CachedEntry {
    setting: ManagedSetting,
    inserted_at: DateTime<Utc>,
}

/// TTL-based cache for settings synchronization.
///
/// Entries automatically expire after the configured time-to-live.
/// Default TTL is 5 minutes.
pub struct SyncCache {
    entries: HashMap<String, CachedEntry>,
    ttl: Duration,
    last_updated: Option<DateTime<Utc>>,
}

impl SyncCache {
    /// Create a new cache with the default 5-minute TTL.
    pub fn new() -> Self {
        Self::with_ttl(Duration::minutes(5))
    }

    /// Create a new cache with a custom TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            last_updated: None,
        }
    }

    /// Get a cached setting by key.
    ///
    /// Returns `None` if the key is not present or the entry has expired.
    pub fn get(&mut self, key: &str) -> Option<ManagedSetting> {
        self.evict_expired();
        self.entries.get(key).map(|e| e.setting.clone())
    }

    /// Insert or update a setting in the cache.
    pub fn set(&mut self, key: &str, setting: ManagedSetting) -> Result<()> {
        let now = Utc::now();
        self.entries.insert(
            key.to_owned(),
            CachedEntry {
                setting,
                inserted_at: now,
            },
        );
        self.last_updated = Some(now);
        Ok(())
    }

    /// Invalidate a single cache entry.
    pub fn invalidate(&mut self, key: &str) -> Result<()> {
        self.entries.remove(key);
        Ok(())
    }

    /// Invalidate all cache entries.
    pub fn invalidate_all(&mut self) -> Result<()> {
        self.entries.clear();
        self.last_updated = None;
        Ok(())
    }

    /// Get a diagnostic snapshot of the cache state.
    pub fn state(&mut self) -> SyncCacheState {
        self.evict_expired();
        let ttl_remaining = self.ttl.num_seconds();
        SyncCacheState {
            entries: self.entries.len(),
            last_updated: self.last_updated,
            ttl_remaining,
        }
    }

    /// Number of non-expired entries.
    pub fn len(&mut self) -> usize {
        self.evict_expired();
        self.entries.len()
    }

    /// Whether the cache is empty (after evicting expired entries).
    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    /// Remove all expired entries.
    fn evict_expired(&mut self) {
        let now = Utc::now();
        self.entries.retain(|_, entry| {
            let age = now - entry.inserted_at;
            age < self.ttl
        });
    }
}

impl Default for SyncCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_setting(key: &str, val: i32) -> ManagedSetting {
        ManagedSetting::new(key, json!(val), crate::types::SettingSource::Remote)
    }

    #[test]
    fn new_cache_is_empty() {
        let mut cache = SyncCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn set_and_get() {
        let mut cache = SyncCache::new();
        cache.set("k", make_setting("k", 1)).expect("set");
        let s = cache.get("k").expect("some");
        assert_eq!(s.value, json!(1));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let mut cache = SyncCache::new();
        assert!(cache.get("missing").is_none());
    }

    #[test]
    fn invalidate_removes_entry() {
        let mut cache = SyncCache::new();
        cache.set("k", make_setting("k", 1)).expect("set");
        cache.invalidate("k").expect("invalidate");
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn invalidate_all_clears_everything() {
        let mut cache = SyncCache::new();
        cache.set("a", make_setting("a", 1)).expect("set");
        cache.set("b", make_setting("b", 2)).expect("set");
        cache.invalidate_all().expect("invalidate_all");
        assert!(cache.is_empty());
    }

    #[test]
    fn ttl_expiry() {
        let mut cache = SyncCache::with_ttl(Duration::milliseconds(1));
        cache.set("k", make_setting("k", 1)).expect("set");
        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn state_snapshot() {
        let mut cache = SyncCache::new();
        cache.set("k", make_setting("k", 1)).expect("set");
        let state = cache.state();
        assert_eq!(state.entries, 1);
        assert!(state.last_updated.is_some());
        assert_eq!(state.ttl_remaining, 300); // 5 minutes
    }

    #[test]
    fn update_existing_key() {
        let mut cache = SyncCache::new();
        cache.set("k", make_setting("k", 1)).expect("set");
        cache.set("k", make_setting("k", 2)).expect("set");
        let s = cache.get("k").expect("some");
        assert_eq!(s.value, json!(2));
    }

    #[test]
    fn default_trait() {
        let mut cache = SyncCache::default();
        assert!(cache.is_empty());
    }
}
