//! Unbound provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via Unbound.
//! Supports custom metadata headers and cache token tracking.
//! Supports dynamic model loading from the Unbound API.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::UnboundConfig;

/// Unbound API provider handler.
///
/// Unbound provides access to multiple LLM providers through a unified API
/// with custom metadata tracking and cache token support.
pub struct UnboundHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID.
    model_id: String,
    /// API key for authentication.
    api_key: String,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
}

impl UnboundHandler {
    /// Create a new Unbound handler from configuration.
    pub fn new(config: UnboundConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(4096),
                context_window: 128000,
                supports_prompt_cache: false,
                input_price: Some(0.0),
                output_price: Some(0.0),
                description: Some("Unbound model (unknown)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "unbound".to_string(),
            base_url: UnboundConfig::DEFAULT_BASE_URL.to_string(),
            api_key: config.api_key.clone(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::Unbound,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            api_key: config.api_key,
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new Unbound handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = UnboundConfig::from_settings(settings).ok_or_else(|| {
            roo_provider::ProviderError::ApiKeyRequired
        })?;
        Self::new(config)
    }

    /// Fetches available models from the Unbound API.
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

        let url = format!("{}/models", UnboundConfig::DEFAULT_BASE_URL.trim_end_matches('/'));

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
                "unbound", status, body,
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
                    max_tokens: Some(4096),
                    context_window: 128000,
                    description: Some(format!("Unbound model: {}", id)),
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
impl Provider for UnboundHandler {
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
        ProviderName::Unbound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;

    #[test]
    fn test_default_model_exists() {
        let all_models = models::models();
        assert!(all_models.contains_key(models::DEFAULT_MODEL_ID));
    }

    #[test]
    fn test_all_models_have_required_fields() {
        for (id, info) in models::models() {
            assert!(info.max_tokens.is_some(), "Model '{}' missing max_tokens", id);
        }
    }

    #[test]
    fn test_default_url() {
        assert_eq!(UnboundConfig::DEFAULT_BASE_URL, "https://api.getunbound.ai/v1");
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = UnboundHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: Some("claude-3-5-sonnet".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "claude-3-5-sonnet");
    }

    #[test]
    fn test_provider_name() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Unbound);
    }

    #[test]
    fn test_fallback_model_info() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: Some("unknown-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_temperature_config() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: None,
            temperature: Some(0.5),
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config);
        assert!(handler.is_ok());
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: Some("dynamic-model-y".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "dynamic-model-y".to_string(),
            ModelInfo {
                max_tokens: Some(8192),
                context_window: 200000,
                description: Some("Dynamically loaded Unbound model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "dynamic-model-y");
        assert_eq!(info.context_window, 200000);
        assert_eq!(info.max_tokens, Some(8192));
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = UnboundConfig {
            api_key: "test-key".to_string(),
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = UnboundHandler::new(config).unwrap();

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
