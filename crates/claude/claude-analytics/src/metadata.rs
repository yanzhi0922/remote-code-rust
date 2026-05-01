//! Event metadata types and builder.
//!
//! Provides `EventMetadata` for attaching context to analytics events,
//! along with a builder pattern and a utility to strip protocol fields.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// EventMetadata
// ---------------------------------------------------------------------------

/// Metadata attached to an analytics event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EventMetadata {
    /// Session ID associated with the event.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Model used for the operation.
    #[serde(default)]
    pub model: Option<String>,
    /// Provider that handled the operation.
    #[serde(default)]
    pub provider: Option<String>,
    /// Tool that was invoked (if applicable).
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Duration of the operation in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Number of tokens consumed.
    #[serde(default)]
    pub token_count: Option<u64>,
    /// Whether the operation succeeded.
    #[serde(default)]
    pub success: bool,
    /// Type of error if the operation failed.
    #[serde(default)]
    pub error_type: Option<String>,
    /// Additional custom properties.
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl EventMetadata {
    /// Create a new empty metadata instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Set the model name.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the provider name.
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set the tool name.
    pub fn with_tool_name(mut self, tool: impl Into<String>) -> Self {
        self.tool_name = Some(tool.into());
        self
    }

    /// Set the duration in milliseconds.
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Set the token count.
    pub fn with_token_count(mut self, count: u64) -> Self {
        self.token_count = Some(count);
        self
    }

    /// Set whether the operation succeeded.
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set the error type.
    pub fn with_error_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = Some(error_type.into());
        self
    }

    /// Add a custom extra field.
    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }
}

// ---------------------------------------------------------------------------
// AnalyticsEvent
// ---------------------------------------------------------------------------

/// A named analytics event with metadata and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyticsEvent {
    /// Event name (e.g. "tool_use", "query", "compact").
    pub name: String,
    /// Event metadata.
    pub metadata: EventMetadata,
    /// When the event was created.
    pub timestamp: DateTime<Utc>,
}

impl AnalyticsEvent {
    /// Create a new analytics event with the given name and metadata.
    pub fn new(name: impl Into<String>, metadata: EventMetadata) -> Self {
        Self {
            name: name.into(),
            metadata,
            timestamp: Utc::now(),
        }
    }

    /// Create a simple event with just a name.
    pub fn simple(name: impl Into<String>) -> Self {
        Self::new(name, EventMetadata::default())
    }
}

// ---------------------------------------------------------------------------
// strip_proto_fields
// ---------------------------------------------------------------------------

/// Remove `_PROTO_*` keys from a metadata's extra fields.
///
/// Protocol fields are internal implementation details that should not
/// be sent to analytics backends.
pub fn strip_proto_fields(metadata: &mut EventMetadata) {
    metadata.extra.retain(|key, _| !key.starts_with("_PROTO_"));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_builder_pattern() {
        let meta = EventMetadata::new()
            .with_session_id("sess-123")
            .with_model("gpt-4")
            .with_provider("openai")
            .with_tool_name("read_file")
            .with_duration_ms(150)
            .with_token_count(500)
            .with_success(true);
        assert_eq!(meta.session_id, Some("sess-123".to_string()));
        assert_eq!(meta.model, Some("gpt-4".to_string()));
        assert_eq!(meta.provider, Some("openai".to_string()));
        assert_eq!(meta.tool_name, Some("read_file".to_string()));
        assert_eq!(meta.duration_ms, Some(150));
        assert_eq!(meta.token_count, Some(500));
        assert!(meta.success);
    }

    #[test]
    fn metadata_with_error() {
        let meta = EventMetadata::new()
            .with_success(false)
            .with_error_type("timeout");
        assert!(!meta.success);
        assert_eq!(meta.error_type, Some("timeout".to_string()));
    }

    #[test]
    fn metadata_with_extra_fields() {
        let meta = EventMetadata::new()
            .with_extra("custom_key", serde_json::json!("value"))
            .with_extra("count", serde_json::json!(42));
        assert_eq!(meta.extra.len(), 2);
        assert_eq!(meta.extra["custom_key"], serde_json::json!("value"));
    }

    #[test]
    fn analytics_event_creation() {
        let meta = EventMetadata::new().with_tool_name("bash");
        let event = AnalyticsEvent::new("tool_use", meta);
        assert_eq!(event.name, "tool_use");
        assert_eq!(event.metadata.tool_name, Some("bash".to_string()));
    }

    #[test]
    fn analytics_event_simple() {
        let event = AnalyticsEvent::simple("app_start");
        assert_eq!(event.name, "app_start");
        assert!(event.metadata.session_id.is_none());
    }

    #[test]
    fn strip_proto_fields_removes_prefixed_keys() {
        let mut meta = EventMetadata::new()
            .with_extra("_PROTO_VERSION", serde_json::json!("1.0"))
            .with_extra("_PROTO_INTERNAL", serde_json::json!("secret"))
            .with_extra("visible_key", serde_json::json!("keep"));
        strip_proto_fields(&mut meta);
        assert_eq!(meta.extra.len(), 1);
        assert!(meta.extra.contains_key("visible_key"));
        assert!(!meta.extra.contains_key("_PROTO_VERSION"));
    }

    #[test]
    fn metadata_serialization_roundtrip() {
        let meta = EventMetadata::new()
            .with_session_id("s1")
            .with_model("m1")
            .with_success(true);
        let json = serde_json::to_string(&meta).expect("serialize");
        let parsed: EventMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(meta, parsed);
    }

    #[test]
    fn event_serialization_roundtrip() {
        let event = AnalyticsEvent::new("test_event", EventMetadata::new().with_duration_ms(100));
        let json = serde_json::to_string(&event).expect("serialize");
        let parsed: AnalyticsEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event.name, parsed.name);
        assert_eq!(event.metadata.duration_ms, parsed.metadata.duration_ms);
    }
}
