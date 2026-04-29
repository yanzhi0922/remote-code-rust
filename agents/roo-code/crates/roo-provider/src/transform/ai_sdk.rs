//! Vercel AI SDK format conversion utilities.
//!
//! Derived from `src/api/transform/ai-sdk.ts`.
//! Transforms between Anthropic/OpenAI formats and Vercel AI SDK `ModelMessage`
//! format.  These utilities are designed to be reused across different AI SDK
//! providers.

use serde_json::{json, Value};

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};

use crate::error::{ProviderError, Result};

// ---------------------------------------------------------------------------
// AiSdkStreamPart — stream event types
// ---------------------------------------------------------------------------

/// Stream part types emitted by the Vercel AI SDK.
///
/// This is a Rust representation of the AI SDK's `TextStreamPart` plus
/// extended event types (`text`, `reasoning`) that are emitted at runtime
/// but not included in the TypeScript type definitions.
#[derive(Debug, Clone, PartialEq)]
pub enum AiSdkStreamPart {
    /// Text content delta.
    Text {
        text: String,
    },
    /// Reasoning/thinking content delta.
    Reasoning {
        text: String,
    },
    /// Start of a tool call.
    ToolCallStart {
        id: String,
        name: String,
    },
    /// Delta content for a tool call.
    ToolCallDelta {
        id: String,
        delta: String,
    },
    /// End of a tool call.
    ToolCallEnd {
        id: String,
    },
    /// Complete tool call with all arguments.
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    /// Grounding source (e.g. from Gemini).
    Grounding {
        title: String,
        url: String,
    },
    /// Error during streaming.
    Error {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Converts Anthropic-style [`ApiMessage`]s into Vercel AI SDK `ModelMessage`
/// JSON objects.
///
/// # Format differences
/// - Tool results are emitted as separate `role: "tool"` messages (AI SDK
///   requirement).
/// - Tool use blocks become `tool-call` parts with `toolCallId` / `toolName`.
/// - Image sources are converted to data-URL or direct URL format.
///
/// Source: `src/api/transform/ai-sdk.ts` — `convertToAiSdkMessages`
pub fn convert_to_ai_sdk_messages(messages: &[ApiMessage]) -> Vec<Value> {
    // First pass: build a map of tool call IDs → tool names from assistant messages
    let mut tool_call_id_to_name = std::collections::HashMap::new();
    for message in messages {
        if message.role == MessageRole::Assistant {
            for block in &message.content {
                if let ContentBlock::ToolUse { id, name, .. } = block {
                    tool_call_id_to_name.insert(id.clone(), name.clone());
                }
            }
        }
    }

    let mut model_messages: Vec<Value> = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::User => {
                process_user_message_ai_sdk(message, &tool_call_id_to_name, &mut model_messages);
            }
            MessageRole::Assistant => {
                process_assistant_message_ai_sdk(message, &mut model_messages);
            }
        }
    }

    model_messages
}

/// Converts OpenAI-style function tool definitions to AI SDK tool format.
///
/// Each tool definition with `type: "function"` is converted to an object
/// with `description` and `inputSchema` fields.
///
/// Returns `None` if the input slice is empty.
///
/// Source: `src/api/transform/ai-sdk.ts` — `convertToolsForAiSdk`
pub fn convert_tools_for_ai_sdk(tools: &[Value]) -> Option<Vec<Value>> {
    if tools.is_empty() {
        return None;
    }

    let converted: Vec<Value> = tools
        .iter()
        .filter_map(|t| {
            if t.get("type").and_then(|v| v.as_str()) == Some("function") {
                let func = t.get("function")?;
                Some(json!({
                    "description": func.get("description").unwrap_or(&Value::Null),
                    "inputSchema": func.get("parameters").unwrap_or(&Value::Null),
                }))
            } else {
                None
            }
        })
        .collect();

    if converted.is_empty() {
        None
    } else {
        Some(converted)
    }
}

