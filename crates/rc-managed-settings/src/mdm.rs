//! MDM (Mobile Device Management) enterprise settings support.
//!
//! Provides loading and applying of MDM profiles that enforce organization-level
//! settings which cannot be overridden by users.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An MDM profile containing enterprise-managed settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdmProfile {
    /// Unique identifier for this profile.
    pub identifier: String,
    /// Profile version (monotonically increasing).
    pub version: u32,
    /// The managed settings.
    pub settings: HashMap<String, Value>,
    /// Whether this profile's settings are enforced (cannot be overridden).
    pub enforced: bool,
}

impl MdmProfile {
    /// Create a new MDM profile.
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            version: 1,
            settings: HashMap::new(),
            enforced: true,
        }
    }

    /// Builder-style setter for version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Builder-style setter for enforced flag.
    pub fn with_enforced(mut self, enforced: bool) -> Self {
        self.enforced = enforced;
        self
    }

    /// Insert a setting into the profile.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.settings.insert(key.into(), value);
    }

    /// Get a setting from the profile.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.settings.get(key)
    }

    /// Number of settings in the profile.
    pub fn len(&self) -> usize {
        self.settings.len()
    }

    /// Whether the profile has no settings.
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

/// MDM profile loader and manager.
pub struct MdmManager {
    profile: Option<MdmProfile>,
    profile_path: Option<PathBuf>,
}

impl MdmManager {
    /// Create a new MDM manager.
    pub fn new() -> Self {
        Self {
            profile: None,
            profile_path: None,
        }
    }

    /// Create a manager with a specific profile path.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            profile: None,
            profile_path: Some(path),
        }
    }

    /// Attempt to load an MDM profile from the configured path.
    ///
    /// On macOS this looks in `~/Library/Managed Preferences/`.
    /// On other platforms it uses the explicitly configured path or returns `None`.
    pub fn load_mdm_profile(&mut self) -> Result<Option<MdmProfile>> {
        let path = match &self.profile_path {
            Some(p) => p.clone(),
            None => self.default_profile_path()?,
        };

        if !path.exists() {
            tracing::debug!("No MDM profile found at {:?}", path);
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)?;
        let profile: MdmProfile = serde_json::from_str(&content)?;
        self.profile = Some(profile.clone());
        Ok(Some(profile))
    }

    /// Apply a loaded MDM profile directly.
    pub fn apply_mdm_profile(&mut self, profile: MdmProfile) -> Result<()> {
        tracing::info!(
            "Applying MDM profile '{}' v{} ({} settings, enforced={})",
            profile.identifier,
            profile.version,
            profile.settings.len(),
            profile.enforced,
        );
        self.profile = Some(profile);
        Ok(())
    }

    /// Check whether a specific key is managed by the MDM profile.
    ///
    /// A key is managed if the profile exists, is enforced, and contains the key.
    pub fn is_key_managed(&self, key: &str) -> bool {
        match &self.profile {
            Some(profile) if profile.enforced => profile.settings.contains_key(key),
            _ => false,
        }
    }

    /// Get the currently loaded profile, if any.
    pub fn profile(&self) -> Option<&MdmProfile> {
        self.profile.as_ref()
    }

    /// Get the list of all managed keys.
    pub fn managed_keys(&self) -> Vec<String> {
        match &self.profile {
            Some(profile) if profile.enforced => {
                profile.settings.keys().cloned().collect()
            }
            _ => Vec::new(),
        }
    }

    /// Resolve the default platform-specific profile path.
    fn default_profile_path(&self) -> Result<PathBuf> {
        // Check for environment variable override first
        if let Ok(path) = std::env::var("RC_MDM_PROFILE_PATH") {
            return Ok(PathBuf::from(path));
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs_sys::home_dir() {
                return Ok(home.join("Library").join("Managed Preferences").join("remote-code.json"));
            }
        }

        // Fallback: no default path on unsupported platforms
        Ok(PathBuf::from("/nonexistent/mdm-profile.json"))
    }
}

