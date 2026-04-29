//! LM Studio provider handler.
//!
//! Uses the OpenAI-compatible chat completions API provided by LM Studio.
//! Supports `<think/>` tag processing via [`TagMatcher`] for reasoning
//! content classification.
//! Supports dynamic model loading from the LM Studio `/v1/models` endpoint.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use futures::StreamExt;
use roo_provider::error::Result;
use roo_provider::handler::{ApiStream, CreateMessageMetadata, Provider};
use roo_provider::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
use roo_types::api::{ApiStreamChunk, ProviderName};
use roo_types::model::{ModelInfo, ModelRecord};

use crate::models;
use crate::types::LmStudioConfig;

// ---------------------------------------------------------------------------
// TagMatcher — <think/> tag processing
// ---------------------------------------------------------------------------

/// Processes `<think/>` tags in streaming text, classifying content as
/// either reasoning (inside `<think >...</think >` blocks) or regular text.
///
/// This handles the common case where `<think >` and `</think >` tags
/// arrive as complete tokens. Partial tags at chunk boundaries are
/// buffered until more data arrives.
pub(crate) struct TagMatcher {
    /// Whether we're currently inside a `<think >` block.
    inside: bool,
    /// Buffer for potential partial tag matches at chunk boundaries.
    buffer: String,
}

/// Result of tag matching: `(is_reasoning, text)`.
type TagResult = (bool, String);

impl TagMatcher {
    /// Create a new `TagMatcher` starting outside any tag.
    pub fn new() -> Self {
        Self {
            inside: false,
            buffer: String::new(),
        }
    }

    /// Feed new text and return classified chunks.
    ///
    /// Each returned tuple is `(is_reasoning, text_content)`.
    pub fn update(&mut self, chunk: &str) -> Vec<TagResult> {
        self.buffer.push_str(chunk);
        self.drain_completed()
    }

    /// Flush any remaining buffered content.
    #[allow(dead_code)]
    pub fn finalize(&mut self) -> Vec<TagResult> {
        let mut out = self.drain_completed();
        if !self.buffer.is_empty() {
            out.push((self.inside, std::mem::take(&mut self.buffer)));
        }
        out
    }

    /// Process the buffer, extracting content around complete tags.
    fn drain_completed(&mut self) -> Vec<TagResult> {
        let mut results = Vec::new();

        loop {
            let open_idx = self.buffer.find("<think");
            let close_idx = self.buffer.find("</think");

            let next = match (open_idx, close_idx) {
                (Some(o), Some(c)) if o <= c => Some(("open", o)),
                (Some(_), Some(c)) => Some(("close", c)),
                (Some(_), None) => Some(("open", open_idx.unwrap())),
                (None, Some(_)) => Some(("close", close_idx.unwrap())),
                (None, None) => None,
            };

            match next {
                Some(("open", idx)) => {
                    if let Some(gt_off) = self.buffer[idx..].find('>') {
                        let tag_end = idx + gt_off + 1;
                        let tag = &self.buffer[idx..tag_end];

                        if idx > 0 {
                            let text = self.buffer[..idx].to_string();
                            if !text.is_empty() {
                                results.push((self.inside, text));
                            }
                        }
                        if !tag.contains('/') {
                            self.inside = true;
                        }
                        self.buffer = self.buffer[tag_end..].to_string();
                    } else {
                        // Incomplete opening tag — emit safe prefix
                        if idx > 0 {
                            let text = self.buffer[..idx].to_string();
                            if !text.is_empty() {
                                results.push((self.inside, text));
                            }
                            self.buffer = self.buffer[idx..].to_string();
                        }
                        break;
                    }
                }
                Some(("close", idx)) => {
                    if let Some(gt_off) = self.buffer[idx..].find('>') {
                        let tag_end = idx + gt_off + 1;

                        if idx > 0 {
                            let text = self.buffer[..idx].to_string();
                            if !text.is_empty() {
                                results.push((self.inside, text));
                            }
                        }
                        self.inside = false;
                        self.buffer = self.buffer[tag_end..].to_string();
                    } else {
                        if idx > 0 {
                            let text = self.buffer[..idx].to_string();
                            if !text.is_empty() {
                                results.push((self.inside, text));
                            }
                            self.buffer = self.buffer[idx..].to_string();
                        }
                        break;
                    }
                }
                None => {
                    // No tags at all — emit everything
                    if !self.buffer.is_empty() {
                        let text = std::mem::take(&mut self.buffer);
                        if !text.is_empty() {
                            results.push((self.inside, text));
                        }
                    }
                    break;
                }
                _ => unreachable!(),
            }
        }

        results
    }
}

// ---------------------------------------------------------------------------
// LmStudioHandler
// ---------------------------------------------------------------------------

