//! Shared Responses API logic for OpenAI Native and Codex providers.
//!
//! This module contains the common functionality used by both
//! [`crate::handler::OpenAiNativeHandler`] and [`crate::codex_handler::OpenAiCodexHandler`]:
//!
//! - **Conversation formatting** — converting [`ApiMessage`]s to Responses API input
//! - **Request body building** — constructing the JSON body for `/v1/responses`
//! - **SSE event parsing** — processing Server-Sent Events from the Responses API
//! - **Tool schema helpers** — `ensureAllRequired` and `ensureAdditionalPropertiesFalse`

use roo_provider::error::{ProviderError, Result};
use roo_types::api::ApiStreamChunk;
use serde_json::{json, Value};

use crate::types::ResponsesApiRequestBody;

// ---------------------------------------------------------------------------
// Tool schema helpers
// ---------------------------------------------------------------------------

/// Ensures all object properties are marked as required and `additionalProperties: false`.
///
/// This is needed for OpenAI's strict mode on function/tool schemas.
/// MCP tools use [`ensure_additional_properties_false`] instead.
pub fn ensure_all_required(schema: &Value) -> Value {
    if !schema.is_object() {
        return schema.clone();
    }
    let obj = schema.as_object().unwrap();

    if obj.get("type").and_then(|v| v.as_str()) != Some("object") {
        return schema.clone();
    }

    let mut result = obj.clone();
    result.insert("additionalProperties".to_string(), json!(false));

    if let Some(properties) = result.get("properties").and_then(|p| p.as_object()).cloned() {
        let all_keys: Vec<String> = properties.keys().cloned().collect();
        result.insert("required".to_string(), json!(all_keys));

        let mut new_props = properties.clone();
        for (key, prop) in properties.iter() {
            let updated = if prop.get("type").and_then(|v| v.as_str()) == Some("object") {
                ensure_all_required(prop)
            } else if prop.get("type").and_then(|v| v.as_str()) == Some("array")
                && prop
                    .get("items")
                    .and_then(|i| i.get("type"))
                    .and_then(|v| v.as_str())
                    == Some("object")
            {
                let mut updated_val = prop.clone();
                if let Some(items) = prop.get("items").cloned() {
                    updated_val
                        .as_object_mut()
                        .unwrap()
                        .insert("items".to_string(), ensure_all_required(&items));
                }
                updated_val
            } else {
                continue;
            };
            new_props.insert(key.clone(), updated);
        }
        result.insert("properties".to_string(), Value::Object(new_props));
    }

    Value::Object(result)
}

/// Adds `additionalProperties: false` to all nested object schemas without
/// modifying the `required` array.
///
/// Used for MCP tools with `strict: false` to comply with OpenAI Responses
/// API requirements.
pub fn ensure_additional_properties_false(schema: &Value) -> Value {
    if !schema.is_object() {
        return schema.clone();
    }
    let obj = schema.as_object().unwrap();

    if obj.get("type").and_then(|v| v.as_str()) != Some("object") {
        return schema.clone();
    }

    let mut result = obj.clone();
    result.insert("additionalProperties".to_string(), json!(false));

    if let Some(properties) = result.get("properties").and_then(|p| p.as_object()) {
        let mut new_props = properties.clone();
        for (key, prop) in properties.iter() {
            let updated = if prop.get("type").and_then(|v| v.as_str()) == Some("object") {
                ensure_additional_properties_false(prop)
            } else if prop.get("type").and_then(|v| v.as_str()) == Some("array")
                && prop
                    .get("items")
                    .and_then(|i| i.get("type"))
                    .and_then(|v| v.as_str())
                    == Some("object")
            {
                let mut updated_val = prop.clone();
                if let Some(items) = prop.get("items").cloned() {
                    updated_val
                        .as_object_mut()
                        .unwrap()
                        .insert("items".to_string(), ensure_additional_properties_false(&items));
                }
                updated_val
            } else {
                continue;
            };
            new_props.insert(key.clone(), updated);
        }
        result.insert("properties".to_string(), Value::Object(new_props));
    }

    Value::Object(result)
}

