//! In-process protocol translator that bridges Codex's Responses API expectations
//! to an upstream provider that speaks the Anthropic Messages API.
//!
//! Architecture:
//! ```text
//! codex-core  ──POST /v1/responses──▸  AnthropicProxy (localhost)
//!                                         │
//!                                         │ translate request:
//!                                         │   ResponsesApiRequest → Anthropic Messages
//!                                         │
//!                                         ├──POST /v1/messages──▸ upstream (e.g. MiniMax)
//!                                         │
//!                                         │ translate SSE response:
//!                                         │   Anthropic events → Responses events
//!                                         │
//!                                     ◂──┘
//! ```

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::post;
use futures::StreamExt;
use futures::stream::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

// ── Public API ──────────────────────────────────────────────────────────────

/// Configuration for the protocol translator proxy.
#[derive(Debug, Clone)]
pub struct AnthropicProxyConfig {
    /// The upstream Anthropic Messages API base URL (e.g. `https://api.minimaxi.com/anthropic`).
    pub upstream_url: String,
    /// Bearer token for the upstream API.
    pub api_key: String,
    /// Model name to use (overrides whatever codex sends).
    pub model: Option<String>,
}

/// A running protocol translator proxy.
pub struct AnthropicProxy {
    /// The address the proxy is listening on.
    pub listen_addr: SocketAddr,
    /// Shutdown signal sender.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl AnthropicProxy {
    /// Start the proxy on a random port.
    pub async fn start(config: AnthropicProxyConfig) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let listen_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let state = Arc::new(ProxyState {
            http: Client::new(),
            config,
        });

        let app = Router::new()
            .route("/v1/responses", post(handle_responses))
            .fallback(fallback_handler)
            .with_state(state);

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            if let Err(e) = server.await {
                tracing::error!("anthropic proxy server error: {e}");
            }
        });

        tracing::info!("anthropic proxy listening on {listen_addr}");

        Ok(Self {
            listen_addr,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Return the base URL that codex should use to reach this proxy.
    pub fn proxy_base_url(&self) -> String {
        format!("http://{}/v1", self.listen_addr)
    }

    /// Stop the proxy.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for AnthropicProxy {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

// ── Internal state ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct ProxyState {
    http: Client,
    config: AnthropicProxyConfig,
}

// ── Request / Response types ────────────────────────────────────────────────

/// The request body that codex sends (Responses API shape).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ResponsesApiRequest {
    model: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    input: Vec<serde_json::Value>,
    #[serde(default)]
    tools: Vec<serde_json::Value>,
    #[serde(default = "default_true_val")]
    stream: bool,
    #[serde(default)]
    tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
    #[serde(default)]
    text: Option<serde_json::Value>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    service_tier: Option<String>,
    #[serde(default)]
    parallel_tool_calls: Option<bool>,
}

fn default_true_val() -> bool {
    true
}

/// The request body sent to the upstream Anthropic Messages API.
#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    stream: bool,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

// ── SSE event types from upstream Anthropic Messages API ────────────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AnthropicSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    message: Option<AnthropicMessageStart>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<AnthropicContentBlock>,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AnthropicMessageStart {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type", default)]
    delta_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
}

// ── Handler ─────────────────────────────────────────────────────────────────

async fn handle_responses(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let req: ResponsesApiRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to parse responses request: {e}");
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid request body: {e}"),
            )
                .into_response();
        }
    };

    tracing::debug!(
        "translating request for model={} ({} input items, {} tools)",
        req.model,
        req.input.len(),
        req.tools.len()
    );

    let anthropic_req = translate_request(&req, &state.config);
    let upstream_url = format!("{}/v1/messages", state.config.upstream_url_trimmed());

    let mut upstream_headers = HeaderMap::new();
    upstream_headers.insert("content-type", HeaderValue::from_static("application/json"));
    upstream_headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    upstream_headers.insert(
        "x-api-key",
        HeaderValue::from_str(&state.config.api_key)
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    // Forward select headers from the original request
    for key in ["openai-conversation-id", "openai-subagent-epoch"] {
        if let Some(val) = headers.get(key) {
            upstream_headers.insert(key, val.clone());
        }
    }

    let http = &state.http;
    let response = match http
        .post(&upstream_url)
        .headers(upstream_headers)
        .json(&anthropic_req)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("upstream request failed: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream request failed: {e}"),
            )
                .into_response();
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        tracing::error!("upstream returned {}: {}", status, body);
        return (status, body).into_response();
    }

    // Stream the translated SSE back to the caller
    let stream = translate_sse_response(response);

    let sse = Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text(""),
    );

    sse.into_response()
}