/// LM Studio API provider handler.
///
/// Wraps an [`OpenAiCompatibleProvider`] internally and adds
/// `<think/>` tag processing for reasoning content classification.
pub struct LmStudioHandler {
    inner: OpenAiCompatibleProvider,
    model_id: String,
    model_info: ModelInfo,
    /// Base URL for API requests.
    base_url: String,
    #[allow(dead_code)]
    speculative_decoding_enabled: bool,
    #[allow(dead_code)]
    draft_model_id: Option<String>,
    /// Cache for dynamically fetched models.
    dynamic_models: RwLock<Option<ModelRecord>>,
}

impl LmStudioHandler {
    /// Create a new LM Studio handler from configuration.
    pub fn new(config: LmStudioConfig) -> Result<Self> {
        let model_id = config
            .model_id
            .unwrap_or_else(|| models::default_model_id());
        let model_info = models::default_model_info();

        let compatible_config = OpenAiCompatibleConfig {
            provider_name: "lmstudio".to_string(),
            base_url: config.base_url.clone(),
            api_key: LmStudioConfig::PLACEHOLDER_API_KEY.to_string(),
            default_model_id: models::default_model_id(),
            default_temperature: config.temperature.unwrap_or(models::DEFAULT_TEMPERATURE),
            model_id: Some(model_id.clone()),
            model_info: model_info.clone(),
            provider_name_enum: ProviderName::LmStudio,
            request_timeout: config.request_timeout,
            reasoning_effort: None,
        };

        let inner = OpenAiCompatibleProvider::new(compatible_config)?;

        Ok(Self {
            inner,
            model_id,
            model_info,
            base_url: config.base_url,
            speculative_decoding_enabled: config.speculative_decoding_enabled,
            draft_model_id: config.draft_model_id,
            dynamic_models: RwLock::new(None),
        })
    }

    /// Create a new LM Studio handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config = LmStudioConfig::from_settings(settings);
        Self::new(config)
    }

    /// Fetches available models from the LM Studio API.
    ///
    /// Uses the OpenAI-compatible `/v1/models` endpoint.
    /// Results are cached in memory; subsequent calls return the cached list.
    ///
    /// For local LM Studio instances, connection failures are handled gracefully.
    pub async fn fetch_models(&self) -> Result<ModelRecord> {
        // Check cache first
        {
            let cache = self.dynamic_models.read().unwrap();
            if let Some(ref models) = *cache {
                return Ok(models.clone());
            }
        }

        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(roo_provider::ProviderError::Reqwest)?;

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => {
                // For local providers, connection failure is expected if not running
                let empty: ModelRecord = HashMap::new();
                *self.dynamic_models.write().unwrap() = Some(empty.clone());
                return Ok(empty);
            }
        };

        if !response.status().is_success() {
            let empty: ModelRecord = HashMap::new();
            *self.dynamic_models.write().unwrap() = Some(empty.clone());
            return Ok(empty);
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
                    context_window: 200_000,
                    description: Some(format!("LM Studio model: {}", id)),
                    input_price: Some(0.0),
                    output_price: Some(0.0),
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
    ///
    /// LM Studio uses dynamic models, so we skip static lookup and
    /// check dynamic cache first, then fall back to defaults.
    fn resolve_model_info(&self) -> (String, ModelInfo) {
        // Try dynamic cache first (LM Studio models are loaded dynamically)
        if let Ok(cache) = self.dynamic_models.read() {
            if let Some(ref dynamic) = *cache {
                if let Some(info) = dynamic.get(&self.model_id) {
                    return (self.model_id.clone(), info.clone());
                }
            }
        }

        // Fallback to stored model info (set at construction from defaults)
        (self.model_id.clone(), self.model_info.clone())
    }
}

