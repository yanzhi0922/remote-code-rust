//! Mistral message format conversion.
//!
//! Derived from `src/api/transform/mistral-format.ts`.
//! Converts Anthropic-style [`ApiMessage`] into Mistral-compatible JSON objects.
//! Mistral has strict requirements on tool-call IDs (exactly 9 alphanumeric
//! characters) and message ordering (user → assistant → tool → assistant).

use serde_json::{json, Value};

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Normalises a tool-call ID to Mistral's requirements.
///
/// Mistral requires tool-call IDs to consist of **only** alphanumeric
/// characters (`a-z`, `A-Z`, `0-9`) and be **exactly 9 characters** long.
///
/// # Algorithm
/// 1. Strip every non-alphanumeric character from `id`.
/// 2. If the remaining string is ≥ 9 chars, take the first 9.
/// 3. Otherwise, right-pad with `'0'` to reach length 9.
///
/// # Examples
/// ```
/// use roo_provider::transform::mistral_format::normalize_mistral_tool_call_id;
///
/// assert_eq!(normalize_mistral_tool_call_id("call_5019f900a247472bacde0b82"), "call5019f");
/// assert_eq!(normalize_mistral_tool_call_id("toolu_123"),  "toolu1230");
/// assert_eq!(normalize_mistral_tool_call_id("abc"),        "abc000000");
/// ```
pub fn normalize_mistral_tool_call_id(id: &str) -> String {
    let alphanumeric: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

    if alphanumeric.len() >= 9 {
        alphanumeric[..9].to_string()
    } else {
        // pad with '0' on the right
        let mut padded = alphanumeric;
        while padded.len() < 9 {
            padded.push('0');
        }
        padded
    }
}

/// Converts a slice of [`ApiMessage`] into Mistral-compatible JSON messages.
///
/// # Mistral message ordering rules
/// - The sequence must follow: **user → assistant → tool → assistant**.
/// - Tool messages **must** immediately follow an assistant message.
/// - When a user message contains both `tool_result` blocks and text/image
///   content, the tool results are emitted as `tool` messages and any
///   non-tool user content is **skipped** (Mistral does not allow user
///   messages after tool messages).
///
/// # Output shape
/// Each returned [`Value`] is a JSON object with at least a `"role"` key.
/// - **system / user (text-only):** `{ "role": "…", "content": "…" }`
/// - **user (with images):** `{ "role": "user", "content": [ … ] }`
/// - **assistant:** `{ "role": "assistant", "content": "…", "toolCalls": [ … ] }`
/// - **tool:** `{ "role": "tool", "toolCallId": "…", "content": "…" }`
pub fn convert_to_mistral_messages(messages: &[ApiMessage]) -> Vec<Value> {
    let mut mistral_messages: Vec<Value> = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::User => process_user_message(message, &mut mistral_messages),
            MessageRole::Assistant => process_assistant_message(message, &mut mistral_messages),
        }
    }

    mistral_messages
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Handle a user-role [`ApiMessage`].
fn process_user_message(message: &ApiMessage, out: &mut Vec<Value>) {
    let mut non_tool_parts: Vec<Value> = Vec::new();
    let mut tool_results: Vec<Value> = Vec::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                non_tool_parts.push(json!({ "type": "text", "text": text }));
            }
            ContentBlock::Image { source } => {
                let data_url = match source {
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    }
                    ImageSource::Url { url } => url.clone(),
                };
                non_tool_parts.push(json!({
                    "type": "image_url",
                    "imageUrl": { "url": data_url }
                }));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let result_text = extract_tool_result_text(content);
                tool_results.push(json!({
                    "role": "tool",
                    "toolCallId": normalize_mistral_tool_call_id(tool_use_id),
                    "content": result_text,
                }));
            }
            // Skip thinking / redacted_thinking / tool_use in user messages
            _ => {}
        }
    }

    // Mistral: tool results must follow assistant; if there are tool results
    // we skip non-tool user content to maintain valid ordering.
    if !tool_results.is_empty() {
        out.extend(tool_results);
    } else if !non_tool_parts.is_empty() {
        // Emit as user message
        if non_tool_parts.len() == 1 {
            // Single text part → simple string content
            if let Some(text) = non_tool_parts[0].get("text").and_then(|t| t.as_str()) {
                out.push(json!({
                    "role": "user",
                    "content": text,
                }));
                return;
            }
        }
        out.push(json!({
            "role": "user",
            "content": non_tool_parts,
        }));
    }
}

