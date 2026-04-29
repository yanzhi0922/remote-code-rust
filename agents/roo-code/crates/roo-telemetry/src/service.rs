use std::collections::HashMap;

use serde_json::Value;

use crate::client::TelemetryClient;
use crate::types::{TelemetryEvent, TelemetryEventName, TelemetrySetting};

/// Telemetry service wrapper that manages multiple telemetry clients.
///
/// Provides a unified interface for capturing events and exceptions
/// across all registered clients.
pub struct TelemetryService {
    clients: Vec<Box<dyn TelemetryClient>>,
}

impl TelemetryService {
    /// Create a new telemetry service with no clients.
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Register a telemetry client.
    pub fn register(&mut self, client: Box<dyn TelemetryClient>) {
        self.clients.push(client);
    }

    /// Check if the service is ready (has at least one client).
    pub fn is_ready(&self) -> bool {
        !self.clients.is_empty()
    }

    /// Update the telemetry state on all clients based on user preference.
    pub fn update_telemetry_state(&mut self, is_opted_in: bool) {
        if !self.is_ready() {
            return;
        }
        for client in &mut self.clients {
            client.update_telemetry_state(is_opted_in);
        }
    }

    /// Capture a telemetry event on all registered clients.
    pub fn capture_event(
        &self,
        event_name: TelemetryEventName,
        properties: Option<HashMap<String, Value>>,
    ) {
        if !self.is_ready() {
            return;
        }

        let event = TelemetryEvent {
            event_name,
            properties,
        };

        for client in &self.clients {
            client.capture(event.clone());
        }
    }

    /// Capture an exception on all registered clients.
    pub fn capture_exception(&self, error: &dyn std::error::Error) {
        if !self.is_ready() {
            return;
        }

        for client in &self.clients {
            client.capture_exception(error, None);
        }
    }

