//! Core types for the managed settings system.

use serde::{Deserialize, Serialize};

/// Where a setting originates from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    /// Locally defined setting.
    Local,
    /// User-level setting.
    User,
    /// Project-scoped setting.
    Project,
    /// Enterprise / organization setting.
    Enterprise,
    /// Remotely synced setting.
    Remote,
}

impl SettingSource {
    /// Numeric priority — higher value wins during conflict resolution.
    pub fn priority(&self) -> u32 {
        match self {
            Self::Local => 0,
            Self::User => 1,
            Self::Project => 2,
            Self::Remote => 3,
            Self::Enterprise => 4,
        }
    }
}

impl std::fmt::Display for SettingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
            Self::Enterprise => write!(f, "enterprise"),
            Self::Remote => write!(f, "remote"),
        }
    }
}

/// A single managed setting entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedSetting {
    /// The setting key (dot-separated path, e.g. `editor.tab_size`).
    pub key: String,
    /// The setting value.
    pub value: serde_json::Value,
    /// Where this setting came from.
    pub source: SettingSource,
    /// Explicit priority (overrides source-based priority when set).
    pub priority: u32,
    /// Whether this setting is locked and cannot be overridden.
    pub locked: bool,
}

impl ManagedSetting {
    /// Create a new setting with default priority from its source.
    pub fn new(key: impl Into<String>, value: serde_json::Value, source: SettingSource) -> Self {
        Self {
            key: key.into(),
            value,
            source,
            priority: source.priority(),
            locked: false,
        }
    }

    /// Builder-style setter for `locked`.
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Builder-style setter for explicit `priority`.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Effective priority — the higher of source-based and explicit priority.
    pub fn effective_priority(&self) -> u32 {
        self.priority.max(self.source.priority())
    }
}

/// Policy governing how settings may be overridden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPolicy {
    /// Whether local overrides are allowed at all.
    pub allow_overrides: bool,
    /// Keys that are restricted (cannot be changed locally).
    pub restricted_keys: Vec<String>,
    /// Whether to audit all setting changes.
    pub audit_changes: bool,
}

impl Default for SettingsPolicy {
    fn default() -> Self {
        Self {
            allow_overrides: true,
            restricted_keys: Vec::new(),
            audit_changes: false,
        }
    }
}

/// A payload for syncing settings between nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsSyncPayload {
    /// The settings in this payload.
    pub settings: Vec<ManagedSetting>,
    /// Monotonically increasing version.
    pub version: u64,
    /// Checksum of the serialized settings.
    pub checksum: String,
}

impl SettingsSyncPayload {
    /// Create a new sync payload, computing the checksum automatically.
    pub fn new(settings: Vec<ManagedSetting>, version: u64) -> Self {
        let checksum = compute_checksum(&settings);
        Self {
            settings,
            version,
            checksum,
        }
    }
}

/// Simple checksum: xxhash-style fold of the serialized JSON.
fn compute_checksum(settings: &[ManagedSetting]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for s in settings {
        s.key.hash(&mut hasher);
        s.value.to_string().hash(&mut hasher);
        format!("{:?}", s.source).hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn setting_source_priority_order() {
        assert!(SettingSource::Enterprise.priority() > SettingSource::Remote.priority());
        assert!(SettingSource::Remote.priority() > SettingSource::Project.priority());
        assert!(SettingSource::Project.priority() > SettingSource::User.priority());
        assert!(SettingSource::User.priority() > SettingSource::Local.priority());
    }

    #[test]
    fn setting_source_display() {
        assert_eq!(SettingSource::Local.to_string(), "local");
        assert_eq!(SettingSource::User.to_string(), "user");
        assert_eq!(SettingSource::Project.to_string(), "project");
        assert_eq!(SettingSource::Enterprise.to_string(), "enterprise");
        assert_eq!(SettingSource::Remote.to_string(), "remote");
    }

    #[test]
    fn managed_setting_new() {
        let s = ManagedSetting::new("key", json!(42), SettingSource::Local);
        assert_eq!(s.key, "key");
        assert_eq!(s.value, json!(42));
        assert_eq!(s.source, SettingSource::Local);
        assert!(!s.locked);
    }

    #[test]
    fn managed_setting_with_locked() {
        let s = ManagedSetting::new("k", json!(1), SettingSource::Enterprise).with_locked(true);
        assert!(s.locked);
    }

    #[test]
    fn managed_setting_effective_priority() {
        let s = ManagedSetting::new("k", json!(1), SettingSource::Local).with_priority(100);
        assert_eq!(s.effective_priority(), 100);
    }

    #[test]
    fn settings_policy_default() {
        let p = SettingsPolicy::default();
        assert!(p.allow_overrides);
        assert!(p.restricted_keys.is_empty());
        assert!(!p.audit_changes);
    }

    #[test]
    fn sync_payload_new() {
        let settings = vec![ManagedSetting::new("a", json!(true), SettingSource::Remote)];
        let payload = SettingsSyncPayload::new(settings, 1);
        assert_eq!(payload.version, 1);
        assert!(!payload.checksum.is_empty());
        assert_eq!(payload.settings.len(), 1);
    }

    #[test]
    fn serde_roundtrip_managed_setting() {
        let s = ManagedSetting::new("editor.tab_size", json!(4), SettingSource::User);
        let json_str = serde_json::to_string(&s).expect("serialize");
        let back: ManagedSetting = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(back.key, s.key);
        assert_eq!(back.value, s.value);
    }

    #[test]
    fn serde_roundtrip_setting_source() {
        for src in [
            SettingSource::Local,
            SettingSource::User,
            SettingSource::Project,
            SettingSource::Enterprise,
            SettingSource::Remote,
        ] {
            let j = serde_json::to_string(&src).expect("ser");
            let back: SettingSource = serde_json::from_str(&j).expect("de");
            assert_eq!(src, back);
        }
    }
}