/// Handle an assistant-role [`ApiMessage`].
fn process_assistant_message(message: &ApiMessage, out: &mut Vec<Value>) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                let args = if input.is_string() {
                    input.clone()
                } else {
                    json!(input.to_string())
                };
                tool_calls.push(json!({
                    "id": normalize_mistral_tool_call_id(id),
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": args,
                    }
                }));
            }
            // Skip thinking / redacted_thinking / tool_result / image in assistant
            _ => {}
        }
    }

    let content = if text_parts.is_empty() {
        Value::Null
    } else {
        json!(text_parts.join("\n"))
    };

    let mut assistant_msg = json!({
        "role": "assistant",
        "content": content,
    });

    if !tool_calls.is_empty() {
        assistant_msg
            .as_object_mut()
            .expect("assistant_msg is always an object")
            .insert("toolCalls".to_string(), json!(tool_calls));
    }

    out.push(assistant_msg);
}

/// Extract a plain-text string from a slice of [`ToolResultContent`].
fn extract_tool_result_text(content: &[ToolResultContent]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            ToolResultContent::Image { .. } => None,
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
    use roo_types::api::{ContentBlock, MessageRole};

    // -- normalize_mistral_tool_call_id ---------------------------------------

    #[test]
    fn test_normalize_id_long_alphanumeric() {
        // "call_5019f900a247472bacde0b82" → strip '_' → "call5019f900a247472bacde0b82" → first 9
        assert_eq!(
            normalize_mistral_tool_call_id("call_5019f900a247472bacde0b82"),
            "call5019f"
        );
    }

    #[test]
    fn test_normalize_id_short() {
        // "toolu_123" → "toolu123" → pad to "toolu1230"
        assert_eq!(normalize_mistral_tool_call_id("toolu_123"), "toolu1230");
    }

    #[test]
    fn test_normalize_id_already_9_chars() {
        assert_eq!(normalize_mistral_tool_call_id("abc123XYZ"), "abc123XYZ");
    }

    #[test]
    fn test_normalize_id_very_short() {
        assert_eq!(normalize_mistral_tool_call_id("ab"), "ab0000000");
    }

    #[test]
    fn test_normalize_id_special_chars_only() {
        // All non-alphanumeric → empty → pad to "000000000"
        assert_eq!(normalize_mistral_tool_call_id("---"), "000000000");
    }

    // -- convert_to_mistral_messages ------------------------------------------

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

    #[test]
    fn test_simple_user_assistant_conversation() {
        let messages = vec![
            ApiMessage {
                role: MessageRole::User,
                content: vec![text_block("Hello")],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
            ApiMessage {
                role: MessageRole::Assistant,
                content: vec![text_block("Hi there!")],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
        ];

        let result = convert_to_mistral_messages(&messages);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "Hello");
        assert_eq!(result[1]["role"], "assistant");
        assert_eq!(result[1]["content"], "Hi there!");
    }

    #[test]
    fn test_tool_use_and_result() {
        let messages = vec![
            // Assistant with tool_use
            ApiMessage {
                role: MessageRole::Assistant,
                content: vec![
                    text_block("Let me check."),
                    tool_use_block("call_abc123", "read_file", r#"{"path":"test.rs"}"#),
                ],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
            // User with tool_result
            ApiMessage {
                role: MessageRole::User,
                content: vec![tool_result_block("call_abc123", "file contents here")],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
            reasoning_details: None,
            },
        ];

        let result = convert_to_mistral_messages(&messages);
        assert_eq!(result.len(), 2);

        // Assistant message with toolCalls
        assert_eq!(result[0]["role"], "assistant");
        let tool_calls = result[0]["toolCalls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], normalize_mistral_tool_call_id("call_abc123"));

        // Tool message
        assert_eq!(result[1]["role"], "tool");
        assert_eq!(result[1]["content"], "file contents here");
    }

    #[test]
    fn test_tool_result_skips_non_tool_content() {
        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: vec![
                tool_result_block("toolu_1", "result text"),
                text_block("This should be skipped"),
            ],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];

        let result = convert_to_mistral_messages(&messages);
        // Only the tool message should appear; text is skipped
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
    }

    #[test]
    fn test_assistant_with_no_text_but_tool_calls() {
        let messages = vec![ApiMessage {
            role: MessageRole::Assistant,
            content: vec![tool_use_block("call_001", "list_files", r#"{"path":"."}"#)],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];

        let result = convert_to_mistral_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        // content should be null (no text parts)
        assert!(result[0]["content"].is_null());
        // toolCalls should be present
        assert!(result[0]["toolCalls"].is_array());
    }

    #[test]
    fn test_empty_messages() {
        let result = convert_to_mistral_messages(&[]);
        assert!(result.is_empty());
    }
}
