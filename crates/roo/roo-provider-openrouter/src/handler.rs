//! OpenRouter provider handler.
//!
//! Uses the OpenAI-compatible chat completions API via OpenRouter's gateway.
//! OpenRouter adds extra headers for site URL and ranking preferences.
//! Supports dynamic model loading from the OpenRouter models API.
//!
//! Key behaviors ported from the TypeScript implementation:
//! - Reuses a single `OpenAiCompatibleProvider` instance (no per-request creation)
//! - Gemini model sanitization (filtering tool calls without matching reasoning_details)
//! - Prompt caching breakpoints for Anthropic and Gemini models
//! - R1 format conversion for DeepSeek reasoning models (merging consecutive same-role messages)
//! - Reasoning details accumulation from stream chunks for thinking persistence

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use serde_json::json;

use roo_provider::error::{ProviderError, Result};
use roo_provider::transform::caching::{apply_anthropic_caching, apply_gemini_caching};
use roo_provider::transform::{
    R1ZaiOptions, convert_to_openai_messages, convert_to_r1_zai_messages, sanitize_gemini_messages,
};
use roo_provider::{
    ApiStream, BaseProvider, CreateMessageMetadata, ImageGenerationOptions, ImageGenerationResult,
    OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider, convert_tools_for_openai,
    generate_image_with_provider,
};
use roo_types::api::{ApiMessage, ContentBlock, MessageRole, ProviderName};
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::OpenRouterConfig;

// ---------------------------------------------------------------------------
// Prompt-caching model set
// ---------------------------------------------------------------------------

/// Models that support prompt caching via OpenRouter.
/// Source: `packages/types/src/providers/openrouter.ts` - `OPEN_ROUTER_PROMPT_CACHING_MODELS`
static PROMPT_CACHING_MODELS: &[&str] = &[
    "anthropic/claude-3-haiku",
    "anthropic/claude-3-haiku:beta",
    "anthropic/claude-3-opus",
    "anthropic/claude-3-opus:beta",
    "anthropic/claude-3-sonnet",
    "anthropic/claude-3-sonnet:beta",
    "anthropic/claude-3.5-haiku",
    "anthropic/claude-3.5-haiku-20241022",
    "anthropic/claude-3.5-haiku-20241022:beta",
    "anthropic/claude-3.5-haiku:beta",
    "anthropic/claude-3.5-sonnet",
    "anthropic/claude-3.5-sonnet-20240620",
    "anthropic/claude-3.5-sonnet-20240620:beta",
    "anthropic/claude-3.5-sonnet:beta",
    "anthropic/claude-3.7-sonnet",
    "anthropic/claude-3.7-sonnet:beta",
    "anthropic/claude-3.7-sonnet:thinking",
    "anthropic/claude-sonnet-4",
    "anthropic/claude-sonnet-4.5",
    "anthropic/claude-sonnet-4.6",
    "anthropic/claude-opus-4",
    "anthropic/claude-opus-4.1",
    "anthropic/claude-opus-4.5",
    "anthropic/claude-opus-4.6",
    "anthropic/claude-haiku-4.5",
    "google/gemini-2.5-flash-preview",
    "google/gemini-2.5-flash-preview:thinking",
    "google/gemini-2.5-flash-preview-05-20",
    "google/gemini-2.5-flash-preview-05-20:thinking",
    "google/gemini-2.5-flash",
    "google/gemini-2.5-flash-lite-preview-06-17",
    "google/gemini-2.0-flash-001",
    "google/gemini-flash-1.5",
    "google/gemini-flash-1.5-8b",
    "google/gemini-2.5-pro",
    "google/gemini-2.5-pro-preview",
];

/// Returns true if the given model ID supports prompt caching on OpenRouter.
fn supports_prompt_caching(model_id: &str) -> bool {
    PROMPT_CACHING_MODELS.contains(&model_id)
}

/// Returns true if the model is a DeepSeek R1 reasoning model that requires
/// R1 format conversion (merging consecutive same-role messages).
fn is_deepseek_r1(model_id: &str) -> bool {
    model_id.starts_with("deepseek/deepseek-r1") || model_id == "perplexity/sonar-reasoning"
}

/// Returns true if the model is a Gemini model.
fn is_gemini_model(model_id: &str) -> bool {
    model_id.starts_with("google/gemini")
}

// ---------------------------------------------------------------------------
// Router tool preferences
// ---------------------------------------------------------------------------