    /// Capture a task created event.
    pub fn capture_task_created(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::TaskCreated, Some(props));
    }

    /// Capture a task restarted event.
    pub fn capture_task_restarted(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::TaskRestarted, Some(props));
    }

    /// Capture a task completed event.
    pub fn capture_task_completed(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::TaskCompleted, Some(props));
    }

    /// Capture a conversation message event.
    pub fn capture_conversation_message(&self, task_id: &str, source: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert("source".to_string(), Value::String(source.to_string()));
        self.capture_event(TelemetryEventName::TaskConversationMessage, Some(props));
    }

    /// Capture an LLM completion event.
    pub fn capture_llm_completion(
        &self,
        task_id: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_write_tokens: u64,
        cache_read_tokens: u64,
        cost: Option<f64>,
    ) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert("inputTokens".to_string(), Value::Number(input_tokens.into()));
        props.insert("outputTokens".to_string(), Value::Number(output_tokens.into()));
        props.insert("cacheWriteTokens".to_string(), Value::Number(cache_write_tokens.into()));
        props.insert("cacheReadTokens".to_string(), Value::Number(cache_read_tokens.into()));
        if let Some(c) = cost {
            props.insert("cost".to_string(), serde_json::json!(c));
        }
        self.capture_event(TelemetryEventName::TaskLlmCompletion, Some(props));
    }

    /// Capture a mode switch event.
    pub fn capture_mode_switch(&self, task_id: &str, new_mode: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert("newMode".to_string(), Value::String(new_mode.to_string()));
        self.capture_event(TelemetryEventName::ModeSwitch, Some(props));
    }

    /// Capture a tool usage event.
    pub fn capture_tool_usage(&self, task_id: &str, tool: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert("tool".to_string(), Value::String(tool.to_string()));
        self.capture_event(TelemetryEventName::ToolUsed, Some(props));
    }

    /// Capture a sliding window truncation event.
    pub fn capture_sliding_window_truncation(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::SlidingWindowTruncation, Some(props));
    }

    /// Capture a context condensed event.
    /// Corresponds to TS: `captureContextCondensed(taskId, isAutomaticTrigger, usedCustomPrompt?)`
    pub fn capture_context_condensed(
        &self,
        task_id: &str,
        is_automatic_trigger: bool,
        used_custom_prompt: Option<bool>,
    ) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert(
            "isAutomaticTrigger".to_string(),
            Value::Bool(is_automatic_trigger),
        );
        if let Some(used) = used_custom_prompt {
            props.insert("usedCustomPrompt".to_string(), Value::Bool(used));
        }
        self.capture_event(TelemetryEventName::ContextCondensed, Some(props));
    }

    /// Check if telemetry is enabled on any client.
    pub fn is_telemetry_enabled(&self) -> bool {
        self.is_ready() && self.clients.iter().any(|c| c.is_telemetry_enabled())
    }

    /// Shutdown all clients.
    pub fn shutdown(&self) {
        if !self.is_ready() {
            return;
        }
        for client in &self.clients {
            client.shutdown();
        }
    }

    /// Get the number of registered clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Capture a checkpoint created event.
    /// Corresponds to TS: `captureCheckpointCreated(taskId)`
    pub fn capture_checkpoint_created(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::CheckpointCreated, Some(props));
    }

    /// Capture a checkpoint diffed event.
    /// Corresponds to TS: `captureCheckpointDiffed(taskId)`
    pub fn capture_checkpoint_diffed(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::CheckpointDiffed, Some(props));
    }

    /// Capture a checkpoint restored event.
    /// Corresponds to TS: `captureCheckpointRestored(taskId)`
    pub fn capture_checkpoint_restored(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::CheckpointRestored, Some(props));
    }

    /// Capture a code action used event.
    /// Corresponds to TS: `captureCodeActionUsed(actionType)`
    pub fn capture_code_action_used(&self, action_type: &str) {
        let mut props = HashMap::new();
        props.insert(
            "actionType".to_string(),
            Value::String(action_type.to_string()),
        );
        self.capture_event(TelemetryEventName::CodeActionUsed, Some(props));
    }

    /// Capture a prompt enhanced event.
    /// Corresponds to TS: `capturePromptEnhanced(taskId?)`
    pub fn capture_prompt_enhanced(&self, task_id: Option<&str>) {
        let mut props = HashMap::new();
        if let Some(id) = task_id {
            props.insert("taskId".to_string(), Value::String(id.to_string()));
        }
        self.capture_event(TelemetryEventName::PromptEnhanced, Some(props));
    }

    /// Capture a diff application error event.
    /// Corresponds to TS: `captureDiffApplicationError(taskId, consecutiveMistakeCount)`
    pub fn capture_diff_application_error(&self, task_id: &str, consecutive_mistake_count: u32) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        props.insert(
            "consecutiveMistakeCount".to_string(),
            Value::Number(consecutive_mistake_count.into()),
        );
        self.capture_event(TelemetryEventName::DiffApplicationError, Some(props));
    }

    /// Capture a shell integration error event.
    /// Corresponds to TS: `captureShellIntegrationError(taskId)`
    pub fn capture_shell_integration_error(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::ShellIntegrationError, Some(props));
    }

    /// Capture a consecutive mistake error event.
    /// Corresponds to TS: `captureConsecutiveMistakeError(taskId)`
    pub fn capture_consecutive_mistake_error(&self, task_id: &str) {
        let mut props = HashMap::new();
        props.insert("taskId".to_string(), Value::String(task_id.to_string()));
        self.capture_event(TelemetryEventName::ConsecutiveMistakeError, Some(props));
    }

    /// Capture a tab shown event.
    /// Corresponds to TS: `captureTabShown(tab)`
    pub fn capture_tab_shown(&self, tab: &str) {
        let mut props = HashMap::new();
        props.insert("tab".to_string(), Value::String(tab.to_string()));
        self.capture_event(TelemetryEventName::TabShown, Some(props));
    }

    /// Capture a mode setting changed event.
    /// Corresponds to TS: `captureModeSettingChanged(settingName)`
    pub fn capture_mode_setting_changed(&self, setting_name: &str) {
        let mut props = HashMap::new();
        props.insert(
            "settingName".to_string(),
            Value::String(setting_name.to_string()),
        );
        self.capture_event(TelemetryEventName::ModeSettingsChanged, Some(props));
    }

    /// Capture a custom mode created event.
    /// Corresponds to TS: `captureCustomModeCreated(modeSlug, modeName)`
    pub fn capture_custom_mode_created(&self, mode_slug: &str, mode_name: &str) {
        let mut props = HashMap::new();
        props.insert("modeSlug".to_string(), Value::String(mode_slug.to_string()));
        props.insert("modeName".to_string(), Value::String(mode_name.to_string()));
        self.capture_event(TelemetryEventName::CustomModeCreated, Some(props));
    }

    /// Capture a marketplace item installed event.
    /// Corresponds to TS: `captureMarketplaceItemInstalled(itemId, itemType, itemName, target, properties?)`
    pub fn capture_marketplace_item_installed(
        &self,
        item_id: &str,
        item_type: &str,
        item_name: &str,
        target: &str,
        extra_properties: Option<HashMap<String, Value>>,
    ) {
        let mut props = HashMap::new();
        props.insert("itemId".to_string(), Value::String(item_id.to_string()));
        props.insert("itemType".to_string(), Value::String(item_type.to_string()));
        props.insert("itemName".to_string(), Value::String(item_name.to_string()));
        props.insert("target".to_string(), Value::String(target.to_string()));
        if let Some(extra) = extra_properties {
            props.extend(extra);
        }
        self.capture_event(TelemetryEventName::MarketplaceItemInstalled, Some(props));
    }

    /// Capture a marketplace item removed event.
    /// Corresponds to TS: `captureMarketplaceItemRemoved(itemId, itemType, itemName, target)`
    pub fn capture_marketplace_item_removed(
        &self,
        item_id: &str,
        item_type: &str,
        item_name: &str,
        target: &str,
    ) {
        let mut props = HashMap::new();
        props.insert("itemId".to_string(), Value::String(item_id.to_string()));
        props.insert("itemType".to_string(), Value::String(item_type.to_string()));
        props.insert("itemName".to_string(), Value::String(item_name.to_string()));
        props.insert("target".to_string(), Value::String(target.to_string()));
        self.capture_event(TelemetryEventName::MarketplaceItemRemoved, Some(props));
    }

    /// Capture a title button clicked event.
    /// Corresponds to TS: `captureTitleButtonClicked(button)`
    pub fn capture_title_button_clicked(&self, button: &str) {
        let mut props = HashMap::new();
        props.insert("button".to_string(), Value::String(button.to_string()));
        self.capture_event(TelemetryEventName::TitleButtonClicked, Some(props));
    }

    /// Capture telemetry settings changed event.
    pub fn capture_telemetry_settings_changed(
        &self,
        previous_setting: &TelemetrySetting,
        new_setting: &TelemetrySetting,
    ) {
        let mut props = HashMap::new();
        props.insert(
            "previousSetting".to_string(),
            serde_json::json!(previous_setting),
        );
        props.insert("newSetting".to_string(), serde_json::json!(new_setting));
        self.capture_event(TelemetryEventName::TelemetrySettingsChanged, Some(props));
    }
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::BaseTelemetryClient;
    use crate::types::SubscriptionType;

    #[test]
    fn test_new_service_is_not_ready() {
        let service = TelemetryService::new();
        assert!(!service.is_ready());
        assert_eq!(service.client_count(), 0);
    }

    #[test]
    fn test_register_client() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        assert!(service.is_ready());
        assert_eq!(service.client_count(), 1);
    }

    #[test]
    fn test_register_multiple_clients() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.register(Box::new(BaseTelemetryClient::new(None, true)));
        assert_eq!(service.client_count(), 2);
    }

    #[test]
    fn test_update_telemetry_state_no_clients() {
        let mut service = TelemetryService::new();
        // Should not panic
        service.update_telemetry_state(true);
    }

    #[test]
    fn test_capture_event_no_clients() {
        let service = TelemetryService::new();
        // Should not panic
        service.capture_event(TelemetryEventName::TaskCreated, None);
    }

    #[test]
    fn test_capture_event_with_client() {
        let mut service = TelemetryService::new();
        let client = BaseTelemetryClient::new(None, false);
        service.register(Box::new(client));
        service.update_telemetry_state(true);
        service.capture_event(TelemetryEventName::TaskCreated, None);
    }

    #[test]
    fn test_capture_task_created() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_task_created("task-123");
    }

    #[test]
    fn test_capture_task_restarted() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_task_restarted("task-123");
    }

    #[test]
    fn test_capture_task_completed() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_task_completed("task-123");
    }

    #[test]
    fn test_capture_conversation_message() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_conversation_message("task-123", "user");
    }

    #[test]
    fn test_capture_llm_completion() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_llm_completion("task-123", 100, 50, 10, 20, Some(0.05));
    }

    #[test]
    fn test_capture_mode_switch() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_mode_switch("task-123", "architect");
    }

    #[test]
    fn test_capture_tool_usage() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_tool_usage("task-123", "read_file");
    }

    #[test]
    fn test_capture_exception_no_clients() {
        let service = TelemetryService::new();
        // Should not panic
        service.capture_exception(&std::io::Error::new(std::io::ErrorKind::Other, "test"));
    }

    #[test]
    fn test_is_telemetry_enabled_no_clients() {
        let service = TelemetryService::new();
        assert!(!service.is_telemetry_enabled());
    }

    #[test]
    fn test_is_telemetry_enabled_with_disabled_client() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        assert!(!service.is_telemetry_enabled());
    }

    #[test]
    fn test_is_telemetry_enabled_with_enabled_client() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        assert!(service.is_telemetry_enabled());
    }

    #[test]
    fn test_shutdown() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.shutdown();
    }

    #[test]
    fn test_shutdown_no_clients() {
        let service = TelemetryService::new();
        service.shutdown();
    }

    #[test]
    fn test_capture_telemetry_settings_changed() {
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(None, false)));
        service.update_telemetry_state(true);
        service.capture_telemetry_settings_changed(
            &TelemetrySetting::Unset,
            &TelemetrySetting::Enabled,
        );
    }

    #[test]
    fn test_default_service() {
        let service = TelemetryService::default();
        assert!(!service.is_ready());
    }

    #[test]
    fn test_capture_with_subscription_filtered() {
        let sub = crate::types::TelemetryEventSubscription {
            subscription_type: SubscriptionType::Include,
            events: vec![TelemetryEventName::TaskCreated],
        };
        let mut service = TelemetryService::new();
        service.register(Box::new(BaseTelemetryClient::new(Some(sub), false)));
        service.update_telemetry_state(true);
        // TaskCreated should be captured, ToolUsed should not
        service.capture_task_created("task-1");
        service.capture_tool_usage("task-1", "read_file");
    }
}
