//! DeepSeek provider handler.
//!
//! Uses the OpenAI-compatible chat completions API.
//! Supports extended thinking mode via `deepseek-reasoner`.
//!
//! Key differences from the base OpenAI-compatible provider:
//! - Messages are converted using R1 format (merges consecutive same-role messages)
//! - Thinking mode is enabled for `deepseek-reasoner` models
//! - Custom usage metrics for DeepSeek-specific cache token fields

use async_trait::async_trait;
use roo_provider::{
    transform::{convert_to_r1_zai_messages, R1ZaiOptions},
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::DeepSeekConfig;

/// Default temperature for DeepSeek models.
/// Source: `packages/types/src/providers/deepseek.ts` — `DEEP_SEEK_DEFAULT_TEMPERATURE`
const DEEP_SEEK_DEFAULT_TEMPERATURE: f64 = 0.3;

/// DeepSeek API provider handler.
pub struct DeepSeekHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID (used for thinking model detection).
    model_id: String,
}

impl DeepSeekHandler {
    /// Create a new DeepSeek handler from configuration.
    pub fn new(config: DeepSeekConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 128_000,
                supports_prompt_cache: true,
                input_price: Some(0.28),
                output_price: Some(0.42),
                description: Some("DeepSeek model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "deepseek".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(DEEP_SEEK_DEFAULT_TEMPERATURE),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::DeepSeek,
            request_timeout: config.request_timeout,
            reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self { inner, model_id })
    }

