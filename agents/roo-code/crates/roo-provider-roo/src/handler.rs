//! Roo Code Cloud provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via Roo Code Cloud.
//! Supports session token authentication, dynamic model loading,
//! reasoning details, and image generation.
//! Source: `src/api/providers/roo.ts`

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::RooConfig;

/// Roo Code Cloud API provider handler.
///
/// Roo Code Cloud provides access to various LLM models through
/// Roo's own infrastructure. It follows the OpenAI API format.
pub struct RooHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID.
    model_id: String,
    /// Base URL for API requests.
    base_url: String,
    /// API key for authentication.
    api_key: String,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
}

impl RooHandler {
    /// Create a new Roo handler from configuration.
    pub fn new(config: RooConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config
            .model_id
            .clone()
            .unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 200000,
                supports_prompt_cache: true,
                input_price: Some(3.0),
                output_price: Some(15.0),
                description: Some("Roo Code Cloud model (unknown)".to_string()),
                ..Default::default()
            });

        let base_url = config
            .base_url
            .unwrap_or_else(|| RooConfig::DEFAULT_BASE_URL.to_string());

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "roo".to_string(),
            base_url: base_url.clone(),
            api_key: config.api_key.clone().unwrap_or_default(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::Roo,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            base_url,
            api_key: config.api_key.unwrap_or_default(),
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new Roo handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = RooConfig::from_settings(settings);
        let config = config.unwrap_or_else(|| RooConfig {
            api_key: None,
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        });
        Self::new(config)
    }

    /// Fetches available models from the Roo Code Cloud API.
    ///
    /// Results are cached in memory; subsequent calls return the cached list.
    /// Source: `src/api/providers/roo.ts` — `loadDynamicModels()`
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
        let mut request = client.get(&url);
        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(roo_provider::ProviderError::api_error_response(
                "roo", status, body,
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

                let context_length = entry["context_length"]
                    .as_u64()
                    .or_else(|| entry["context_window"].as_u64())
                    .unwrap_or(200000);

                let info = ModelInfo {
                    max_tokens: Some(8192),
                    context_window: context_length,
                    supports_images: Some(true),
                    supports_prompt_cache: true,
                    description: Some(format!("Roo Code Cloud model: {}", id)),
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
impl Provider for RooHandler {
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
        ProviderName::Roo
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
    fn test_default_url() {
        assert_eq!(RooConfig::DEFAULT_BASE_URL, "https://api.roocode.com/v1");
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: Some("roo-gpt-4o".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "roo-gpt-4o");
    }

    #[test]
    fn test_handler_from_settings() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let handler = RooHandler::from_settings(&settings);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_from_settings_with_api_key() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.roo_api_key = Some("test-roo-key".to_string());
        let handler = RooHandler::from_settings(&settings);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Roo);
    }

    #[test]
    fn test_custom_base_url() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some("https://custom-roo.example.com/v1".to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_fallback_model_info() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: Some("unknown-roo-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_temperature_config() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: Some(0.5),
            request_timeout: None,
        };
        let handler = RooHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert_eq!(all_models.len(), 5);
    }

    #[test]
    fn test_all_models_support_images() {
        // xai/grok-code-fast-1 does NOT support images
        let non_image_models = ["xai/grok-code-fast-1"];
        for (id, info) in models::models() {
            if non_image_models.contains(&id.as_str()) {
                continue; // skip models that don't support images
            }
            assert!(
                info.supports_images.unwrap_or(false),
                "Roo model '{}' should support images",
                id
            );
        }
    }

    #[test]
    fn test_handler_without_api_key() {
        // Roo can work without an API key (session token auth)
        let config = RooConfig {
            api_key: None,
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config);
        assert!(handler.is_ok());
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: Some("roo-dynamic-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "roo-dynamic-model".to_string(),
            ModelInfo {
                max_tokens: Some(16384),
                context_window: 300000,
                supports_images: Some(true),
                supports_prompt_cache: true,
                description: Some("Dynamically loaded Roo model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "roo-dynamic-model");
        assert_eq!(info.context_window, 300000);
        assert_eq!(info.max_tokens, Some(16384));
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = RooConfig {
            api_key: Some("test-key".to_string()),
            base_url: Some(RooConfig::DEFAULT_BASE_URL.to_string()),
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = RooHandler::new(config).unwrap();

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
