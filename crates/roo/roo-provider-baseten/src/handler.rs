//! Baseten provider handler.
//!
//! Uses the OpenAI-compatible chat completions API.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::BasetenConfig;

/// Default temperature for Baseten models.
const DEFAULT_TEMPERATURE: f64 = 0.5;

/// Baseten API provider handler.
pub struct BasetenHandler {
    inner: OpenAiCompatibleProvider,
}

impl BasetenHandler {
    /// Create a new Baseten handler from configuration.
    pub fn new(config: BasetenConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(16_384),
                context_window: 200_000,
                input_price: Some(0.6),
                output_price: Some(2.2),
                description: Some("Baseten model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "baseten".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            model_id: Some(model_id),
            model_info,
            provider_name_enum: ProviderName::Baseten,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self { inner })
    }

    /// Create a new Baseten handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config =
            BasetenConfig::from_settings(settings).ok_or(roo_provider::ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }
}

#[async_trait]
impl Provider for BasetenHandler {
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
        ProviderName::Baseten
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
    fn test_deepseek_r1_has_thinking_enabled() {
        let all_models = models::models();
        let r1 = all_models
            .get("deepseek-ai/DeepSeek-R1")
            .expect("DeepSeek-R1 should exist");
        assert_eq!(r1.supports_reasoning_budget, Some(true));
    }

    #[test]
    fn test_baseten_config_default_url() {
        assert_eq!(
            BasetenConfig::DEFAULT_BASE_URL,
            "https://inference.baseten.co/v1"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = BasetenHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = BasetenConfig {
            api_key: "test-key".to_string(),
            base_url: BasetenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = BasetenHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = BasetenConfig {
            api_key: "test-key".to_string(),
            base_url: BasetenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = BasetenHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = BasetenConfig {
            api_key: "test-key".to_string(),
            base_url: BasetenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("deepseek-ai/DeepSeek-V3.1".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = BasetenHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "deepseek-ai/DeepSeek-V3.1");
    }

    #[test]
    fn test_handler_unknown_model_falls_back() {
        let config = BasetenConfig {
            api_key: "test-key".to_string(),
            base_url: BasetenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("nonexistent-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = BasetenHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "nonexistent-model");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_provider_name() {
        let config = BasetenConfig {
            api_key: "test-key".to_string(),
            base_url: BasetenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = BasetenHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Baseten);
    }

    #[test]
    fn test_from_settings_with_api_key() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.baseten_api_key = Some("test-key".to_string());
        let result = BasetenHandler::from_settings(&settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_from_settings_with_model_id() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.baseten_api_key = Some("test-key".to_string());
        settings.baseten_model_id = Some("deepseek-ai/DeepSeek-V3.2".to_string());
        let handler = BasetenHandler::from_settings(&settings).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "deepseek-ai/DeepSeek-V3.2");
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
}