    /// Create a new DeepSeek handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = DeepSeekConfig::from_settings(settings).ok_or_else(|| {
            roo_provider::ProviderError::ApiKeyRequired
        })?;
        Self::new(config)
    }

    /// Check if the model is a thinking model (deepseek-reasoner or deepseek-r1 variants).
    ///
    /// Source: `src/api/providers/deepseek.ts` — `isThinkingModel` check
    fn is_thinking_model(model_id: &str) -> bool {
        model_id.contains("deepseek-reasoner") || model_id.contains("deepseek-r1")
    }

    /// Build a custom request body for DeepSeek with R1 format messages and thinking mode.
    fn build_request_body(
        &self,
        system_prompt: &str,
        messages: &[roo_types::api::ApiMessage],
        tools: Option<&Vec<serde_json::Value>>,
        metadata: &CreateMessageMetadata,
    ) -> serde_json::Value {
        let (_, info) = self.inner.get_model();
        let max_tokens = info.max_tokens;
        let model = &self.model_id;
        let temperature = DEEP_SEEK_DEFAULT_TEMPERATURE;

        let is_thinking = Self::is_thinking_model(model);

        // Convert messages using R1 format (merges consecutive same-role messages).
        // For thinking models, enable merge_tool_result_text to preserve reasoning_content
        // during tool call sequences.
        // Source: `src/api/providers/deepseek.ts` — convertToR1Format with mergeToolResultText
        let options = R1ZaiOptions {
            merge_tool_result_text: is_thinking,
            preserve_reasoning: true,
        };

        // Prepend system prompt as a user message (matching TS behavior)
        let mut all_messages = vec![roo_types::api::ApiMessage {
            role: roo_types::api::MessageRole::User,
            content: vec![roo_types::api::ContentBlock::Text {
                text: system_prompt.to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];
        all_messages.extend(messages.iter().cloned());

        let r1_messages = convert_to_r1_zai_messages(&all_messages, options);

        let mut body = serde_json::json!({
            "model": model,
            "temperature": temperature,
            "messages": r1_messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "parallel_tool_calls": metadata.parallel_tool_calls.unwrap_or(true),
        });

        // Add max_tokens if model specifies it
        if let Some(max_tokens) = max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        // Add tools if provided
        if let Some(tools) = roo_provider::base_provider::convert_tools_for_openai(tools) {
            body["tools"] = serde_json::json!(tools);
        }

        // Add tool_choice if specified
        if let Some(ref tool_choice) = metadata.tool_choice {
            body["tool_choice"] = tool_choice.clone();
        }

        // Enable thinking mode for deepseek-reasoner models.
        // Source: `src/api/providers/deepseek.ts` — thinking: { type: "enabled" }
        if is_thinking {
            body["thinking"] = serde_json::json!({ "type": "enabled" });
        }

        body
    }
}

#[async_trait]
impl Provider for DeepSeekHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<roo_types::api::ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream, roo_provider::ProviderError> {
        let body = self.build_request_body(system_prompt, &messages, tools.as_ref(), &metadata);
        self.inner.create_message_from_body(body).await
    }

    fn get_model(&self) -> (String, ModelInfo) {
        self.inner.get_model()
    }

    async fn complete_prompt(
        &self,
        prompt: &str,
    ) -> Result<String, roo_provider::ProviderError> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::DeepSeek
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;

    #[test]
    fn test_default_model_exists() {
        let all_models = models::models();
        assert!(
            all_models.contains_key(models::DEFAULT_MODEL_ID),
            "Default model '{}' should exist in models map",
            models::DEFAULT_MODEL_ID
        );
    }

    #[test]
    fn test_all_models_have_required_fields() {
        for (id, info) in models::models() {
            assert!(
                info.max_tokens.is_some(),
                "Model '{}' missing max_tokens",
                id
            );
            assert!(
                info.input_price.is_some(),
                "Model '{}' missing input_price",
                id
            );
            assert!(
                info.output_price.is_some(),
                "Model '{}' missing output_price",
                id
            );
        }
    }

    #[test]
    fn test_reasoner_has_thinking_enabled() {
        let all_models = models::models();
        let reasoner = all_models.get("deepseek-reasoner").expect("reasoner should exist");
        assert_eq!(reasoner.supports_reasoning_budget, Some(true));
    }

    #[test]
    fn test_is_thinking_model() {
        assert!(DeepSeekHandler::is_thinking_model("deepseek-reasoner"));
        assert!(DeepSeekHandler::is_thinking_model("deepseek-r1-0528"));
        assert!(!DeepSeekHandler::is_thinking_model("deepseek-chat"));
        assert!(!DeepSeekHandler::is_thinking_model("deepseek-chat-v3-0324"));
    }

    #[test]
    fn test_deepseek_config_default_url() {
        assert_eq!(
            DeepSeekConfig::DEFAULT_BASE_URL,
            "https://api.deepseek.com"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = DeepSeekHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("deepseek-reasoner".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "deepseek-reasoner");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::DeepSeek);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-test".to_string());
        settings.api_model_id = Some("deepseek-reasoner".to_string());

        let config = DeepSeekConfig::from_settings(&settings).unwrap();
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model_id, Some("deepseek-reasoner".to_string()));
    }

    #[test]
    fn test_config_from_settings_custom_base_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-test".to_string());
        settings.deep_seek_base_url = Some("https://custom.deepseek.api".to_string());

        let config = DeepSeekConfig::from_settings(&settings).unwrap();
        assert_eq!(config.base_url, "https://custom.deepseek.api");
    }

    #[test]
    fn test_config_from_settings_no_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        assert!(DeepSeekConfig::from_settings(&settings).is_none());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 4, "Should have at least 4 DeepSeek models");
    }

    #[test]
    fn test_all_models_have_descriptions() {
        for (id, info) in models::models() {
            assert!(
                info.description.is_some(),
                "Model '{}' missing description",
                id
            );
        }
    }

    #[test]
    fn test_default_temperature() {
        assert_eq!(DEEP_SEEK_DEFAULT_TEMPERATURE, 0.3);
    }

    #[test]
    fn test_build_request_body_includes_thinking_for_reasoner() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("deepseek-reasoner".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();

        let body = handler.build_request_body(
            "You are a helpful assistant.",
            &[],
            None,
            &CreateMessageMetadata::default(),
        );

        // Thinking mode should be enabled for deepseek-reasoner
        assert_eq!(body["thinking"]["type"], "enabled");
    }

    #[test]
    fn test_build_request_body_no_thinking_for_chat() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("deepseek-chat".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();

        let body = handler.build_request_body(
            "You are a helpful assistant.",
            &[],
            None,
            &CreateMessageMetadata::default(),
        );

        // Thinking mode should NOT be enabled for deepseek-chat
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn test_build_request_body_uses_r1_format() {
        let config = DeepSeekConfig {
            api_key: "test-key".to_string(),
            base_url: DeepSeekConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = DeepSeekHandler::new(config).unwrap();

        // Create consecutive user messages that should be merged by R1 format
        let messages = vec![
            roo_types::api::ApiMessage {
                role: roo_types::api::MessageRole::User,
                content: vec![roo_types::api::ContentBlock::Text {
                    text: "Hello".to_string(),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
            roo_types::api::ApiMessage {
                role: roo_types::api::MessageRole::User,
                content: vec![roo_types::api::ContentBlock::Text {
                    text: "World".to_string(),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
        ];

        let body = handler.build_request_body(
            "System prompt",
            &messages,
            None,
            &CreateMessageMetadata::default(),
        );

        // The system prompt + 2 user messages should be merged into fewer messages
        let msgs = body["messages"].as_array().unwrap();
        // System prompt becomes first user message, then the two user messages should be merged
        assert!(msgs.len() < 3, "R1 format should merge consecutive same-role messages");
    }
}
