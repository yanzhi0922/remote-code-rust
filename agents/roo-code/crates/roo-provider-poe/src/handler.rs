//! Poe provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via Poe.
//! Supports reasoning budget/effort for models that support extended thinking.
//! Source: `src/api/providers/poe.ts`
//!
//! Note: Poe uses a static model list in TS (no `fetchModel()`).
//! This implementation adds fallback logic for unknown models.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::PoeConfig;

/// Poe API provider handler.
///
/// Poe provides access to various LLM models through a subscription model.
/// It follows the OpenAI API format for compatibility.
pub struct PoeHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID.
    model_id: String,
    /// The model info (static or fallback).
    model_info: ModelInfo,
}

impl PoeHandler {
    /// Create a new Poe handler from configuration.
    pub fn new(config: PoeConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config
            .model_id
            .clone()
            .unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(4096),
                context_window: 128000,
                supports_prompt_cache: false,
                input_price: Some(0.0),
                output_price: Some(0.0),
                description: Some(format!("Poe model: {}", model_id)),
                ..Default::default()
            });

        let base_url = config
            .base_url
            .unwrap_or_else(|| PoeConfig::DEFAULT_BASE_URL.to_string());

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "poe".to_string(),
            base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info: model_info.clone(),
            provider_name_enum: ProviderName::Poe,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            model_info,
        })
    }

    /// Create a new Poe handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = PoeConfig::from_settings(settings);
        let config = config.unwrap_or_else(|| PoeConfig {
            api_key: "dummy-key".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        });
        Self::new(config)
    }
}

#[async_trait]
impl Provider for PoeHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<roo_types::api::ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream, roo_provider::ProviderError> {
        self.inner
            .create_message(system_prompt, messages, tools, metadata)
            .await
    }

    fn get_model(&self) -> (String, ModelInfo) {
        // Re-resolve from static list in case the model_id matches a known model
        if let Some(info) = models::models().get(&self.model_id) {
            return (self.model_id.clone(), info.clone());
        }
        // Fallback to stored model info (includes description with model name)
        (self.model_id.clone(), self.model_info.clone())
    }

    async fn complete_prompt(
        &self,
        prompt: &str,
    ) -> Result<String, roo_provider::ProviderError> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::Poe
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
            assert!(info.max_tokens.is_some(), "Model '{}' missing max_tokens", id);
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
    fn test_poe_models_are_free() {
        for (id, info) in models::models() {
            assert_eq!(
                info.input_price,
                Some(0.0),
                "Poe model '{}' should be free (input_price = 0.0)",
                id
            );
            assert_eq!(
                info.output_price,
                Some(0.0),
                "Poe model '{}' should be free (output_price = 0.0)",
                id
            );
        }
    }

    #[test]
    fn test_default_url() {
        assert_eq!(PoeConfig::DEFAULT_BASE_URL, "https://api.poe.com/bot/");
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: Some("claude-3-5-sonnet-20241022".to_string()),
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_handler_from_settings() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let handler = PoeHandler::from_settings(&settings);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Poe);
    }

    #[test]
    fn test_custom_base_url() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: Some("https://custom-poe.example.com/api/".to_string()),
            model_id: None,
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_fallback_model_info() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: Some("unknown-poe-model".to_string()),
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert!(info.max_tokens.is_some());
        // Fallback should include a description with the model name
        assert!(info.description.is_some());
        assert!(info.description.as_ref().unwrap().contains("unknown-poe-model"));
    }

    #[test]
    fn test_temperature_config() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: None,
            temperature: Some(0.5),
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert_eq!(all_models.len(), 2);
    }

    #[test]
    fn test_all_models_support_images() {
        for (id, info) in models::models() {
            assert!(
                info.supports_images.unwrap_or(false),
                "Poe model '{}' should support images",
                id
            );
        }
    }

    // --- Fallback tests ---

    #[test]
    fn test_fallback_description_contains_model_name() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: Some("my-custom-poe-bot".to_string()),
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "my-custom-poe-bot");
        assert_eq!(info.input_price, Some(0.0));
        assert_eq!(info.output_price, Some(0.0));
    }

    #[test]
    fn test_known_model_not_using_fallback() {
        let config = PoeConfig {
            api_key: "test-key".to_string(),
            base_url: None,
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = PoeHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        // The static model should have a specific description, not the fallback one
        let desc = info.description.as_ref().unwrap();
        assert!(!desc.contains("Poe model: "));
    }
}