impl Default for MdmManager {
    fn default() -> Self {
        Self::new()
    }
}

// Minimal home_dir helper (avoids adding `dirs` crate dependency)
#[allow(dead_code)]
mod dirs_sys {
    use std::path::PathBuf;
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mdm_profile_new() {
        let p = MdmProfile::new("com.example.profile");
        assert_eq!(p.identifier, "com.example.profile");
        assert_eq!(p.version, 1);
        assert!(p.enforced);
        assert!(p.is_empty());
    }

    #[test]
    fn mdm_profile_insert_and_get() {
        let mut p = MdmProfile::new("test");
        p.insert("key1", json!(42));
        assert_eq!(p.len(), 1);
        assert_eq!(p.get("key1"), Some(&json!(42)));
        assert_eq!(p.get("missing"), None);
    }

    #[test]
    fn mdm_profile_with_version() {
        let p = MdmProfile::new("test").with_version(5);
        assert_eq!(p.version, 5);
    }

    #[test]
    fn mdm_profile_with_enforced() {
        let p = MdmProfile::new("test").with_enforced(false);
        assert!(!p.enforced);
    }

    #[test]
    fn mdm_manager_new_has_no_profile() {
        let mgr = MdmManager::new();
        assert!(mgr.profile().is_none());
        assert!(mgr.managed_keys().is_empty());
    }

    #[test]
    fn mdm_manager_apply_profile() {
        let mut mgr = MdmManager::new();
        let mut profile = MdmProfile::new("test-profile");
        profile.insert("security.level", json!("high"));
        profile.insert("audit.enabled", json!(true));

        mgr.apply_mdm_profile(profile).expect("apply");
        assert!(mgr.profile().is_some());
        assert_eq!(mgr.managed_keys().len(), 2);
    }

    #[test]
    fn mdm_manager_is_key_managed() {
        let mut mgr = MdmManager::new();
        let mut profile = MdmProfile::new("test");
        profile.insert("managed.key", json!(1));
        mgr.apply_mdm_profile(profile).expect("apply");

        assert!(mgr.is_key_managed("managed.key"));
        assert!(!mgr.is_key_managed("unmanaged.key"));
    }

    #[test]
    fn mdm_manager_not_enforced_not_managed() {
        let mut mgr = MdmManager::new();
        let mut profile = MdmProfile::new("test").with_enforced(false);
        profile.insert("key", json!(1));
        mgr.apply_mdm_profile(profile).expect("apply");

        assert!(!mgr.is_key_managed("key"));
        assert!(mgr.managed_keys().is_empty());
    }

    #[test]
    fn mdm_manager_load_nonexistent() {
        let mut mgr = MdmManager::with_path(PathBuf::from("/nonexistent/profile.json"));
        let result = mgr.load_mdm_profile().expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn mdm_manager_load_valid_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("profile.json");
        let profile = MdmProfile::new("test-file").with_version(3);
        let json = serde_json::to_string(&profile).expect("serialize");
        std::fs::write(&path, json).expect("write");

        let mut mgr = MdmManager::with_path(path);
        let loaded = mgr.load_mdm_profile().expect("load");
        assert!(loaded.is_some());
        assert_eq!(loaded.expect("some").identifier, "test-file");
    }

    #[test]
    fn serde_roundtrip_mdm_profile() {
        let mut p = MdmProfile::new("com.test").with_version(2);
        p.insert("k1", json!("v1"));
        p.insert("k2", json!(42));
        let json_str = serde_json::to_string(&p).expect("serialize");
        let back: MdmProfile = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(back.identifier, p.identifier);
        assert_eq!(back.version, p.version);
        assert_eq!(back.settings.len(), 2);
    }

    #[test]
    fn default_trait() {
        let mgr = MdmManager::default();
        assert!(mgr.profile().is_none());
    }
}
