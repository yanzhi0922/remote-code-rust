//! DeepSeek R1 / Z.ai GLM message format conversion (merged module).
//!
//! Derived from `src/api/transform/r1-format.ts` and
//! `src/api/transform/zai-format.ts`.  The two TypeScript modules are
//! ~90 % identical; this single Rust module exposes a unified conversion
//! function parameterised by [`R1ZaiOptions`].
//!
//! Key features:
//! - Consecutive messages with the same role are **merged** (required by
//!   DeepSeek Reasoner and Z.ai GLM).
//! - `reasoning_content` is preserved on assistant messages for interleaved
//!   thinking mode.
//! - Optional `merge_tool_result_text` merges text that follows tool results
//!   into the last tool message (prevents reasoning_content from being
//!   dropped when the provider sees a user message).

use serde_json::{json, Value};

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options that control how messages are converted.
#[derive(Debug, Clone)]
pub struct R1ZaiOptions {
    /// When `true` and a user message contains tool results **followed by**
    /// text content, the text is appended to the last tool message instead of
    /// creating a new user message.
    ///
    /// This is critical for DeepSeek / Z.ai interleaved thinking because a
    /// user message causes the provider to drop `reasoning_content`.
    pub merge_tool_result_text: bool,

    /// When `true`, preserve `reasoning_content` on assistant messages.
    /// When `false`, reasoning content is stripped from the output.
    pub preserve_reasoning: bool,
}

impl Default for R1ZaiOptions {
    fn default() -> Self {
        Self {
            merge_tool_result_text: false,
            preserve_reasoning: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Converts Anthropic-style [`ApiMessage`]s into OpenAI-compatible JSON
/// messages suitable for DeepSeek R1 or Z.ai GLM endpoints.
///
/// # Message merging
/// Consecutive messages with the **same role** are merged by concatenating
/// their text content.  This is required because DeepSeek Reasoner does not
/// support successive messages with the same role.
///
/// # Reasoning preservation
/// When [`R1ZaiOptions::preserve_reasoning`] is `true`, the `reasoning`
/// field on [`ApiMessage`] is emitted as `reasoning_content` in the output
/// JSON (a non-standard extension used by DeepSeek / Z.ai).
///
/// # Tool-result text merging
/// When [`R1ZaiOptions::merge_tool_result_text`] is `true` and a user
/// message contains tool results followed by text, the text is merged into
/// the last tool message to avoid creating a user message that would cause
/// `reasoning_content` to be dropped.
pub fn convert_to_r1_zai_messages(messages: &[ApiMessage], options: R1ZaiOptions) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::new();

    for message in messages {
        let reasoning_content = if options.preserve_reasoning {
            message.reasoning.as_deref()
        } else {
            None
        };

        match message.role {
            MessageRole::User => {
                process_user_message(message, reasoning_content, &options, &mut result);
            }
            MessageRole::Assistant => {
                process_assistant_message(message, reasoning_content, &options, &mut result);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Process a user-role message.
fn process_user_message(
    message: &ApiMessage,
    _reasoning_content: Option<&str>,
    options: &R1ZaiOptions,
    result: &mut Vec<Value>,
) {
    // Partition content blocks
    let mut text_parts: Vec<String> = Vec::new();
    let mut image_parts: Vec<Value> = Vec::new();
    let mut tool_results: Vec<(String, String)> = Vec::new(); // (tool_use_id, content)

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::Image { source } => {
                let url = match source {
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    }
                    ImageSource::Url { url } => url.clone(),
                };
                image_parts.push(json!({
                    "type": "image_url",
                    "image_url": { "url": url }
                }));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let text = extract_tool_result_text(content);
                tool_results.push((tool_use_id.clone(), text));
            }
            // Skip thinking / redacted_thinking / tool_use in user messages
            _ => {}
        }
    }

    // Emit tool messages first (they must follow assistant tool_use)
    for (tool_use_id, content) in &tool_results {
        result.push(json!({
            "role": "tool",
            "tool_call_id": tool_use_id,
            "content": content,
        }));
    }

    // Handle text/image content after tool results
    if !text_parts.is_empty() || !image_parts.is_empty() {
        let should_merge = options.merge_tool_result_text
            && !tool_results.is_empty()
            && image_parts.is_empty();

        if should_merge {
            // Merge text into the last tool message
            if let Some(last) = result.last_mut() {
                if last["role"] == "tool" {
                    let additional = text_parts.join("\n");
                    let existing = last["content"].as_str().unwrap_or("").to_string();
                    last["content"] = json!(format!("{existing}\n\n{additional}"));
                }
            }
        } else {
            // Build user content
            let content = build_user_content(&text_parts, &image_parts);

            // Try to merge with last message if it's also a user message
            if let Some(last) = result.last_mut() {
                if last["role"] == "user" {
                    merge_user_content(last, &content);
                    return;
                }
            }
            result.push(json!({ "role": "user", "content": content }));
        }
    }
}

/// Process an assistant-role message.
fn process_assistant_message(
    message: &ApiMessage,
    reasoning_content: Option<&str>,
    options: &R1ZaiOptions,
    result: &mut Vec<Value>,
) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    let mut extracted_reasoning: Option<String> = None;

    for block in &message.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                let args = if input.is_string() {
                    input.as_str().unwrap_or_default().to_string()
                } else {
                    input.to_string()
                };
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": args,
                    }
                }));
            }
            ContentBlock::Thinking { thinking, .. } => {
                // Extract reasoning from thinking blocks (task stores it this way)
                if !thinking.is_empty() {
                    extracted_reasoning = Some(thinking.clone());
                }
            }
            // Skip redacted_thinking / tool_result / image in assistant
            _ => {}
        }
    }

    // Use reasoning from content blocks if not provided at top level
    let final_reasoning = reasoning_content
        .map(|s| s.to_string())
        .or(extracted_reasoning);

    let content_value = if text_parts.is_empty() {
        Value::Null
    } else {
        json!(text_parts.join("\n"))
    };

    let mut assistant_msg = json!({
        "role": "assistant",
        "content": content_value,
    });

    if !tool_calls.is_empty() {
        assistant_msg
            .as_object_mut()
            .expect("assistant_msg is always an object")
            .insert("tool_calls".to_string(), json!(tool_calls));
    }

    if options.preserve_reasoning {
        if let Some(ref reasoning) = final_reasoning {
            assistant_msg
                .as_object_mut()
                .expect("assistant_msg is always an object")
                .insert("reasoning_content".to_string(), json!(reasoning));
        }
    }

    // Try to merge with last message if it's also an assistant message
    // (only if neither has tool_calls)
    let can_merge = tool_calls.is_empty();
    if can_merge {
        if let Some(last) = result.last_mut() {
            if last["role"] == "assistant" && last.get("tool_calls").is_none() {
                // Merge text content
                let new_text = assistant_msg["content"].as_str().unwrap_or("");
                if !new_text.is_empty() {
                    let existing = last["content"].as_str().unwrap_or("");
                    if !existing.is_empty() {
                        last["content"] = json!(format!("{existing}\n{new_text}"));
                    } else {
                        last["content"] = json!(new_text);
                    }
                }
                // Preserve reasoning_content from the new message
                if options.preserve_reasoning {
                    if let Some(ref reasoning) = final_reasoning {
                        last.as_object_mut()
                            .expect("last is always an object")
                            .insert("reasoning_content".to_string(), json!(reasoning));
                    }
                }
                return;
            }
        }
    }

    result.push(assistant_msg);
}

