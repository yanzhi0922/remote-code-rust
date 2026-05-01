//! Requesty provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via Requesty router.
//! Supports trace_id and mode tracking for observability.
//! Supports dynamic model loading from the Requesty API.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::RequestyConfig;

/// Requesty API provider handler.
///
/// Requesty is an LLM router that provides observability features
/// like trace_id tracking and mode identification.
pub struct RequestyHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID.
    model_id: String,
    /// Base URL for API requests (includes /v1 suffix).
    base_url: String,
    /// API key for authentication.
    api_key: String,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
}

impl RequestyHandler {
    /// Create a new Requesty handler from configuration.
    pub fn new(config: RequestyConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 200000,
                supports_prompt_cache: true,
                input_price: Some(3.0),
                output_price: Some(15.0),
                description: Some("Requesty model (unknown variant)".to_string()),
                ..Default::default()
            });

        // Ensure base URL ends with /v1 for OpenAI compatibility
        let base_url = if config.base_url.ends_with("/v1") {
            config.base_url.clone()
        } else {
            format!("{}/v1", config.base_url.trim_end_matches('/'))
        };

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "requesty".to_string(),
            base_url: base_url.clone(),
            api_key: config.api_key.clone(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::Requesty,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            base_url,
            api_key: config.api_key,
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new Requesty handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = RequestyConfig::from_settings(settings).ok_or_else(|| {
            roo_provider::ProviderError::ApiKeyRequired
        })?;
        Self::new(config)
    }

    /// Fetches available models from the Requesty API.
    ///
    /// Results are cached in memory; subsequent calls return the cached list.
    pub async fn fetch_models(&self) -> roo_provider::error::Result<ModelRecord> {
        // Check cache first
        {
            let cache = self.dynamic_models.read().unwrap();
            if let Some(ref models) = *cache {
                return Ok(models.clone());
            }
        }

        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(roo_provider::ProviderError::api_error_response(
                "requesty", status, body,
            ));
        }

        let body = response.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&body)?;

        let mut model_map: ModelRecord = HashMap::new();

        if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
            for entry in data {
                let id = entry["id"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }

                let info = ModelInfo {
                    max_tokens: Some(8192),
                    context_window: 128000,
                    description: Some(format!("Requesty model: {}", id)),
                    ..Default::default()
                };
                model_map.insert(id, info);
            }
        }

        // Cache result
        *self.dynamic_models.write().unwrap() = Some(model_map.clone());

        Ok(model_map)
    }

    /// Resolves model info for the configured model ID.
    fn resolve_model_info(&self) -> (String, ModelInfo) {
        // Try static models first
        if let Some(info) = models::models().get(&self.model_id) {
            return (self.model_id.clone(), info.clone());
        }

        // Try dynamic cache
        if let Ok(cache) = self.dynamic_models.read() {
            if let Some(ref dynamic) = *cache {
                if let Some(info) = dynamic.get(&self.model_id) {
                    return (self.model_id.clone(), info.clone());
                }
            }
        }

        // Fallback to inner provider
        self.inner.get_model()
    }
}

#[async_trait]
impl Provider for RequestyHandler {
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
        self.resolve_model_info()
    }

    async fn complete_prompt(
        &self,
        prompt: &str,
    ) -> Result<String, roo_provider::ProviderError> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::Requesty
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
            assert!(info.max_tokens.is_some(), "Model '{}' missing max_tokens", id);
            assert!(info.input_price.is_some(), "Model '{}' missing input_price", id);
            assert!(info.output_price.is_some(), "Model '{}' missing output_price", id);
        }
    }

    #[test]
    fn test_default_url() {
        assert_eq!(RequestyConfig::DEFAULT_BASE_URL, "https://api.requesty.ai");
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = RequestyHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("gpt-4o".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "gpt-4o");
    }

    #[test]
    fn test_provider_name() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Requesty);
    }

    #[test]
    fn test_base_url_v1_suffix() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: "https://custom.requesty.ai".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_base_url_already_has_v1() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: "https://custom.requesty.ai/v1".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_fallback_model_info() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("unknown-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_temperature_config() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: Some(0.7),
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_request_timeout_config() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: Some(60000),
        };
        let handler = RequestyHandler::new(config);
        assert!(handler.is_ok());
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("dynamic-model-x".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "dynamic-model-x".to_string(),
            ModelInfo {
                max_tokens: Some(16384),
                context_window: 256000,
                description: Some("Dynamically loaded model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "dynamic-model-x");
        assert_eq!(info.context_window, 256000);
        assert_eq!(info.max_tokens, Some(16384));
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = RequestyConfig {
            api_key: "test-key".to_string(),
            base_url: RequestyConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RequestyHandler::new(config).unwrap();

        // Populate dynamic cache with different info
        let mut dynamic = HashMap::new();
        dynamic.insert(
            models::DEFAULT_MODEL_ID.to_string(),
            ModelInfo {
                max_tokens: Some(999),
                context_window: 999,
                description: Some("dynamic override".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        // Static model info should take priority
        let (_, info) = handler.get_model();
        assert_ne!(info.context_window, 999);
    }
}
