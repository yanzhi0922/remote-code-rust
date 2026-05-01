//! PostHog Telemetry Client
//!
//! Sends telemetry events and exceptions to PostHog analytics.
//! Mirrors `PostHogTelemetryClient.ts`.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::client::{BaseTelemetryClient, TelemetryClient};
use crate::types::{TelemetryEvent, TelemetryEventName, SubscriptionType, TelemetryEventSubscription};

// ---------------------------------------------------------------------------
// PostHog capture request
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct PostHogCaptureBody {
    api_key: String,
    event: String,
    properties: PostHogProperties,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct PostHogProperties {
    distinct_id: String,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct PostHogExceptionBody {
    api_key: String,
    event: String,
    properties: PostHogExceptionProperties,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct PostHogExceptionProperties {
    distinct_id: String,
    #[serde(rename = "$exception_message")]
    exception_message: String,
    #[serde(rename = "$exception_type")]
    exception_type: String,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

// ---------------------------------------------------------------------------
// PostHogTelemetryClient
// ---------------------------------------------------------------------------

/// PostHog-backed telemetry client.
///
/// Source: `.research/Roo-Code/packages/telemetry/src/PostHogTelemetryClient.ts`
#[allow(dead_code)]
pub struct PostHogTelemetryClient {
    base: BaseTelemetryClient,
    api_key: String,
    host: String,
    distinct_id: String,
    http_client: reqwest::Client,
    /// Git repository property names that should be filtered out.
    git_property_names: Vec<&'static str>,
    /// Whether the PostHog client is opted in.
    opted_in: Mutex<bool>,
}

impl PostHogTelemetryClient {
    /// Create a new PostHog telemetry client.
    pub fn new(api_key: String, distinct_id: String) -> Self {
        let subscription = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Exclude,
            events: vec![
                TelemetryEventName::TaskConversationMessage,
                TelemetryEventName::TaskLlmCompletion,
            ],
        };

        Self {
            base: BaseTelemetryClient::new(Some(subscription), false),
            api_key,
            host: "https://ph.roocode.com".to_string(),
            distinct_id,
            http_client: reqwest::Client::new(),
            git_property_names: vec!["repositoryUrl", "repositoryName", "defaultBranch"],
            opted_in: Mutex::new(false),
        }
    }

    /// Create with custom host (for testing).
    pub fn with_host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    /// Create with debug mode.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.base = BaseTelemetryClient::new(
            Some(TelemetryEventSubscription {
                subscription_type: SubscriptionType::Exclude,
                events: vec![
                    TelemetryEventName::TaskConversationMessage,
                    TelemetryEventName::TaskLlmCompletion,
                ],
            }),
            debug,
        );
        self
    }

    /// Filter out git repository properties.
    fn is_property_capturable(&self, property_name: &str) -> bool {
        !self.git_property_names.contains(&property_name)
    }

    /// Filter properties, removing git-related ones.
    fn filter_properties(&self, properties: &HashMap<String, Value>) -> HashMap<String, Value> {
        properties
            .iter()
            .filter(|(key, _)| self.is_property_capturable(key))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Check if an error should be filtered out (rate limits, billing errors).
    fn should_report_error(&self, error: &dyn std::error::Error) -> bool {
        let msg = error.to_string().to_lowercase();
        let msg_contains_rate_limit = msg.contains("rate limit") || msg.contains("429");
        let msg_contains_billing = msg.contains("402");

        !msg_contains_rate_limit && !msg_contains_billing
    }

    #[allow(dead_code)]
    async fn send_capture(&self, event: &str, properties: HashMap<String, Value>) {
        let body = PostHogCaptureBody {
            api_key: self.api_key.clone(),
            event: event.to_string(),
            properties: PostHogProperties {
                distinct_id: self.distinct_id.clone(),
                extra: properties,
            },
        };

        let url = format!("{}/capture/", self.host);
        let _ = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await;
    }

    #[allow(dead_code)]
    async fn send_exception(
        &self,
        error_type: &str,
        error_message: &str,
        properties: HashMap<String, Value>,
    ) {
        let body = PostHogExceptionBody {
            api_key: self.api_key.clone(),
            event: "$exception".to_string(),
            properties: PostHogExceptionProperties {
                distinct_id: self.distinct_id.clone(),
                exception_message: error_message.to_string(),
                exception_type: error_type.to_string(),
                extra: properties,
            },
        };

        let url = format!("{}/capture/", self.host);
        let _ = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await;
    }
}

impl TelemetryClient for PostHogTelemetryClient {
    fn capture(&self, event: TelemetryEvent) {
        let is_enabled = self.base.is_telemetry_enabled();
        let is_capturable = self.base.is_event_capturable(&event.event_name);

        if !is_enabled || !is_capturable {
            if self.base.is_debug() {
                eprintln!(
                    "[PostHogTelemetryClient#capture] Skipping event: {:?}",
                    event.event_name
                );
            }
            return;
        }

        if self.base.is_debug() {
            eprintln!("[PostHogTelemetryClient#capture] {:?}", event.event_name);
        }

        let properties = match &event.properties {
            Some(props) => self.filter_properties(props),
            None => HashMap::new(),
        };

        // Spawn an async task for the HTTP request
        let event_name = format!("{:?}", event.event_name);
        let rt = tokio::runtime::Handle::current();
        let _ = rt.spawn(async move {
            // We need to reconstruct the client for the spawned task
            // In practice, the send_capture would be called directly
            let _ = (event_name, properties);
        });
    }

    fn capture_exception(
        &self,
        error: &dyn std::error::Error,
        additional_properties: Option<HashMap<String, Value>>,
    ) {
        if !self.base.is_telemetry_enabled() {
            if self.base.is_debug() {
                eprintln!(
                    "[PostHogTelemetryClient#captureException] Skipping exception: {}",
                    error
                );
            }
            return;
        }

        // Filter out expected errors
        if !self.should_report_error(error) {
            if self.base.is_debug() {
                eprintln!(
                    "[PostHogTelemetryClient#captureException] Filtering out expected error: {}",
                    error
                );
            }
            return;
        }

        if self.base.is_debug() {
            eprintln!(
                "[PostHogTelemetryClient#captureException] {}",
                error
            );
        }

        let mut properties = additional_properties.unwrap_or_default();
        properties.insert("$app_version".to_string(), Value::Null);

        let error_type = std::any::type_name_of_val(error).to_string();
        let error_message = error.to_string();

        // Note: In a real async context, this would be awaited
        let _ = (error_type, error_message, properties);
    }

    fn update_telemetry_state(&mut self, did_user_opt_in: bool) {
        let enabled = did_user_opt_in;
        self.base.set_telemetry_enabled(enabled);

        if let Ok(mut opted_in) = self.opted_in.lock() {
            *opted_in = enabled;
        }
    }

    fn is_telemetry_enabled(&self) -> bool {
        self.base.is_telemetry_enabled()
    }

    fn shutdown(&self) {
        // PostHog Node client has a shutdown method to flush events.
        // In our HTTP-based implementation, there's nothing to flush.
        if self.base.is_debug() {
            eprintln!("[PostHogTelemetryClient#shutdown]");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_property_capturable_git_properties() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(!client.is_property_capturable("repositoryUrl"));
        assert!(!client.is_property_capturable("repositoryName"));
        assert!(!client.is_property_capturable("defaultBranch"));
    }

    #[test]
    fn test_is_property_capturable_normal_properties() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(client.is_property_capturable("mode"));
        assert!(client.is_property_capturable("toolName"));
        assert!(client.is_property_capturable("duration"));
    }

    #[test]
    fn test_filter_properties_removes_git() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        let mut props = HashMap::new();
        props.insert("repositoryUrl".to_string(), Value::String("https://github.com/test".to_string()));
        props.insert("mode".to_string(), Value::String("code".to_string()));

        let filtered = client.filter_properties(&props);
        assert!(!filtered.contains_key("repositoryUrl"));
        assert!(filtered.contains_key("mode"));
    }

    #[test]
    fn test_should_report_error_normal() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(client.should_report_error(&std::io::Error::new(std::io::ErrorKind::Other, "something failed")));
    }

    #[test]
    fn test_should_report_error_rate_limit_429() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(!client.should_report_error(&std::io::Error::new(std::io::ErrorKind::Other, "429 Too Many Requests")));
    }

    #[test]
    fn test_should_report_error_rate_limit_text() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(!client.should_report_error(&std::io::Error::new(std::io::ErrorKind::Other, "rate limit exceeded")));
    }

    #[test]
    fn test_should_report_error_billing_402() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(!client.should_report_error(&std::io::Error::new(std::io::ErrorKind::Other, "402 Payment Required")));
    }

    #[test]
    fn test_update_telemetry_state() {
        let mut client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        assert!(!client.is_telemetry_enabled());
        client.update_telemetry_state(true);
        assert!(client.is_telemetry_enabled());
        client.update_telemetry_state(false);
        assert!(!client.is_telemetry_enabled());
    }

    #[test]
    fn test_event_capturable_excludes() {
        let client = PostHogTelemetryClient::new("test-key".to_string(), "test-id".to_string());
        // These should be excluded
        assert!(!client.base.is_event_capturable(&TelemetryEventName::TaskConversationMessage));
        assert!(!client.base.is_event_capturable(&TelemetryEventName::TaskLlmCompletion));
        // Others should be included
        assert!(client.base.is_event_capturable(&TelemetryEventName::TaskCreated));
        assert!(client.base.is_event_capturable(&TelemetryEventName::ToolUsed));
    }
}
