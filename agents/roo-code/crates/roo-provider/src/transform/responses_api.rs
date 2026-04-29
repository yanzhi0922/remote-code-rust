//! OpenAI Responses API format conversion (input + stream parsing).
//!
//! Derived from `src/api/transform/responses-api-input.ts` and
//! `src/api/transform/responses-api-stream.ts`.
//!
//! # Input conversion
//! [`convert_to_responses_api_input`] transforms Anthropic-style
//! [`ApiMessage`]s into the JSON structure required by the
//! **Responses API** (`POST /v1/responses`):
//! - `input_text` / `input_image` for user content
//! - `function_call` for tool use
//! - `function_call_output` for tool results
//!
//! # Stream parsing
//! [`ResponseApiEvent`] enumerates the relevant SSE event types and
//! [`parse_responses_api_stream`] parses a single SSE data payload into an
//! optional event. [`normalize_usage`] converts a raw usage JSON object into
//! an [`ApiStreamChunk`] with standardised token counts.

use serde_json::{json, Value};

use roo_types::api::{
    ApiMessage, ApiStreamChunk, ContentBlock, ImageSource, MessageRole, ToolResultContent,
};

use crate::error::Result;

// ---------------------------------------------------------------------------
// Input conversion
// ---------------------------------------------------------------------------

/// Converts Anthropic-style [`ApiMessage`]s into Responses API input items.
///
/// # Key differences from Chat Completions format
/// - Content parts use `{ "type": "input_text" }` instead of `{ "type": "text" }`
/// - Images use `{ "type": "input_image" }` instead of `{ "type": "image_url" }`
/// - Tool results become `{ "type": "function_call_output", "call_id": "…" }`
/// - Tool uses become `{ "type": "function_call", "call_id": "…" }`
/// - System prompt goes via the `instructions` parameter, not as a message
///
/// # Output
/// Each element in the returned `Vec<Value>` is one of:
/// - `{ "role": "user", "content": [ { "type": "input_text", … } ] }`
/// - `{ "type": "message", "role": "assistant", "content": [ { "type": "output_text", … } ] }`
/// - `{ "type": "function_call", "call_id": "…", "name": "…", "arguments": "…" }`
/// - `{ "type": "function_call_output", "call_id": "…", "output": "…" }`
pub fn convert_to_responses_api_input(messages: &[ApiMessage]) -> Vec<Value> {
    let mut input: Vec<Value> = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::Assistant => {
                process_assistant_for_input(message, &mut input);
            }
            MessageRole::User => {
                process_user_for_input(message, &mut input);
            }
        }
    }

    input
}

/// Process an assistant message for Responses API input.
fn process_assistant_for_input(message: &ApiMessage, input: &mut Vec<Value>) {
    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                input.push(json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": text }]
                }));
            }
            ContentBlock::ToolUse { id, name, input: tool_input } => {
                let args = if tool_input.is_string() {
                    tool_input.as_str().unwrap_or_default().to_string()
                } else {
                    tool_input.to_string()
                };
                input.push(json!({
                    "type": "function_call",
                    "call_id": id,
                    "name": name,
                    "arguments": args,
                }));
            }
            ContentBlock::Thinking { thinking, .. } => {
                if !thinking.trim().is_empty() {
                    input.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": format!("[Thinking] {thinking}") }]
                    }));
                }
            }
            // Skip redacted_thinking / tool_result / image in assistant
            _ => {}
        }
    }
}