async fn fallback_handler(uri: axum::http::Uri) -> impl IntoResponse {
    tracing::warn!("proxy received unexpected request: {uri}");
    (StatusCode::NOT_FOUND, "not found")
}

// ── Request translation ─────────────────────────────────────────────────────

fn translate_request(
    req: &ResponsesApiRequest,
    config: &AnthropicProxyConfig,
) -> AnthropicMessagesRequest {
    let model = config.model.clone().unwrap_or_else(|| req.model.clone());
    let system = if req.instructions.is_empty() {
        None
    } else {
        Some(req.instructions.clone())
    };

    let messages = translate_input_to_messages(&req.input);

    let tools = if req.tools.is_empty() {
        None
    } else {
        Some(translate_tools(&req.tools))
    };

    let tool_choice = req.tool_choice.as_ref().and_then(|tc| {
        // Map "auto" / "required" / "none" to Anthropic format
        match tc {
            serde_json::Value::String(s) => Some(serde_json::json!({
                "type": match s.as_str() {
                    "auto" => "auto",
                    "required" => "any",
                    "none" => "none",
                    other => other,
                }
            })),
            serde_json::Value::Object(_) => Some(tc.clone()),
            _ => None,
        }
    });

    AnthropicMessagesRequest {
        model,
        system,
        messages,
        tools,
        tool_choice,
        stream: req.stream,
        max_tokens: 16384,
        temperature: None,
    }
}

/// Convert Responses API `input` items to Anthropic `messages` array.
fn translate_input_to_messages(input: &[serde_json::Value]) -> Vec<AnthropicMessage> {
    let mut messages = Vec::new();

    for item in input {
        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match item_type {
            "message" => {
                let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                let content = extract_message_content(item);
                if let Some(content) = content {
                    messages.push(AnthropicMessage {
                        role: role.to_string(),
                        content,
                    });
                }
            }
            "function_call" | "function_call_output" => {
                // Tool call / tool result — map to Anthropic format
                let role = if item_type == "function_call" {
                    "assistant"
                } else {
                    "user"
                };
                let content = translate_tool_item(item, item_type);
                messages.push(AnthropicMessage {
                    role: role.to_string(),
                    content,
                });
            }
            _ => {
                // Try to extract role + content for unknown types
                if let Some(role) = item.get("role").and_then(|r| r.as_str()) {
                    let content = extract_message_content(item)
                        .unwrap_or(serde_json::json!(item.to_string()));
                    messages.push(AnthropicMessage {
                        role: role.to_string(),
                        content,
                    });
                }
            }
        }
    }

    // Ensure messages alternate user/assistant (Anthropic requirement)
    ensure_alternating_roles(&mut messages);

    messages
}

fn extract_message_content(item: &serde_json::Value) -> Option<serde_json::Value> {
    let content = item.get("content")?;

    if content.is_string() {
        return Some(content.clone());
    }

    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for part in arr {
            let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match part_type {
                "input_text" | "output_text" => {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        parts.push(serde_json::json!({
                            "type": "text",
                            "text": text
                        }));
                    }
                }
                "input_image" => {
                    if let Some(url) = part.get("image_url").and_then(|u| u.as_str()) {
                        parts.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": url
                            }
                        }));
                    } else if let Some(data) = part.get("data").and_then(|d| d.as_str()) {
                        let media_type = part
                            .get("media_type")
                            .and_then(|m| m.as_str())
                            .unwrap_or("image/png");
                        parts.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": media_type,
                                "data": data
                            }
                        }));
                    }
                }
                _ => {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        parts.push(serde_json::json!({
                            "type": "text",
                            "text": text
                        }));
                    }
                }
            }
        }
        if !parts.is_empty() {
            return Some(serde_json::Value::Array(parts));
        }
    }

    None
}

