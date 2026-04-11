//! LLM provider client with retry logic and message formatting.
//!
//! Supports OpenAI, Anthropic, Amazon Bedrock, and Google Vertex AI protocols.
//! Handles message conversion, response parsing, exponential back-off retries,
//! and mock-mode responses for testing.
//!
//! # Error classification
//!
//! The [`ProviderError`] enum provides structured error classification matching
//! upstream Claude Code's `categorizeRetryableAPIError`. Each variant carries
//! enough context for the caller to decide whether to retry, compact, or abort.

pub mod context;
pub mod cost;
pub mod failover;
pub mod model_info;
pub mod streaming;

pub use streaming::StreamingCallbacks;

use anyhow::{Context, Result, anyhow};
use rc_config::ProviderConfig;
use rc_core::{
    ConversationEntry, ConversationRole, ProviderProtocol, ProviderResponse, ToolCall, UsageSummary,
};
use rc_tools::builtin_tool_specs;
use reqwest::Client;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, RETRY_AFTER,
    USER_AGENT,
};
use serde_json::{Value, json};
use std::time::Duration;

/// HTTP client for communicating with LLM provider APIs.
#[derive(Debug, Clone)]
pub struct ProviderClient {
    http: Client,
}

impl ProviderClient {
    /// Create a new provider client.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP client cannot be constructed.
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .build()
            .context("failed to build the provider HTTP client")?;
        Ok(Self { http })
    }

    /// Send a completion request to the configured provider.
    ///
    /// Automatically selects the correct protocol (OpenAI / Anthropic) based on
    /// the provider configuration and retries on transient failures.
    ///
    /// # Errors
    /// Returns an error if the API request fails after all retries are exhausted.
    pub async fn complete(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        if provider.name == "mock"
            || provider.api_key.as_deref() == Some("mock")
            || provider.base_url.as_deref() == Some("mock://provider")
        {
            return Ok(mock_response(conversation));
        }

        match provider.protocol {
            ProviderProtocol::OpenAi => self.complete_openai(provider, conversation).await,
            ProviderProtocol::Anthropic => self.complete_anthropic(provider, conversation).await,
            ProviderProtocol::Bedrock => {
                // Placeholder: Bedrock uses SigV4 auth which requires AWS SDK.
                // Fall back to OpenAI-compatible endpoint format for now.
                self.complete_openai(provider, conversation).await
            }
            ProviderProtocol::Vertex => {
                // Placeholder: Vertex AI uses Google OAuth2 which requires gcloud auth.
                // Fall back to OpenAI-compatible endpoint format for now.
                self.complete_openai(provider, conversation).await
            }
        }
    }

    async fn complete_openai(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let body = json!({
            "model": provider.model,
            "messages": to_openai_messages(conversation),
            "tools": builtin_tool_specs()
                .into_iter()
                .map(|tool| tool.to_openai_schema())
                .collect::<Vec<_>>(),
            "tool_choice": "auto",
            "temperature": 0.1,
            "max_tokens": provider.max_output_tokens,
            "stream": false,
        });
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;
        let response = self
            .send_json_request(provider, base_url, &body, "openai-compatible")
            .await?;
        parse_openai_response(response.0, response.1)
    }

    async fn complete_anthropic(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let (system, messages) = to_anthropic_messages(conversation);
        let mut body = json!({
            "model": provider.model,
            "system": system,
            "messages": messages,
            "tools": builtin_tool_specs()
                .into_iter()
                .map(|tool| tool.to_anthropic_schema())
                .collect::<Vec<_>>(),
            "max_tokens": provider.max_output_tokens,
            "stream": false,
        });
        // Detect resume: if there are tool-role entries, this is a continued conversation.
        let is_resume = conversation
            .iter()
            .any(|entry| matches!(entry.role, ConversationRole::Tool));
        add_stable_cache_control(&mut body, is_resume);
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;
        let response = self
            .send_json_request(provider, base_url, &body, "anthropic-compatible")
            .await?;
        parse_anthropic_response(response.0, response.1)
    }

    async fn send_json_request(
        &self,
        provider: &ProviderConfig,
        base_url: &str,
        body: &Value,
        label: &str,
    ) -> Result<(u16, String)> {
        let mut attempt = 0u32;
        loop {
            let response = self
                .http
                .post(base_url)
                .headers(build_headers(provider)?)
                .timeout(Duration::from_millis(provider.timeout_ms))
                .json(body)
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let retry_after = parse_retry_after(response.headers(), provider);
                    let text = response
                        .text()
                        .await
                        .with_context(|| format!("failed to read {label} response body"))?;
                    if is_retryable_http_status(status) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, retry_after))
                            .await;
                        attempt += 1;
                        continue;
                    }
                    return Ok((status, text));
                }
                Err(error) => {
                    if is_retryable_transport_error(&error) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(error).with_context(|| format!("{label} request failed"));
                }
            }
        }
    }
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn compute_retry_delay(
    provider: &ProviderConfig,
    attempt: u32,
    retry_after: Option<Duration>,
) -> Duration {
    if let Some(retry_after) = retry_after {
        return retry_after;
    }
    let multiplier = 2u64.saturating_pow(attempt.min(16));
    let delay_ms = provider
        .retry_initial_backoff_ms
        .saturating_mul(multiplier)
        .min(provider.retry_max_backoff_ms);
    Duration::from_millis(delay_ms.max(1))
}

