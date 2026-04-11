use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use rc_config::ProviderConfig;
use rc_core::{ConversationEntry, ProviderProtocol, ProviderResponse, ToolCall, UsageSummary};
use rc_tools::builtin_tool_specs;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;

use crate::{ProviderClient, build_headers, to_anthropic_messages, to_openai_messages};

// ---------------------------------------------------------------------------
// Streaming callbacks
// ---------------------------------------------------------------------------

/// Type alias for a single-argument streaming callback.
type TextCallback = Box<dyn Fn(&str) + Send>;

/// Type alias for a two-argument streaming callback (id, name/delta).
type PairCallback = Box<dyn Fn(&str, &str) + Send>;

/// Type alias for a usage callback (input tokens, output tokens).
type UsageCallback = Box<dyn Fn(u64, u64) + Send>;

/// Optional callbacks for observing streaming events in real time.
///
/// All callback fields are `Option<...>` so callers can subscribe to only the
/// events they care about.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct StreamingCallbacks {
    /// Fired for every text delta received from the provider.
    pub on_text_delta: Option<TextCallback>,
    /// Fired when a tool call starts (id and name are available).
    pub on_tool_call_start: Option<PairCallback>,
    /// Fired for every incremental tool-call input delta.
    pub on_tool_call_delta: Option<PairCallback>,
    /// Fired when usage information becomes available (input, output tokens).
    pub on_usage: Option<UsageCallback>,
}

// ---------------------------------------------------------------------------
// ProviderClient streaming implementation
// ---------------------------------------------------------------------------

impl ProviderClient {
    /// # Errors
    /// Returns an error if the provider API request fails.
    pub async fn complete_streaming(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        self.complete_streaming_with_callbacks(provider, conversation, None)
            .await
    }

