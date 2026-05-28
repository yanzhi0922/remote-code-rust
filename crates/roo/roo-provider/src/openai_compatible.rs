//! OpenAI-compatible provider base class.
//!
//! Derived from `src/api/providers/base-openai-compatible-provider.ts`.
//! Handles SSE stream parsing, usage metrics, and tool call processing.

use std::collections::HashSet;
use std::pin::Pin;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt, TryStreamExt};
use serde::Deserialize;

use roo_types::api::{ApiMessage, ApiStreamChunk, ProviderName};
use roo_types::model::ModelInfo;

use crate::base_provider::{BaseProvider, convert_tools_for_openai};
use crate::error::{ProviderError, Result};
use crate::handler::{ApiStream, CreateMessageMetadata, Provider};
use crate::transform::openai_format::convert_to_openai_messages;

// ---------------------------------------------------------------------------
// OpenAI SSE response types
// ---------------------------------------------------------------------------

/// A chunk from the OpenAI streaming API.
#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Option<Vec<OpenAiChoice>>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    delta: Option<OpenAiDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    /// OpenRouter reasoning_details array (used by Gemini 3, Claude, etc.)
    /// See: https://openrouter.ai/docs/use-cases/reasoning-tokens#preserving-reasoning-blocks
    reasoning_details: Option<Vec<OpenAiReasoningDetail>>,
}

/// A single reasoning detail entry from OpenRouter.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiReasoningDetail {
    #[serde(rename = "type")]
    detail_type: Option<String>,
    text: Option<String>,
    summary: Option<String>,
    data: Option<String>,
    id: Option<Option<String>>,
    format: Option<String>,
    signature: Option<String>,
    index: Option<u64>,
}

// ---------------------------------------------------------------------------
// ReasoningDetailsAccumulator — merges fragments across SSE chunks
// Source: `src/api/providers/openrouter.ts` and `src/api/providers/roo.ts`
// ---------------------------------------------------------------------------

/// Accumulated reasoning detail entry being built across SSE chunks.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct AccumulatedReasoningDetail {
    detail_type: String,
    text: Option<String>,
    summary: Option<String>,
    data: Option<String>,
    id: Option<String>,
    format: Option<String>,
    signature: Option<String>,
    index: u64,
}

/// Accumulates reasoning detail fragments across SSE chunks into a Map,
/// keyed by `"type-index"` (OpenRouter style).
///
/// Source: `src/api/providers/openrouter.ts` — `reasoningDetailsAccumulator`
#[derive(Debug, Clone, Default)]
struct ReasoningDetailsAccumulator {
    entries: std::collections::HashMap<String, AccumulatedReasoningDetail>,
}

