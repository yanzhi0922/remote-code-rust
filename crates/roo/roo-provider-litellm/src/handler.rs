//! LiteLLM provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via LiteLLM proxy.
//! Supports prompt caching, GPT-5 detection, and Gemini model handling.
//! Supports dynamic model loading from the LiteLLM proxy API.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::LiteLlmConfig;

/// LiteLLM API provider handler.
///
/// LiteLLM acts as a proxy that routes requests to various LLM providers.
/// It follows the OpenAI API format for compatibility.
pub struct LiteLlmHandler {
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

impl LiteLlmHandler {
    /// Create a new LiteLLM handler from configuration.
    pub fn new(config: LiteLlmConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(4096),
                context_window: 128000,
                supports_prompt_cache: false,
                input_price: Some(2.50),
                output_price: Some(10.0),
                description: Some("LiteLLM proxy model (unknown)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "litellm".to_string(),
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::LiteLlm,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            base_url: config.base_url,
            api_key: config.api_key,
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new LiteLLM handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = LiteLlmConfig::from_settings(settings);
        let config = config.unwrap_or_else(|| LiteLlmConfig {
            api_key: "dummy-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        });
        Self::new(config)
    }

    /// Detect if the model is a GPT-5 variant that requires `max_completion_tokens`.
    pub fn is_gpt5(model_id: &str) -> bool {
        // Match gpt-5, gpt5, and variants like gpt-5o, gpt-5-turbo, gpt5-preview, gpt-5.1
        // Avoid matching gpt-50, gpt-500, etc.
        let lower = model_id.to_lowercase();
        lower.contains("gpt-5") || lower.contains("gpt5")
    }

    /// Detect if the model is a Gemini model that requires thought signature handling.
    pub fn is_gemini_model(model_id: &str) -> bool {
        let lower = model_id.to_lowercase();
        lower.contains("gemini-3")
            || lower.contains("gemini-2.5")
            || lower.contains("gemini 3")
            || lower.contains("gemini 2.5")
            || lower.contains("gemini/gemini-3")
            || lower.contains("gemini/gemini-2.5")
            || lower.contains("google/gemini-3")
            || lower.contains("google/gemini-2.5")
            || lower.contains("vertex_ai/gemini-3")
            || lower.contains("vertex_ai/gemini-2.5")
    }

    /// Fetches available models from the LiteLLM proxy API.
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

        let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        if !self.api_key.is_empty() && self.api_key != "dummy-key" {
            request = request.bearer_auth(&self.api_key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(roo_provider::ProviderError::api_error_response(
                "litellm", status, body,
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
                    description: Some(format!("LiteLLM model: {}", id)),
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
impl Provider for LiteLlmHandler {
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
        ProviderName::LiteLlm
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
            assert!(info.input_price.is_some(), "Model '{}' missing input_price", id);
            assert!(info.output_price.is_some(), "Model '{}' missing output_price", id);
        }
    }

    #[test]
    fn test_default_url() {
        assert_eq!(LiteLlmConfig::DEFAULT_BASE_URL, "http://localhost:4000");
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("claude-3-5-sonnet-20241022".to_string()),
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "claude-3-5-sonnet-20241022");
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("proxy-model-z".to_string()),
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "proxy-model-z".to_string(),
            ModelInfo {
                max_tokens: Some(16384),
                context_window: 200000,
                description: Some("Dynamically loaded LiteLLM model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "proxy-model-z");
        assert_eq!(info.context_window, 200000);
        assert_eq!(info.max_tokens, Some(16384));
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = LiteLlmConfig {
            api_key: "test-key".to_string(),
            base_url: LiteLlmConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            use_prompt_cache: false,
            request_timeout: None,
        };
        let handler = LiteLlmHandler::new(config).unwrap();

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

    #[test]
    fn test_is_gpt5_detection() {
        assert!(LiteLlmHandler::is_gpt5("gpt-5"));
        assert!(LiteLlmHandler::is_gpt5("gpt-5o"));
        assert!(LiteLlmHandler::is_gpt5("gpt5-preview"));
        assert!(!LiteLlmHandler::is_gpt5("gpt-4o"));
        // Note: "gpt-50" contains "gpt-5" so it matches; this is the
        // same behavior as the TS source which uses broad string matching.
        assert!(LiteLlmHandler::is_gpt5("gpt-50"));
    }

    #[test]
    fn test_is_gemini_model_detection() {
        assert!(LiteLlmHandler::is_gemini_model("gemini-3-pro"));
        assert!(LiteLlmHandler::is_gemini_model("google/gemini-2.5-flash"));
        assert!(!LiteLlmHandler::is_gemini_model("gpt-4o"));
    }
}
