//! Streaming support for provider responses.
//!
//! Extends [`ProviderClient`] with
//! [`complete_streaming`](crate::ProviderClient::complete_streaming) which
//! processes server-sent events (SSE) from OpenAI- and Anthropic-compatible
//! APIs, invoking optional callbacks for text deltas, tool-call progress, and
//! usage telemetry.

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use rc_config::ProviderConfig;
use rc_core::{ConversationEntry, ProviderProtocol, ProviderResponse, ToolCall, UsageSummary};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use crate::{
    ProviderClient, build_anthropic_request_body, build_headers, build_openai_request_body,
    provider_for_request,
};

// ---------------------------------------------------------------------------
// Streaming callbacks
// ---------------------------------------------------------------------------

/// Type alias for a single-argument streaming callback.
type TextCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Type alias for a two-argument streaming callback (id, name/delta).
type PairCallback = Box<dyn Fn(&str, &str) + Send + Sync>;

/// Type alias for a usage callback (input tokens, output tokens).
type UsageCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

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
        self.complete_streaming_with_callbacks_and_discovered_tools(
            provider,
            conversation,
            None,
            &BTreeSet::new(),
            None,
        )
        .await
    }

    /// Streaming completion with optional real-time callbacks.
    ///
    /// If the streaming connection fails mid-request, automatically falls back
    /// to a non-streaming completion for resilience during long-running sessions.
    ///
    /// # Errors
    /// Returns an error if both streaming and non-streaming attempts fail.
    pub async fn complete_streaming_with_callbacks(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<StreamingCallbacks>,
    ) -> Result<ProviderResponse> {
        self.complete_streaming_with_callbacks_and_discovered_tools(
            provider,
            conversation,
            callbacks,
            &BTreeSet::new(),
            None,
        )
        .await
    }

    /// Streaming completion with carried deferred-tool discovery state.
    ///
    /// # Errors
    /// Returns an error if both streaming and non-streaming attempts fail.
    pub async fn complete_streaming_with_callbacks_and_discovered_tools(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<StreamingCallbacks>,
        carried_discovered_tools: &BTreeSet<String>,
        request_context: Option<&crate::query_source::ProviderRequestContext>,
    ) -> Result<ProviderResponse> {
        if provider.name == "mock"
            || provider.api_key.as_deref() == Some("mock")
            || provider.base_url.as_deref() == Some("mock://provider")
        {
            return Ok(crate::mock_response(conversation));
        }

        let streamed_tool_activity = Arc::new(AtomicBool::new(false));
        let tracked_callbacks =
            wrap_streaming_callbacks(callbacks, Arc::clone(&streamed_tool_activity));

        let result = match provider.protocol {
            ProviderProtocol::OpenAi => {
                self.complete_streaming_openai(
                    provider,
                    conversation,
                    Some(&tracked_callbacks),
                    carried_discovered_tools,
                    request_context,
                )
                .await
            }
            ProviderProtocol::Anthropic => {
                self.complete_streaming_anthropic(
                    provider,
                    conversation,
                    Some(&tracked_callbacks),
                    carried_discovered_tools,
                    request_context,
                )
                .await
            }
            // Native Bedrock/Vertex use non-streaming for now (SSE event-stream
            // parsing for Bedrock is not yet implemented).  If a base_url is set
            // (proxy mode) we fall back to OpenAI-compatible streaming.
            ProviderProtocol::Bedrock | ProviderProtocol::Vertex => {
                if provider.base_url.is_some() {
                    self.complete_streaming_openai(
                        provider,
                        conversation,
                        Some(&tracked_callbacks),
                        carried_discovered_tools,
                        request_context,
                    )
                    .await
                } else {
                    // Native mode — fall back to non-streaming completion.
                    self.complete_with_discovered_tools(
                        provider,
                        conversation,
                        carried_discovered_tools,
                        request_context,
                    )
                    .await
                }
            }
        };

        // If streaming failed, fall back to non-streaming completion.
        // This handles mid-stream disconnects, SSE parsing errors, and
        // other transient streaming failures common in long-running sessions.
        match result {
            Ok(response) => Ok(response),
            Err(streaming_error) => {
                if should_fallback_after_streaming_error(
                    &streaming_error,
                    streamed_tool_activity.load(Ordering::Relaxed),
                ) {
                    tracing::warn!(
                        "Streaming failed, falling back to non-streaming: {streaming_error:#}"
                    );
                    self.complete_with_discovered_tools(
                        provider,
                        conversation,
                        carried_discovered_tools,
                        request_context,
                    )
                    .await
                } else {
                    if streamed_tool_activity.load(Ordering::Relaxed) {
                        tracing::warn!(
                            "Streaming failed after tool activity; refusing non-streaming fallback: {streaming_error:#}"
                        );
                    }
                    Err(streaming_error)
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn complete_streaming_openai(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        callbacks: Option<&StreamingCallbacks>,
        carried_discovered_tools: &BTreeSet<String>,
        request_context: Option<&crate::query_source::ProviderRequestContext>,
    ) -> Result<ProviderResponse> {
        let effective_provider = provider_for_request(provider, request_context);
        let body = build_openai_request_body(
            &effective_provider,
            conversation,
            carried_discovered_tools,
            true,
        )
        .await;
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;

        let response = self
            .send_streaming_request(
                &effective_provider,
                base_url,
                &body,
                "openai-compatible",
                request_context,
            )
            .await?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls_map: HashMap<usize, OpenAiToolCallAccumulator> = HashMap::new();
        let mut finish_reason = "stop".to_owned();
        let mut usage = UsageSummary::default();
        let mut request_id: Option<String> = None;

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
                    if request_id.is_none() {
                        request_id = parsed
                            .get("id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                    }

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
                            if let Some(cb) =
                                callbacks.as_ref().and_then(|c| c.on_text_delta.as_ref())
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
                        if let Some(cb) = callbacks.as_ref().and_then(|c| c.on_usage.as_ref()) {
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
            thinking: None,
            content_blocks: Vec::new(),
            tool_calls,
            request_id,
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
        carried_discovered_tools: &BTreeSet<String>,
        request_context: Option<&crate::query_source::ProviderRequestContext>,
    ) -> Result<ProviderResponse> {
        let effective_provider = provider_for_request(provider, request_context);
        let body = build_anthropic_request_body(
            &effective_provider,
            conversation,
            carried_discovered_tools,
            request_context,
            true,
        )
        .await;
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;

        let response = self
            .send_streaming_request(
                &effective_provider,
                base_url,
                &body,
                "anthropic-compatible",
                request_context,
            )
            .await?;

        let mut content_block_accumulators: BTreeMap<usize, AnthropicContentAccumulator> =
            BTreeMap::new();
        let mut usage = UsageSummary::default();
        let mut stop_reason = "end_turn".to_owned();
        let mut request_id: Option<String> = None;

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
                            if let Some(msg) = event.get("message") {
                                if request_id.is_none() {
                                    request_id = msg
                                        .get("id")
                                        .and_then(Value::as_str)
                                        .map(ToOwned::to_owned);
                                }
                                if let Some(u) = msg.get("usage") {
                                    let inp =
                                        u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
                                    usage.input_tokens = inp;
                                    usage.cache_read_input_tokens = u
                                        .get("cache_read_input_tokens")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(0);
                                    usage.cache_creation_input_tokens = u
                                        .get("cache_creation_input_tokens")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(0);
                                    if let Some(cb) =
                                        callbacks.as_ref().and_then(|c| c.on_usage.as_ref())
                                    {
                                        cb(inp, 0);
                                    }
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
                                    let text = content_block
                                        .and_then(|b| b.get("text"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
                                    content_block_accumulators
                                        .insert(index, AnthropicContentAccumulator::Text { text });
                                }
                                "thinking" => {
                                    let thinking = content_block
                                        .and_then(|b| b.get("thinking"))
                                        .or_else(|| content_block.and_then(|b| b.get("text")))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned();
                                    let signature = content_block
                                        .and_then(|b| b.get("signature"))
                                        .and_then(Value::as_str)
                                        .map(ToOwned::to_owned);
                                    content_block_accumulators.insert(
                                        index,
                                        AnthropicContentAccumulator::Thinking {
                                            thinking,
                                            signature,
                                        },
                                    );
                                }
                                "tool_use" | "server_tool_use" => {
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
                                    if let Some(cb) = callbacks
                                        .as_ref()
                                        .and_then(|c| c.on_tool_call_start.as_ref())
                                    {
                                        cb(&id, &name);
                                    }
                                    content_block_accumulators.insert(
                                        index,
                                        AnthropicContentAccumulator::ToolUse(
                                            AnthropicToolUseAccumulator {
                                                block_type: block_type.to_owned(),
                                                id,
                                                name,
                                                partial_json: String::new(),
                                            },
                                        ),
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

                            if delta_type == "thinking_delta"
                                && let Some(thinking) = delta
                                    .and_then(|d| d.get("thinking"))
                                    .and_then(Value::as_str)
                                && let Some(AnthropicContentAccumulator::Thinking {
                                    thinking: existing,
                                    ..
                                }) = content_block_accumulators.get_mut(&index)
                            {
                                existing.push_str(thinking);
                            } else if delta_type == "signature_delta"
                                && let Some(signature) = delta
                                    .and_then(|d| d.get("signature"))
                                    .and_then(Value::as_str)
                                && let Some(AnthropicContentAccumulator::Thinking {
                                    signature: existing,
                                    ..
                                }) = content_block_accumulators.get_mut(&index)
                            {
                                *existing = Some(signature.to_owned());
                            } else if delta_type == "text_delta"
                                && let Some(text) =
                                    delta.and_then(|d| d.get("text")).and_then(Value::as_str)
                                && let Some(AnthropicContentAccumulator::Text { text: existing }) =
                                    content_block_accumulators.get_mut(&index)
                            {
                                // Fire on_text_delta callback.
                                if let Some(cb) =
                                    callbacks.as_ref().and_then(|c| c.on_text_delta.as_ref())
                                {
                                    cb(text);
                                }
                                existing.push_str(text);
                            }

                            if (delta_type == "input_json_delta"
                                || matches!(
                                    content_block_accumulators.get(&index),
                                    Some(AnthropicContentAccumulator::ToolUse(_))
                                ))
                                && let Some(partial) = delta
                                    .and_then(|d| d.get("partial_json"))
                                    .and_then(Value::as_str)
                                && let Some(AnthropicContentAccumulator::ToolUse(acc)) =
                                    content_block_accumulators.get_mut(&index)
                            {
                                // Fire on_tool_call_delta callback.
                                if let Some(cb) = callbacks
                                    .as_ref()
                                    .and_then(|c| c.on_tool_call_delta.as_ref())
                                {
                                    cb(&acc.id, partial);
                                }
                                acc.partial_json.push_str(partial);
                            }
                        }
                        "content_block_stop" => {}
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

        let (raw_text, thinking_text, content_blocks, tool_calls) =
            finalize_anthropic_content_blocks(content_block_accumulators);
        Ok(ProviderResponse {
            text: crate::strip_reasoning_tags(&raw_text),
            history_text: Some(raw_text),
            thinking: thinking_text,
            content_blocks,
            tool_calls,
            request_id,
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
        request_context: Option<&crate::query_source::ProviderRequestContext>,
    ) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        loop {
            maybe_dump_streaming_request_body(label, body);
            let response = self
                .http
                .post(base_url)
                .headers(build_headers(provider, Some(body), request_context)?)
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
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

fn maybe_dump_streaming_request_body(label: &str, body: &Value) {
    let Ok(dir) = std::env::var("REMOTE_CODE_DUMP_PROVIDER_REQUEST_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let path = dir.join(format!("{timestamp}-{label}.json"));
    if let Ok(bytes) = serde_json::to_vec_pretty(body) {
        let _ = std::fs::write(path, bytes);
    }
}

fn wrap_streaming_callbacks(
    callbacks: Option<StreamingCallbacks>,
    streamed_tool_activity: Arc<AtomicBool>,
) -> StreamingCallbacks {
    let callbacks = callbacks.unwrap_or_default();
    let StreamingCallbacks {
        on_text_delta,
        on_tool_call_start,
        on_tool_call_delta,
        on_usage,
    } = callbacks;

    let start_activity = Arc::clone(&streamed_tool_activity);
    let tracked_tool_call_start = Box::new(move |tool_call_id: &str, tool_name: &str| {
        start_activity.store(true, Ordering::Relaxed);
        if let Some(callback) = on_tool_call_start.as_ref() {
            callback(tool_call_id, tool_name);
        }
    });

    let delta_activity = Arc::clone(&streamed_tool_activity);
    let tracked_tool_call_delta = Box::new(move |tool_call_id: &str, delta: &str| {
        delta_activity.store(true, Ordering::Relaxed);
        if let Some(callback) = on_tool_call_delta.as_ref() {
            callback(tool_call_id, delta);
        }
    });

    StreamingCallbacks {
        on_text_delta,
        on_tool_call_start: Some(tracked_tool_call_start),
        on_tool_call_delta: Some(tracked_tool_call_delta),
        on_usage,
    }
}

fn should_fallback_after_streaming_error(
    error: &anyhow::Error,
    streamed_tool_activity: bool,
) -> bool {
    let err_str = format!("{error:#}").to_ascii_lowercase();
    let is_streaming_error = err_str.contains("streaming")
        || err_str.contains("chunk")
        || err_str.contains("connection")
        || err_str.contains("broken pipe")
        || err_str.contains("reset")
        || err_str.contains("unexpected eof");
    is_streaming_error && !streamed_tool_activity
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
    block_type: String,
    id: String,
    name: String,
    partial_json: String,
}

enum AnthropicContentAccumulator {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    ToolUse(AnthropicToolUseAccumulator),
}

fn finalize_anthropic_content_blocks(
    accumulators: BTreeMap<usize, AnthropicContentAccumulator>,
) -> (String, Option<String>, Vec<Value>, Vec<ToolCall>) {
    let mut raw_text_parts = Vec::new();
    let mut thinking_parts = Vec::new();
    let mut content_blocks = Vec::new();
    let mut tool_calls = Vec::new();

    for accumulator in accumulators.into_values() {
        match accumulator {
            AnthropicContentAccumulator::Text { text } => {
                if text.is_empty() {
                    continue;
                }
                raw_text_parts.push(text.clone());
                content_blocks.push(json!({
                    "type": "text",
                    "text": text,
                }));
            }
            AnthropicContentAccumulator::Thinking {
                thinking,
                signature,
            } => {
                if thinking.is_empty() && signature.is_none() {
                    continue;
                }
                if !thinking.is_empty() {
                    thinking_parts.push(thinking.clone());
                }
                let mut block = json!({
                    "type": "thinking",
                    "thinking": thinking,
                });
                if let Some(signature) = signature {
                    block["signature"] = Value::String(signature);
                }
                content_blocks.push(block);
            }
            AnthropicContentAccumulator::ToolUse(acc) => {
                if acc.id.is_empty() || acc.name.is_empty() {
                    continue;
                }
                let input = if acc.partial_json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str::<Value>(&acc.partial_json)
                        .ok()
                        .unwrap_or_else(|| json!({}))
                };
                tool_calls.push(ToolCall {
                    id: acc.id.clone(),
                    name: acc.name.clone(),
                    input: input.clone(),
                });
                content_blocks.push(json!({
                    "type": acc.block_type,
                    "id": acc.id,
                    "name": acc.name,
                    "input": input,
                }));
            }
        }
    }

    let raw_text = raw_text_parts.join("");
    let thinking_text = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join(""))
    };

    (raw_text, thinking_text, content_blocks, tool_calls)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::{
        AnthropicContentAccumulator, AnthropicToolUseAccumulator,
        finalize_anthropic_content_blocks, is_retryable_http_status,
        should_fallback_after_streaming_error,
    };

    #[test]
    fn streaming_errors_fallback_before_tool_activity() {
        assert!(should_fallback_after_streaming_error(
            &anyhow!("streaming connection reset by peer"),
            false,
        ));
    }

    #[test]
    fn streaming_errors_do_not_fallback_after_tool_activity() {
        assert!(!should_fallback_after_streaming_error(
            &anyhow!("streaming connection reset by peer"),
            true,
        ));
    }

    #[test]
    fn non_streaming_errors_do_not_trigger_fallback() {
        assert!(!should_fallback_after_streaming_error(
            &anyhow!("provider request failed (401): unauthorized"),
            false,
        ));
    }

    #[test]
    fn overloaded_529_is_retryable_for_streaming_requests() {
        assert!(is_retryable_http_status(529));
    }

    #[test]
    fn anthropic_streaming_finalizer_preserves_block_order_and_text() {
        let mut accumulators = BTreeMap::new();
        accumulators.insert(
            0,
            AnthropicContentAccumulator::Thinking {
                thinking: "plan".to_owned(),
                signature: Some("sig".to_owned()),
            },
        );
        accumulators.insert(
            1,
            AnthropicContentAccumulator::Text {
                text: "reply".to_owned(),
            },
        );
        accumulators.insert(
            2,
            AnthropicContentAccumulator::ToolUse(AnthropicToolUseAccumulator {
                block_type: "tool_use".to_owned(),
                id: "call-2".to_owned(),
                name: "read_file".to_owned(),
                partial_json: r#"{"path":"src/lib.rs"}"#.to_owned(),
            }),
        );
        accumulators.insert(
            3,
            AnthropicContentAccumulator::ToolUse(AnthropicToolUseAccumulator {
                block_type: "tool_use".to_owned(),
                id: "call-3".to_owned(),
                name: "read_file".to_owned(),
                partial_json: r#"{"path":"src/main.rs"}"#.to_owned(),
            }),
        );

        let (raw_text, thinking_text, content_blocks, tool_calls) =
            finalize_anthropic_content_blocks(accumulators);

        assert_eq!(raw_text, "reply");
        assert_eq!(thinking_text.as_deref(), Some("plan"));
        assert_eq!(content_blocks.len(), 4);
        assert_eq!(content_blocks[0]["type"], "thinking");
        assert_eq!(content_blocks[0]["signature"], "sig");
        assert_eq!(content_blocks[1]["type"], "text");
        assert_eq!(content_blocks[2]["id"], "call-2");
        assert_eq!(content_blocks[3]["id"], "call-3");
        assert_eq!(tool_calls[0].id, "call-2");
        assert_eq!(tool_calls[1].id, "call-3");
        assert_eq!(tool_calls[0].input, json!({"path":"src/lib.rs"}));
    }
}