/// Checks whether a tool name is an MCP tool (has the `mcp__` prefix).
fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp__")
}

// ---------------------------------------------------------------------------
// Conversation formatting
// ---------------------------------------------------------------------------

/// Converts a slice of [`ApiMessage`]s into the Responses API input format.
///
/// This delegates to the existing
/// [`roo_provider::transform::responses_api::convert_to_responses_api_input`].
pub fn format_full_conversation(messages: &[roo_types::api::ApiMessage]) -> Vec<Value> {
    roo_provider::transform::responses_api::convert_to_responses_api_input(messages)
}

// ---------------------------------------------------------------------------
// Request body building
// ---------------------------------------------------------------------------

/// Parameters needed to build a Responses API request body.
pub struct RequestBodyParams<'a> {
    /// Model ID string.
    pub model_id: &'a str,
    /// Formatted conversation input.
    pub formatted_input: Vec<Value>,
    /// System prompt (becomes `instructions`).
    pub system_prompt: &'a str,
    /// Tool definitions (OpenAI function format).
    pub tools: Option<&'a [Value]>,
    /// Tool choice strategy.
    pub tool_choice: Option<&'a Value>,
    /// Whether to allow parallel tool calls.
    pub parallel_tool_calls: Option<bool>,
    /// Reasoning effort (e.g. "low", "medium", "high").
    pub reasoning_effort: Option<&'a str>,
    /// Whether to enable reasoning summary.
    pub enable_reasoning_summary: bool,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Whether the model supports temperature.
    pub supports_temperature: Option<bool>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<u64>,
    /// Whether the model supports verbosity.
    pub supports_verbosity: Option<bool>,
    /// Verbosity level (e.g. "low", "medium", "high").
    pub verbosity: Option<&'a str>,
    /// Service tier (e.g. "default", "flex", "priority").
    pub service_tier: Option<&'a str>,
    /// Prompt cache retention policy.
    pub prompt_cache_retention: Option<&'a str>,
    /// Whether to stream the response.
    pub stream: bool,
}