#[async_trait]
impl Provider for LmStudioHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<roo_types::api::ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        let stream = self
            .inner
            .create_message(system_prompt, messages, tools, metadata)
            .await?;

        // Wrap the stream with TagMatcher processing
        let matcher = Arc::new(Mutex::new(TagMatcher::new()));

        let processed = stream.flat_map(move |chunk_result| {
            let m = matcher.clone();
            let results: Vec<Result<ApiStreamChunk>> = match chunk_result {
                Ok(ApiStreamChunk::Text { ref text }) => {
                    let mut guard = m.lock().unwrap();
                    let tag_results = guard.update(text);
                    tag_results
                        .into_iter()
                        .map(|(is_reasoning, t)| {
                            Ok(if is_reasoning {
                                ApiStreamChunk::Reasoning {
                                    text: t,
                                    signature: None,
                                }
                            } else {
                                ApiStreamChunk::Text { text: t }
                            })
                        })
                        .collect()
                }
                Ok(chunk) => vec![Ok(chunk)],
                Err(e) => vec![Err(e)],
            };
            futures::stream::iter(results)
        });

        Ok(Box::pin(processed))
    }

    fn get_model(&self) -> (String, ModelInfo) {
        self.resolve_model_info()
    }

    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        self.inner.complete_prompt(prompt).await
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::LmStudio
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TagMatcher tests ----

    #[test]
    fn test_tag_matcher_plain_text() {
        let mut m = TagMatcher::new();
        let results = m.update("hello world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (false, "hello world".to_string()));
    }

    #[test]
    fn test_tag_matcher_think_block() {
        let mut m = TagMatcher::new();
        let results = m.update("<think >reasoning here</think >");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (true, "reasoning here".to_string()));
    }

    #[test]
    fn test_tag_matcher_mixed_content() {
        let mut m = TagMatcher::new();
        let results = m.update("before<think >inside</think >after");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], (false, "before".to_string()));
        assert_eq!(results[1], (true, "inside".to_string()));
        assert_eq!(results[2], (false, "after".to_string()));
    }

    #[test]
    fn test_tag_matcher_self_closing() {
        let mut m = TagMatcher::new();
        let results = m.update("before<think/>after");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (false, "before".to_string()));
        assert_eq!(results[1], (false, "after".to_string()));
    }

    #[test]
    fn test_tag_matcher_finalize_empty() {
        let mut m = TagMatcher::new();
        // No updates — finalize returns nothing
        let results = m.finalize();
        assert!(results.is_empty());
    }

    #[test]
    fn test_tag_matcher_finalize_after_full_emit() {
        let mut m = TagMatcher::new();
        let results = m.update("hello world");
        assert_eq!(results.len(), 1);
        // Buffer was drained by update, finalize returns nothing
        let final_results = m.finalize();
        assert!(final_results.is_empty());
    }

    // ---- Handler tests ----

    #[test]
    fn test_handler_creation_no_api_key_required() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("my-local-model".to_string()),
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "my-local-model");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::LmStudio);
    }

    #[test]
    fn test_config_default_url() {
        assert_eq!(
            LmStudioConfig::DEFAULT_BASE_URL,
            "http://localhost:1234/v1"
        );
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.lm_studio_model_id = Some("test-model".to_string());

        let config = LmStudioConfig::from_settings(&settings);
        assert_eq!(config.model_id, Some("test-model".to_string()));
        assert_eq!(config.base_url, LmStudioConfig::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_config_from_settings_custom_url() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.lm_studio_base_url = Some("http://192.168.1.100:1234".to_string());

        let config = LmStudioConfig::from_settings(&settings);
        assert_eq!(config.base_url, "http://192.168.1.100:1234/v1");
    }

    #[test]
    fn test_config_from_settings_speculative_decoding() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.lm_studio_speculative_decoding_enabled = Some(true);
        settings.lm_studio_draft_model_id = Some("draft-model".to_string());

        let config = LmStudioConfig::from_settings(&settings);
        assert!(config.speculative_decoding_enabled);
        assert_eq!(config.draft_model_id, Some("draft-model".to_string()));
    }

    #[test]
    fn test_default_model_info_sane() {
        let info = models::default_model_info();
        assert_eq!(info.max_tokens, Some(8192));
        assert_eq!(info.context_window, 200_000);
        assert_eq!(info.input_price, Some(0.0));
        assert_eq!(info.output_price, Some(0.0));
        assert!(info.description.is_some());
    }

    #[test]
    fn test_placeholder_api_key() {
        assert_eq!(LmStudioConfig::PLACEHOLDER_API_KEY, "noop");
    }

    // --- Dynamic model loading tests ---

    #[test]
    fn test_dynamic_models_cache_initially_empty() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();
        let cache = handler.dynamic_models.read().unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_resolve_model_uses_dynamic_when_not_in_static() {
        let config = LmStudioConfig {
            base_url: LmStudioConfig::DEFAULT_BASE_URL.to_string(),
            model_id: Some("downloaded-model".to_string()),
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();

        // Populate dynamic cache
        let mut dynamic = HashMap::new();
        dynamic.insert(
            "downloaded-model".to_string(),
            ModelInfo {
                max_tokens: Some(4096),
                context_window: 32768,
                description: Some("Downloaded LM Studio model".to_string()),
                input_price: Some(0.0),
                output_price: Some(0.0),
                ..Default::default()
            },
        );
        *handler.dynamic_models.write().unwrap() = Some(dynamic);

        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "downloaded-model");
        assert_eq!(info.context_window, 32768);
        assert_eq!(info.max_tokens, Some(4096));
    }

    #[test]
    fn test_fetch_models_handles_connection_failure_gracefully() {
        let config = LmStudioConfig {
            base_url: "http://localhost:19998/v1".to_string(),
            model_id: None,
            temperature: None,
            request_timeout: None,
            speculative_decoding_enabled: false,
            draft_model_id: None,
        };
        let handler = LmStudioHandler::new(config).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(handler.fetch_models());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