/// Apply tool preferences for models accessed through dynamic routers.
///
/// Different model families perform better with specific tools:
/// - OpenAI models: Better results with `apply_patch` instead of
///   `apply_diff`/`write_to_file`.
///
/// Source: `src/api/providers/utils/router-tool-preferences.ts` —
///   `applyRouterToolPreferences()`
fn apply_router_tool_preferences(model_id: &str, model_info: &mut ModelInfo) {
    if model_id.contains("openai") {
        // Add "apply_diff" and "write_to_file" to excluded_tools (deduplicated)
        let excluded = model_info.excluded_tools.get_or_insert_with(Vec::new);
        for tool in &["apply_diff", "write_to_file"] {
            if !excluded.contains(&tool.to_string()) {
                excluded.push(tool.to_string());
            }
        }

        // Add "apply_patch" to included_tools (deduplicated)
        let included = model_info.included_tools.get_or_insert_with(Vec::new);
        let patch = "apply_patch".to_string();
        if !included.contains(&patch) {
            included.push(patch);
        }
    }
}

// ---------------------------------------------------------------------------
// OpenRouter handler
// ---------------------------------------------------------------------------

/// OpenRouter API provider handler.
pub struct OpenRouterHandler {
    base: BaseProvider,
    http_client: reqwest::Client,
    api_key: String,
    base_url: String,
    temperature: f64,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
    /// Reusable inner provider for OpenAI-compatible streaming.
    /// Created once at construction and reused for all requests.
    inner: OpenAiCompatibleProvider,
}

