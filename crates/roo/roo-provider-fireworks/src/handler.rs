//! Fireworks AI provider handler.
//!
//! Uses the OpenAI-compatible chat completions API.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::FireworksConfig;

/// Default temperature for Fireworks models.
const DEFAULT_TEMPERATURE: f64 = 0.5;

/// Fireworks AI provider handler.
pub struct FireworksHandler {
    inner: OpenAiCompatibleProvider,
}

impl FireworksHandler {
    /// Create a new Fireworks handler from configuration.
    pub fn new(config: FireworksConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(16_384),
                context_window: 262_144,
                input_price: Some(0.6),
                output_price: Some(2.5),
                description: Some("Fireworks model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "fireworks".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            model_id: Some(model_id),
            model_info,
            provider_name_enum: ProviderName::Fireworks,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self { inner })
    }

    /// Create a new Fireworks handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config =
            FireworksConfig::from_settings(settings).ok_or(roo_provider::ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }
}

#[async_trait]
impl Provider for FireworksHandler {
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
        self.inner.get_model()
    }

    async fn complete_prompt(
        &self,
        prompt: &str,
    ) -> Result<String, roo_provider::ProviderError> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::Fireworks
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
            "Default model '{}' should exist",
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
    fn test_fireworks_config_default_url() {
        assert_eq!(
            FireworksConfig::DEFAULT_BASE_URL,
            "https://api.fireworks.ai/inference/v1"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = FireworksHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: FireworksConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: FireworksConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: FireworksConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("accounts/fireworks/models/deepseek-v3".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "accounts/fireworks/models/deepseek-v3");
    }

    #[test]
    fn test_handler_unknown_model_falls_back() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: FireworksConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("nonexistent-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "nonexistent-model");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_handler_custom_base_url() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: "https://custom.fireworks.api/v1".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = FireworksConfig {
            api_key: "test-key".to_string(),
            base_url: FireworksConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = FireworksHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Fireworks);
    }

    #[test]
    fn test_from_settings_with_api_key() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("test-key".to_string());
        let result = FireworksHandler::from_settings(&settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_from_settings_with_custom_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("test-key".to_string());
        settings.fireworks_base_url = Some("https://custom.fireworks.api/v1".to_string());
        let handler = FireworksHandler::from_settings(&settings).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
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
    fn test_model_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 10, "Expected at least 10 models, got {}", all_models.len());
    }

    #[test]
    fn test_kimi_k2_thinking_has_thinking_enabled() {
        let all_models = models::models();
        let thinking = all_models
            .get("accounts/fireworks/models/kimi-k2-thinking")
            .expect("kimi-k2-thinking should exist");
        assert_eq!(thinking.supports_reasoning_budget, Some(true));
    }
}