/// Build the `content` field for a user message.
fn build_user_content(text_parts: &[String], image_parts: &[Value]) -> Value {
    if !image_parts.is_empty() {
        let mut parts: Vec<Value> = Vec::new();
        if !text_parts.is_empty() {
            parts.push(json!({ "type": "text", "text": text_parts.join("\n") }));
        }
        parts.extend(image_parts.iter().cloned());
        json!(parts)
    } else {
        json!(text_parts.join("\n"))
    }
}

/// Merge new user content into an existing user message.
fn merge_user_content(last: &mut Value, new_content: &Value) {
    if last["content"].is_string() && new_content.is_string() {
        let existing = last["content"].as_str().unwrap_or("");
        let new_text = new_content.as_str().unwrap_or("");
        last["content"] = json!(format!("{existing}\n{new_text}"));
    } else {
        // Convert both to arrays and concatenate
        let mut existing_parts = content_to_array(&last["content"]);
        let new_parts = content_to_array(new_content);
        existing_parts.extend(new_parts);
        last["content"] = json!(existing_parts);
    }
}

/// Convert a content value to a Vec of content parts.
fn content_to_array(content: &Value) -> Vec<Value> {
    if content.is_array() {
        content.as_array().unwrap().clone()
    } else if content.is_string() {
        vec![json!({ "type": "text", "text": content })]
    } else {
        vec![]
    }
}