/// Builds a Responses API request body from the given parameters.
///
/// This is the shared logic used by both Native and Codex handlers.
/// The Codex handler omits `max_output_tokens`, `service_tier`, and
/// `prompt_cache_retention` by passing `None` for those fields.
pub fn build_request_body(params: RequestBodyParams<'_>) -> ResponsesApiRequestBody {
    let tools = params.tools.map(|tools| {
        tools
            .iter()
            .filter(|tool| tool.get("type").and_then(|v| v.as_str()) == Some("function"))
            .map(|tool| {
                let func = tool.get("function").unwrap_or(&json!(null));
                let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let is_mcp = is_mcp_tool(name);

                let parameters = func.get("parameters").map(|schema| {
                    if is_mcp {
                        ensure_additional_properties_false(schema)
                    } else {
                        ensure_all_required(schema)
                    }
                });

                let mut tool_obj = serde_json::Map::new();
                tool_obj.insert("type".to_string(), json!("function"));
                tool_obj.insert("name".to_string(), json!(name));
                if let Some(desc) = func.get("description") {
                    tool_obj.insert("description".to_string(), desc.clone());
                }
                if let Some(p) = parameters {
                    tool_obj.insert("parameters".to_string(), p);
                }
                tool_obj.insert("strict".to_string(), json!(!is_mcp));

                Value::Object(tool_obj)
            })
            .collect::<Vec<Value>>()
    });

    let reasoning = params.reasoning_effort.map(|effort| {
        let mut r = serde_json::Map::new();
        r.insert("effort".to_string(), json!(effort));
        if params.enable_reasoning_summary {
            r.insert("summary".to_string(), json!("auto"));
        }
        Value::Object(r)
    });

    let text = if params.supports_verbosity == Some(true) {
        let verbosity = params.verbosity.unwrap_or("medium");
        Some(json!({ "verbosity": verbosity }))
    } else {
        None
    };

    let include = params
        .reasoning_effort
        .map(|_| vec!["reasoning.encrypted_content".to_string()]);

    ResponsesApiRequestBody {
        model: params.model_id.to_string(),
        input: params.formatted_input,
        stream: params.stream,
        instructions: Some(params.system_prompt.to_string()),
        tools,
        tool_choice: params.tool_choice.cloned(),
        parallel_tool_calls: params.parallel_tool_calls,
        temperature: if params.supports_temperature != Some(false) {
            Some(params.temperature.unwrap_or(0.0))
        } else {
            None
        },
        max_output_tokens: params.max_output_tokens,
        reasoning,
        text,
        store: Some(false),
        include,
        service_tier: params.service_tier.map(|s| s.to_string()),
        prompt_cache_retention: params.prompt_cache_retention.map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// SSE event parsing
// ---------------------------------------------------------------------------

/// Parses a single SSE event data string from the Responses API stream.
///
/// The `data` parameter should be the raw JSON string from an SSE `data:` field.
/// Returns `Ok(None)` for ignorable events, `Ok(Some(chunk))` for meaningful
/// events, and `Err(...)` for error events.
pub fn parse_sse_event(data: &str, provider_name: &str) -> Result<Option<ApiStreamChunk>> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(None);
    }

    let event: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let event_type = event["type"].as_str().unwrap_or("");

    match event_type {
        // Text content deltas
        "response.output_text.delta" | "response.text.delta" => {
            if let Some(delta) = event["delta"].as_str() {
                if !delta.is_empty() {
                    return Ok(Some(ApiStreamChunk::Text {
                        text: delta.to_string(),
                    }));
                }
            }
            Ok(None)
        }

        // Text done — fallback
        "response.output_text.done" | "response.text.done" => {
            let done_text = event["text"]
                .as_str()
                .or_else(|| event["output_text"].as_str())
                .or_else(|| event["delta"].as_str());
            if let Some(text) = done_text {
                if !text.is_empty() {
                    return Ok(Some(ApiStreamChunk::Text {
                        text: text.to_string(),
                    }));
                }
            }
            Ok(None)
        }

        // Content part events
        "response.content_part.added" | "response.content_part.done" => {
            let part = &event["part"];
            // TS checks both `part.text` (string) and `part.text.value` (object with value field)
            let part_text = part
                .get("text")
                .and_then(|t| t.as_str())
                .or_else(|| part.get("text").and_then(|t| t.get("value")).and_then(|v| v.as_str()));
            if let Some(text) = part_text {
                if !text.is_empty() {
                    return Ok(Some(ApiStreamChunk::Text {
                        text: text.to_string(),
                    }));
                }
            }
            Ok(None)
        }

        // Reasoning deltas
        "response.reasoning_text.delta"
        | "response.reasoning.delta"
        | "response.reasoning_summary_text.delta"
        | "response.reasoning_summary.delta" => {
            if let Some(delta) = event["delta"].as_str() {
                if !delta.is_empty() {
                    return Ok(Some(ApiStreamChunk::Reasoning {
                        text: delta.to_string(),
                        signature: None,
                    }));
                }
            }
            Ok(None)
        }

        // Refusal deltas
        "response.refusal.delta" => {
            if let Some(delta) = event["delta"].as_str() {
                if !delta.is_empty() {
                    return Ok(Some(ApiStreamChunk::Text {
                        text: format!("[Refusal] {}", delta),
                    }));
                }
            }
            Ok(None)
        }

        // Tool/function call deltas — emit ToolCallPartial for NativeToolCallParser
        // (matches TS behavior: yields tool_call_partial chunks)
        "response.function_call_arguments.delta" | "response.tool_call_arguments.delta" => {
            let call_id = event["call_id"]
                .as_str()
                .or_else(|| event["tool_call_id"].as_str())
                .or_else(|| event["id"].as_str());
            let name = event["name"]
                .as_str()
                .or_else(|| event["function_name"].as_str());
            let args = event["delta"]
                .as_str()
                .or_else(|| event["arguments"].as_str());
            let index = event["index"].as_u64().unwrap_or(0);

            // TS requires both id and name to be non-empty for a valid partial
            if let (Some(id), Some(n)) = (call_id, name) {
                if !id.is_empty() && !n.is_empty() {
                    return Ok(Some(ApiStreamChunk::ToolCallPartial {
                        index,
                        id: Some(id.to_string()),
                        name: Some(n.to_string()),
                        arguments: args.map(|s| s.to_string()),
                    }));
                }
            }
            Ok(None)
        }

        // Tool/function call completion
        "response.function_call_arguments.done" | "response.tool_call_arguments.done" => {
            Ok(None)
        }

        // Output item events
        "response.output_item.added" | "response.output_item.done" => {
            let item = &event["item"];
            if item.is_null() {
                return Ok(None);
            }
            let item_type = item["type"].as_str().unwrap_or("");

            match item_type {
                "text" | "output_text" => {
                    if let Some(text) = item["text"].as_str() {
                        if !text.is_empty() {
                            return Ok(Some(ApiStreamChunk::Text {
                                text: text.to_string(),
                            }));
                        }
                    }
                }
                "reasoning" => {
                    if let Some(text) = item["text"].as_str() {
                        if !text.is_empty() {
                            return Ok(Some(ApiStreamChunk::Reasoning {
                                text: text.to_string(),
                                signature: None,
                            }));
                        }
                    }
                    if let Some(summary) = item["summary"].as_array() {
                        for s in summary {
                            if s["type"] == "summary_text" {
                                if let Some(text) = s["text"].as_str() {
                                    if !text.is_empty() {
                                        return Ok(Some(ApiStreamChunk::Reasoning {
                                            text: text.to_string(),
                                            signature: None,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
                "message" => {
                    if let Some(content) = item["content"].as_array() {
                        for c in content {
                            if c["type"] == "text" || c["type"] == "output_text" {
                                if let Some(text) = c["text"].as_str() {
                                    if !text.is_empty() {
                                        return Ok(Some(ApiStreamChunk::Text {
                                            text: text.to_string(),
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
                "function_call" | "tool_call" => {
                    if event_type == "response.output_item.done" {
                        let call_id = item
                            .get("call_id")
                            .or_else(|| item.get("tool_call_id"))
                            .or_else(|| item.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let args_raw = item
                            .get("arguments")
                            .or_else(|| item.get("function").and_then(|f| f.get("arguments")))
                            .or_else(|| item.get("input"));
                        let args = match args_raw {
                            Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
                            Some(v) if v.is_object() => v.to_string(),
                            _ => String::new(),
                        };

                        if !call_id.is_empty() && !name.is_empty() {
                            return Ok(Some(ApiStreamChunk::ToolCall {
                                id: call_id.to_string(),
                                name: name.to_string(),
                                arguments: args,
                            }));
                        }
                    }
                }
                _ => {}
            }
            Ok(None)
        }

        // Completion events
        "response.completed" | "response.done" => {
            if let Some(output) = event
                .get("response")
                .and_then(|r| r.get("output"))
                .and_then(|o| o.as_array())
            {
                for output_item in output {
                    if output_item["type"] == "message" {
                        if let Some(content) = output_item["content"].as_array() {
                            for c in content {
                                if c["type"] == "output_text" || c["type"] == "text" {
                                    if let Some(text) = c["text"].as_str() {
                                        if !text.is_empty() {
                                            return Ok(Some(ApiStreamChunk::Text {
                                                text: text.to_string(),
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if output_item["type"] == "reasoning" {
                        if let Some(summary) = output_item["summary"].as_array() {
                            for s in summary {
                                if s["type"] == "summary_text" {
                                    if let Some(text) = s["text"].as_str() {
                                        if !text.is_empty() {
                                            return Ok(Some(ApiStreamChunk::Reasoning {
                                                text: text.to_string(),
                                                signature: None,
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let usage = event
                .get("response")
                .and_then(|r| r.get("usage"))
                .or_else(|| event.get("usage"));

            if let Some(usage) = usage {
                let chunk = roo_provider::transform::responses_api::normalize_usage(usage);
                return Ok(Some(chunk));
            }

            Ok(None)
        }

        // Error events
        "error" | "response.error" => {
            let msg = event["error"]["message"]
                .as_str()
                .or_else(|| event["message"].as_str())
                .unwrap_or("Unknown error");
            Err(ProviderError::api_error(provider_name, msg))
        }

        "response.failed" => {
            let msg = event["error"]["message"]
                .as_str()
                .or_else(|| event["message"].as_str())
                .unwrap_or("Response failed");
            Err(ProviderError::api_error(provider_name, msg))
        }

        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, MessageRole};

    fn make_message(role: MessageRole, content: Vec<ContentBlock>) -> roo_types::api::ApiMessage {
        roo_types::api::ApiMessage {
            role,
            content,
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    #[test]
    fn test_ensure_all_required_simple() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        });
        let result = ensure_all_required(&schema);
        let required = result["required"].as_array().unwrap();
        assert!(required.contains(&json!("name")));
        assert!(required.contains(&json!("age")));
        assert_eq!(result["additionalProperties"], json!(false));
    }

    #[test]
    fn test_ensure_all_required_nested() {
        let schema = json!({
            "type": "object",
            "properties": {
                "outer": {
                    "type": "object",
                    "properties": {
                        "inner": { "type": "string" }
                    }
                }
            }
        });
        let result = ensure_all_required(&schema);
        let outer = &result["properties"]["outer"];
        assert_eq!(outer["additionalProperties"], json!(false));
    }

    #[test]
    fn test_ensure_additional_properties_false_no_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let result = ensure_additional_properties_false(&schema);
        assert_eq!(result["additionalProperties"], json!(false));
        assert!(result.get("required").is_none());
    }

    #[test]
    fn test_ensure_all_required_non_object_passthrough() {
        let schema = json!("hello");
        let result = ensure_all_required(&schema);
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_is_mcp_tool() {
        assert!(is_mcp_tool("mcp__server__tool"));
        assert!(!is_mcp_tool("read_file"));
    }

    #[test]
    fn test_format_full_conversation_empty() {
        let result = format_full_conversation(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_full_conversation_user_text() {
        let msg = make_message(
            MessageRole::User,
            vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
        );
        let result = format_full_conversation(&[msg]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
    }

    #[test]
    fn test_build_request_body_minimal() {
        let params = RequestBodyParams {
            model_id: "gpt-5.1-codex-max",
            formatted_input: vec![json!({"role": "user", "content": []})],
            system_prompt: "You are helpful.",
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            reasoning_effort: None,
            enable_reasoning_summary: false,
            temperature: None,
            supports_temperature: None,
            max_output_tokens: None,
            supports_verbosity: None,
            verbosity: None,
            service_tier: None,
            prompt_cache_retention: None,
            stream: true,
        };
        let body = build_request_body(params);
        assert_eq!(body.model, "gpt-5.1-codex-max");
        assert!(body.stream);
        assert_eq!(body.instructions.as_deref(), Some("You are helpful."));
    }

    #[test]
    fn test_build_request_body_with_reasoning() {
        let params = RequestBodyParams {
            model_id: "o3",
            formatted_input: vec![],
            system_prompt: "test",
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            reasoning_effort: Some("high"),
            enable_reasoning_summary: true,
            temperature: None,
            supports_temperature: Some(false),
            max_output_tokens: Some(100_000),
            supports_verbosity: None,
            verbosity: None,
            service_tier: None,
            prompt_cache_retention: None,
            stream: true,
        };
        let body = build_request_body(params);
        let reasoning = body.reasoning.unwrap();
        assert_eq!(reasoning["effort"], "high");
        assert_eq!(reasoning["summary"], "auto");
        assert!(body.temperature.is_none());
        assert_eq!(body.max_output_tokens, Some(100_000));
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let tools = vec![json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }
            }
        })];

        let params = RequestBodyParams {
            model_id: "gpt-5.1",
            formatted_input: vec![],
            system_prompt: "test",
            tools: Some(&tools),
            tool_choice: Some(&json!("auto")),
            parallel_tool_calls: Some(true),
            reasoning_effort: None,
            enable_reasoning_summary: false,
            temperature: Some(0.5),
            supports_temperature: Some(true),
            max_output_tokens: None,
            supports_verbosity: None,
            verbosity: None,
            service_tier: Some("flex"),
            prompt_cache_retention: Some("24h"),
            stream: true,
        };
        let body = build_request_body(params);
        let tools = body.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "read_file");
        assert_eq!(tools[0]["strict"], true);
        assert_eq!(body.service_tier.as_deref(), Some("flex"));
    }

    #[test]
    fn test_build_request_body_mcp_tools_not_strict() {
        let tools = vec![json!({
            "type": "function",
            "function": {
                "name": "mcp__server__tool",
                "description": "MCP tool",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    }
                }
            }
        })];

        let params = RequestBodyParams {
            model_id: "gpt-5.1",
            formatted_input: vec![],
            system_prompt: "test",
            tools: Some(&tools),
            tool_choice: None,
            parallel_tool_calls: None,
            reasoning_effort: None,
            enable_reasoning_summary: false,
            temperature: None,
            supports_temperature: None,
            max_output_tokens: None,
            supports_verbosity: None,
            verbosity: None,
            service_tier: None,
            prompt_cache_retention: None,
            stream: true,
        };
        let body = build_request_body(params);
        let tools = body.tools.unwrap();
        assert_eq!(tools[0]["strict"], false);
        assert!(tools[0]["parameters"].get("required").is_none());
    }

    // -- SSE event parsing tests ----------------------------------------------

    #[test]
    fn test_parse_sse_event_text_delta() {
        let data = r#"{"type":"response.output_text.delta","delta":"Hello"}"#;
        let chunk = parse_sse_event(data, "test").unwrap().unwrap();
        assert!(matches!(chunk, ApiStreamChunk::Text { ref text } if text == "Hello"));
    }

    #[test]
    fn test_parse_sse_event_reasoning_delta() {
        let data = r#"{"type":"response.reasoning_summary_text.delta","delta":"thinking..."}"#;
        let chunk = parse_sse_event(data, "test").unwrap().unwrap();
        assert!(matches!(chunk, ApiStreamChunk::Reasoning { ref text, .. } if text == "thinking..."));
    }

    #[test]
    fn test_parse_sse_event_done_marker() {
        let chunk = parse_sse_event("[DONE]", "test").unwrap();
        assert!(chunk.is_none());
    }

    #[test]
    fn test_parse_sse_event_error() {
        let data = r#"{"type":"error","error":{"message":"Rate limited"}}"#;
        let result = parse_sse_event(data, "OpenAI Native");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sse_event_empty() {
        let chunk = parse_sse_event("", "test").unwrap();
        assert!(chunk.is_none());
    }

    #[test]
    fn test_parse_sse_event_usage() {
        let data = r#"{"type":"response.completed","response":{"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let chunk = parse_sse_event(data, "test").unwrap().unwrap();
        assert!(matches!(chunk, ApiStreamChunk::Usage { .. }));
    }

    #[test]
    fn test_parse_sse_event_unknown_type() {
        let data = r#"{"type":"response.web_search_call.searching"}"#;
        let chunk = parse_sse_event(data, "test").unwrap();
        assert!(chunk.is_none());
    }
}