/// Parses a single AI SDK stream event JSON string and returns the
/// corresponding [`AiSdkStreamPart`], or `None` for lifecycle events that
/// don't need to be forwarded.
///
/// # Errors
/// Returns an error if `data` is not valid JSON or has an unexpected shape.
///
/// Source: `src/api/transform/ai-sdk.ts` — `processAiSdkStreamPart`
pub fn process_ai_sdk_stream_part(data: &str) -> Result<Option<AiSdkStreamPart>> {
    let value: Value = serde_json::from_str(data)
        .map_err(|e| ProviderError::ParseError(format!("Invalid JSON in stream part: {e}")))?;

    let event_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProviderError::ParseError("Missing 'type' field in stream part".into()))?;

    let part = match event_type {
        "text" | "text-delta" => value
            .get("text")
            .and_then(|v| v.as_str())
            .map(|text| AiSdkStreamPart::Text {
                text: text.to_string(),
            }),

        "reasoning" | "reasoning-delta" => value
            .get("text")
            .and_then(|v| v.as_str())
            .map(|text| AiSdkStreamPart::Reasoning {
                text: text.to_string(),
            }),

        "tool-input-start" => {
            let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = value.get("toolName").and_then(|v| v.as_str()).unwrap_or("");
            Some(AiSdkStreamPart::ToolCallStart {
                id: id.to_string(),
                name: name.to_string(),
            })
        }

        "tool-input-delta" => {
            let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let delta = value.get("delta").and_then(|v| v.as_str()).unwrap_or("");
            Some(AiSdkStreamPart::ToolCallDelta {
                id: id.to_string(),
                delta: delta.to_string(),
            })
        }

        "tool-input-end" => {
            let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
            Some(AiSdkStreamPart::ToolCallEnd {
                id: id.to_string(),
            })
        }

        "tool-call" => {
            let id = value
                .get("toolCallId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = value
                .get("toolName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let input = value.get("input");
            let arguments = match input {
                Some(v) if v.is_string() => v.as_str().unwrap_or_default().to_string(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            Some(AiSdkStreamPart::ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                arguments,
            })
        }

        "source" => {
            let url = value.get("url").and_then(|v| v.as_str());
            url.map(|url| AiSdkStreamPart::Grounding {
                title: value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Source")
                    .to_string(),
                url: url.to_string(),
            })
        }

        "error" => {
            let error_msg = match value.get("error") {
                Some(v) if v.is_string() => v.as_str().unwrap_or("Unknown error").to_string(),
                Some(v) => v.to_string(),
                None => "Unknown error".to_string(),
            };
            Some(AiSdkStreamPart::Error {
                message: error_msg,
            })
        }

        // Lifecycle events — skip
        "text-start" | "text-end" | "reasoning-start" | "reasoning-end"
        | "start-step" | "finish-step" | "start" | "finish" | "abort"
        | "file" | "tool-result" | "tool-error" | "raw" => None,

        _ => None,
    };

    Ok(part)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Process a user-role message for AI SDK conversion.
fn process_user_message_ai_sdk(
    message: &ApiMessage,
    tool_call_id_to_name: &std::collections::HashMap<String, String>,
    out: &mut Vec<Value>,
) {
    let mut parts: Vec<Value> = Vec::new();
    let mut tool_results: Vec<Value> = Vec::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                parts.push(json!({ "type": "text", "text": text }));
            }
            ContentBlock::Image { source } => {
                match source {
                    ImageSource::Base64 { media_type, data } => {
                        parts.push(json!({
                            "type": "image",
                            "image": format!("data:{media_type};base64,{data}"),
                            "mimeType": media_type,
                        }));
                    }
                    ImageSource::Url { url } => {
                        parts.push(json!({
                            "type": "image",
                            "image": url,
                        }));
                    }
                }
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let content_str = extract_tool_result_text(content);
                let tool_name = tool_call_id_to_name
                    .get(tool_use_id)
                    .cloned()
                    .unwrap_or_else(|| "unknown_tool".to_string());
                tool_results.push(json!({
                    "type": "tool-result",
                    "toolCallId": tool_use_id,
                    "toolName": tool_name,
                    "output": { "type": "text", "value": if content_str.is_empty() { "(empty)" } else { &content_str } },
                }));
            }
            // Skip thinking / redacted_thinking / tool_use in user messages
            _ => {}
        }
    }

    // AI SDK requires tool results in separate "tool" role messages
    if !tool_results.is_empty() {
        out.push(json!({
            "role": "tool",
            "content": tool_results,
        }));
    }

    // Add user message with only text/image content
    if !parts.is_empty() {
        out.push(json!({
            "role": "user",
            "content": parts,
        }));
    }
}

/// Process an assistant-role message for AI SDK conversion.
fn process_assistant_message_ai_sdk(message: &ApiMessage, out: &mut Vec<Value>) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "type": "tool-call",
                    "toolCallId": id,
                    "toolName": name,
                    "input": input,
                }));
            }
            // Skip thinking / redacted_thinking / tool_result / image in assistant
            _ => {}
        }
    }

    let mut content: Vec<Value> = Vec::new();
    if !text_parts.is_empty() {
        content.push(json!({ "type": "text", "text": text_parts.join("\n") }));
    }
    content.extend(tool_calls);

    if content.is_empty() {
        content.push(json!({ "type": "text", "text": "" }));
    }

    out.push(json!({
        "role": "assistant",
        "content": content,
    }));
}