/// Process a user message for Responses API input.
fn process_user_for_input(message: &ApiMessage, input: &mut Vec<Value>) {
    let mut content_parts: Vec<Value> = Vec::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                content_parts.push(json!({ "type": "input_text", "text": text }));
            }
            ContentBlock::Image { source } => {
                let url = match source {
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    }
                    ImageSource::Url { url } => url.clone(),
                };
                content_parts.push(json!({
                    "type": "input_image",
                    "detail": "auto",
                    "image_url": url,
                }));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                // Flush pending user content before the tool result
                if !content_parts.is_empty() {
                    input.push(json!({ "role": "user", "content": content_parts }));
                    content_parts = Vec::new();
                }

                let output = extract_tool_result_output(content);
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_use_id,
                    "output": output,
                }));
            }
            // Skip thinking / redacted_thinking / tool_use in user messages
            _ => {}
        }
    }

    // Flush remaining user content
    if !content_parts.is_empty() {
        input.push(json!({ "role": "user", "content": content_parts }));
    }
}

/// Extract output text from tool result content, defaulting to "(empty)".
fn extract_tool_result_output(content: &[ToolResultContent]) -> String {
    let text: String = content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            ToolResultContent::Image { .. } => None,
        })
        .collect::<Vec<&str>>()
        .join("\n");

    if text.is_empty() {
        "(empty)".to_string()
    } else {
        text
    }
}

// ---------------------------------------------------------------------------
// Stream parsing
// ---------------------------------------------------------------------------

/// A parsed event from the Responses API SSE stream.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseApiEvent {
    /// Incremental text delta.
    TextDelta {
        /// The text fragment.
        delta: String,
    },

    /// Incremental reasoning/thinking delta.
    ReasoningDelta {
        /// The reasoning text fragment.
        delta: String,
    },

    /// A completed function/tool call.
    FunctionCall {
        /// The call ID.
        id: String,
        /// The function name.
        name: String,
        /// The JSON arguments string.
        arguments: String,
    },

    /// Token usage information.
    Usage {
        /// Number of input tokens.
        input_tokens: u64,
        /// Number of output tokens.
        output_tokens: u64,
    },
}

/// Parses a single SSE `data:` payload from the Responses API stream.
///
/// The `data` parameter should be the raw JSON string after the `data: `
/// prefix (without leading/trailing whitespace).
///
/// Returns `Ok(None)` for unrecognised or ignorable events.
///
/// # Recognised event types
/// - `response.output_text.delta` / `response.text.delta` → [`TextDelta`]
/// - `response.reasoning_text.delta` / `response.reasoning.delta` /
///   `response.reasoning_summary_text.delta` / `response.reasoning_summary.delta`
///   → [`ReasoningDelta`]
/// - `response.output_item.done` (with `function_call` or `tool_call` item)
///   → [`FunctionCall`]
/// - `response.completed` / `response.done` → [`Usage`]
///
/// [`TextDelta`]: ResponseApiEvent::TextDelta
/// [`ReasoningDelta`]: ResponseApiEvent::ReasoningDelta
/// [`FunctionCall`]: ResponseApiEvent::FunctionCall
/// [`Usage`]: ResponseApiEvent::Usage
pub fn parse_responses_api_stream(data: &str) -> Result<Option<ResponseApiEvent>> {
    let trimmed = data.trim();

    // Skip empty or "[DONE]" markers
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(None);
    }

    let event: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Ok(None), // Not valid JSON, skip
    };

    let event_type = event["type"].as_str().unwrap_or("");

    // Text content deltas
    if event_type == "response.output_text.delta" || event_type == "response.text.delta" {
        if let Some(delta) = event["delta"].as_str() {
            if !delta.is_empty() {
                return Ok(Some(ResponseApiEvent::TextDelta {
                    delta: delta.to_string(),
                }));
            }
        }
        return Ok(None);
    }

    // Reasoning deltas
    if event_type == "response.reasoning_text.delta"
        || event_type == "response.reasoning.delta"
        || event_type == "response.reasoning_summary_text.delta"
        || event_type == "response.reasoning_summary.delta"
    {
        if let Some(delta) = event["delta"].as_str() {
            if !delta.is_empty() {
                return Ok(Some(ResponseApiEvent::ReasoningDelta {
                    delta: delta.to_string(),
                }));
            }
        }
        return Ok(None);
    }

    // Completed function calls
    if event_type == "response.output_item.done" {
        let item = &event["item"];
        let item_type = item["type"].as_str().unwrap_or("");
        if item_type == "function_call" || item_type == "tool_call" {
            let call_id = item
                .get("call_id")
                .or_else(|| item.get("tool_call_id"))
                .or_else(|| item.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = item
                .get("name")
                .or_else(|| item.get("function").and_then(|f| f.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
                return Ok(Some(ResponseApiEvent::FunctionCall {
                    id: call_id.to_string(),
                    name: name.to_string(),
                    arguments: args,
                }));
            }
        }
        return Ok(None);
    }

    // Completion events — extract usage
    if event_type == "response.completed" || event_type == "response.done" {
        let usage = event
            .get("response")
            .and_then(|r| r.get("usage"))
            .or_else(|| event.get("usage"));

        if let Some(usage) = usage {
            let chunk = normalize_usage(usage);
            if let ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                ..
            } = chunk
            {
                return Ok(Some(ResponseApiEvent::Usage {
                    input_tokens,
                    output_tokens,
                }));
            }
        }
        return Ok(None);
    }

    Ok(None)
}

