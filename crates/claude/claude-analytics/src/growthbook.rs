//! Feature flag system (GrowthBook-style).
//!
//! Provides a feature flag system for controlling feature availability
//! at runtime, with support for refreshing flags from a remote source.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// FeatureFlag
// ---------------------------------------------------------------------------

/// A single feature flag with its state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureFlag {
    /// Unique key identifying the feature flag.
    pub key: String,
    /// Whether the feature is enabled.
    pub enabled: bool,
    /// Optional variation value for the flag.
    #[serde(default)]
    pub variation: Option<String>,
}

impl FeatureFlag {
    /// Create a new enabled feature flag.
    pub fn enabled(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            enabled: true,
            variation: None,
        }
    }

    /// Create a new disabled feature flag.
    pub fn disabled(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            enabled: false,
            variation: None,
        }
    }

    /// Set the variation for this flag.
    pub fn with_variation(mut self, variation: impl Into<String>) -> Self {
        self.variation = Some(variation.into());
        self
    }
}

// ---------------------------------------------------------------------------
// GrowthBookConfig
// ---------------------------------------------------------------------------

/// Configuration for the GrowthBook feature flag client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrowthBookConfig {
    /// API endpoint for fetching feature flags.
    pub api_endpoint: String,
    /// Decryption key for encrypted flag payloads.
    #[serde(default)]
    pub decryption_key: Option<String>,
}

impl Default for GrowthBookConfig {
    fn default() -> Self {
        Self {
            api_endpoint: "https://api.growthbook.io".to_string(),
            decryption_key: None,
        }
    }
}

// ---------------------------------------------------------------------------
// FeatureFlags
// ---------------------------------------------------------------------------

/// Collection of feature flags with lookup capabilities.
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    flags: Arc<Mutex<HashMap<String, FeatureFlag>>>,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureFlags {
    /// Create a new empty feature flag collection.
    pub fn new() -> Self {
        Self {
            flags: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a feature flag collection with the default flags.
    pub fn with_defaults() -> Self {
        let flags = Self::new();
        flags.set_flag(FeatureFlag::disabled("auto_compact_enabled"));
        flags.set_flag(FeatureFlag::disabled("fast_mode_enabled"));
        flags.set_flag(FeatureFlag::enabled("skill_search_enabled"));
        flags.set_flag(FeatureFlag::disabled("auto_mode_enabled"));
        flags
    }

    /// Set a feature flag.
    pub fn set_flag(&self, flag: FeatureFlag) {
        if let Ok(mut flags) = self.flags.lock() {
            flags.insert(flag.key.clone(), flag);
        }
    }

    /// Check if a feature flag is enabled.
    ///
    /// Returns `false` if the flag does not exist or is disabled.
    pub fn is_enabled(&self, flag_key: &str) -> bool {
        self.flags
            .lock()
            .ok()
            .and_then(|flags| flags.get(flag_key).map(|f| f.enabled))
            .unwrap_or(false)
    }

    /// Get the variation value for a feature flag.
    pub fn get_variation(&self, flag_key: &str) -> Option<String> {
        self.flags
            .lock()
            .ok()
            .and_then(|flags| flags.get(flag_key).and_then(|f| f.variation.clone()))
    }

    /// Get all flags as a vector.
    pub fn all_flags(&self) -> Vec<FeatureFlag> {
        self.flags
            .lock()
            .ok()
            .map(|flags| flags.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Refresh flags from a remote source.
    ///
    /// Fetches feature flags from the GrowthBook API endpoint specified in
    /// the configuration. On success, the in-memory flag map is replaced
    /// with the fetched data. On failure, the existing flags are preserved
    /// and the error is logged.
    pub async fn refresh_flags(&self, config: &GrowthBookConfig) -> anyhow::Result<()> {
        if config.api_endpoint.is_empty() {
            return Ok(());
        }
        let client = reqwest::Client::new();
        let resp = client
            .get(&config.api_endpoint)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
                Ok(body) => {
                    if let Some(flags_map) = body.get("features").and_then(|f| f.as_object())
                        && let Ok(mut flags) = self.flags.lock()
                    {
                        flags.clear();
                        for (key, value) in flags_map {
                            if let Ok(flag) = serde_json::from_value::<FeatureFlag>(value.clone()) {
                                flags.insert(key.clone(), flag);
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("GrowthBook: failed to parse response: {e}");
                    Ok(())
                }
            },
            Ok(r) => {
                tracing::warn!("GrowthBook API returned status {}", r.status());
                Ok(())
            }
            Err(e) => {
                tracing::warn!("GrowthBook API request failed: {e}");
                Ok(())
            }
        }
    }

    /// Number of flags currently loaded.
    pub fn len(&self) -> usize {
        self.flags.lock().map(|f| f.len()).unwrap_or(0)
    }

    /// Whether there are no flags loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flag_enabled() {
        let flag = FeatureFlag::enabled("test_flag");
        assert!(flag.enabled);
        assert_eq!(flag.key, "test_flag");
        assert!(flag.variation.is_none());
    }

    #[test]
    fn feature_flag_disabled() {
        let flag = FeatureFlag::disabled("test_flag");
        assert!(!flag.enabled);
    }

    #[test]
    fn feature_flag_with_variation() {
        let flag = FeatureFlag::enabled("test_flag").with_variation("variant_a");
        assert_eq!(flag.variation, Some("variant_a".to_string()));
    }

    #[test]
    fn feature_flags_default_collection() {
        let flags = FeatureFlags::with_defaults();
        assert!(!flags.is_enabled("auto_compact_enabled"));
        assert!(!flags.is_enabled("fast_mode_enabled"));
        assert!(flags.is_enabled("skill_search_enabled"));
        assert!(!flags.is_enabled("auto_mode_enabled"));
    }

    #[test]
    fn feature_flags_set_and_get() {
        let flags = FeatureFlags::new();
        assert!(!flags.is_enabled("my_flag"));

        flags.set_flag(FeatureFlag::enabled("my_flag"));
        assert!(flags.is_enabled("my_flag"));
    }

    #[test]
    fn feature_flags_variation() {
        let flags = FeatureFlags::new();
        assert!(flags.get_variation("my_flag").is_none());

        flags.set_flag(FeatureFlag::enabled("my_flag").with_variation("blue"));
        assert_eq!(flags.get_variation("my_flag"), Some("blue".to_string()));
    }

    #[test]
    fn feature_flags_all_flags() {
        let flags = FeatureFlags::with_defaults();
        let all = flags.all_flags();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn feature_flags_nonexistent_returns_false() {
        let flags = FeatureFlags::new();
        assert!(!flags.is_enabled("does_not_exist"));
    }

    #[test]
    fn growthbook_config_default() {
        let config = GrowthBookConfig::default();
        assert!(!config.api_endpoint.is_empty());
        assert!(config.decryption_key.is_none());
    }

    #[test]
    fn growthbook_config_serialization() {
        let config = GrowthBookConfig {
            api_endpoint: "https://custom.api.com".to_string(),
            decryption_key: Some("secret".to_string()),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: GrowthBookConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[tokio::test]
    async fn refresh_flags_placeholder_succeeds() {
        let flags = FeatureFlags::new();
        let config = GrowthBookConfig::default();
        let result = flags.refresh_flags(&config).await;
        assert!(result.is_ok());
    }
}
