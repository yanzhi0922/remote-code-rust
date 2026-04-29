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

use crate::base_provider::{convert_tools_for_openai, BaseProvider};
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
struct OpenAiPromptTokensDetails {
    cached_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// Usage metrics
// ---------------------------------------------------------------------------

/// Processes OpenAI usage metrics into an ApiStreamChunk.
///
/// Source: `src/api/providers/base-openai-compatible-provider.ts` — `processUsageMetrics`
pub fn process_usage_metrics(
    usage: &OpenAiUsage,
    model_info: &ModelInfo,
) -> ApiStreamChunk {
    let input_tokens = usage.prompt_tokens.unwrap_or(0);
    let output_tokens = usage.completion_tokens.unwrap_or(0);
    let cache_write_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cache_write_tokens)
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
        if cache_write_tokens > 0 { Some(cache_write_tokens) } else { None },
        if cache_read_tokens > 0 { Some(cache_read_tokens) } else { None },
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
            client_builder =
                client_builder.timeout(std::time::Duration::from_millis(timeout));
        }

        let http_client = client_builder
            .build()
            .map_err(ProviderError::Reqwest)?;

        Ok(Self {
            base,
            http_client,
            api_key: config.api_key,
            base_url: config.base_url,
            provider_name_str: config.provider_name,
            default_temperature: config.default_temperature,
            reasoning_effort: config.reasoning_effort,
        })
    }

    /// Build the request body for a streaming chat completion.
    fn build_stream_request_body(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&Vec<serde_json::Value>>,
        metadata: &CreateMessageMetadata,
    ) -> Result<serde_json::Value> {
        let (model, info) = self.base.get_model();

        let max_tokens = info.max_tokens;
        let temperature = self.default_temperature;

        let openai_messages = convert_to_openai_messages(messages, None)?;

        let mut system_and_messages = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt
        })];
        system_and_messages.extend(openai_messages);

        let mut body = serde_json::json!({
            "model": model,
            "temperature": temperature,
            "messages": system_and_messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "parallel_tool_calls": metadata.parallel_tool_calls.unwrap_or(true),
        });

        if let Some(max_tokens) = max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        if let Some(tools) = convert_tools_for_openai(tools) {
            body["tools"] = serde_json::json!(tools);
        }

        if let Some(ref tool_choice) = metadata.tool_choice {
            body["tool_choice"] = tool_choice.clone();
        }

        // Add reasoning_effort if configured.
        // Source: `src/api/providers/openai.ts` — `reasoning_effort` in request body
        if let Some(ref effort) = self.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        Ok(body)
    }

    /// Create a streaming response from the API.
    async fn create_stream(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&Vec<serde_json::Value>>,
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
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
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
            .map(move |event| {
                match event {
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
                    Err(e) => Some(Err(ProviderError::StreamError(format!(
                        "SSE error: {e}"
                    )))),
                }
            })
            .filter_map(|item| async move { item })
            .map_err(move |e| {
                ProviderError::StreamError(format!("{provider_name}: {e}"))
            });

        Ok(Box::pin(stream))
    }

    /// Create a message from a pre-built request body.
    ///
    /// This allows providers that need custom message formatting (e.g. DeepSeek
    /// with R1 format) to build their own request body while still reusing the
    /// HTTP client and SSE stream parsing infrastructure.
    pub async fn create_message_from_body(
        &self,
        body: serde_json::Value,
    ) -> Result<ApiStream> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
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
            .map(move |event| {
                match event {
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
                    Err(e) => Some(Err(ProviderError::StreamError(format!(
                        "SSE error: {e}"
                    )))),
                }
            })
            .filter_map(|item| async move { item })
            .map_err(move |e| {
                ProviderError::StreamError(format!("{provider_name}: {e}"))
            });

        let (_, model_info) = self.base.get_model();

        // Process the stream into ApiStreamChunks
        let mut active_tool_call_ids: HashSet<String> = HashSet::new();
        let model_info = model_info.clone();

        let processed = stream.flat_map(move |chunk_result| {
            let results: Vec<Result<ApiStreamChunk>> = match chunk_result {
                Ok(chunk) => {
                    let delta = chunk.choices.as_ref().and_then(|c| c.first()).and_then(|c| c.delta.as_ref());
                    let finish_reason = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.finish_reason.as_ref())
                        .cloned();

                    let mut results: Vec<Result<ApiStreamChunk>> = Vec::new();

                    // Handle content
                    if let Some(delta) = delta {
                        if let Some(ref content) = delta.content {
                            results.push(Ok(ApiStreamChunk::Text {
                                text: content.clone(),
                            }));
                        }

                        // Handle reasoning_details (OpenRouter format for Gemini 3, Claude, etc.)
                        // Priority: reasoning_details > reasoning_content > reasoning
                        // If reasoning_details has displayable content, skip top-level reasoning
                        // to avoid duplicate display (matches TS behavior).
                        let mut has_reasoning_from_details = false;
                        if let Some(ref details) = delta.reasoning_details {
                            for detail in details {
                                let reasoning_text = match detail.detail_type.as_deref() {
                                    Some("reasoning.text") => detail.text.as_deref(),
                                    Some("reasoning.summary") => detail.summary.as_deref(),
                                    _ => None, // Skip reasoning.encrypted and other types
                                };
                                if let Some(text) = reasoning_text {
                                    if !text.is_empty() {
                                        has_reasoning_from_details = true;
                                        results.push(Ok(ApiStreamChunk::Reasoning {
                                            text: text.to_string(),
                                            signature: None,
                                        }));
                                    }
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
                            } else if let Some(ref reasoning) = delta.reasoning {
                                if !reasoning.trim().is_empty() {
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: reasoning.clone(),
                                        signature: None,
                                    }));
                                }
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
                                    name: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.name.clone()),
                                    arguments: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.clone()),
                                }));
                            }
                        }
                    }

                    // Emit tool_call_end events when finish_reason is "tool_calls"
                    if finish_reason.as_deref() == Some("tool_calls") && !active_tool_call_ids.is_empty() {
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
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        let stream = self
            .create_stream(system_prompt, &messages, tools.as_ref(), &metadata)
            .await?;

        let (_, model_info) = self.base.get_model();

        // Process the stream into ApiStreamChunks
        let mut active_tool_call_ids: HashSet<String> = HashSet::new();
        let model_info = model_info.clone();

        let processed = stream.flat_map(move |chunk_result| {
            let results: Vec<Result<ApiStreamChunk>> = match chunk_result {
                Ok(chunk) => {
                    let delta = chunk.choices.as_ref().and_then(|c| c.first()).and_then(|c| c.delta.as_ref());
                    let finish_reason = chunk
                        .choices
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.finish_reason.as_ref())
                        .cloned();

                    let mut results: Vec<Result<ApiStreamChunk>> = Vec::new();

                    // Handle content
                    if let Some(delta) = delta {
                        if let Some(ref content) = delta.content {
                            results.push(Ok(ApiStreamChunk::Text {
                                text: content.clone(),
                            }));
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
                                if let Some(text) = reasoning_text {
                                    if !text.is_empty() {
                                        has_reasoning_from_details = true;
                                        results.push(Ok(ApiStreamChunk::Reasoning {
                                            text: text.to_string(),
                                            signature: None,
                                        }));
                                    }
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
                            } else if let Some(ref reasoning) = delta.reasoning {
                                if !reasoning.trim().is_empty() {
                                    results.push(Ok(ApiStreamChunk::Reasoning {
                                        text: reasoning.clone(),
                                        signature: None,
                                    }));
                                }
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
                                    name: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.name.clone()),
                                    arguments: tool_call
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.clone()),
                                }));
                            }
                        }
                    }

                    // Emit tool_call_end events when finish_reason is "tool_calls"
                    if finish_reason.as_deref() == Some("tool_calls") && !active_tool_call_ids.is_empty() {
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
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&self.provider_name_str, e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response(
                &self.provider_name_str,
                status,
                text,
            ));
        }

        let resp: serde_json::Value = response
            .json()
            .await
            .map_err(ProviderError::Reqwest)?;

        Ok(resp["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    fn provider_name(&self) -> ProviderName {
        self.base.provider_name_value
    }
}
