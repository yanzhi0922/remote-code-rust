use std::collections::HashMap;

use serde_json::Value;

use crate::types::{TelemetryEvent, TelemetryEventName, TelemetryEventSubscription, SubscriptionType};

/// Error type for telemetry client operations.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryClientError {
    #[error("Client is not initialized")]
    NotInitialized,

    #[error("Telemetry is disabled")]
    Disabled,

    #[error("Capture failed: {0}")]
    CaptureFailed(String),
}

/// Trait for telemetry clients that can capture events and exceptions.
pub trait TelemetryClient: Send + Sync {
    /// Capture a telemetry event.
    fn capture(&self, event: TelemetryEvent);

    /// Capture an exception/error.
    fn capture_exception(&self, error: &dyn std::error::Error, additional_properties: Option<HashMap<String, Value>>);

    /// Update the telemetry state based on user preference.
    fn update_telemetry_state(&mut self, did_user_opt_in: bool);

    /// Check if telemetry is currently enabled.
    fn is_telemetry_enabled(&self) -> bool;

    /// Shutdown the client and flush any pending events.
    fn shutdown(&self);
}

/// A base implementation of a telemetry client with common logic.
///
/// Provides subscription-based event filtering and telemetry state management.
/// Subclasses should override `capture` and `capture_exception` for specific backends.
#[derive(Debug)]
pub struct BaseTelemetryClient {
    /// Whether telemetry is enabled.
    telemetry_enabled: bool,
    /// Optional subscription filter for events.
    subscription: Option<TelemetryEventSubscription>,
    /// Whether debug mode is enabled.
    debug: bool,
    /// Buffer of captured events (for testing/inspection).
    captured_events: std::sync::Mutex<Vec<TelemetryEvent>>,
}

impl BaseTelemetryClient {
    /// Create a new base telemetry client.
    pub fn new(subscription: Option<TelemetryEventSubscription>, debug: bool) -> Self {
        Self {
            telemetry_enabled: false,
            subscription,
            debug,
            captured_events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Check if an event should be captured based on the subscription filter.
    pub fn is_event_capturable(&self, event_name: &TelemetryEventName) -> bool {
        match &self.subscription {
            None => true,
            Some(sub) => match sub.subscription_type {
                SubscriptionType::Include => sub.events.contains(event_name),
                SubscriptionType::Exclude => !sub.events.contains(event_name),
            },
        }
    }

    /// Get all captured events (for testing).
    pub fn get_captured_events(&self) -> Vec<TelemetryEvent> {
        self.captured_events.lock().unwrap().clone()
    }

    /// Check if debug mode is enabled.
    pub fn is_debug(&self) -> bool {
        self.debug
    }

    /// Check if telemetry is currently enabled.
    pub fn is_telemetry_enabled(&self) -> bool {
        self.telemetry_enabled
    }

    /// Set telemetry enabled state.
    pub fn set_telemetry_enabled(&mut self, enabled: bool) {
        self.telemetry_enabled = enabled;
    }
}

impl TelemetryClient for BaseTelemetryClient {
    fn capture(&self, event: TelemetryEvent) {
        if !self.telemetry_enabled {
            return;
        }

        if !self.is_event_capturable(&event.event_name) {
            return;
        }

        if self.debug {
            eprintln!(
                "[Telemetry] Capturing event: {:?} with properties: {:?}",
                event.event_name, event.properties
            );
        }

        self.captured_events.lock().unwrap().push(event);
    }

    fn capture_exception(&self, error: &dyn std::error::Error, additional_properties: Option<HashMap<String, Value>>) {
        if !self.telemetry_enabled {
            return;
        }

        let mut properties = HashMap::new();
        properties.insert("error_message".to_string(), Value::String(error.to_string()));

        if let Some(additional) = additional_properties {
            properties.extend(additional);
        }

        let event = TelemetryEvent {
            event_name: TelemetryEventName::SchemaValidationError,
            properties: Some(properties),
        };

        self.captured_events.lock().unwrap().push(event);
    }

    fn update_telemetry_state(&mut self, did_user_opt_in: bool) {
        self.telemetry_enabled = did_user_opt_in;
    }

    fn is_telemetry_enabled(&self) -> bool {
        self.telemetry_enabled
    }

    fn shutdown(&self) {
        // Flush any pending events - in a real implementation this would
        // send buffered events to the telemetry backend
        if self.debug {
            eprintln!("[Telemetry] Shutting down client");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_client_new() {
        let client = BaseTelemetryClient::new(None, false);
        assert!(!client.is_telemetry_enabled());
        assert!(!client.is_debug());
    }

    #[test]
    fn test_base_client_new_with_debug() {
        let client = BaseTelemetryClient::new(None, true);
        assert!(client.is_debug());
    }

    #[test]
    fn test_update_telemetry_state_enabled() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        assert!(client.is_telemetry_enabled());
    }

    #[test]
    fn test_update_telemetry_state_disabled() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        client.update_telemetry_state(false);
        assert!(!client.is_telemetry_enabled());
    }

    #[test]
    fn test_capture_event_disabled() {
        let client = BaseTelemetryClient::new(None, false);
        client.capture(TelemetryEvent {
            event_name: TelemetryEventName::TaskCreated,
            properties: None,
        });
        assert!(client.get_captured_events().is_empty());
    }

    #[test]
    fn test_capture_event_enabled() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        client.capture(TelemetryEvent {
            event_name: TelemetryEventName::TaskCreated,
            properties: None,
        });
        assert_eq!(client.get_captured_events().len(), 1);
    }

    #[test]
    fn test_is_event_capturable_no_subscription() {
        let client = BaseTelemetryClient::new(None, false);
        assert!(client.is_event_capturable(&TelemetryEventName::TaskCreated));
        assert!(client.is_event_capturable(&TelemetryEventName::ToolUsed));
    }

    #[test]
    fn test_is_event_capturable_include_subscription() {
        let sub = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Include,
            events: vec![TelemetryEventName::TaskCreated, TelemetryEventName::TaskCompleted],
        };
        let client = BaseTelemetryClient::new(Some(sub), false);
        assert!(client.is_event_capturable(&TelemetryEventName::TaskCreated));
        assert!(!client.is_event_capturable(&TelemetryEventName::ToolUsed));
    }

