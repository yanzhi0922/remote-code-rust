//! Settings synchronization engine.
//!
//! Handles pulling settings from remote sources, applying them locally,
//! and resolving conflicts based on source priority.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use anyhow::Result;
use tracing::warn;

use crate::security_check::SecurityChecker;
use crate::sync_cache::SyncCache;
use crate::types::ManagedSetting;

/// The main managed settings service.
///
/// Coordinates remote sync, local application, conflict resolution, and
/// security checks for managed settings.
pub struct ManagedSettingsService {
    settings: Mutex<HashMap<String, ManagedSetting>>,
    cache: Mutex<SyncCache>,
    checker: SecurityChecker,
    remote_settings: Mutex<Vec<ManagedSetting>>,
    sync_interval: Duration,
}

impl ManagedSettingsService {
    /// Create a new service with default configuration.
    pub fn new() -> Self {
        Self {
            settings: Mutex::new(HashMap::new()),
            cache: Mutex::new(SyncCache::new()),
            checker: SecurityChecker::new(),
            remote_settings: Mutex::new(Vec::new()),
            sync_interval: Duration::from_secs(300),
        }
    }

    /// Create a new service with a custom sync interval.
    pub fn with_sync_interval(interval: Duration) -> Self {
        let mut svc = Self::new();
        svc.sync_interval = interval;
        svc
    }

    /// Simulate pulling settings from a remote source.
    ///
    /// In a real implementation this would make an HTTP request to a
    /// settings server. Here it returns the stored "remote" settings.
    pub fn sync_from_remote(&self) -> Result<Vec<ManagedSetting>> {
        let remote = lock_or_recover(&self.remote_settings).clone();

        // Cache the remote settings
        let mut cache = lock_or_recover(&self.cache);
        for setting in &remote {
            cache.set(&setting.key, setting.clone())?;
        }

        Ok(remote)
    }

    /// Apply a list of settings, respecting security checks and priorities.
    pub fn apply_settings(&self, settings: &[ManagedSetting]) -> Result<Vec<String>> {
        let mut applied = Vec::new();
        let mut guard = lock_or_recover(&self.settings);

        for setting in settings {
            // Security check
            let check = self.checker.check_setting(&setting.key, &setting.value);
            if !check.allowed {
                tracing::warn!(
                    "Skipping setting '{}' — security check failed: {}",
                    setting.key,
                    check.reason.as_deref().unwrap_or("unknown")
                );
                continue;
            }

            // Check if existing setting is locked
            if let Some(existing) = guard.get(&setting.key) {
                if existing.locked {
                    tracing::warn!("Skipping setting '{}' — locked by policy", setting.key);
                    continue;
                }
                // Only override if new setting has higher or equal priority
                if setting.effective_priority() < existing.effective_priority() {
                    continue;
                }
            }

            applied.push(setting.key.clone());
            guard.insert(setting.key.clone(), setting.clone());
        }

        Ok(applied)
    }

    /// Resolve a conflict between a local and remote setting.
    ///
    /// Priority order: Enterprise > Remote > Project > User > Local.
    pub fn resolve_conflict(local: &ManagedSetting, remote: &ManagedSetting) -> ManagedSetting {
        if local.effective_priority() >= remote.effective_priority() {
            local.clone()
        } else {
            remote.clone()
        }
    }

    /// Get the effective (merged) settings.
    ///
    /// Returns all settings sorted by effective priority.
    pub fn get_effective_settings(&self) -> Vec<ManagedSetting> {
        let guard = lock_or_recover(&self.settings);
        let mut settings: Vec<ManagedSetting> = guard.values().cloned().collect();
        settings.sort_by_key(|setting| Reverse(setting.effective_priority()));
        settings
    }

    /// Get a single effective setting by key.
    pub fn get(&self, key: &str) -> Option<ManagedSetting> {
        let guard = lock_or_recover(&self.settings);
        guard.get(key).cloned()
    }

    /// Set a remote setting (for testing sync scenarios).
    pub fn set_remote(&self, setting: ManagedSetting) {
        lock_or_recover(&self.remote_settings).push(setting);
    }

    /// Get the configured sync interval.
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// Run a full sync cycle: pull remote → apply.
    pub fn full_sync(&self) -> Result<Vec<String>> {
        let remote = self.sync_from_remote()?;
        self.apply_settings(&remote)
    }
}