/// Extract plain text from a slice of [`ToolResultContent`].
fn extract_tool_result_text(content: &[ToolResultContent]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            ToolResultContent::Image { .. } => Some("(image)"),
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, ImageSource, ToolResultContent};

    fn make_user_message(content: Vec<ContentBlock>) -> ApiMessage {
        ApiMessage {
            role: MessageRole::User,
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

    fn make_assistant_message(content: Vec<ContentBlock>) -> ApiMessage {
        ApiMessage {
            role: MessageRole::Assistant,
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
    fn test_convert_text_only_user_message() {
        let messages = vec![make_user_message(vec![ContentBlock::Text {
            text: "hello".to_string(),
        }])];

        let result = convert_to_ai_sdk_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "hello");
    }

    #[test]
    fn test_convert_assistant_with_tool_use() {
        let messages = vec![make_assistant_message(vec![
            ContentBlock::Text {
                text: "Let me help".to_string(),
            },
            ContentBlock::ToolUse {
                id: "call_123".to_string(),
                name: "read_file".to_string(),
                input: json!({"path": "test.rs"}),
            },
        ])];

        let result = convert_to_ai_sdk_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "tool-call");
        assert_eq!(content[1]["toolCallId"], "call_123");
    }

    #[test]
    fn test_convert_tool_result_to_tool_role() {
        let messages = vec![
            make_assistant_message(vec![ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                input: json!({}),
            }]),
            make_user_message(vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "file contents".to_string(),
                }],
                is_error: None,
            }]),
        ];

        let result = convert_to_ai_sdk_messages(&messages);
        // Assistant + tool role message
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["role"], "tool");
        let tool_content = result[1]["content"].as_array().unwrap();
        assert_eq!(tool_content[0]["type"], "tool-result");
        assert_eq!(tool_content[0]["toolCallId"], "call_1");
        assert_eq!(tool_content[0]["toolName"], "read_file");
    }

    #[test]
    fn test_convert_image_base64() {
        let messages = vec![make_user_message(vec![
            ContentBlock::Text {
                text: "See image".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgo=".to_string(),
                },
            },
        ])];

        let result = convert_to_ai_sdk_messages(&messages);
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[1]["type"], "image");
        assert!(content[1]["image"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_convert_tools_for_ai_sdk() {
        let tools = vec![json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}
            }
        })];

        let result = convert_tools_for_ai_sdk(&tools).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["description"], "Read a file");
    }

    #[test]
    fn test_convert_tools_empty() {
        assert!(convert_tools_for_ai_sdk(&[]).is_none());
    }

    #[test]
    fn test_process_stream_text_delta() {
        let part = process_ai_sdk_stream_part(r#"{"type":"text-delta","text":"hello"}"#)
            .unwrap();
        assert_eq!(
            part,
            Some(AiSdkStreamPart::Text {
                text: "hello".to_string(),
            })
        );
    }

    #[test]
    fn test_process_stream_tool_call() {
        let part = process_ai_sdk_stream_part(
            r#"{"type":"tool-call","toolCallId":"id1","toolName":"read_file","input":{"path":"a.rs"}}"#,
        )
        .unwrap();
        assert_eq!(
            part,
            Some(AiSdkStreamPart::ToolCall {
                id: "id1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a.rs"}"#.to_string(),
            })
        );
    }

    #[test]
    fn test_process_stream_lifecycle_event_skipped() {
        let part = process_ai_sdk_stream_part(r#"{"type":"finish","usage":{"promptTokens":10}}"#)
            .unwrap();
        assert!(part.is_none());
    }

    #[test]
    fn test_process_stream_error() {
        let part =
            process_ai_sdk_stream_part(r#"{"type":"error","error":"Rate limit exceeded"}"#)
                .unwrap();
        assert_eq!(
            part,
            Some(AiSdkStreamPart::Error {
                message: "Rate limit exceeded".to_string(),
            })
        );
    }

    #[test]
    fn test_process_stream_invalid_json() {
        let result = process_ai_sdk_stream_part("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_process_stream_reasoning() {
        let part =
            process_ai_sdk_stream_part(r#"{"type":"reasoning","text":"thinking..."}"#).unwrap();
        assert_eq!(
            part,
            Some(AiSdkStreamPart::Reasoning {
                text: "thinking...".to_string(),
            })
        );
    }
}