    /// Streaming completion with optional real-time callbacks.
    ///
    /// # Errors
    /// Returns an error if the provider API request fails.
    pub async fn complete_streaming_with_callbacks(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<StreamingCallbacks>,
    ) -> Result<ProviderResponse> {
        if provider.name == "mock"
            || provider.api_key.as_deref() == Some("mock")
            || provider.base_url.as_deref() == Some("mock://provider")
        {
            return Ok(crate::mock_response(conversation));
        }

        match provider.protocol {
            ProviderProtocol::OpenAi | ProviderProtocol::Bedrock | ProviderProtocol::Vertex => {
                self.complete_streaming_openai(provider, conversation, callbacks.as_ref())
                    .await
            }
            ProviderProtocol::Anthropic => {
                self.complete_streaming_anthropic(provider, conversation, callbacks.as_ref())
                    .await
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn complete_streaming_openai(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<&StreamingCallbacks>,
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
            "stream": true,
        });
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;

        let response = self
            .send_streaming_request(provider, base_url, &body, "openai-compatible")
            .await?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls_map: HashMap<usize, OpenAiToolCallAccumulator> = HashMap::new();
        let mut finish_reason = "stop".to_owned();
        let mut usage = UsageSummary::default();

        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.with_context(|| "failed to read streaming chunk")?;
            sse_buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(event_end) = sse_buffer.find("\n\n") {
                let event_text = sse_buffer[..event_end].to_owned();
                sse_buffer = sse_buffer[event_end + 2..].to_owned();

                for line in event_text.lines() {
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    if let Some(choice) = parsed
                        .get("choices")
                        .and_then(Value::as_array)
                        .and_then(|choices| choices.first())
                    {
                        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str)
                            && reason != "null"
                        {
                            reason.clone_into(&mut finish_reason);
                        }

                        let delta = choice.get("delta");

                        if let Some(content) =
                            delta.and_then(|d| d.get("content")).and_then(Value::as_str)
                        {
                            // Fire on_text_delta callback.
                            if let Some(cb) = callbacks
                                .as_ref()
                                .and_then(|c| c.on_text_delta.as_ref())
                            {
                                cb(content);
                            }
                            text_parts.push(content.to_owned());
                        }

                        if let Some(tc_deltas) = delta
                            .and_then(|d| d.get("tool_calls"))
                            .and_then(Value::as_array)
                        {
                            for tc_delta in tc_deltas {
                                #[allow(clippy::cast_possible_truncation)]
                                let index =
                                    tc_delta.get("index").and_then(Value::as_u64).unwrap_or(0)
                                        as usize;
                                let accumulator = tool_calls_map.entry(index).or_default();
                                if let Some(id) = tc_delta.get("id").and_then(Value::as_str) {
                                    accumulator.id = Some(id.to_owned());
                                }
                                if let Some(func) = tc_delta.get("function") {
                                    if let Some(name) = func.get("name").and_then(Value::as_str) {
                                        accumulator.name = Some(name.to_owned());
                                        // Fire on_tool_call_start when we first see the name.
                                        if let Some(cb) = callbacks
                                            .as_ref()
                                            .and_then(|c| c.on_tool_call_start.as_ref())
                                            && let Some(ref id) = accumulator.id
                                        {
                                            cb(id, name);
                                        }
                                    }
                                    if let Some(args) =
                                        func.get("arguments").and_then(Value::as_str)
                                    {
                                        // Fire on_tool_call_delta for incremental input.
                                        if let Some(cb) = callbacks
                                            .as_ref()
                                            .and_then(|c| c.on_tool_call_delta.as_ref())
                                            && let Some(ref id) = accumulator.id
                                        {
                                            cb(id, args);
                                        }
                                        accumulator.arguments.push_str(args);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(u) = parsed.get("usage") {
                        let inp = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
                        let out = u
                            .get("completion_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        usage.input_tokens = inp;
                        usage.output_tokens = out;
                        // Fire on_usage callback.
                        if let Some(cb) =
                            callbacks.as_ref().and_then(|c| c.on_usage.as_ref())
                        {
                            cb(inp, out);
                        }
                    }
                }
            }
        }

        let raw_text = text_parts.join("");
        let tool_calls = tool_calls_map
            .into_iter()
            .filter_map(|(_, acc)| {
                let id = acc.id?;
                let name = acc.name?;
                let input = serde_json::from_str(&acc.arguments)
                    .ok()
                    .unwrap_or_else(|| json!({}));
                Some(ToolCall { id, name, input })
            })
            .collect::<Vec<_>>();

        Ok(ProviderResponse {
            text: crate::strip_reasoning_tags(&raw_text),
            history_text: Some(raw_text),
            content_blocks: Vec::new(),
            tool_calls,
            usage,
            stop_reason: finish_reason,
        })
    }

    #[allow(clippy::too_many_lines)]
    async fn complete_streaming_anthropic(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<&StreamingCallbacks>,
    ) -> Result<ProviderResponse> {
        let (system, messages) = to_anthropic_messages(conversation);
        let body = json!({
            "model": provider.model,
            "system": system,
            "messages": messages,
            "tools": builtin_tool_specs()
                .into_iter()
                .map(|tool| tool.to_anthropic_schema())
                .collect::<Vec<_>>(),
            "max_tokens": provider.max_output_tokens,
            "stream": true,
        });
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;

        let response = self
            .send_streaming_request(provider, base_url, &body, "anthropic-compatible")
            .await?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut content_blocks: Vec<Value> = Vec::new();
        let mut tool_use_accumulators: HashMap<usize, AnthropicToolUseAccumulator> = HashMap::new();
        let mut current_text_block_index: Option<usize> = None;
        let mut usage = UsageSummary::default();
        let mut stop_reason = "end_turn".to_owned();

        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.with_context(|| "failed to read streaming chunk")?;
            sse_buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(event_end) = sse_buffer.find("\n\n") {
                let event_text = sse_buffer[..event_end].to_owned();
                sse_buffer = sse_buffer[event_end + 2..].to_owned();

                for line in event_text.lines() {
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" {
                        continue;
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");

                    match event_type {
                        "message_start" => {
                            if let Some(msg) = event.get("message")
                                && let Some(u) = msg.get("usage")
                            {
                                let inp =
                                    u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                usage.input_tokens = inp;
                                if let Some(cb) =
                                    callbacks.as_ref().and_then(|c| c.on_usage.as_ref())
                                {
                                    cb(inp, 0);
                                }
                            }
                        }
                        "content_block_start" => {
                            #[allow(clippy::cast_possible_truncation)]
                            let index =
                                event.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                            let content_block = event.get("content_block");
                            let block_type = content_block
                                .and_then(|b| b.get("type"))
                                .and_then(Value::as_str)
                                .unwrap_or("");

                            match block_type {
                                "text" => {
                                    current_text_block_index = Some(index);
                                }
                                "tool_use" => {
                                    let id = content_block
                                        .and_then(|b| b.get("id"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
                                    let name = content_block
                                        .and_then(|b| b.get("name"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
                                    // Fire on_tool_call_start callback.
                                    if let Some(cb) =
                                        callbacks.as_ref().and_then(|c| c.on_tool_call_start.as_ref())
                                    {
                                        cb(&id, &name);
                                    }
                                    tool_use_accumulators.insert(
                                        index,
                                        AnthropicToolUseAccumulator {
                                            id,
                                            name,
                                            partial_json: String::new(),
                                        },
                                    );
                                }
                                _ => {}
                            }
                        }
                        "content_block_delta" => {
                            #[allow(clippy::cast_possible_truncation)]
                            let index =
                                event.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                            let delta = event.get("delta");
                            let delta_type = delta.and_then(Value::as_str).unwrap_or_else(|| {
                                delta
                                    .and_then(|d| d.get("type"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                            });

                            if (delta_type == "text_delta"
                                || current_text_block_index == Some(index))
                                && let Some(text) =
                                    delta.and_then(|d| d.get("text")).and_then(Value::as_str)
                            {
                                // Fire on_text_delta callback.
                                if let Some(cb) =
                                    callbacks.as_ref().and_then(|c| c.on_text_delta.as_ref())
                                {
                                    cb(text);
                                }
                                text_parts.push(text.to_owned());
                            }

                            if (delta_type == "input_json_delta"
                                || tool_use_accumulators.contains_key(&index))
                                && let Some(partial) = delta
                                    .and_then(|d| d.get("partial_json"))
                                    .and_then(Value::as_str)
                                && let Some(acc) = tool_use_accumulators.get_mut(&index)
                            {
                                // Fire on_tool_call_delta callback.
                                if let Some(cb) =
                                    callbacks.as_ref().and_then(|c| c.on_tool_call_delta.as_ref())
                                {
                                    cb(&acc.id, partial);
                                }
                                acc.partial_json.push_str(partial);
                            }
                        }
                        "content_block_stop" => {
                            #[allow(clippy::cast_possible_truncation)]
                            let index =
                                event.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                            if current_text_block_index == Some(index) {
                                current_text_block_index = None;
                            }
                        }
                        "message_delta" => {
                            if let Some(delta) = event.get("delta")
                                && let Some(reason) =
                                    delta.get("stop_reason").and_then(Value::as_str)
                            {
                                reason.clone_into(&mut stop_reason);
                            }
                            if let Some(u) = event.get("usage") {
                                let out =
                                    u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
                                usage.output_tokens = out;
                                // Fire on_usage callback with final output token count.
                                if let Some(cb) =
                                    callbacks.as_ref().and_then(|c| c.on_usage.as_ref())
                                {
                                    cb(usage.input_tokens, out);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let raw_text = text_parts.join("");
        let tool_calls: Vec<ToolCall> = tool_use_accumulators
            .into_iter()
            .filter_map(|(_, acc)| {
                if acc.id.is_empty() || acc.name.is_empty() {
                    return None;
                }
                let input = if acc.partial_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&acc.partial_json)
                        .ok()
                        .unwrap_or_else(|| json!({}))
                };
                Some(ToolCall {
                    id: acc.id,
                    name: acc.name,
                    input,
                })
            })
            .collect();

        if !tool_calls.is_empty() {
            for tc in &tool_calls {
                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.input,
                }));
            }
        }

        Ok(ProviderResponse {
            text: crate::strip_reasoning_tags(&raw_text),
            history_text: Some(raw_text),
            content_blocks,
            tool_calls,
            usage,
            stop_reason,
        })
    }

    async fn send_streaming_request(
        &self,
        provider: &ProviderConfig,
        base_url: &str,
        body: &Value,
        label: &str,
    ) -> Result<reqwest::Response> {
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
                    if status >= 400
                        && is_retryable_http_status(status)
                        && attempt < provider.max_retries
                    {
                        let retry_after = parse_retry_after(response.headers(), provider);
                        tokio::time::sleep(compute_retry_delay(provider, attempt, retry_after))
                            .await;
                        attempt += 1;
                        continue;
                    }
                    if status >= 400 {
                        let status_code = response.status().as_u16();
                        let text = response.text().await.with_context(|| {
                            format!("failed to read {label} error response body")
                        })?;
                        let error_message = serde_json::from_str::<Value>(&text)
                            .ok()
                            .and_then(|v| {
                                v.get("error")
                                    .and_then(|e| e.get("message"))
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                            })
                            .or_else(|| {
                                serde_json::from_str::<Value>(&text).ok().and_then(|v| {
                                    v.get("message").and_then(Value::as_str).map(str::to_owned)
                                })
                            })
                            .unwrap_or_else(|| "provider error".to_owned());
                        return Err(anyhow!(
                            "provider request failed ({status_code}): {error_message}"
                        ));
                    }
                    return Ok(response);
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

fn parse_retry_after(
    headers: &reqwest::header::HeaderMap,
    provider: &ProviderConfig,
) -> Option<Duration> {
    if !provider.respect_retry_after {
        return None;
    }
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[derive(Default)]
struct OpenAiToolCallAccumulator {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

struct AnthropicToolUseAccumulator {
    id: String,
    name: String,
    partial_json: String,
}
