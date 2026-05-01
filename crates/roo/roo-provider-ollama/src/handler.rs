//! Ollama provider handler.
//!
//! Uses the OpenAI-compatible chat completions API provided by Ollama.
//! Ollama does not require an API key for local instances.
//! Supports dynamic model loading from the Ollama `/api/tags` endpoint.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roo_provider::{
    ApiStream, CreateMessageMetadata, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use roo_types::api::ProviderName;
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::OllamaConfig;

/// Ollama API provider handler.
pub struct OllamaHandler {
    inner: OpenAiCompatibleProvider,
    /// The configured model ID.
    model_id: String,
    /// Base URL for API requests.
    base_url: String,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
}

impl OllamaHandler {
    /// Create a new Ollama handler from configuration.
    pub fn new(config: OllamaConfig) -> Result<Self, roo_provider::ProviderError> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 131072,
                description: Some("Ollama model (unknown variant)".to_string()),
                ..Default::default()
            });

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "ollama".to_string(),
            base_url: config.base_url.clone(),
            // Ollama doesn't require an API key, use a placeholder
            api_key: "ollama".to_string(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(0.0),
            model_id: Some(model_id.clone()),
            model_info,
            provider_name_enum: ProviderName::Ollama,
            request_timeout: config.request_timeout,
        reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            base_url: config.base_url,
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new Ollama handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self, roo_provider::ProviderError> {
        let config = OllamaConfig::from_settings(settings);
        Self::new(config)
    }

    /// Fetches available models from the Ollama API.
    ///
    /// Uses the Ollama-specific `/api/tags` endpoint which returns
    /// a list of locally available models.
    /// Results are cached in memory; subsequent calls return the cached list.
    ///
    /// For local Ollama instances, connection failures are handled gracefully.
    pub async fn fetch_models(&self) -> roo_provider::error::Result<ModelRecord> {
        // Check cache first
        {
            let cache = self.dynamic_models.read().unwrap();
            if let Some(ref models) = *cache {
                return Ok(models.clone());
            }
        }

        // Ollama uses /api/tags, not /v1/models
        // Convert base_url from http://host:port/v1 to http://host:port/api/tags
        let tags_url = self.base_url.trim_end_matches("/v1").trim_end_matches('/');
        let url = format!("{}/api/tags", tags_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(roo_provider::ProviderError::Reqwest)?;

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => {
                // For local providers, connection failure is expected if not running
                // Return empty map rather than error
                let empty: ModelRecord = HashMap::new();
                *self.dynamic_models.write().unwrap() = Some(empty.clone());
                return Ok(empty);
            }
        };

        if !response.status().is_success() {
            // Gracefully handle failures for local provider
            let empty: ModelRecord = HashMap::new();
            *self.dynamic_models.write().unwrap() = Some(empty.clone());
            return Ok(empty);
        }

        let body = response.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&body)?;

        let mut model_map: ModelRecord = HashMap::new();

        // Ollama /api/tags returns { "models": [ { "name": "...", "model": "...", ... } ] }
        if let Some(models_arr) = parsed.get("models").and_then(|m| m.as_array()) {
            for entry in models_arr {
                let name = entry["name"].as_str()
                    .or_else(|| entry["model"].as_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }

                // Ollama may append :latest to model names
                let clean_name = name.trim_end_matches(":latest").to_string();

                let context_length = entry.get("parameters")
                    .and_then(|p| p.get("num_ctx"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(131072);

                let info = ModelInfo {
                    max_tokens: Some(8192),
                    context_window: context_length,
                    description: Some(format!("Ollama model: {}", clean_name)),
                    ..Default::default()
                };
                model_map.insert(clean_name, info);
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
impl Provider for OllamaHandler {
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
        ProviderName::Ollama
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
    fn test_all_models_have_max_tokens() {
        for (id, info) in models::models() {
            assert!(
                info.max_tokens.is_some(),
                "Model '{}' missing max_tokens",
                id
            );
        }
    }

    #[test]
    fn test_config_default_url() {
        assert_eq!(
            OllamaConfig::DEFAULT_BASE_URL,
            "http://localhost:11434/v1"
        );
    }

    #[test]
    fn test_handler_creation_no_api_key_required() {
        // Ollama doesn't require an API key
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("mistral".to_string()),
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "mistral");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Ollama);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_model_id = Some("codellama".to_string());

        let config = OllamaConfig::from_settings(&settings);
        assert_eq!(config.model_id, Some("codellama".to_string()));
        assert_eq!(config.base_url, OllamaConfig::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_config_from_settings_custom_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.ollama_base_url = Some("http://192.168.1.100:11434/v1".to_string());

        let config = OllamaConfig::from_settings(&settings);
        assert_eq!(config.base_url, "http://192.168.1.100:11434/v1");
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("my-custom-model".to_string()),
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "my-custom-model".to_string(),
            ModelInfo {
                max_tokens: Some(4096),
                context_window: 8192,
                description: Some("Custom Ollama model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "my-custom-model");
        assert_eq!(info.context_window, 8192);
        assert_eq!(info.max_tokens, Some(4096));
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = OllamaConfig {
            base_url: OllamaConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some(models::DEFAULT_MODEL_ID.to_string()),
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();

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
    fn test_fetch_models_handles_connection_failure_gracefully() {
        // Use a port that's very unlikely to have a server running
        let config = OllamaConfig {
            base_url: "http://localhost:19999/v1".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            api_options: None,
        };
        let handler = OllamaHandler::new(config).unwrap();

        // This should not panic or error — it should return an empty map
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(handler.fetch_models());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