fn parse_retry_after(headers: &HeaderMap, provider: &ProviderConfig) -> Option<Duration> {
    if !provider.respect_retry_after {
        return None;
    }
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn build_headers(provider: &ProviderConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("remote-code-rust/{}", env!("CARGO_PKG_VERSION")))?,
    );
    if let Some(api_key) = &provider.api_key {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))?,
        );
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(api_key)?,
        );
    }
    if matches!(provider.protocol, ProviderProtocol::Anthropic) {
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
    }
    for (name, value) in &provider.request_header_overrides {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .with_context(|| format!("invalid header name {name}"))?;
        let header_value =
            HeaderValue::from_str(value).with_context(|| format!("invalid header {name}"))?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn to_openai_messages(conversation: &[ConversationEntry]) -> Vec<Value> {
    conversation
        .iter()
        .map(|entry| match entry.role {
            ConversationRole::System | ConversationRole::User => json!({
                "role": role_name(&entry.role),
                "content": entry.history_text(),
            }),
            ConversationRole::Assistant => {
                let mut message = json!({
                    "role": "assistant",
                    "content": entry.history_text(),
                });
                if !entry.tool_calls.is_empty() {
                    message["tool_calls"] = Value::Array(
                        entry
                            .tool_calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "id": call.id,
                                    "type": "function",
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.input.to_string(),
                                    }
                                })
                            })
                            .collect(),
                    );
                }
                message
            }
            ConversationRole::Tool => json!({
                "role": "tool",
                "tool_call_id": entry.tool_call_id,
                "content": entry.text,
            }),
        })
        .collect()
}

fn to_anthropic_messages(conversation: &[ConversationEntry]) -> (String, Vec<Value>) {
    let system = conversation
        .iter()
        .filter(|entry| matches!(entry.role, ConversationRole::System))
        .map(ConversationEntry::history_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    let messages = conversation
        .iter()
        .filter(|entry| !matches!(entry.role, ConversationRole::System))
        .map(|entry| match entry.role {
            ConversationRole::User => json!({
                "role": "user",
                "content": [{"type": "text", "text": entry.history_text()}],
            }),
            ConversationRole::Assistant => {
                if entry.content_blocks.is_empty() {
                    let mut blocks = Vec::new();
                    if !entry.history_text().is_empty() {
                        blocks.push(json!({"type": "text", "text": entry.history_text()}));
                    }
                    for call in &entry.tool_calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call.id,
                            "name": call.name,
                            "input": call.input,
                        }));
                    }
                    json!({
                        "role": "assistant",
                        "content": blocks,
                    })
                } else {
                    json!({
                        "role": "assistant",
                        "content": entry.content_blocks,
                    })
                }
            }
            ConversationRole::Tool => json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": entry.tool_call_id,
                    "content": entry.text,
                    "is_error": entry.is_error,
                }],
            }),
            ConversationRole::System => Value::Null,
        })
        .filter(|value| !value.is_null())
        .collect();
    (system, messages)
}