/// Normalises a raw usage JSON object into an [`ApiStreamChunk::Usage`].
///
/// The input `usage` value may use various field names depending on the
/// provider:
/// - `input_tokens` or `prompt_tokens`
/// - `output_tokens` or `completion_tokens`
/// - `cache_read_input_tokens` or nested `cached_tokens`
/// - `cache_creation_input_tokens` or `cache_write_tokens`
/// - `output_tokens_details.reasoning_tokens`
pub fn normalize_usage(usage: &Value) -> ApiStreamChunk {
    let input_details = usage
        .get("input_tokens_details")
        .or_else(|| usage.get("prompt_tokens_details"));
    let cached_tokens = input_details
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(cached_tokens);

    let cache_write_tokens = usage
        .get("cache_creation_input_tokens")
        .or_else(|| usage.get("cache_write_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let reasoning_tokens = usage
        .get("output_tokens_details")
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(|v| v.as_u64());

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
        reasoning_tokens,
        total_cost: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, MessageRole};

    // -- Helpers --------------------------------------------------------------

    fn text_block(text: &str) -> ContentBlock {
        ContentBlock::Text {
            text: text.to_string(),
        }
    }

    fn tool_use_block(id: &str, name: &str, input: &str) -> ContentBlock {
        ContentBlock::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input: serde_json::from_str(input).unwrap_or(json!(input)),
        }
    }

    fn tool_result_block(tool_use_id: &str, text: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![ToolResultContent::Text {
                text: text.to_string(),
            }],
            is_error: None,
        }
    }

    fn make_message(role: MessageRole, content: Vec<ContentBlock>) -> ApiMessage {
        ApiMessage {
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

    // -- Input conversion tests -----------------------------------------------

    #[test]
    fn test_input_simple_user_text() {
        let messages = vec![make_message(MessageRole::User, vec![text_block("Hello")])];
        let result = convert_to_responses_api_input(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "input_text");
        assert_eq!(content[0]["text"], "Hello");
    }

    #[test]
    fn test_input_assistant_text() {
        let messages = vec![make_message(MessageRole::Assistant, vec![text_block("Hi!")])];
        let result = convert_to_responses_api_input(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "message");
        assert_eq!(result[0]["role"], "assistant");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "output_text");
    }

    #[test]
    fn test_input_tool_use_becomes_function_call() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![tool_use_block("call_1", "read_file", r#"{"path":"a.rs"}"#)],
        )];
        let result = convert_to_responses_api_input(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "function_call");
        assert_eq!(result[0]["call_id"], "call_1");
        assert_eq!(result[0]["name"], "read_file");
    }

    #[test]
    fn test_input_tool_result_becomes_function_call_output() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![tool_result_block("call_1", "file contents")],
        )];
        let result = convert_to_responses_api_input(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "function_call_output");
        assert_eq!(result[0]["call_id"], "call_1");
        assert_eq!(result[0]["output"], "file contents");
    }

    #[test]
    fn test_input_user_text_flushed_before_tool_result() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![
                text_block("Some text"),
                tool_result_block("call_1", "result"),
                text_block("After result"),
            ],
        )];
        let result = convert_to_responses_api_input(&messages);
        // Should be: user(input_text) + function_call_output + user(input_text)
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[1]["type"], "function_call_output");
        assert_eq!(result[2]["role"], "user");
    }

    // -- Stream parsing tests -------------------------------------------------

    #[test]
    fn test_parse_text_delta() {
        let data = r#"{"type":"response.output_text.delta","delta":"Hello"}"#;
        let event = parse_responses_api_stream(data).unwrap().unwrap();
        assert_eq!(
            event,
            ResponseApiEvent::TextDelta {
                delta: "Hello".to_string()
            }
        );
    }

    #[test]
    fn test_parse_reasoning_delta() {
        let data = r#"{"type":"response.reasoning_text.delta","delta":"thinking..."}"#;
        let event = parse_responses_api_stream(data).unwrap().unwrap();
        assert_eq!(
            event,
            ResponseApiEvent::ReasoningDelta {
                delta: "thinking...".to_string()
            }
        );
    }

    #[test]
    fn test_parse_function_call_done() {
        let data = r#"{"type":"response.output_item.done","item":{"type":"function_call","call_id":"call_1","name":"read_file","arguments":"{\"path\":\"a.rs\"}"}}"#;
        let event = parse_responses_api_stream(data).unwrap().unwrap();
        match event {
            ResponseApiEvent::FunctionCall { id, name, arguments } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
                assert!(arguments.contains("path"));
            }
            _ => panic!("Expected FunctionCall"),
        }
    }

    #[test]
    fn test_parse_completed_with_usage() {
        let data = r#"{"type":"response.completed","response":{"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let event = parse_responses_api_stream(data).unwrap().unwrap();
        assert_eq!(
            event,
            ResponseApiEvent::Usage {
                input_tokens: 100,
                output_tokens: 50
            }
        );
    }

    #[test]
    fn test_parse_done_marker_returns_none() {
        let event = parse_responses_api_stream("[DONE]").unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_invalid_json_returns_none() {
        let event = parse_responses_api_stream("not json").unwrap();
        assert!(event.is_none());
    }

    // -- normalize_usage tests ------------------------------------------------

    #[test]
    fn test_normalize_usage_basic() {
        let usage = json!({
            "input_tokens": 100,
            "output_tokens": 50
        });
        let chunk = normalize_usage(&usage);
        match chunk {
            ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 50);
            }
            _ => panic!("Expected Usage"),
        }
    }

    #[test]
    fn test_normalize_usage_with_cache_tokens() {
        let usage = json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 80,
            "cache_creation_input_tokens": 20
        });
        let chunk = normalize_usage(&usage);
        match chunk {
            ApiStreamChunk::Usage {
                cache_read_tokens,
                cache_write_tokens,
                ..
            } => {
                assert_eq!(cache_read_tokens, Some(80));
                assert_eq!(cache_write_tokens, Some(20));
            }
            _ => panic!("Expected Usage"),
        }
    }

    #[test]
    fn test_normalize_usage_with_reasoning_tokens() {
        let usage = json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "output_tokens_details": {
                "reasoning_tokens": 30
            }
        });
        let chunk = normalize_usage(&usage);
        match chunk {
            ApiStreamChunk::Usage {
                reasoning_tokens, ..
            } => {
                assert_eq!(reasoning_tokens, Some(30));
            }
            _ => panic!("Expected Usage"),
        }
    }
}