/// Lock a `std::sync::Mutex`, recovering from poison by logging a warning
/// and accessing the inner value anyway.
fn lock_or_recover<'a, T>(mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("Mutex poisoned — another thread panicked while holding the lock; recovering");
            poisoned.into_inner()
        }
    }
}

impl Default for ManagedSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SettingSource;
    use serde_json::json;

    fn make_setting(key: &str, val: i32, source: SettingSource) -> ManagedSetting {
        ManagedSetting::new(key, json!(val), source)
    }

    #[test]
    fn new_service_is_empty() {
        let svc = ManagedSettingsService::new();
        assert!(svc.get_effective_settings().is_empty());
    }

    #[test]
    fn apply_settings_basic() {
        let svc = ManagedSettingsService::new();
        let settings = vec![make_setting("tab_size", 4, SettingSource::Local)];
        let applied = svc.apply_settings(&settings).expect("apply");
        assert_eq!(applied, vec!["tab_size"]);
        let effective = svc.get_effective_settings();
        assert_eq!(effective.len(), 1);
    }

    #[test]
    fn apply_settings_rejects_restricted() {
        let svc = ManagedSettingsService::new();
        let settings = vec![ManagedSetting::new(
            "api_key",
            json!("secret"),
            SettingSource::Local,
        )];
        let applied = svc.apply_settings(&settings).expect("apply");
        assert!(applied.is_empty());
    }

    #[test]
    fn apply_settings_respects_locked() {
        let svc = ManagedSettingsService::new();
        // First apply a locked enterprise setting
        let locked = make_setting("policy", 1, SettingSource::Enterprise).with_locked(true);
        svc.apply_settings(&[locked]).expect("apply");

        // Try to override with local
        let local = make_setting("policy", 2, SettingSource::Local);
        svc.apply_settings(&[local]).expect("apply");

        let effective = svc.get("policy").expect("some");
        assert_eq!(effective.value, json!(1)); // Still enterprise value
    }

    #[test]
    fn resolve_conflict_enterprise_wins() {
        let local = make_setting("k", 1, SettingSource::Local);
        let enterprise = make_setting("k", 2, SettingSource::Enterprise);
        let resolved = ManagedSettingsService::resolve_conflict(&local, &enterprise);
        assert_eq!(resolved.value, json!(2));
    }

    #[test]
    fn resolve_conflict_local_loses_to_remote() {
        let local = make_setting("k", 1, SettingSource::Local);
        let remote = make_setting("k", 2, SettingSource::Remote);
        let resolved = ManagedSettingsService::resolve_conflict(&local, &remote);
        assert_eq!(resolved.value, json!(2));
    }

    #[test]
    fn sync_from_remote_returns_settings() {
        let svc = ManagedSettingsService::new();
        svc.set_remote(make_setting("remote_key", 42, SettingSource::Remote));
        let remote = svc.sync_from_remote().expect("sync");
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].key, "remote_key");
    }

    #[test]
    fn full_sync_pulls_and_applies() {
        let svc = ManagedSettingsService::new();
        svc.set_remote(make_setting("synced", 99, SettingSource::Remote));
        let applied = svc.full_sync().expect("full_sync");
        assert_eq!(applied, vec!["synced"]);
    }

    #[test]
    fn get_existing_key() {
        let svc = ManagedSettingsService::new();
        svc.apply_settings(&[make_setting("k", 7, SettingSource::User)])
            .expect("apply");
        let s = svc.get("k").expect("some");
        assert_eq!(s.value, json!(7));
    }

    #[test]
    fn get_missing_key() {
        let svc = ManagedSettingsService::new();
        assert!(svc.get("nonexistent").is_none());
    }

    #[test]
    fn effective_settings_sorted_by_priority() {
        let svc = ManagedSettingsService::new();
        svc.apply_settings(&[make_setting("low", 1, SettingSource::Local)])
            .expect("apply");
        svc.apply_settings(&[make_setting("high", 2, SettingSource::Enterprise)])
            .expect("apply");
        let settings = svc.get_effective_settings();
        assert_eq!(settings[0].key, "high");
        assert_eq!(settings[1].key, "low");
    }

    #[test]
    fn default_trait() {
        let svc = ManagedSettingsService::default();
        assert!(svc.get_effective_settings().is_empty());
    }
}