fn parse_openai_response(status: u16, raw_text: String) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(&raw_text)
        .with_context(|| format!("provider returned non-JSON output: {}", truncate(&raw_text)))?;
    if status >= 400 {
        let error_message = payload
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("provider error");
        return Err(anyhow!(
            "provider request failed ({status}): {error_message}"
        ));
    }

    let choice = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| anyhow!("provider response did not include choices[0].message"))?;

    let tool_calls = choice
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|tool_calls| {
            tool_calls
                .iter()
                .filter_map(parse_openai_tool_call)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let raw_assistant_text = coerce_text_content(choice.get("content")).trim().to_owned();
    let usage = payload.get("usage").cloned().unwrap_or_default();

    Ok(ProviderResponse {
        text: strip_reasoning_tags(&raw_assistant_text),
        history_text: Some(raw_assistant_text),
        content_blocks: Vec::new(),
        tool_calls,
        usage: UsageSummary {
            input_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            output_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        },
        stop_reason: payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .unwrap_or("stop")
            .to_owned(),
    })
}

fn parse_anthropic_response(status: u16, raw_text: String) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(&raw_text)
        .with_context(|| format!("provider returned non-JSON output: {}", truncate(&raw_text)))?;
    if status >= 400 {
        let error_message = payload
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("provider error");
        return Err(anyhow!(
            "provider request failed ({status}): {error_message}"
        ));
    }
    let blocks = payload
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let text = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let tool_calls = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(parse_anthropic_tool_call)
        .collect::<Vec<_>>();
    let usage = payload.get("usage").cloned().unwrap_or_default();

    Ok(ProviderResponse {
        text: strip_reasoning_tags(&text),
        history_text: Some(text),
        content_blocks: blocks,
        tool_calls,
        usage: UsageSummary {
            input_tokens: usage
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            output_tokens: usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        },
        stop_reason: payload
            .get("stop_reason")
            .and_then(Value::as_str)
            .unwrap_or("stop")
            .to_owned(),
    })
}

fn parse_openai_tool_call(value: &Value) -> Option<ToolCall> {
    let function = value.get("function")?;
    let id = value.get("id")?.as_str()?.to_owned();
    let name = function.get("name")?.as_str()?.to_owned();
    let input = function
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| json!({}));
    Some(ToolCall { id, name, input })
}

fn parse_anthropic_tool_call(value: &Value) -> Option<ToolCall> {
    let id = value.get("id")?.as_str()?.to_owned();
    let name = value.get("name")?.as_str()?.to_owned();
    let input = value.get("input").cloned().unwrap_or_else(|| json!({}));
    Some(ToolCall { id, name, input })
}

fn coerce_text_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                if let Some(text) = item.as_str() {
                    return Some(text.to_owned());
                }
                item.get("text")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn role_name(role: &ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    }
}

fn mock_response(conversation: &[ConversationEntry]) -> ProviderResponse {
    let user_prompt = conversation
        .iter()
        .rev()
        .find(|entry| matches!(entry.role, ConversationRole::User))
        .map_or_else(
            || "No prompt supplied.".to_owned(),
            ConversationEntry::history_text,
        );
    let has_tool_result_after_latest_user = conversation
        .iter()
        .rev()
        .take_while(|entry| !matches!(entry.role, ConversationRole::User))
        .any(|entry| matches!(entry.role, ConversationRole::Tool));
    ProviderResponse {
        text: if has_tool_result_after_latest_user {
            "mock provider observed the tool result and is ready to finish.".to_owned()
        } else {
            format!("mock provider response: {}", truncate(&user_prompt))
        },
        history_text: Some(user_prompt.clone()),
        content_blocks: Vec::new(),
        tool_calls: if !has_tool_result_after_latest_user
            && user_prompt.to_ascii_lowercase().contains("list files")
        {
            vec![ToolCall {
                id: "mock-tool-call-1".to_owned(),
                name: builtin_tool_specs()
                    .first()
                    .map_or_else(|| "list_directory".to_owned(), |tool| tool.name.clone()),
                input: json!({"path": ".", "recursive": false, "max_entries": 32}),
            }]
        } else {
            Vec::new()
        },
        usage: UsageSummary {
            input_tokens: 16,
            output_tokens: 12,
        },
        stop_reason: "end_turn".to_owned(),
    }
}

