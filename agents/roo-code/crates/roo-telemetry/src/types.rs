use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Names of telemetry events that can be captured.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventName {
    TaskCreated,
    TaskRestarted,
    TaskCompleted,
    TaskConversationMessage,
    TaskLlmCompletion,
    ModeSwitch,
    ToolUsed,
    CheckpointCreated,
    CheckpointDiffed,
    CheckpointRestored,
    ContextCondensed,
    SlidingWindowTruncation,
    CodeActionUsed,
    PromptEnhanced,
    SchemaValidationError,
    DiffApplicationError,
    ShellIntegrationError,
    ConsecutiveMistakeError,
    TabShown,
    ModeSettingsChanged,
    CustomModeCreated,
    MarketplaceItemInstalled,
    MarketplaceItemRemoved,
    TitleButtonClicked,
    TelemetrySettingsChanged,
}

/// A telemetry event with a name and optional properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// The name of the event.
    pub event_name: TelemetryEventName,
    /// Optional properties associated with the event.
    pub properties: Option<HashMap<String, Value>>,
}

/// Telemetry setting state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetrySetting {
    /// Telemetry is explicitly enabled.
    Enabled,
    /// Telemetry is explicitly disabled.
    Disabled,
    /// Telemetry setting is not configured (unset).
    Unset,
}

/// Subscription type for filtering which events a client captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionType {
    /// Only capture events in the list (allowlist).
    Include,
    /// Capture all events except those in the list (denylist).
    Exclude,
}

/// A subscription that filters which events a telemetry client captures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEventSubscription {
    /// The type of subscription filter.
    pub subscription_type: SubscriptionType,
    /// The list of event names to include or exclude.
    pub events: Vec<TelemetryEventName>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_event_name_serialization() {
        let name = TelemetryEventName::TaskCreated;
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"task_created\"");
        let deserialized: TelemetryEventName = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TelemetryEventName::TaskCreated);
    }

    #[test]
    fn test_telemetry_event_name_all_variants() {
        let variants = [
            TelemetryEventName::TaskCreated,
            TelemetryEventName::TaskRestarted,
            TelemetryEventName::TaskCompleted,
            TelemetryEventName::TaskConversationMessage,
            TelemetryEventName::TaskLlmCompletion,
            TelemetryEventName::ModeSwitch,
            TelemetryEventName::ToolUsed,
            TelemetryEventName::CheckpointCreated,
            TelemetryEventName::CheckpointDiffed,
            TelemetryEventName::CheckpointRestored,
            TelemetryEventName::ContextCondensed,
            TelemetryEventName::SlidingWindowTruncation,
            TelemetryEventName::CodeActionUsed,
            TelemetryEventName::PromptEnhanced,
            TelemetryEventName::SchemaValidationError,
            TelemetryEventName::DiffApplicationError,
            TelemetryEventName::ShellIntegrationError,
            TelemetryEventName::ConsecutiveMistakeError,
            TelemetryEventName::TabShown,
            TelemetryEventName::ModeSettingsChanged,
            TelemetryEventName::CustomModeCreated,
            TelemetryEventName::MarketplaceItemInstalled,
            TelemetryEventName::MarketplaceItemRemoved,
            TelemetryEventName::TitleButtonClicked,
            TelemetryEventName::TelemetrySettingsChanged,
        ];
        // Verify we have at least 20 variants
        assert!(variants.len() >= 20, "Expected at least 20 event name variants");

        // Verify all serialize and deserialize correctly
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let back: TelemetryEventName = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, back);
        }
    }

    #[test]
    fn test_telemetry_event_serialization() {
        let mut props = HashMap::new();
        props.insert("task_id".to_string(), Value::String("abc123".to_string()));
        let event = TelemetryEvent {
            event_name: TelemetryEventName::TaskCreated,
            properties: Some(props),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_name, TelemetryEventName::TaskCreated);
        assert_eq!(
            deserialized.properties.unwrap().get("task_id").unwrap(),
            &Value::String("abc123".to_string())
        );
    }

    #[test]
    fn test_telemetry_event_no_properties() {
        let event = TelemetryEvent {
            event_name: TelemetryEventName::TaskCompleted,
            properties: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();
        assert!(deserialized.properties.is_none());
    }

    #[test]
    fn test_telemetry_setting_serialization() {
        let settings = [
            TelemetrySetting::Enabled,
            TelemetrySetting::Disabled,
            TelemetrySetting::Unset,
        ];
        for setting in &settings {
            let json = serde_json::to_string(setting).unwrap();
            let back: TelemetrySetting = serde_json::from_str(&json).unwrap();
            assert_eq!(*setting, back);
        }
    }

    #[test]
    fn test_subscription_type_serialization() {
        let sub = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Include,
            events: vec![TelemetryEventName::TaskCreated, TelemetryEventName::TaskCompleted],
        };
        let json = serde_json::to_string(&sub).unwrap();
        let deserialized: TelemetryEventSubscription = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subscription_type, SubscriptionType::Include);
        assert_eq!(deserialized.events.len(), 2);
    }

    #[test]
    fn test_subscription_exclude_type() {
        let sub = TelemetryEventSubscription {
            subscription_type: SubscriptionType::Exclude,
            events: vec![TelemetryEventName::TaskCreated],
        };
        let json = serde_json::to_string(&sub).unwrap();
        let deserialized: TelemetryEventSubscription = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subscription_type, SubscriptionType::Exclude);
    }

    #[test]
    fn test_telemetry_event_name_equality() {
        assert_eq!(TelemetryEventName::TaskCreated, TelemetryEventName::TaskCreated);
        assert_ne!(TelemetryEventName::TaskCreated, TelemetryEventName::TaskCompleted);
    }

    #[test]
    fn test_telemetry_setting_equality() {
        assert_eq!(TelemetrySetting::Enabled, TelemetrySetting::Enabled);
        assert_ne!(TelemetrySetting::Enabled, TelemetrySetting::Disabled);
    }
}