impl ReasoningDetailsAccumulator {
    /// Process a list of reasoning details from an SSE chunk delta.
    /// Merges text/summary/data fragments into existing entries.
    fn accumulate(&mut self, details: &[OpenAiReasoningDetail]) {
        for (i, detail) in details.iter().enumerate() {
            let detail_type = detail.detail_type.as_deref().unwrap_or("");
            if detail_type.is_empty() {
                continue;
            }

            let index = detail.index.unwrap_or(i as u64);
            let key = format!("{detail_type}-{index}");

            let entry = self
                .entries
                .entry(key)
                .or_insert_with(|| AccumulatedReasoningDetail {
                    detail_type: detail_type.to_string(),
                    index,
                    ..Default::default()
                });

            // Concatenate text fragments
            if let Some(ref text) = detail.text {
                if let Some(ref mut existing) = entry.text {
                    existing.push_str(text);
                } else {
                    entry.text = Some(text.clone());
                }
            }

            // Concatenate summary fragments
            if let Some(ref summary) = detail.summary {
                if let Some(ref mut existing) = entry.summary {
                    existing.push_str(summary);
                } else {
                    entry.summary = Some(summary.clone());
                }
            }

            // Concatenate data fragments
            if let Some(ref data) = detail.data {
                if let Some(ref mut existing) = entry.data {
                    existing.push_str(data);
                } else {
                    entry.data = Some(data.clone());
                }
            }

            // Update metadata from later chunks (take last seen)
            if let Some(ref id) = detail.id
                && let Some(inner_id) = id.as_deref()
            {
                entry.id = Some(inner_id.to_string());
            }
            if let Some(ref format) = detail.format {
                entry.format = Some(format.clone());
            }
            if let Some(ref sig) = detail.signature {
                entry.signature = Some(sig.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ThinkTagMatcher — extracts <think_open>...<think_close> from content
// Source: `.research/Roo-Code/src/utils/tag-matcher.ts` + usage in
//         `base-openai-compatible-provider.ts` line 120
// ---------------------------------------------------------------------------

/// Lightweight streaming matcher for `<think_open>...<think_close>` regions.
///
/// Some providers (e.g. DeepSeek R1 via certain OpenAI-compatible endpoints)
/// embed reasoning content inside `<think_open>` tags in the regular `content`
/// field rather than using `reasoning_content`. This matcher splits content
/// into reasoning chunks (inside tags) and text chunks (outside tags).
#[derive(Debug, Clone)]
struct ThinkTagMatcher {
    /// Current state machine position.
    state: ThinkTagState,
    /// Current nesting depth (how many open tags we've seen).
    depth: usize,
    /// Index into the tag name being matched.
    index: usize,
    /// Buffer for the current chunk being built.
    buffer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThinkTagState {
    /// Outside any tag — content is regular text.
    Text,
    /// Possibly inside `<think_open` opening tag.
    TagOpen,
    /// Possibly inside `</think_open>` closing tag — skipped the `/`.
    TagClose,
}

impl ThinkTagMatcher {
    fn new() -> Self {
        Self {
            state: ThinkTagState::Text,
            depth: 0,
            index: 0,
            buffer: String::new(),
        }
    }

    /// Process a content chunk and return extracted reasoning/text segments.
    ///
    /// Returns a list of `(is_reasoning, text)` tuples.
    fn update(&mut self, chunk: &str) -> Vec<(bool, String)> {
        let mut results = Vec::new();
        // Tag name without angle brackets — matches both "think_open" and "think_close".
        // We match on "think_open" for opening and the same for closing after seeing `</`.
        const OPEN_TAG: &str = "think_open";
        const CLOSE_TAG: &str = "think_close";

        for ch in chunk.chars() {
            self.buffer.push(ch);

            match self.state {
                ThinkTagState::Text => {
                    if ch == '<' {
                        self.state = ThinkTagState::TagOpen;
                        self.index = 0;
                    } else {
                        // Flush buffer as a text or reasoning chunk
                        self.flush(&mut results);
                    }
                }
                ThinkTagState::TagOpen => {
                    if ch == '/' && self.index == 0 {
                        // This is a closing tag: `</...`
                        self.state = ThinkTagState::TagClose;
                    } else if self.index < OPEN_TAG.len()
                        && OPEN_TAG.as_bytes()[self.index] == (ch as u8)
                    {
                        self.index += 1;
                    } else if ch == '>' && self.index == OPEN_TAG.len() {
                        // Matched `<think_open>`
                        self.state = ThinkTagState::Text;
                        self.depth += 1;
                        self.buffer.clear(); // Discard the tag itself
                    } else {
                        // Not a matching tag — flush as text and go back to Text state
                        self.state = ThinkTagState::Text;
                        self.flush(&mut results);
                    }
                }
                ThinkTagState::TagClose => {
                    if self.index < CLOSE_TAG.len()
                        && CLOSE_TAG.as_bytes()[self.index] == (ch as u8)
                    {
                        self.index += 1;
                    } else if ch == '>' && self.index == CLOSE_TAG.len() {
                        // Matched `</think_close>`
                        self.state = ThinkTagState::Text;
                        if self.depth > 0 {
                            self.depth -= 1;
                        }
                        self.buffer.clear(); // Discard the tag itself
                    } else {
                        // Not a matching tag — flush and go back
                        self.state = ThinkTagState::Text;
                        self.flush(&mut results);
                    }
                }
            }
        }

        results
    }

    /// Flush the current buffer as a chunk.
    fn flush(&mut self, results: &mut Vec<(bool, String)>) {
        if !self.buffer.is_empty() {
            let is_reasoning = self.depth > 0;
            results.push((is_reasoning, std::mem::take(&mut self.buffer)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallDelta {
    index: u64,
    id: Option<String>,
    function: Option<OpenAiFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    prompt_tokens_details: Option<OpenAiPromptTokensDetails>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiPromptTokensDetails {
    cached_tokens: Option<u64>,
    /// OpenAI standard field for cache write tokens.
    cache_write_tokens: Option<u64>,
    /// DeepSeek-specific field name for prompt cache miss tokens.
    /// Source: `src/api/providers/deepseek.ts` — `processUsageMetrics`
    #[serde(default)]
    cache_miss_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// Usage metrics
// ---------------------------------------------------------------------------

/// Processes OpenAI usage metrics into an ApiStreamChunk.
///
/// Source: `src/api/providers/base-openai-compatible-provider.ts` — `processUsageMetrics`
pub fn process_usage_metrics(usage: &OpenAiUsage, model_info: &ModelInfo) -> ApiStreamChunk {
    let input_tokens = usage.prompt_tokens.unwrap_or(0);
    let output_tokens = usage.completion_tokens.unwrap_or(0);
    let cache_write_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cache_write_tokens.or(d.cache_miss_tokens))
        .unwrap_or(0);
    let cache_read_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);

    let total_cost = calculate_api_cost_openai(
        model_info,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
    );

    ApiStreamChunk::Usage {
        input_tokens,
        output_tokens,
        cache_write_tokens: if cache_write_tokens > 0 {
            Some(cache_write_tokens)
        } else {
            None
        },
        cache_read_tokens: if cache_read_tokens > 0 {
            Some(cache_read_tokens)
        } else {
            None
        },
        reasoning_tokens: None,
        total_cost: Some(total_cost),
    }
}

/// Calculates API cost based on token usage and model pricing.
/// Delegates to the shared cost module.
fn calculate_api_cost_openai(
    model_info: &ModelInfo,
    input_tokens: u64,
    output_tokens: u64,
    cache_write_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    crate::cost::calculate_api_cost(
        model_info,
        input_tokens,
        output_tokens,
        if cache_write_tokens > 0 {
            Some(cache_write_tokens)
        } else {
            None
        },
        if cache_read_tokens > 0 {
            Some(cache_read_tokens)
        } else {
            None
        },
    )
}

// ---------------------------------------------------------------------------
// OpenAI-compatible provider
// ---------------------------------------------------------------------------

/// Configuration for an OpenAI-compatible provider.
pub struct OpenAiCompatibleConfig {
    pub provider_name: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model_id: String,
    pub default_temperature: f64,
    pub model_id: Option<String>,
    pub model_info: ModelInfo,
    pub provider_name_enum: ProviderName,
    pub request_timeout: Option<u64>,
    /// Optional reasoning effort level (e.g. "low", "medium", "high").
    ///
    /// Source: `src/api/providers/openai.ts` — `reasoning_effort` in request body
    pub reasoning_effort: Option<String>,
    /// Whether streaming is enabled. When false, falls back to non-streaming
    /// chat completion.
    /// Source: `src/api/providers/openai.ts` — `openAiStreamingEnabled`
    pub streaming_enabled: Option<bool>,
    /// Whether to use `max_completion_tokens` instead of `max_tokens` in the
    /// request body. This is required for O-family models (o1, o3, o4) and
    /// certain other newer models.
    ///
    /// Source: `src/api/providers/openai.ts` — `includeMaxTokens`
    pub include_max_tokens: Option<bool>,
    /// Additional JSON fields to merge into the request body.
    /// Used by providers like Ollama that accept extra parameters (e.g. `num_ctx`).
    pub extra_body_fields: Option<serde_json::Value>,
}

/// Base class for OpenAI-compatible API providers.
///
/// Source: `src/api/providers/base-openai-compatible-provider.ts`
pub struct OpenAiCompatibleProvider {
    base: BaseProvider,
    http_client: reqwest::Client,
    api_key: String,
    base_url: String,
    provider_name_str: String,
    default_temperature: f64,
    reasoning_effort: Option<String>,
    streaming_enabled: bool,
    include_max_tokens: bool,
    extra_body_fields: Option<serde_json::Value>,
    /// Cache for the OpenAI-converted tool schemas. Tools are static for a
    /// given session, so converting once avoids repeated schema traversal.
    converted_tools_cache: std::sync::OnceLock<Vec<serde_json::Value>>,
}

impl OpenAiCompatibleProvider {
    /// Create a new OpenAI-compatible provider.
    pub fn new(config: OpenAiCompatibleConfig) -> Result<Self> {
        let model_id = config
            .model_id
            .unwrap_or_else(|| config.default_model_id.clone());

        let base = BaseProvider::new(model_id, config.model_info, config.provider_name_enum);

        let mut client_builder = reqwest::Client::builder();
        if let Some(timeout) = config.request_timeout {
            client_builder = client_builder.timeout(std::time::Duration::from_millis(timeout));
        }

        let http_client = client_builder.build().map_err(ProviderError::Reqwest)?;

        Ok(Self {
            base,
            http_client,
            api_key: config.api_key,
            base_url: config.base_url,
            provider_name_str: config.provider_name,
            default_temperature: config.default_temperature,
            reasoning_effort: config.reasoning_effort,
            streaming_enabled: config.streaming_enabled.unwrap_or(true),
            include_max_tokens: config.include_max_tokens.unwrap_or(false),
            extra_body_fields: config.extra_body_fields,
            converted_tools_cache: std::sync::OnceLock::new(),
        })
    }

    /// Build the request body for a streaming chat completion.
    fn build_stream_request_body(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&[serde_json::Value]>,
        metadata: &CreateMessageMetadata,
    ) -> Result<serde_json::Value> {
        let (model, info) = self.base.get_model();

        let max_tokens = info.max_tokens;

        // Detect o-family reasoning models (o1, o3, o4-mini).
        // Source: `src/api/providers/openai.ts` — `handleO3FamilyMessage`
        let is_o_family = model.contains("o1") || model.contains("o3") || model.contains("o4");

        let openai_messages = convert_to_openai_messages(messages, None)?;

        // o-family models use "developer" role with formatting prefix.
        let system_msg = if is_o_family {
            serde_json::json!({
                "role": "developer",
                "content": format!("Formatting re-enabled\n{}", system_prompt)
            })
        } else {
            serde_json::json!({
                "role": "system",
                "content": system_prompt
            })
        };

        let mut system_and_messages = vec![system_msg];
        system_and_messages.extend(openai_messages);

        let mut body = serde_json::json!({
            "model": model,
            "messages": system_and_messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "parallel_tool_calls": metadata.parallel_tool_calls.unwrap_or(true),
        });

        // o-family models do not accept temperature.
        if !is_o_family {
            let temperature = self.default_temperature;
            body["temperature"] = serde_json::json!(temperature);
        }

        // o-family models use max_completion_tokens instead of max_tokens.
        // When include_max_tokens is configured, also use max_completion_tokens
        // for non-o-family models (for providers that require it).
        // Source: `src/api/providers/openai.ts` — `includeMaxTokens`
        if let Some(max_tokens) = max_tokens {
            if is_o_family || self.include_max_tokens {
                body["max_completion_tokens"] = serde_json::json!(max_tokens);
            } else {
                body["max_tokens"] = serde_json::json!(max_tokens);
            }
        }

        if let Some(tools) = tools {
            let converted = self
                .converted_tools_cache
                .get_or_init(|| convert_tools_for_openai(Some(tools)).unwrap_or_default());
            if !converted.is_empty() {
                body["tools"] = serde_json::json!(converted);
            }
        }

        if let Some(ref tool_choice) = metadata.tool_choice {
            body["tool_choice"] = tool_choice.clone();
        }

        // Add reasoning_effort if configured.
        // Source: `src/api/providers/openai.ts` — `reasoning_effort` in request body
        if let Some(ref effort) = self.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        // Merge extra body fields (e.g. Ollama's `num_ctx`).
        if let Some(ref extra) = self.extra_body_fields
            && let Some(obj) = body.as_object_mut()
            && let Some(extra_obj) = extra.as_object()
        {
            for (key, value) in extra_obj {
                obj.insert(key.clone(), value.clone());
            }
        }

        Ok(body)
    }

    /// Create a streaming response from the API.
    async fn create_stream(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&[serde_json::Value]>,
        metadata: &CreateMessageMetadata,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<OpenAiStreamChunk>> + Send>>> {
        let body = self.build_stream_request_body(system_prompt, messages, tools, metadata)?;
        let (_model, _) = self.base.get_model();

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/RooVetGit/Roo-Cline")
            .header("X-Title", "Roo Code")
            .header("User-Agent", "Roo Code")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                &self.provider_name_str,
                status,
                text,
            ));
        }

        // Parse SSE stream
        let provider_name = self.provider_name_str.clone();
        let stream = response
            .bytes_stream()
            .eventsource()
            .map(move |event| match event {
                Ok(event) => {
                    if event.data == "[DONE]" {
                        return None;
                    }
                    match serde_json::from_str::<OpenAiStreamChunk>(&event.data) {
                        Ok(chunk) => Some(Ok(chunk)),
                        Err(e) => Some(Err(ProviderError::ParseError(format!(
                            "Failed to parse stream chunk: {e}"
                        )))),
                    }
                }
                Err(e) => Some(Err(ProviderError::StreamError(format!("SSE error: {e}")))),
            })
            .filter_map(|item| async move { item })
            .map_err(move |e| ProviderError::StreamError(format!("{provider_name}: {e}")));

        Ok(Box::pin(stream))
    }

    /// Create a message from a pre-built request body.
    ///
    /// This allows providers that need custom message formatting (e.g. DeepSeek
    /// with R1 format) to build their own request body while still reusing the
    /// HTTP client and SSE stream parsing infrastructure.
    pub async fn create_message_from_body(&self, body: serde_json::Value) -> Result<ApiStream> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/RooVetGit/Roo-Cline")
            .header("X-Title", "Roo Code")
            .header("User-Agent", "Roo Code")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                &self.provider_name_str,
                status,
                text,
            ));
        }

        // Parse SSE stream
        let provider_name = self.provider_name_str.clone();
        let stream = response
            .bytes_stream()
            .eventsource()
            .map(move |event| match event {
                Ok(event) => {
                    if event.data == "[DONE]" {
                        return None;
                    }
                    match serde_json::from_str::<OpenAiStreamChunk>(&event.data) {
                        Ok(chunk) => Some(Ok(chunk)),
                        Err(e) => Some(Err(ProviderError::ParseError(format!(
                            "Failed to parse stream chunk: {e}"
                        )))),
                    }
                }
                Err(e) => Some(Err(ProviderError::StreamError(format!("SSE error: {e}")))),
            })
            .filter_map(|item| async move { item })
            .map_err(move |e| ProviderError::StreamError(format!("{provider_name}: {e}")));

        let (_, model_info) = self.base.get_model();

        // Process the stream into ApiStreamChunks
        let mut active_tool_call_ids: HashSet<String> = HashSet::new();
        let model_info = model_info.clone();
        let tag_matcher = std::sync::Arc::new(std::sync::Mutex::new(ThinkTagMatcher::new()));
        let reasoning_accumulator =
            std::sync::Arc::new(std::sync::Mutex::new(ReasoningDetailsAccumulator::default()));

        let processed = stream.flat_map(move |chunk_result| {
            let results: Vec<Result<ApiStreamChunk>> = match chunk_result {
                Ok(chunk) => {
                    let delta = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.delta.as_ref());
                    let finish_reason = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.finish_reason.as_ref())
                        .cloned();

                    let mut results: Vec<Result<ApiStreamChunk>> = Vec::new();

                    // Handle content — run through ThinkTagMatcher to extract
                    // <think_open>...<think_close> regions as reasoning.
                    // Source: `.research/Roo-Code/src/api/providers/base-openai-compatible-provider.ts` line 120
                    if let Some(delta) = delta {
                        if let Some(ref content) = delta.content {
                            if let Ok(mut matcher) = tag_matcher.lock() {
                                for (is_reasoning, text) in matcher.update(content) {
                                    if text.is_empty() {
                                        continue;
                                    }
                                    if is_reasoning {
                                        results.push(Ok(ApiStreamChunk::Reasoning {
                                            text,
                                            signature: None,
                                        }));
                                    } else {
                                        results.push(Ok(ApiStreamChunk::Text { text }));
                                    }
                                }
                            } else {
                                results.push(Ok(ApiStreamChunk::Text {
                                    text: content.clone(),
                                }));
                            }
                        }

                        // Handle reasoning_details (OpenRouter format for Gemini 3, Claude, etc.)
                        // Priority: reasoning_details > reasoning_content > reasoning
                        // If reasoning_details has displayable content, skip top-level reasoning
                        // to avoid duplicate display (matches TS behavior).
                        //
                        // Also accumulates fragments into a Map for later retrieval via
                        // consolidate_reasoning_details().
                        // Source: `src/api/providers/openrouter.ts` — reasoningDetailsAccumulator
                        let mut has_reasoning_from_details = false;
                        if let Some(ref details) = delta.reasoning_details {
                            // Accumulate into the Map for later retrieval
                            if let Ok(mut acc) = reasoning_accumulator.lock() {
                                acc.accumulate(details);
                            }
                            for detail in details {
                                let reasoning_text = match detail.detail_type.as_deref() {
                                    Some("reasoning.text") => detail.text.as_deref(),
                                    Some("reasoning.summary") => detail.summary.as_deref(),
                                    _ => None, // Skip reasoning.encrypted and other types
                                };
                                if let Some(text) = reasoning_text
                                    && !text.is_empty()
                                {
                                    has_reasoning_from_details = true;
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: text.to_string(),
                                        signature: None,
                                    }));
                                }
                            }
                        }

                        // Handle reasoning content (fallback when no reasoning_details)
                        if !has_reasoning_from_details {
                            if let Some(ref reasoning) = delta.reasoning_content {
                                if !reasoning.trim().is_empty() {
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: reasoning.clone(),
                                        signature: None,
                                    }));
                                }
                            } else if let Some(ref reasoning) = delta.reasoning
                                && !reasoning.trim().is_empty()
                            {
                                results.push(Ok(ApiStreamChunk::Reasoning {
                                    text: reasoning.clone(),
                                    signature: None,
                                }));
                            }
                        }

                        // Handle tool calls
                        if let Some(ref tool_calls) = delta.tool_calls {
                            for tool_call in tool_calls {
                                if let Some(ref id) = tool_call.id {
                                    active_tool_call_ids.insert(id.clone());
                                }
                                results.push(Ok(ApiStreamChunk::ToolCallPartial {
                                    index: tool_call.index,
                                    id: tool_call.id.clone(),
                                    name: tool_call.function.as_ref().and_then(|f| f.name.clone()),
                                    arguments: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.clone()),
                                }));
                            }
                        }
                    }

                    // Emit tool_call_end events when finish_reason is "tool_calls"
                    if finish_reason.as_deref() == Some("tool_calls")
                        && !active_tool_call_ids.is_empty()
                    {
                        for id in active_tool_call_ids.drain() {
                            results.push(Ok(ApiStreamChunk::ToolCallEnd { id }));
                        }
                    }

                    // Handle usage
                    if let Some(ref usage) = chunk.usage {
                        results.push(Ok(process_usage_metrics(usage, &model_info)));
                    }

                    results
                }
                Err(e) => vec![Err(e)],
            };

            futures::stream::iter(results)
        });

        Ok(Box::pin(processed))
    }

    /// Create a message using developer role for O-family models.
    ///
    /// This method builds a request body with the system message using
    /// the `"developer"` role (required by o1/o3/o4 models), omits the
    /// `temperature` parameter, adds `reasoning_effort`, and uses
    /// `max_completion_tokens` instead of `max_tokens`.
    ///
    /// Source: `src/api/providers/openai.ts` — `handleO3FamilyMessage`
    pub async fn create_message_with_developer_role(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&[serde_json::Value]>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        // Build the base body using the standard method.
        // The builder already detects o-family models and sets developer role,
        // omits temperature, and uses max_completion_tokens. We override
        // explicitly here for safety.
        let mut body = self.build_stream_request_body(
            &format!("Formatting re-enabled\n{}", system_prompt),
            &messages,
            tools,
            &metadata,
        )?;

        // Ensure the system message uses "developer" role
        if let Some(msgs) = body.get_mut("messages")
            && let Some(msgs_arr) = msgs.as_array_mut()
            && let Some(first) = msgs_arr.first_mut()
            && first.get("role").and_then(|r| r.as_str()) == Some("system")
        {
            first["role"] = serde_json::Value::String("developer".to_string());
        }

        // Remove temperature for O-family models
        if let Some(obj) = body.as_object_mut() {
            obj.remove("temperature");
        }

        // Ensure reasoning_effort is set
        if let Some(ref effort) = self.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        // Use max_completion_tokens instead of max_tokens
        if let Some(obj) = body.as_object_mut()
            && let Some(max_tokens) = obj.remove("max_tokens")
        {
            obj.insert("max_completion_tokens".to_string(), max_tokens);
        }

        // When streaming is disabled, convert to non-streaming body
        if !self.streaming_enabled {
            body["stream"] = serde_json::json!(false);
            if let Some(obj) = body.as_object_mut() {
                obj.remove("stream_options");
            }
        }

        self.create_message_from_body(body).await
    }

    /// Non-streaming fallback when streaming is disabled.
    ///
    /// Source: `src/api/providers/openai.ts` — the `else` branch when
    /// `openAiStreamingEnabled` is false. Makes a single chat completion
    /// request and wraps the result into ApiStreamChunks.
    async fn create_message_non_streaming(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&[serde_json::Value]>,
        metadata: &CreateMessageMetadata,
    ) -> Result<ApiStream> {
        let body = self.build_stream_request_body(system_prompt, messages, tools, metadata)?;
        // Override stream to false for non-streaming
        let mut body = body;
        body["stream"] = serde_json::json!(false);
        body.as_object_mut().map(|o| o.remove("stream_options"));

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/RooVetGit/Roo-Cline")
            .header("X-Title", "Roo Code")
            .header("User-Agent", "Roo Code")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                &self.provider_name_str,
                status,
                text,
            ));
        }

        let (_, model_info) = self.base.get_model();

        let resp: serde_json::Value = response.json().await.map_err(ProviderError::Reqwest)?;

        let mut chunks: Vec<Result<ApiStreamChunk>> = Vec::new();

        // Emit tool calls if present
        if let Some(tool_calls) = resp
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("tool_calls"))
            .and_then(|tc| tc.as_array())
        {
            for (i, tc) in tool_calls.iter().enumerate() {
                if tc.get("type").and_then(|t| t.as_str()) == Some("function") {
                    chunks.push(Ok(ApiStreamChunk::ToolCall {
                        id: tc
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        name: tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string(),
                        arguments: tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("{}")
                            .to_string(),
                    }));
                    // Also emit tool_call_end
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                        chunks.push(Ok(ApiStreamChunk::ToolCallEnd { id: id.to_string() }));
                    }
                    let _ = i; // suppress unused warning
                }
            }
        }

        // Emit text content
        let content = resp
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        chunks.push(Ok(ApiStreamChunk::Text {
            text: content.to_string(),
        }));

        // Emit usage
        if let Some(usage) = resp.get("usage") {
            let openai_usage = OpenAiUsage {
                prompt_tokens: usage.get("prompt_tokens").and_then(|v| v.as_u64()),
                completion_tokens: usage.get("completion_tokens").and_then(|v| v.as_u64()),
                prompt_tokens_details: usage
                    .get("prompt_tokens_details")
                    .and_then(|d| serde::Deserialize::deserialize(d).ok()),
            };
            chunks.push(Ok(process_usage_metrics(&openai_usage, &model_info)));
        }

        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&[serde_json::Value]>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        // When streaming is disabled, fall back to non-streaming chat completion.
        // Source: `src/api/providers/openai.ts` — `openAiStreamingEnabled ?? true`
        if !self.streaming_enabled {
            return self
                .create_message_non_streaming(system_prompt, messages, tools, &metadata)
                .await;
        }

        let stream = self
            .create_stream(system_prompt, messages, tools, &metadata)
            .await?;

        let (_, model_info) = self.base.get_model();

        // Process the stream into ApiStreamChunks
        let mut active_tool_call_ids: HashSet<String> = HashSet::new();
        let model_info = model_info.clone();
        let tag_matcher = std::sync::Arc::new(std::sync::Mutex::new(ThinkTagMatcher::new()));
        let _reasoning_accumulator =
            std::sync::Arc::new(std::sync::Mutex::new(ReasoningDetailsAccumulator::default()));

        let processed = stream.flat_map(move |chunk_result| {
            let results: Vec<Result<ApiStreamChunk>> = match chunk_result {
                Ok(chunk) => {
                    let delta = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.delta.as_ref());
                    let finish_reason = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.finish_reason.as_ref())
                        .cloned();

                    let mut results: Vec<Result<ApiStreamChunk>> = Vec::new();

                    // Handle content — run through ThinkTagMatcher to extract
                    // <think_open>...<think_close> regions as reasoning.
                    // Source: `.research/Roo-Code/src/api/providers/base-openai-compatible-provider.ts` line 120
                    if let Some(delta) = delta {
                        if let Some(ref content) = delta.content {
                            if let Ok(mut matcher) = tag_matcher.lock() {
                                for (is_reasoning, text) in matcher.update(content) {
                                    if text.is_empty() {
                                        continue;
                                    }
                                    if is_reasoning {
                                        results.push(Ok(ApiStreamChunk::Reasoning {
                                            text,
                                            signature: None,
                                        }));
                                    } else {
                                        results.push(Ok(ApiStreamChunk::Text { text }));
                                    }
                                }
                            } else {
                                results.push(Ok(ApiStreamChunk::Text {
                                    text: content.clone(),
                                }));
                            }
                        }

                        // Handle reasoning_details (OpenRouter format for Gemini 3, Claude, etc.)
                        let mut has_reasoning_from_details = false;
                        if let Some(ref details) = delta.reasoning_details {
                            for detail in details {
                                let reasoning_text = match detail.detail_type.as_deref() {
                                    Some("reasoning.text") => detail.text.as_deref(),
                                    Some("reasoning.summary") => detail.summary.as_deref(),
                                    _ => None,
                                };
                                if let Some(text) = reasoning_text
                                    && !text.is_empty()
                                {
                                    has_reasoning_from_details = true;
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: text.to_string(),
                                        signature: None,
                                    }));
                                }
                            }
                        }

                        // Handle reasoning content (fallback when no reasoning_details)
                        if !has_reasoning_from_details {
                            if let Some(ref reasoning) = delta.reasoning_content {
                                if !reasoning.trim().is_empty() {
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: reasoning.clone(),
                                        signature: None,
                                    }));
                                }
                            } else if let Some(ref reasoning) = delta.reasoning
                                && !reasoning.trim().is_empty()
                            {
                                results.push(Ok(ApiStreamChunk::Reasoning {
                                    text: reasoning.clone(),
                                    signature: None,
                                }));
                            }
                        }

                        // Handle tool calls
                        if let Some(ref tool_calls) = delta.tool_calls {
                            for tool_call in tool_calls {
                                if let Some(ref id) = tool_call.id {
                                    active_tool_call_ids.insert(id.clone());
                                }
                                results.push(Ok(ApiStreamChunk::ToolCallPartial {
                                    index: tool_call.index,
                                    id: tool_call.id.clone(),
                                    name: tool_call.function.as_ref().and_then(|f| f.name.clone()),
                                    arguments: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.clone()),
                                }));
                            }
                        }
                    }

                    // Emit tool_call_end events when finish_reason is "tool_calls"
                    if finish_reason.as_deref() == Some("tool_calls")
                        && !active_tool_call_ids.is_empty()
                    {
                        for id in active_tool_call_ids.drain() {
                            results.push(Ok(ApiStreamChunk::ToolCallEnd { id }));
                        }
                    }

                    // Handle usage
                    if let Some(ref usage) = chunk.usage {
                        results.push(Ok(process_usage_metrics(usage, &model_info)));
                    }

                    results
                }
                Err(e) => vec![Err(e)],
            };

            futures::stream::iter(results)
        });

        Ok(Box::pin(processed))
    }

    fn get_model(&self) -> (String, ModelInfo) {
        self.base.get_model()
    }

    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        let (model, _) = self.base.get_model();

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }]
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/RooVetGit/Roo-Cline")
            .header("X-Title", "Roo Code")
            .header("User-Agent", "Roo Code")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                &self.provider_name_str,
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
        self.base.provider_name_value
    }
}