fn truncate(value: &str) -> String {
    value.chars().take(240).collect()
}

fn strip_reasoning_tags(text: &str) -> String {
    let mut remaining = text.to_owned();
    loop {
        let Some(start) = remaining.find("<think>") else {
            break;
        };
        let Some(end) = remaining[start..].find("</think>") else {
            break;
        };
        let end = start + end + "</think>".len();
        remaining.replace_range(start..end, "");
    }
    remaining.trim().to_owned()
}

/// Add stable Anthropic prompt caching markers (`cache_control: {"type": "ephemeral"}`)
/// to strategic locations in the request body so that the system prompt,
/// tool definitions, and the most recent user message are cached server-side.
///
/// When `is_resume` is true (conversation has prior tool results), the tool list
/// is kept exactly as-is to avoid `deferred_tools_delta` cache-miss issues.
fn add_stable_cache_control(body: &mut Value, is_resume: bool) {
    // 1. System message — always ensure array format with cache_control.
    if let Some(system) = body.get_mut("system") {
        if system.is_string() {
            let text = system.as_str().unwrap_or("").to_owned();
            *system = json!([{
                "type": "text",
                "text": text,
                "cache_control": {"type": "ephemeral"}
            }]);
        } else if let Some(system_arr) = system.as_array_mut()
            && let Some(last) = system_arr.last_mut()
        {
            last["cache_control"] = json!({"type": "ephemeral"});
        }
    }

    // 2. Tools — mark the last tool with cache_control.
    //    In resume scenarios, do NOT add or remove any tools to keep the list stable.
    if let Some(tools) = body.get_mut("tools")
        && let Some(tools_arr) = tools.as_array_mut()
        && let Some(last_tool) = tools_arr.last_mut()
    {
        last_tool["cache_control"] = json!({"type": "ephemeral"});
    }

    // 3. Most recent user message — ensure content is array format, mark cache_control.
    if let Some(messages) = body.get_mut("messages")
        && let Some(msg_arr) = messages.as_array_mut()
    {
        for msg in msg_arr.iter_mut().rev() {
            if msg["role"] == "user" {
                if let Some(content) = msg.get_mut("content") {
                    if content.is_string() {
                        let text = content.as_str().unwrap_or("").to_owned();
                        *content = json!([{
                            "type": "text",
                            "text": text,
                            "cache_control": {"type": "ephemeral"}
                        }]);
                    } else if let Some(content_arr) = content.as_array_mut()
                        && let Some(last_block) = content_arr.last_mut()
                    {
                        last_block["cache_control"] = json!({"type": "ephemeral"});
                    }
                }
                break;
            }
        }
    }

    // 4. Resume scenario: tool list must remain identical to avoid cache invalidation.
    //    The `is_resume` flag is recorded here for future use; currently the tool list
    //    is always the full builtin set, which is inherently stable.
    let _ = is_resume;
}

// ---------------------------------------------------------------------------
// Structured error classification
// ---------------------------------------------------------------------------

/// Structured provider error with classification for retry/recovery decisions.
///
/// Matches upstream Claude Code's `categorizeRetryableAPIError` logic.
#[derive(Debug, Clone)]
pub struct ProviderError {
    /// Error category.
    pub category: ErrorCategory,
    /// HTTP status code (if applicable).
    pub status_code: Option<u16>,
    /// Human-readable error message.
    pub message: String,
    /// Provider name that produced the error.
    pub provider_name: String,
    /// Suggested recovery action.
    pub recovery: RecoveryAction,
}

/// Classification of provider errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Rate limit exceeded (429).
    RateLimit,
    /// Authentication failure (401/403).
    Authentication,
    /// Request too large / prompt too long (400/413).
    PromptTooLong,
    /// Model not found or unavailable (404).
    ModelNotFound,
    /// Server error (5xx).
    ServerError,
    /// Network / connectivity error.
    Network,
    /// Timeout.
    Timeout,
    /// Streaming interrupted.
    StreamInterrupted,
    /// Invalid request format.
    InvalidRequest,
    /// Quota / billing exceeded (402).
    QuotaExceeded,
    /// Unknown / unclassified error.
    Unknown,
}

