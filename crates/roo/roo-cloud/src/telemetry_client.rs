/// Telemetry client for sending events to the cloud.
/// Mirrors packages/cloud/src/TelemetryClient.ts

use crate::cloud_api::CloudApi;
use crate::types::CloudError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A telemetry event to be sent to the cloud.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event: String,
    pub properties: Option<HashMap<String, Value>>,
    pub timestamp: Option<u64>,
}

/// Telemetry client for sending events to the Roo Code cloud.
pub struct TelemetryClient {
    api: CloudApi,
    enabled: bool,
}

impl TelemetryClient {
    /// Create a new TelemetryClient.
    pub fn new(version: Option<&str>, enabled: bool) -> Self {
        Self {
            api: CloudApi::new(version),
            enabled,
        }
    }

    /// Check if telemetry is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable telemetry.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Send a telemetry event.
    pub async fn send_event(
        &self,
        event: &str,
        properties: Option<HashMap<String, Value>>,
        token: &str,
    ) -> Result<(), CloudError> {
        if !self.enabled {
            return Ok(());
        }

        let telemetry_event = TelemetryEvent {
            event: event.to_string(),
            properties,
            timestamp: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ),
        };

        let body = serde_json::to_value(&telemetry_event)
            .map_err(|e| CloudError::SerializationError(e.to_string()))?;

        self.api.post("/api/extension/telemetry", Some(body), token).await?;

        Ok(())
    }

    /// Send a batch of telemetry events.
    pub async fn send_batch(
        &self,
        events: Vec<TelemetryEvent>,
        token: &str,
    ) -> Result<(), CloudError> {
        if !self.enabled || events.is_empty() {
            return Ok(());
        }

        let body = serde_json::to_value(&events)
            .map_err(|e| CloudError::SerializationError(e.to_string()))?;

        self.api.post("/api/extension/telemetry/batch", Some(body), token).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = TelemetryClient::new(Some("1.0.0"), true);
        assert!(client.is_enabled());
    }

    #[test]
    fn test_set_enabled() {
        let mut client = TelemetryClient::new(None, true);
        assert!(client.is_enabled());
        client.set_enabled(false);
        assert!(!client.is_enabled());
    }

    #[test]
    fn test_telemetry_event_serialization() {
        let event = TelemetryEvent {
            event: "task_completed".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("duration_ms".to_string(), Value::Number(1234.into()));
                props
            }),
            timestamp: Some(1234567890),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("task_completed"));
        assert!(json.contains("duration_ms"));

        let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();
        assert_eq!("task_completed", deserialized.event);
    }

    #[tokio::test]
    async fn test_send_event_disabled() {
        let client = TelemetryClient::new(None, false);
        let result = client.send_event("test", None, "token").await;
        assert!(result.is_ok()); // Should succeed silently when disabled
    }

    #[tokio::test]
    async fn test_send_batch_disabled() {
        let client = TelemetryClient::new(None, false);
        let events = vec![TelemetryEvent {
            event: "test".to_string(),
            properties: None,
            timestamp: None,
        }];
        let result = client.send_batch(events, "token").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_batch_empty() {
        let client = TelemetryClient::new(None, true);
        let result = client.send_batch(vec![], "token").await;
        assert!(result.is_ok());
    }
}