fn translate_tool_item(item: &serde_json::Value, item_type: &str) -> serde_json::Value {
    match item_type {
        "function_call" => {
            let call_id = item
                .get("call_id")
                .and_then(|c| c.as_str())
                .unwrap_or("unknown");
            let name = item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let args = item
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");

            serde_json::json!([{
                "type": "tool_use",
                "id": call_id,
                "name": name,
                "input": serde_json::from_str::<serde_json::Value>(args).unwrap_or(serde_json::json!({}))
            }])
        }
        "function_call_output" => {
            let call_id = item
                .get("call_id")
                .and_then(|c| c.as_str())
                .unwrap_or("unknown");
            let output = item.get("output").and_then(|o| o.as_str()).unwrap_or("");

            serde_json::json!([{
                "type": "tool_result",
                "tool_use_id": call_id,
                "content": output
            }])
        }
        _ => serde_json::json!(item.to_string()),
    }
}

fn translate_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let tool_type = tool.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match tool_type {
                "function" => {
                    let name = tool.get("name").and_then(|n| n.as_str())?;
                    let description = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    let parameters = tool
                        .get("parameters")
                        .cloned()
                        .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

                    Some(serde_json::json!({
                        "name": name,
                        "description": description,
                        "input_schema": parameters,
                    }))
                }
                _ => None,
            }
        })
        .collect()
}

/// Anthropic requires messages to strictly alternate user/assistant roles.
/// This merges consecutive same-role messages.
fn ensure_alternating_roles(messages: &mut Vec<AnthropicMessage>) {
    if messages.len() <= 1 {
        return;
    }

    let mut merged: Vec<AnthropicMessage> = Vec::new();
    for msg in messages.drain(..) {
        if let Some(last) = merged.last_mut() {
            if last.role == msg.role {
                // Merge content
                last.content = merge_content(&last.content, &msg.content);
                continue;
            }
        }
        merged.push(msg);
    }
    *messages = merged;
}

fn merge_content(a: &serde_json::Value, b: &serde_json::Value) -> serde_json::Value {
    let mut parts = Vec::new();

    fn collect_text(val: &serde_json::Value, parts: &mut Vec<serde_json::Value>) {
        match val {
            serde_json::Value::String(s) => {
                parts.push(serde_json::json!({"type": "text", "text": s}));
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    collect_text(item, parts);
                }
            }
            serde_json::Value::Object(_) => {
                parts.push(val.clone());
            }
            _ => {}
        }
    }

    collect_text(a, &mut parts);
    collect_text(b, &mut parts);

    if parts.len() == 1 {
        parts
            .into_iter()
            .next()
            .expect("single part is present after length check")
    } else {
        serde_json::Value::Array(parts)
    }
}

// ── SSE Response Translation ────────────────────────────────────────────────