/// Suggested recovery action for a provider error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry with exponential backoff.
    Retry,
    /// Retry after compacting the conversation.
    CompactAndRetry,
    /// Failover to a different provider.
    Failover,
    /// Abort the operation.
    Abort,
    /// Ask the user to fix configuration.
    FixConfig,
}

/// Classify an HTTP status code and error message into a structured error.
#[must_use]
pub fn classify_provider_error(
    status_code: u16,
    message: &str,
    provider_name: &str,
) -> ProviderError {
    let (category, recovery) = match status_code {
        429 => (ErrorCategory::RateLimit, RecoveryAction::Retry),
        401 | 403 => (ErrorCategory::Authentication, RecoveryAction::FixConfig),
        402 => (ErrorCategory::QuotaExceeded, RecoveryAction::Failover),
        404 => (ErrorCategory::ModelNotFound, RecoveryAction::FixConfig),
        413 => (ErrorCategory::PromptTooLong, RecoveryAction::CompactAndRetry),
        400 => {
            // Check if it's a prompt-too-long error disguised as 400.
            if message.contains("prompt is too long")
                || message.contains("context_length_exceeded")
                || message.contains("maximum context length")
            {
                (ErrorCategory::PromptTooLong, RecoveryAction::CompactAndRetry)
            } else {
                (ErrorCategory::InvalidRequest, RecoveryAction::Abort)
            }
        }
        500 | 502 | 503 | 504 => (ErrorCategory::ServerError, RecoveryAction::Retry),
        _ => (ErrorCategory::Unknown, RecoveryAction::Retry),
    };

    ProviderError {
        category,
        status_code: Some(status_code),
        message: message.to_owned(),
        provider_name: provider_name.to_owned(),
        recovery,
    }
}

/// Classify a network/transport error.
#[must_use]
pub fn classify_network_error(error: &str, provider_name: &str) -> ProviderError {
    let (category, recovery) = if error.contains("timed out") || error.contains("timeout") {
        (ErrorCategory::Timeout, RecoveryAction::Retry)
    } else if error.contains("connection refused") || error.contains("couldn't connect") {
        (ErrorCategory::Network, RecoveryAction::Retry)
    } else if error.contains("tls") || error.contains("certificate") || error.contains("ssl") {
        (ErrorCategory::Network, RecoveryAction::FixConfig)
    } else if error.contains("dns") || error.contains("resolve") {
        (ErrorCategory::Network, RecoveryAction::FixConfig)
    } else {
        (ErrorCategory::Network, RecoveryAction::Retry)
    };

    ProviderError {
        category,
        status_code: None,
        message: error.to_owned(),
        provider_name: provider_name.to_owned(),
        recovery,
    }
}

/// Check if an error is retryable.
#[must_use]
pub fn is_retryable(error: &ProviderError) -> bool {
    matches!(
        error.recovery,
        RecoveryAction::Retry | RecoveryAction::CompactAndRetry | RecoveryAction::Failover
    )
}

