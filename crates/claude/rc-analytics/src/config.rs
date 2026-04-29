//! Analytics configuration.
//!
//! Provides `AnalyticsConfig` with sensible defaults (analytics disabled
//! by default for privacy).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AnalyticsConfig
// ---------------------------------------------------------------------------

/// Configuration for the analytics system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyticsConfig {
    /// Whether analytics collection is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Optional endpoint URL for sending analytics events.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Optional API key for authenticating with the analytics endpoint.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Interval in seconds between automatic flush operations.
    #[serde(default = "default_flush_interval")]
    pub flush_interval_secs: u64,
    /// Maximum number of events to buffer before forcing a flush.
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,
}

fn default_flush_interval() -> u64 {
    30
}

fn default_max_queue_size() -> usize {
    1000
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            api_key: None,
            flush_interval_secs: default_flush_interval(),
            max_queue_size: default_max_queue_size(),
        }
    }
}

impl AnalyticsConfig {
    /// Create a new config with analytics enabled and the given endpoint.
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            enabled: true,
            endpoint: Some(endpoint.into()),
            api_key: None,
            flush_interval_secs: default_flush_interval(),
            max_queue_size: default_max_queue_size(),
        }
    }

    /// Set the API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the flush interval in seconds.
    pub fn with_flush_interval(mut self, secs: u64) -> Self {
        self.flush_interval_secs = secs;
        self
    }

    /// Set the max queue size.
    pub fn with_max_queue_size(mut self, size: usize) -> Self {
        self.max_queue_size = size;
        self
    }

    /// Check if an endpoint is configured.
    pub fn has_endpoint(&self) -> bool {
        self.endpoint.as_ref().is_some_and(|e| !e.is_empty())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_analytics_disabled() {
        let config = AnalyticsConfig::default();
        assert!(!config.enabled);
        assert!(config.endpoint.is_none());
        assert!(config.api_key.is_none());
        assert_eq!(config.flush_interval_secs, 30);
        assert_eq!(config.max_queue_size, 1000);
    }

    #[test]
    fn config_with_endpoint_enables_analytics() {
        let config = AnalyticsConfig::with_endpoint("https://analytics.example.com");
        assert!(config.enabled);
        assert_eq!(
            config.endpoint,
            Some("https://analytics.example.com".to_string())
        );
    }

    #[test]
    fn config_builder_pattern() {
        let config = AnalyticsConfig::with_endpoint("https://example.com")
            .with_api_key("secret-key")
            .with_flush_interval(60)
            .with_max_queue_size(500);
        assert!(config.enabled);
        assert_eq!(config.api_key, Some("secret-key".to_string()));
        assert_eq!(config.flush_interval_secs, 60);
        assert_eq!(config.max_queue_size, 500);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = AnalyticsConfig::with_endpoint("https://example.com").with_api_key("key123");
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: AnalyticsConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn has_endpoint_checks() {
        let config = AnalyticsConfig::default();
        assert!(!config.has_endpoint());

        let config = AnalyticsConfig::with_endpoint("https://example.com");
        assert!(config.has_endpoint());
    }
}