/// Extract plain text from tool result content blocks.
fn extract_tool_result_text(content: &[ToolResultContent]) -> String {
    content
        .iter()
        .map(|c| match c {
            ToolResultContent::Text { text } => text.as_str(),
            ToolResultContent::Image { .. } => "(image)",
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

    // Helper builders

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

    fn make_message(role: MessageRole, content: Vec<ContentBlock>, reasoning: Option<&str>) -> ApiMessage {
        ApiMessage {
            role,
            content,
            reasoning: reasoning.map(|s| s.to_string()),
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

    // -- Tests ----------------------------------------------------------------

    #[test]
    fn test_consecutive_user_messages_merged() {
        let messages = vec![
            make_message(MessageRole::User, vec![text_block("Hello")], None),
            make_message(MessageRole::User, vec![text_block("World")], None),
        ];

        let result = convert_to_r1_zai_messages(&messages, R1ZaiOptions::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "Hello\nWorld");
    }

    #[test]
    fn test_consecutive_assistant_messages_merged() {
        let messages = vec![
            make_message(MessageRole::Assistant, vec![text_block("Part 1")], None),
            make_message(MessageRole::Assistant, vec![text_block("Part 2")], None),
        ];

        let result = convert_to_r1_zai_messages(&messages, R1ZaiOptions::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"], "Part 1\nPart 2");
    }

    #[test]
    fn test_assistant_with_tool_calls_not_merged() {
        let messages = vec![
            make_message(
                MessageRole::Assistant,
                vec![text_block("Let me check."), tool_use_block("call_1", "read_file", r#"{"path":"a.rs"}"#)],
                None,
            ),
            make_message(MessageRole::Assistant, vec![text_block("Done")], None),
        ];

        let result = convert_to_r1_zai_messages(&messages, R1ZaiOptions::default());
        // Second assistant should NOT merge into first (first has tool_calls)
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_reasoning_content_preserved() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![text_block("Thinking...")],
            Some("I need to analyze this"),
        )];

        let result = convert_to_r1_zai_messages(
            &messages,
            R1ZaiOptions {
                preserve_reasoning: true,
                ..Default::default()
            },
        );
        assert_eq!(result[0]["reasoning_content"], "I need to analyze this");
    }

    #[test]
    fn test_reasoning_content_stripped_when_disabled() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![text_block("Thinking...")],
            Some("I need to analyze this"),
        )];

        let result = convert_to_r1_zai_messages(
            &messages,
            R1ZaiOptions {
                preserve_reasoning: false,
                ..Default::default()
            },
        );
        assert!(result[0].get("reasoning_content").is_none());
    }

    #[test]
    fn test_merge_tool_result_text_into_tool_message() {
        let messages = vec![
            make_message(
                MessageRole::Assistant,
                vec![tool_use_block("call_1", "read_file", r#"{"path":"a.rs"}"#)],
                None,
            ),
            make_message(
                MessageRole::User,
                vec![
                    tool_result_block("call_1", "file contents"),
                    text_block("environment details here"),
                ],
                None,
            ),
        ];

        let result = convert_to_r1_zai_messages(
            &messages,
            R1ZaiOptions {
                merge_tool_result_text: true,
                ..Default::default()
            },
        );
        // Should have: assistant + tool (with merged text)
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["role"], "tool");
        let content = result[1]["content"].as_str().unwrap();
        assert!(content.contains("file contents"));
        assert!(content.contains("environment details here"));
    }

    #[test]
    fn test_tool_result_text_not_merged_when_disabled() {
        let messages = vec![
            make_message(
                MessageRole::Assistant,
                vec![tool_use_block("call_1", "read_file", r#"{"path":"a.rs"}"#)],
                None,
            ),
            make_message(
                MessageRole::User,
                vec![
                    tool_result_block("call_1", "file contents"),
                    text_block("environment details here"),
                ],
                None,
            ),
        ];

        let result = convert_to_r1_zai_messages(
            &messages,
            R1ZaiOptions {
                merge_tool_result_text: false,
                ..Default::default()
            },
        );
        // Should have: assistant + tool + user
        assert_eq!(result.len(), 3);
        assert_eq!(result[1]["role"], "tool");
        assert_eq!(result[2]["role"], "user");
    }

    #[test]
    fn test_empty_messages() {
        let result = convert_to_r1_zai_messages(&[], R1ZaiOptions::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_tool_result_with_image_placeholder() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: vec![
                    ToolResultContent::Text {
                        text: "text part".to_string(),
                    },
                    ToolResultContent::Image {
                        source: ImageSource::Url {
                            url: "https://example.com/img.png".to_string(),
                        },
                    },
                ],
                is_error: None,
            }],
            None,
        )];

        let result = convert_to_r1_zai_messages(&messages, R1ZaiOptions::default());
        assert_eq!(result.len(), 1);
        let content = result[0]["content"].as_str().unwrap();
        assert!(content.contains("text part"));
        assert!(content.contains("(image)"));
    }
}