/// Check if an error indicates the prompt is too long.
#[must_use]
pub fn is_prompt_too_long(error: &ProviderError) -> bool {
    error.category == ErrorCategory::PromptTooLong
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderClient, mock_response, parse_openai_response, strip_reasoning_tags,
        to_openai_messages,
    };
    use axum::{Json, Router, extract::State, routing::post};
    use rc_core::ConversationEntry;
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::net::TcpListener;

    fn test_provider_config(base_url: String) -> rc_config::ProviderConfig {
        rc_config::ProviderConfig {
            name: "custom".to_owned(),
            base_url: Some(base_url),
            api_key: Some("test-key".to_owned()),
            model: Some("test-model".to_owned()),
            protocol: rc_core::ProviderProtocol::OpenAi,
            timeout_ms: 1_000,
            max_output_tokens: 512,
            max_retries: 2,
            retry_initial_backoff_ms: 10,
            retry_max_backoff_ms: 20,
            respect_retry_after: false,
            request_header_overrides: Default::default(),
        }
    }

    #[test]
    fn reasoning_tags_are_removed() {
        assert_eq!(strip_reasoning_tags("<think>abc</think>done"), "done");
    }

    #[test]
    fn mock_provider_uses_latest_prompt() {
        let response = mock_response(&[ConversationEntry::user("hello world")]);
        assert!(response.text.contains("hello world"));
    }

    #[test]
    fn openai_messages_include_user_role() {
        let messages = to_openai_messages(&[ConversationEntry::user("ship it")]);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn openai_response_parser_handles_success() {
        let raw = r#"{"choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#;
        let parsed = parse_openai_response(200, raw.to_owned());
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap_or_else(|error| panic!("parse failed: {error}"));
        assert_eq!(parsed.text, "hello");
        assert_eq!(parsed.usage.output_tokens, 2);
    }

    #[tokio::test]
    async fn provider_retries_retryable_status_then_succeeds() {
        async fn handler(
            State(attempts): State<Arc<AtomicUsize>>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return (
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": {"message": "slow down"}})),
                );
            }
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "choices": [{"message": {"content": "retried ok"}}],
                    "usage": {"prompt_tokens": 3, "completion_tokens": 4}
                })),
            )
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server_attempts = Arc::clone(&attempts);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat/completions", post(handler))
                    .with_state(server_attempts),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let client = ProviderClient::new().unwrap_or_else(|error| panic!("client failed: {error}"));
        let response = client
            .complete(
                &test_provider_config(format!("http://{address}/chat/completions")),
                &[ConversationEntry::user("hello")],
            )
            .await
            .unwrap_or_else(|error| panic!("completion failed: {error}"));

        server.abort();
        assert_eq!(response.text, "retried ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn provider_does_not_retry_non_retryable_status() {
        async fn handler(
            State(attempts): State<Arc<AtomicUsize>>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            attempts.fetch_add(1, Ordering::SeqCst);
            (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"message": "bad api key"}})),
            )
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server_attempts = Arc::clone(&attempts);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat/completions", post(handler))
                    .with_state(server_attempts),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let client = ProviderClient::new().unwrap_or_else(|error| panic!("client failed: {error}"));
        let error = client
            .complete(
                &test_provider_config(format!("http://{address}/chat/completions")),
                &[ConversationEntry::user("hello")],
            )
            .await
            .expect_err("request should fail");

        server.abort();
        assert!(error.to_string().contains("401"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    // ── Error classification tests ────────────────────────────────────

    #[test]
    fn classify_429_as_rate_limit() {
        let err = super::classify_provider_error(429, "rate limited", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::RateLimit);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_401_as_authentication() {
        let err = super::classify_provider_error(401, "invalid api key", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Authentication);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
        assert!(!super::is_retryable(&err));
    }

    #[test]
    fn classify_400_prompt_too_long() {
        let err = super::classify_provider_error(
            400,
            "prompt is too long: maximum context length exceeded",
            "test-provider",
        );
        assert_eq!(err.category, super::ErrorCategory::PromptTooLong);
        assert_eq!(err.recovery, super::RecoveryAction::CompactAndRetry);
        assert!(super::is_prompt_too_long(&err));
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_400_context_length_exceeded() {
        let err = super::classify_provider_error(
            400,
            "context_length_exceeded",
            "test-provider",
        );
        assert_eq!(err.category, super::ErrorCategory::PromptTooLong);
        assert!(super::is_prompt_too_long(&err));
    }

    #[test]
    fn classify_500_as_server_error() {
        let err = super::classify_provider_error(500, "internal server error", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ServerError);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_503_as_server_error() {
        let err = super::classify_provider_error(503, "service unavailable", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ServerError);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_404_as_model_not_found() {
        let err = super::classify_provider_error(404, "model not found", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ModelNotFound);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
    }

    #[test]
    fn classify_402_as_quota_exceeded() {
        let err = super::classify_provider_error(402, "insufficient quota", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::QuotaExceeded);
        assert_eq!(err.recovery, super::RecoveryAction::Failover);
    }

    #[test]
    fn classify_network_timeout() {
        let err = super::classify_network_error("connection timed out", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Timeout);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_network_dns_error() {
        let err = super::classify_network_error("dns resolve failed", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Network);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
    }

    #[test]
    fn classify_400_generic_as_invalid_request() {
        let err = super::classify_provider_error(400, "invalid parameter", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::InvalidRequest);
        assert_eq!(err.recovery, super::RecoveryAction::Abort);
    }
}