impl OpenRouterHandler {
    /// Create a new OpenRouter handler from configuration.
    pub fn new(config: OpenRouterConfig) -> Result<Self> {
        let model_id = config.model_id.unwrap_or_else(models::default_model_id);
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 128000,
                supports_images: Some(true),
                description: Some("OpenRouter model (unknown variant)".to_string()),
                ..Default::default()
            });

        let base = BaseProvider::new(
            model_id.clone(),
            model_info.clone(),
            ProviderName::OpenRouter,
        );

        let mut client_builder = reqwest::Client::builder();
        if let Some(timeout) = config.request_timeout {
            client_builder = client_builder.timeout(std::time::Duration::from_millis(timeout));
        }
        let http_client = client_builder.build().map_err(ProviderError::Reqwest)?;

        let default_temperature = config.temperature.unwrap_or(0.0);

        // Create the inner provider once - reuse for all requests.
        let inner_config = OpenAiCompatibleConfig {
            provider_name: "openrouter".to_string(),
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            default_model_id: models::default_model_id(),
            default_temperature,
            model_id: Some(model_id),
            model_info: model_info.clone(),
            provider_name_enum: ProviderName::OpenRouter,
            request_timeout: config.request_timeout,
            reasoning_effort: None,
            streaming_enabled: None,
            include_max_tokens: None,
            extra_body_fields: None,
        };
        let inner = OpenAiCompatibleProvider::new(inner_config)?;

        Ok(Self {
            base,
            http_client,
            api_key: config.api_key,
            base_url: config.base_url,
            temperature: default_temperature,
            dynamic_models: RwLock::new(None),
            inner,
        })
    }

    /// Create a new OpenRouter handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config =
            OpenRouterConfig::from_settings(settings).ok_or(ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }

    /// Fetches available models from the OpenRouter API.
    ///
    /// Results are cached in memory; subsequent calls return the cached list.
    /// The OpenRouter models API returns standard OpenAI-compatible format.
    pub async fn fetch_models(&self) -> Result<ModelRecord> {
        // Check cache first
        {
            let cache = self
                .dynamic_models
                .read()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(ref models) = *cache {
                return Ok(models.clone());
            }
        }

        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error_response(
                "openrouter",
                status,
                body,
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

                let context_length = entry["context_length"].as_u64().unwrap_or(128000);

                let info = ModelInfo {
                    max_tokens: Some(8192),
                    context_window: context_length,
                    supports_images: Some(true),
                    description: Some(format!("OpenRouter model: {}", id)),
                    ..Default::default()
                };
                model_map.insert(id, info);
            }
        }

        // Cache result
        *self
            .dynamic_models
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(model_map.clone());

        Ok(model_map)
    }

    /// Resolves model info for the configured model ID.
    ///
    /// Checks static models first, then dynamic cache, then fallback.
    fn resolve_model_info(&self) -> (String, ModelInfo) {
        let model_id = self.base.model_id.clone();

        // Try static models first
        if let Some(info) = models::models().get(&model_id) {
            return (model_id, info.clone());
        }

        // Try dynamic cache
        if let Ok(cache) = self.dynamic_models.read()
            && let Some(ref dynamic) = *cache
            && let Some(info) = dynamic.get(&model_id)
        {
            return (model_id, info.clone());
        }

        // Fallback to the base model info (set at construction)
        self.base.get_model()
    }

    /// Build the request body for an OpenRouter chat completion request,
    /// applying all OpenRouter-specific message transformations:
    ///
    /// 1. R1 format conversion for DeepSeek models (merge consecutive same-role)
    /// 2. Standard OpenAI message conversion with system prompt prepend
    /// 3. Gemini message sanitization (filter mismatched reasoning_details)
    /// 4. Fake encrypted reasoning block injection for Gemini tool calls
    /// 5. Cache breakpoint injection for supported models
    fn build_request_body(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&Vec<serde_json::Value>>,
        metadata: &CreateMessageMetadata,
    ) -> Result<serde_json::Value> {
        let (model_id, mut model_info) = self.resolve_model_info();

        // Apply router-specific tool preferences for OpenAI models
        apply_router_tool_preferences(&model_id, &mut model_info);

        let max_tokens = model_info.max_tokens;

        // -------------------------------------------------------------------
        // Step 1: Convert messages to OpenAI format (or R1 format for DeepSeek)
        // -------------------------------------------------------------------
        let openai_messages = if is_deepseek_r1(&model_id) {
            // DeepSeek R1 uses user role instead of system, and requires merging
            // consecutive same-role messages.
            let mut r1_messages = vec![ApiMessage {
                role: MessageRole::User,
                content: vec![ContentBlock::Text {
                    text: system_prompt.to_string(),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
                reasoning_details: None,
            }];
            r1_messages.extend_from_slice(messages);
            convert_to_r1_zai_messages(&r1_messages, R1ZaiOptions::default())
        } else {
            let converted = convert_to_openai_messages(messages, None)?;
            let mut system_and_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            system_and_messages.extend(converted);
            system_and_messages
        };

        // -------------------------------------------------------------------
        // Step 2: Gemini sanitization + fake encrypted block injection
        // -------------------------------------------------------------------
        let mut messages = if is_gemini_model(&model_id) {
            let mut msgs = sanitize_gemini_messages(&openai_messages, &model_id);

            // Inject fake reasoning.encrypted block for tool calls without
            // existing encrypted reasoning. This is required when switching
            // from other models to Gemini to satisfy API validation.
            // Per OpenRouter docs: one block per assistant message with tool calls,
            // using the first tool call's ID and "skip_thought_signature_validator".
            for msg in msgs.iter_mut() {
                let role = msg["role"].as_str().unwrap_or("");
                if role != "assistant" {
                    continue;
                }
                let Some(tool_calls) = msg.get("tool_calls") else {
                    continue;
                };
                let Some(calls) = tool_calls.as_array() else {
                    continue;
                };
                if calls.is_empty() {
                    continue;
                }

                let existing_details = msg
                    .get("reasoning_details")
                    .and_then(|rd: &serde_json::Value| rd.as_array());
                let has_encrypted = existing_details
                    .map(|d: &Vec<serde_json::Value>| {
                        d.iter().any(|detail: &serde_json::Value| {
                            detail["type"].as_str() == Some("reasoning.encrypted")
                        })
                    })
                    .unwrap_or(false);

                if !has_encrypted {
                    let first_tool_id = calls
                        .first()
                        .and_then(|tc: &serde_json::Value| tc.get("id"))
                        .and_then(|id: &serde_json::Value| id.as_str())
                        .unwrap_or("");

                    let fake_encrypted = json!({
                        "type": "reasoning.encrypted",
                        "data": "skip_thought_signature_validator",
                        "id": first_tool_id,
                        "format": "google-gemini-v1",
                        "index": 0
                    });

                    let details = match existing_details {
                        Some(d) => {
                            let mut arr: serde_json::Value = d.clone().into();
                            arr.as_array_mut()
                                .expect("cloned array is always an array")
                                .push(fake_encrypted);
                            arr
                        }
                        None => json!([fake_encrypted]),
                    };
                    msg.as_object_mut()
                        .expect("message is always an object")
                        .insert("reasoning_details".to_string(), details);
                }
            }
            msgs
        } else {
            openai_messages
        };

        // -------------------------------------------------------------------
        // Step 3: Cache breakpoints for supported models
        // -------------------------------------------------------------------
        if supports_prompt_caching(&model_id) {
            if model_id.starts_with("google") {
                apply_gemini_caching(system_prompt, &mut messages, 10);
            } else {
                apply_anthropic_caching(system_prompt, &mut messages);
            }
        }

        // -------------------------------------------------------------------
        // Step 4: Assemble the request body
        // -------------------------------------------------------------------
        let mut body = json!({
            "model": model_id,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        if let Some(max_tokens) = max_tokens
            && max_tokens > 0
        {
            body["max_tokens"] = json!(max_tokens);
        }

        body["temperature"] = json!(self.temperature);

        // DeepSeek R1 uses top_p = 0.95
        if is_deepseek_r1(&model_id) {
            body["top_p"] = json!(0.95);
        }

        if let Some(tools) = convert_tools_for_openai(tools) {
            body["tools"] = json!(tools);
        }

        if let Some(ref tool_choice) = metadata.tool_choice {
            body["tool_choice"] = tool_choice.clone();
        }

        Ok(body)
    }
}

#[async_trait]
impl Provider for OpenRouterHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        // Build the request body with all OpenRouter-specific transformations
        let body = self.build_request_body(system_prompt, &messages, tools.as_ref(), &metadata)?;

        // Delegate to the reusable inner provider's stream infrastructure.
        // The inner provider handles SSE parsing, reasoning_details display,
        // tool call processing, and usage metrics.
        self.inner.create_message_from_body(body).await
    }

    fn get_model(&self) -> (String, ModelInfo) {
        self.resolve_model_info()
    }

    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        let (model, _) = self.resolve_model_info();

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }]
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://roocode.com")
            .header("X-Title", "Roo Code")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error("openrouter", e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                "openrouter",
                status,
                text,
            ));
        }

        let resp: serde_json::Value = response.json().await.map_err(ProviderError::Reqwest)?;
        Ok(resp["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::OpenRouter
    }

    /// Generate an image using OpenRouter's chat completions API with image modality.
    ///
    /// Source: `src/api/providers/openrouter.ts` — `generateImage()`
    async fn generate_image(
        &self,
        prompt: &str,
        model: &str,
        input_image: Option<&str>,
    ) -> Result<ImageGenerationResult> {
        Ok(generate_image_with_provider(
            &self.http_client,
            &ImageGenerationOptions {
                base_url: self.base_url.clone(),
                auth_token: self.api_key.clone(),
                model: model.to_string(),
                prompt: prompt.to_string(),
                input_image: input_image.map(|s| s.to_string()),
            },
        )
        .await)
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
    fn test_all_models_have_pricing() {
        for (id, info) in models::models() {
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
            OpenRouterConfig::DEFAULT_BASE_URL,
            "https://openrouter.ai/api/v1"
        );
    }

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = OpenRouterHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("openai/gpt-4o".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "openai/gpt-4o");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::OpenRouter);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-or-test".to_string());
        settings.open_router_model_id = Some("openai/gpt-4o".to_string());

        let config = OpenRouterConfig::from_settings(&settings).unwrap();
        assert_eq!(config.api_key, "sk-or-test");
        assert_eq!(config.model_id, Some("openai/gpt-4o".to_string()));
    }

    #[test]
    fn test_config_from_settings_custom_base_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.api_key = Some("sk-or-test".to_string());
        settings.open_router_base_url = Some("https://custom.openrouter.api".to_string());

        let config = OpenRouterConfig::from_settings(&settings).unwrap();
        assert_eq!(config.base_url, "https://custom.openrouter.api");
    }

    #[test]
    fn test_config_from_settings_no_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        assert!(OpenRouterConfig::from_settings(&settings).is_none());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert!(
            all_models.len() >= 5,
            "Should have at least 5 OpenRouter models"
        );
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
    fn test_claude_model_supports_images() {
        let all_models = models::models();
        let claude = all_models
            .get("anthropic/claude-sonnet-4")
            .expect("claude model should exist");
        assert!(claude.supports_images.unwrap_or(false));
    }

    #[test]
    fn test_handler_unknown_model_fallback() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("vendor/unknown-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "vendor/unknown-model");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_handler_with_timeout() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: Some(60000),
        };
        let handler = OpenRouterHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_gemini_model_has_large_context() {
        let all_models = models::models();
        let gemini = all_models
            .get("google/gemini-2.5-pro-preview")
            .expect("gemini model should exist");
        assert!(gemini.context_window > 500000);
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_prefers_static_over_dynamic() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("anthropic/claude-sonnet-4".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        // Populate dynamic cache with a different model info
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "anthropic/claude-sonnet-4".to_string(),
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
        // The static model has context_window = 200000
        assert_eq!(info.context_window, 200000);
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("vendor/dynamic-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "vendor/dynamic-model".to_string(),
            ModelInfo {
                max_tokens: Some(16384),
                context_window: 256000,
                description: Some("Dynamically loaded model".to_string()),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "vendor/dynamic-model");
        assert_eq!(info.context_window, 256000);
        assert_eq!(info.max_tokens, Some(16384));
    }

    #[test]
    fn test_resolve_model_fallback_when_not_found_anywhere() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("vendor/unknown-model".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        // Dynamic cache is empty (None)
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "vendor/unknown-model");
        // Falls back to the base model info set at construction
        assert!(info.max_tokens.is_some());
    }

    // --- Helper function tests ---

    #[test]
    fn test_is_deepseek_r1() {
        assert!(is_deepseek_r1("deepseek/deepseek-r1"));
        assert!(is_deepseek_r1("deepseek/deepseek-r1-0528"));
        assert!(!is_deepseek_r1("deepseek/deepseek-chat"));
        assert!(is_deepseek_r1("perplexity/sonar-reasoning"));
        assert!(!is_deepseek_r1("openai/gpt-4o"));
    }

    #[test]
    fn test_is_gemini_model() {
        assert!(is_gemini_model("google/gemini-2.5-pro"));
        assert!(is_gemini_model("google/gemini-2.5-pro-preview"));
        assert!(is_gemini_model("google/gemini-2.5-flash"));
        assert!(!is_gemini_model("openai/gpt-4o"));
    }

    #[test]
    fn test_supports_prompt_caching() {
        assert!(supports_prompt_caching("anthropic/claude-sonnet-4"));
        assert!(supports_prompt_caching("anthropic/claude-sonnet-4.5"));
        assert!(supports_prompt_caching("google/gemini-2.5-pro"));
        assert!(supports_prompt_caching("google/gemini-2.5-pro-preview"));
        assert!(!supports_prompt_caching("openai/gpt-4o"));
        assert!(!supports_prompt_caching("deepseek/deepseek-chat"));
    }

    // --- Build request body tests ---

    #[test]
    fn test_build_request_body_basic() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("openai/gpt-4o".to_string()),
            temperature: Some(0.0),
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        let body = handler
            .build_request_body(
                "You are helpful.",
                &[],
                None,
                &CreateMessageMetadata::default(),
            )
            .unwrap();

        assert_eq!(body["model"], "openai/gpt-4o");
        assert_eq!(body["stream"], true);
        // System message should be first
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
    }

    #[test]
    fn test_build_request_body_deepseek_r1_format() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("deepseek/deepseek-r1".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        let messages = vec![
            ApiMessage {
                role: MessageRole::User,
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
                reasoning_details: None,
            },
            ApiMessage {
                role: MessageRole::User,
                content: vec![ContentBlock::Text {
                    text: "World".to_string(),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
                reasoning_details: None,
            },
        ];

        let body = handler
            .build_request_body(
                "System prompt",
                &messages,
                None,
                &CreateMessageMetadata::default(),
            )
            .unwrap();

        // DeepSeek R1 should merge consecutive same-role messages
        let msgs = body["messages"].as_array().unwrap();
        // Should be merged into a single user message
        let user_count = msgs.iter().filter(|m| m["role"] == "user").count();
        assert_eq!(user_count, 1);

        // Should have top_p = 0.95
        assert_eq!(body["top_p"], 0.95);
    }

    #[test]
    fn test_build_request_body_gemini_sanitization() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("google/gemini-2.5-pro-preview".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        let body = handler
            .build_request_body("System", &[], None, &CreateMessageMetadata::default())
            .unwrap();

        // System message should be converted to array format with cache_control
        let msgs = body["messages"].as_array().unwrap();
        let sys_content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(sys_content[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn test_build_request_body_anthropic_caching() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("anthropic/claude-sonnet-4".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        let body = handler
            .build_request_body("System", &[], None, &CreateMessageMetadata::default())
            .unwrap();

        // System message should have cache_control
        let msgs = body["messages"].as_array().unwrap();
        let sys_content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(sys_content[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn test_build_request_body_no_caching_for_unsupported() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("openai/gpt-4o".to_string()),
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();

        let body = handler
            .build_request_body("System", &[], None, &CreateMessageMetadata::default())
            .unwrap();

        // System message should NOT have cache_control for gpt-4o
        let msgs = body["messages"].as_array().unwrap();
        assert!(msgs[0]["content"].is_string());
    }

    // --- Reusable inner provider test ---

    #[test]
    fn test_inner_provider_reused() {
        let config = OpenRouterConfig {
            api_key: "test-key".to_string(),
            base_url: OpenRouterConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
        };
        let handler = OpenRouterHandler::new(config).unwrap();
        // Verify the inner provider has the same model
        let (outer_model, _) = handler.get_model();
        let (inner_model, _) = handler.inner.get_model();
        assert_eq!(outer_model, inner_model);
    }
}