fn translate_sse_response(
    response: reqwest::Response,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(64);

    tokio::spawn(async move {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut msg_id = String::from("msg_proxy");
        let mut model_name = String::from("unknown");
        let mut text_acc = String::new();
        let mut usage = AnthropicUsage {
            input_tokens: Some(0),
            output_tokens: Some(0),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let mut created = false;
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();

        use futures::TryStreamExt;

        while let Some(chunk) = stream.try_next().await.unwrap_or(None) {
            let text = match std::str::from_utf8(&chunk) {
                Ok(t) => t,
                Err(_) => continue,
            };
            buffer.push_str(text);

            // Extract complete SSE events from buffer
            while let Some(pos) = buffer.find("\n\n") {
                let raw = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let mut event_type = String::new();
                let mut data = String::new();

                for line in raw.lines() {
                    if let Some(rest) = line.strip_prefix("event:") {
                        event_type = rest.trim().to_string();
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        data = rest.trim().to_string();
                    }
                }

                if data.is_empty() {
                    continue;
                }

                let events = translate_anthropic_event(
                    &event_type,
                    &data,
                    &mut msg_id,
                    &mut model_name,
                    &mut text_acc,
                    &mut usage,
                    &mut created,
                    &mut current_tool_id,
                    &mut current_tool_name,
                );

                for event in events {
                    if tx.send(event).await.is_err() {
                        // Receiver dropped — stop processing
                        return;
                    }
                }
            }
        }

        // Handle any remaining data in buffer
        if !buffer.is_empty() {
            let data = buffer.trim();
            if !data.is_empty() {
                // Try to parse as a final event
                let mut event_type = String::new();
                let mut event_data = String::new();
                for line in data.lines() {
                    if let Some(rest) = line.strip_prefix("event:") {
                        event_type = rest.trim().to_string();
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        event_data = rest.trim().to_string();
                    }
                }
                if !event_data.is_empty() {
                    let mut msg_id_tmp = msg_id.clone();
                    let mut model_tmp = model_name.clone();
                    let mut text_tmp = text_acc.clone();
                    let mut usage_tmp = usage.clone();
                    let mut created_tmp = created;
                    let mut tool_id_tmp = current_tool_id.clone();
                    let mut tool_name_tmp = current_tool_name.clone();

                    let events = translate_anthropic_event(
                        &event_type,
                        &event_data,
                        &mut msg_id_tmp,
                        &mut model_tmp,
                        &mut text_tmp,
                        &mut usage_tmp,
                        &mut created_tmp,
                        &mut tool_id_tmp,
                        &mut tool_name_tmp,
                    );
                    for event in events {
                        if tx.send(event).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok)
}

#[allow(clippy::too_many_arguments)]
fn translate_anthropic_event(
    _event_type: &str,
    data: &str,
    msg_id: &mut String,
    model_name: &mut String,
    text_acc: &mut String,
    usage: &mut AnthropicUsage,
    created: &mut bool,
    current_tool_id: &mut String,
    current_tool_name: &mut String,
) -> Vec<Event> {
    let parsed: serde_json::Result<AnthropicSseEvent> = serde_json::from_str(data);
    let event = match parsed {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                "failed to parse anthropic SSE event: {e} (data: {})",
                &data[..data.len().min(200)]
            );
            return Vec::new();
        }
    };

    let mut out = Vec::new();

    match event.event_type.as_str() {
        "message_start" => {
            if let Some(msg) = &event.message {
                if let Some(id) = &msg.id {
                    *msg_id = id.clone();
                }
                if let Some(m) = &msg.model {
                    *model_name = m.clone();
                }
                if let Some(u) = &msg.usage {
                    usage.input_tokens = Some(u.input_tokens.unwrap_or(0));
                }
            }
            // Emit response.created
            if !*created {
                *created = true;
                let resp_id = format!("resp_{}", &msg_id[..msg_id.len().min(24)]);
                out.push(make_event(
                    "response.created",
                    &serde_json::json!({
                        "type": "response.created",
                        "response": {"id": resp_id, "object": "response", "status": "in_progress"}
                    }),
                ));
                // Emit output_item.added for the assistant message
                out.push(make_event(
                    "response.output_item.added",
                    &serde_json::json!({
                        "type": "response.output_item.added",
                        "output_index": 0,
                        "item": {
                            "type": "message",
                            "id": format!("msg_{}", uuid::Uuid::new_v4().as_simple()),
                            "role": "assistant",
                            "content": []
                        }
                    }),
                ));
            }
        }
        "content_block_start" => {
            if let Some(block) = &event.content_block {
                match block.block_type.as_str() {
                    "text" => {
                        // Start of text output — nothing extra needed
                    }
                    "tool_use" => {
                        *current_tool_id = block.id.clone().unwrap_or_default();
                        *current_tool_name = block.name.clone().unwrap_or_default();
                    }
                    _ => {}
                }
            }
        }
        "content_block_delta" => {
            if let Some(delta) = &event.delta {
                match delta.delta_type.as_deref().unwrap_or("") {
                    "text_delta" => {
                        if let Some(text) = &delta.text {
                            *text_acc += text;
                            out.push(make_event(
                                "response.output_text.delta",
                                &serde_json::json!({
                                    "type": "response.output_text.delta",
                                    "output_index": 0,
                                    "content_index": 0,
                                    "delta": text
                                }),
                            ));
                        }
                    }
                    "input_json_delta" => {
                        if let Some(json) = &delta.partial_json {
                            out.push(make_event(
                                "response.custom_tool_call_input.delta",
                                &serde_json::json!({
                                    "type": "response.custom_tool_call_input.delta",
                                    "output_index": 0,
                                    "call_id": &*current_tool_id,
                                    "delta": json
                                }),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_stop" => {
            // If this was a tool_use block, emit the tool call item
            if !current_tool_id.is_empty() && !current_tool_name.is_empty() {
                let tool_name = std::mem::take(current_tool_name);
                let tool_call_id = std::mem::take(current_tool_id);
                out.push(make_event(
                    "response.output_item.done",
                    &serde_json::json!({
                        "type": "response.output_item.done",
                        "output_index": 1,
                        "item": {
                            "type": "custom_tool_call",
                            "id": format!("fc_{}", uuid::Uuid::new_v4().as_simple()),
                            "call_id": tool_call_id,
                            "name": tool_name,
                            "input": "{}"
                        }
                    }),
                ));
                // Add new output_item.added for continued assistant message
                out.push(make_event(
                    "response.output_item.added",
                    &serde_json::json!({
                        "type": "response.output_item.added",
                        "output_index": 2,
                        "item": {
                            "type": "message",
                            "id": format!("msg_{}", uuid::Uuid::new_v4().as_simple()),
                            "role": "assistant",
                            "content": []
                        }
                    }),
                ));
            }
        }
        "message_delta" => {
            if let Some(delta) = &event.delta {
                if let Some(_reason) = &delta.stop_reason {
                    // End of message
                    let accumulated = std::mem::take(text_acc);
                    let final_text = if accumulated.is_empty() {
                        String::new()
                    } else {
                        accumulated
                    };

                    // Emit output_item.done with final text
                    out.push(make_event(
                        "response.output_item.done",
                        &serde_json::json!({
                            "type": "response.output_item.done",
                            "output_index": 0,
                            "item": {
                                "type": "message",
                                "id": format!("msg_{}", uuid::Uuid::new_v4().as_simple()),
                                "role": "assistant",
                                "content": [{"type": "output_text", "text": final_text}]
                            }
                        }),
                    ));

                    // Update usage from message_delta
                    if let Some(u) = &event.usage {
                        usage.output_tokens = Some(u.output_tokens.unwrap_or(0));
                    }

                    // Emit response.completed
                    let resp_id = format!("resp_{}", &msg_id[..msg_id.len().min(24)]);
                    out.push(make_event(
                        "response.completed",
                        &serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": resp_id,
                                "object": "response",
                                "status": "completed",
                                "model": model_name,
                                "usage": {
                                    "input_tokens": usage.input_tokens.unwrap_or(0),
                                    "output_tokens": usage.output_tokens.unwrap_or(0),
                                    "total_tokens": usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0)
                                }
                            }
                        }),
                    ));
                }
            }
        }
        "message_stop" => {
            // Already handled in message_delta
        }
        "ping" => {}
        _ => {
            tracing::debug!("unhandled anthropic SSE event type: {}", event.event_type);
        }
    }

    out
}

fn make_event(event_type: &str, data: &serde_json::Value) -> Event {
    Event::default().event(event_type).data(data.to_string())
}

// ── Helper ──────────────────────────────────────────────────────────────────

impl AnthropicProxyConfig {
    fn upstream_url_trimmed(&self) -> &str {
        self.upstream_url.trim_end_matches('/')
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_basic_request() {
        let input = serde_json::json!([
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Hello"}]
            }
        ]);

        let req = ResponsesApiRequest {
            model: "minimax-m2.7".to_string(),
            instructions: "Be helpful".to_string(),
            input: input.as_array().unwrap().clone(),
            tools: vec![],
            stream: true,
            tool_choice: None,
            reasoning: None,
            text: None,
            include: vec![],
            service_tier: None,
            parallel_tool_calls: None,
        };

        let config = AnthropicProxyConfig {
            upstream_url: "https://api.minimaxi.com/anthropic".to_string(),
            api_key: "test-key".to_string(),
            model: None,
        };

        let result = translate_request(&req, &config);
        assert_eq!(result.model, "minimax-m2.7");
        assert_eq!(result.system, Some("Be helpful".to_string()));
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, "user");
    }

    #[test]
    fn test_translate_tools() {
        let tools = vec![serde_json::json!({
            "type": "function",
            "name": "read_file",
            "description": "Read a file",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}
        })];

        let result = translate_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "read_file");
        assert!(result[0].get("input_schema").is_some());
    }

    #[test]
    fn test_ensure_alternating_roles() {
        let mut messages = vec![
            AnthropicMessage {
                role: "user".into(),
                content: serde_json::json!("a"),
            },
            AnthropicMessage {
                role: "user".into(),
                content: serde_json::json!("b"),
            },
            AnthropicMessage {
                role: "assistant".into(),
                content: serde_json::json!("c"),
            },
            AnthropicMessage {
                role: "assistant".into(),
                content: serde_json::json!("d"),
            },
        ];
        ensure_alternating_roles(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }
}
