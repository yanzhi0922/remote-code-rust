//! Qwen / 通义千问 provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via DashScope.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::QwenConfig;

/// Default temperature for Qwen models.
const DEFAULT_TEMPERATURE: f64 = 0.0;

/// Qwen API provider handler.
pub struct QwenHandler {
    inner: OpenAiCompatibleProvider,
}

impl QwenHandler {
    /// Create a new Qwen handler from configuration.
    pub fn new(config: QwenConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(65_536),
                context_window: 1_000_000,
                input_price: Some(0.0),
                output_price: Some(0.0),
                description: Some("Qwen model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "qwen".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            model_id: Some(model_id),
            model_info,
            provider_name_enum: ProviderName::QwenCode,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self { inner })
    }

    /// Create a new Qwen handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config =
            QwenConfig::from_settings(settings).ok_or(roo_provider::ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }
}

#[async_trait]
impl Provider for QwenHandler {
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
        ProviderName::QwenCode
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
    fn test_qwen_config_default_url() {
        assert_eq!(
            QwenConfig::DEFAULT_BASE_URL,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = QwenHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = QwenConfig {
            api_key: "test-key".to_string(),
            base_url: QwenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = QwenHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = QwenConfig {
            api_key: "test-key".to_string(),
            base_url: QwenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = QwenHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = QwenConfig {
            api_key: "test-key".to_string(),
            base_url: QwenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("qwen3-coder-flash".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = QwenHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "qwen3-coder-flash");
    }

    #[test]
    fn test_handler_unknown_model_falls_back() {
        let config = QwenConfig {
            api_key: "test-key".to_string(),
            base_url: QwenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("nonexistent-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = QwenHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "nonexistent-model");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_provider_name() {
        let config = QwenConfig {
            api_key: "test-key".to_string(),
            base_url: QwenConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = QwenHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::QwenCode);
    }

    #[test]
    fn test_from_settings_with_api_key() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.qwen_api_key = Some("test-key".to_string());
        let result = QwenHandler::from_settings(&settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_from_settings_with_custom_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.qwen_api_key = Some("test-key".to_string());
        settings.qwen_base_url = Some("https://custom.qwen.api/v1".to_string());
        let handler = QwenHandler::from_settings(&settings).unwrap();
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
    fn test_models_are_free() {
        // Qwen Code models are currently free (price = 0)
        for (id, info) in models::models() {
            assert_eq!(
                info.input_price,
                Some(0.0),
                "Model '{}' should be free (input_price = 0)",
                id
            );
            assert_eq!(
                info.output_price,
                Some(0.0),
                "Model '{}' should be free (output_price = 0)",
                id
            );
        }
    }

    #[test]
    fn test_models_have_large_context() {
        let all_models = models::models();
        for (id, info) in &all_models {
            assert!(
                info.context_window >= 1_000_000,
                "Model '{}' should have at least 1M context window",
                id
            );
        }
    }

    #[test]
    fn test_model_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 2, "Expected at least 2 models, got {}", all_models.len());
    }
}