    #[test]
    fn test_is_event_capturable_exclude_subscription() {
        let sub = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Exclude,
            events: vec![TelemetryEventName::TaskCreated],
        };
        let client = BaseTelemetryClient::new(Some(sub), false);
        assert!(!client.is_event_capturable(&TelemetryEventName::TaskCreated));
        assert!(client.is_event_capturable(&TelemetryEventName::ToolUsed));
    }

    #[test]
    fn test_capture_with_include_subscription() {
        let sub = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Include,
            events: vec![TelemetryEventName::TaskCreated],
        };
        let mut client = BaseTelemetryClient::new(Some(sub), false);
        client.update_telemetry_state(true);

        client.capture(TelemetryEvent {
            event_name: TelemetryEventName::TaskCreated,
            properties: None,
        });
        client.capture(TelemetryEvent {
            event_name: TelemetryEventName::ToolUsed,
            properties: None,
        });

        let events = client.get_captured_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, TelemetryEventName::TaskCreated);
    }

    #[test]
    fn test_capture_exception_disabled() {
        let client = BaseTelemetryClient::new(None, false);
        client.capture_exception(&std::io::Error::new(std::io::ErrorKind::Other, "test"), None);
        assert!(client.get_captured_events().is_empty());
    }

    #[test]
    fn test_capture_exception_enabled() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        client.capture_exception(&std::io::Error::new(std::io::ErrorKind::Other, "test error"), None);
        let events = client.get_captured_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_shutdown() {
        let client = BaseTelemetryClient::new(None, true);
        client.shutdown();
        // Should not panic
    }

    #[test]
    fn test_multiple_captures() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        for _ in 0..5 {
            client.capture(TelemetryEvent {
                event_name: TelemetryEventName::TaskCreated,
                properties: None,
            });
        }
        assert_eq!(client.get_captured_events().len(), 5);
    }

    #[test]
    fn test_capture_with_properties() {
        let mut client = BaseTelemetryClient::new(None, false);
        client.update_telemetry_state(true);
        let mut props = HashMap::new();
        props.insert("task_id".to_string(), Value::String("abc".to_string()));
        client.capture(TelemetryEvent {
            event_name: TelemetryEventName::TaskCreated,
            properties: Some(props),
        });
        let events = client.get_captured_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].properties.is_some());
    }
}
