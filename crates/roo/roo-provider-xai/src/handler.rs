//! xAI/Grok provider handler.
//!
//! Uses the OpenAI-compatible chat completions API.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::XaiConfig;

/// xAI API provider handler.
pub struct XaiHandler {
    inner: OpenAiCompatibleProvider,
}

impl XaiHandler {
    /// Create a new xAI handler from configuration.
    pub fn new(config: XaiConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(16384),
                context_window: 131072,
                supports_images: Some(true),
                input_price: Some(3.0),
                output_price: Some(15.0),
                description: Some("xAI model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "xai".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id),
            model_info,
            provider_name_enum: ProviderName::Xai,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self { inner })
    }

    /// Create a new xAI handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config =
            XaiConfig::from_settings(settings).ok_or(roo_provider::ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }
}

#[async_trait]
impl Provider for XaiHandler {
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
        ProviderName::Xai
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
    fn test_config_default_url() {
        assert_eq!(XaiConfig::DEFAULT_BASE_URL, "https://api.x.ai/v1");
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = XaiHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = XaiHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = XaiHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("grok-3-mini".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = XaiHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "grok-3-mini");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = XaiHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Xai);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("xai-test".to_string());
        settings.api_model_id = Some("grok-3-fast".to_string());

        let config = XaiConfig::from_settings(&settings).unwrap();
        assert_eq!(config.api_key, "xai-test");
        assert_eq!(config.model_id, Some("grok-3-fast".to_string()));
    }

    #[test]
    fn test_config_from_settings_custom_base_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("xai-test".to_string());
        settings.xai_base_url = Some("https://custom.x.ai/v1".to_string());

        let config = XaiConfig::from_settings(&settings).unwrap();
        assert_eq!(config.base_url, "https://custom.x.ai/v1");
    }

    #[test]
    fn test_config_from_settings_no_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        assert!(XaiConfig::from_settings(&settings).is_none());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 4, "Should have at least 4 xAI models");
    }

    #[test]
    fn test_mini_model_has_thinking() {
        let all_models = models::models();
        let mini = all_models.get("grok-3-mini").expect("grok-3-mini should exist");
        assert_eq!(mini.supports_reasoning_budget, Some(true));
    }

    #[test]
    fn test_grok3_supports_images() {
        let all_models = models::models();
        let grok3 = all_models.get("grok-3").expect("grok-3 should exist");
        assert!(grok3.supports_images.unwrap_or(false));
    }

    #[test]
    fn test_handler_unknown_model_fallback() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("grok-future".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = XaiHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "grok-future");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_mini_model_cheaper_than_full() {
        let all_models = models::models();
        let grok3 = all_models.get("grok-3").unwrap();
        let mini = all_models.get("grok-3-mini").unwrap();
        assert!(
            mini.input_price.unwrap() < grok3.input_price.unwrap(),
            "Mini should be cheaper than full Grok 3"
        );
    }

    #[test]
    fn test_handler_with_timeout() {
        let config = XaiConfig {
            api_key: "xai-test-key".to_string(),
            base_url: XaiConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: Some(30000),
        };
        let handler = XaiHandler::new(config);
        assert!(handler.is_ok());
    }
}
