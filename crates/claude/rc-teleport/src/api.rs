//! Teleport API client.
//!
//! Provides HTTP client methods for interacting with the teleport service:
//! fetching sessions, uploading bundles, and listing environments.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::environments::Environment;

// ---------------------------------------------------------------------------
// TeleportConfig
// ---------------------------------------------------------------------------

/// Configuration for the teleport API client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeleportConfig {
    /// Base URL for the teleport API endpoint.
    pub base_url: String,
    /// Authentication headers to include in every request.
    #[serde(default)]
    pub auth_headers: Vec<(String, String)>,
}

impl Default for TeleportConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.remote-code.dev/teleport".to_string(),
            auth_headers: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// TeleportSession
// ---------------------------------------------------------------------------

/// Summary of a teleportable session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeleportSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Session title / description.
    pub title: String,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// Brief summary of messages in the session.
    #[serde(default)]
    pub messages_summary: Vec<MessageSummary>,
}

/// Summary of a single message within a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageSummary {
    /// Role of the message sender (e.g. "user", "assistant").
    pub role: String,
    /// Truncated content preview.
    pub preview: String,
    /// Timestamp of the message.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// TeleportResult
// ---------------------------------------------------------------------------

/// Result of a teleport upload operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeleportResult {
    /// The session ID that was teleported.
    pub session_id: String,
    /// The target environment ID.
    pub environment_id: String,
    /// URL to access the teleported session.
    pub access_url: String,
    /// Timestamp of the teleport operation.
    pub teleported_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// TeleportService
// ---------------------------------------------------------------------------

/// Service client for the teleport API.
///
/// Wraps an HTTP client and provides typed methods for teleport operations.
#[derive(Debug, Clone)]
pub struct TeleportService {
    config: TeleportConfig,
    client: reqwest::Client,
}

impl TeleportService {
    /// Create a new teleport service with the given configuration.
    pub fn new(config: TeleportConfig) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    /// Create a new teleport service with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TeleportConfig::default())
    }

    /// Build the full URL for an API endpoint path.
    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Apply authentication headers to a request builder.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        self.config
            .auth_headers
            .iter()
            .fold(builder, |b, (key, value)| {
                b.header(key.as_str(), value.as_str())
            })
    }

    /// Fetch a teleport session by its ID.
    #[instrument(skip(self), fields(session_id = %session_id))]
    pub async fn fetch_session(&self, session_id: &str) -> Result<TeleportSession> {
        let url = self.url(&format!("sessions/{session_id}"));
        debug!(url = %url, "Fetching teleport session");

        let response = self
            .apply_auth(self.client.get(&url))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch session: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to fetch session: HTTP {status} — {body}");
        }

        let session: TeleportSession = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse session response: {e}"))?;

        Ok(session)
    }

    /// Upload a bundle to a specific session.
    #[instrument(skip(self, bundle_data), fields(session_id = %session_id, bundle_size = bundle_data.len()))]
    pub async fn upload_bundle(&self, session_id: &str, bundle_data: &[u8]) -> Result<()> {
        let url = self.url(&format!("sessions/{session_id}/bundle"));
        debug!(url = %url, size = bundle_data.len(), "Uploading bundle");

        let response = self
            .apply_auth(self.client.put(&url))
            .header("Content-Type", "application/octet-stream")
            .body(bundle_data.to_vec())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to upload bundle: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to upload bundle: HTTP {status} — {body}");
        }

        Ok(())
    }

    /// List all available environments for teleportation.
    #[instrument(skip(self))]
    pub async fn list_environments(&self) -> Result<Vec<Environment>> {
        let url = self.url("environments");
        debug!(url = %url, "Listing environments");

        let response = self
            .apply_auth(self.client.get(&url))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list environments: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list environments: HTTP {status} — {body}");
        }

        let environments: Vec<Environment> = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse environments response: {e}"))?;

        Ok(environments)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teleport_config_default() {
        let config = TeleportConfig::default();
        assert!(!config.base_url.is_empty());
        assert!(config.auth_headers.is_empty());
    }

    #[test]
    fn teleport_config_serialization_roundtrip() {
        let config = TeleportConfig {
            base_url: "https://example.com".to_string(),
            auth_headers: vec![("Authorization".to_string(), "Bearer token".to_string())],
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: TeleportConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn teleport_session_serialization() {
        let session = TeleportSession {
            session_id: "sess-123".to_string(),
            title: "Test Session".to_string(),
            created_at: DateTime::UNIX_EPOCH,
            messages_summary: vec![MessageSummary {
                role: "user".to_string(),
                preview: "Hello".to_string(),
                timestamp: DateTime::UNIX_EPOCH,
            }],
        };
        let json = serde_json::to_string(&session).expect("serialize");
        let parsed: TeleportSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.session_id, parsed.session_id);
        assert_eq!(
            session.messages_summary.len(),
            parsed.messages_summary.len()
        );
    }

    #[test]
    fn teleport_result_serialization() {
        let result = TeleportResult {
            session_id: "sess-456".to_string(),
            environment_id: "env-789".to_string(),
            access_url: "https://example.com/access".to_string(),
            teleported_at: DateTime::UNIX_EPOCH,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: TeleportResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result, parsed);
    }

    #[test]
    fn service_url_construction() {
        let config = TeleportConfig {
            base_url: "https://api.example.com/teleport/".to_string(),
            auth_headers: Vec::new(),
        };
        let service = TeleportService::new(config);
        assert_eq!(
            service.url("sessions/abc"),
            "https://api.example.com/teleport/sessions/abc"
        );
        // Double slashes should be handled
        assert_eq!(
            service.url("/sessions/abc"),
            "https://api.example.com/teleport/sessions/abc"
        );
    }

    #[test]
    fn service_with_defaults_creates_client() {
        let service = TeleportService::with_defaults();
        assert_eq!(service.config.base_url, TeleportConfig::default().base_url);
    }
}
