//! OpenAI provider handler.
//!
//! Uses the Chat Completions API with SSE streaming.
//! Supports function calling, tool_choice, and reasoning_effort.

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, Provider,
};
use roo_provider::error::{ProviderError, Result};
use roo_types::api::{ApiMessage, ProviderName};
use roo_types::model::ModelInfo;

use crate::models;
use crate::types::OpenAiConfig;

/// OpenAI API provider handler.
pub struct OpenAiHandler {
    inner: roo_provider::OpenAiCompatibleProvider,
    #[allow(dead_code)]
    org_id: Option<String>,
    #[allow(dead_code)]
    reasoning_effort: Option<String>,
}

impl OpenAiHandler {
    /// Create a new OpenAI handler from configuration.
    pub fn new(config: OpenAiConfig) -> Result<Self> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(16384),
                context_window: 128000,
                supports_images: Some(true),
                input_price: Some(2.5),
                output_price: Some(10.0),
                description: Some("OpenAI model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = roo_provider::OpenAiCompatibleConfig {
            provider_name: "openai".to_string(),
            base_url: config.base_url,
            api_key: config.api_key,
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id),
            model_info,
            provider_name_enum: ProviderName::Openai,
            request_timeout: config.request_timeout,
            reasoning_effort: config.reasoning_effort.clone(),
        };

        let inner = roo_provider::OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            org_id: config.org_id,
            reasoning_effort: config.reasoning_effort,
        })
    }

    /// Create a new OpenAI handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config =
            OpenAiConfig::from_settings(settings).ok_or(ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }
}

#[async_trait]
impl Provider for OpenAiHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        // For now, delegate to the inner OpenAI-compatible provider
        // The reasoning_effort and org_id would be added as custom headers/body
        // in a more complete implementation
        self.inner
            .create_message(system_prompt, messages, tools, metadata)
            .await
    }

    fn get_model(&self) -> (String, ModelInfo) {
        self.inner.get_model()
    }

    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::Openai
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
        assert_eq!(
            OpenAiConfig::DEFAULT_BASE_URL,
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = OpenAiHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: Some("gpt-4.1".to_string()),
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "gpt-4.1");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Openai);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-test".to_string());
        settings.api_model_id = Some("gpt-4o-mini".to_string());

        let config = OpenAiConfig::from_settings(&settings).unwrap();
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model_id, Some("gpt-4o-mini".to_string()));
    }

    #[test]
    fn test_config_from_settings_with_org() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-test".to_string());
        settings.open_ai_org_id = Some("org-123".to_string());

        let config = OpenAiConfig::from_settings(&settings).unwrap();
        assert_eq!(config.org_id, Some("org-123".to_string()));
    }

    #[test]
    fn test_config_from_settings_no_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        assert!(OpenAiConfig::from_settings(&settings).is_none());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 7, "Should have at least 7 OpenAI models");
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
    fn test_gpt4o_supports_images() {
        let all_models = models::models();
        let gpt4o = all_models.get("gpt-4o").expect("gpt-4o should exist");
        assert!(gpt4o.supports_images.unwrap_or(false));
    }

    #[test]
    fn test_handler_with_reasoning_effort() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: Some("o3-mini".to_string()),
            temperature: None,
            reasoning_effort: Some("low".to_string()),
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config).unwrap();
        assert_eq!(handler.reasoning_effort, Some("low".to_string()));
    }

    #[test]
    fn test_handler_unknown_model_fallback() {
        let config = OpenAiConfig {
            api_key: "sk-test".to_string(),
            base_url: OpenAiConfig::DEFAULT_BASE_URL.to_string(),
            org_id: None,
            model_id: Some("gpt-future".to_string()),
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "gpt-future");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_o3_has_higher_pricing() {
        let all_models = models::models();
        let gpt4o = all_models.get("gpt-4o").unwrap();
        let o3 = all_models.get("o3").unwrap();
        assert!(
            o3.input_price.unwrap() > gpt4o.input_price.unwrap(),
            "o3 should cost more than gpt-4o"
        );
    }

    #[test]
    fn test_mini_models_cheaper() {
        let all_models = models::models();
        let gpt4o = all_models.get("gpt-4o").unwrap();
        let mini = all_models.get("gpt-4o-mini").unwrap();
        assert!(
            mini.input_price.unwrap() < gpt4o.input_price.unwrap(),
            "Mini should be cheaper"
        );
    }
}
