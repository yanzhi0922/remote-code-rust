//! VSCode Language Model format conversion utilities.
//!
//! Derived from `src/api/transform/vscode-lm-format.ts`.
//!
//! Converts between Anthropic-style messages and VSCode Language Model
//! chat message format. Since the Rust version doesn't depend on VSCode APIs,
//! we define equivalent types that mirror the VSCode LM API structure.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use roo_types::api::{ContentBlock, ImageSource, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// VSCode LM Types (mirroring vscode.LanguageModelChatMessage)
// ---------------------------------------------------------------------------

/// Role of a chat message participant.
///
/// Mirrors `vscode.LanguageModelChatMessageRole`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VsCodeLmRole {
    User,
    Assistant,
}

/// A content part in a VSCode LM chat message.
///
/// Mirrors the various `vscode.LanguageModel*Part` types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VsCodeLmContentPart {
    /// Text content part (mirrors `vscode.LanguageModelTextPart`).
    #[serde(rename = "text")]
    Text { value: String },

    /// Tool result part (mirrors `vscode.LanguageModelToolResultPart`).
    #[serde(rename = "tool_result")]
    ToolResult {
        call_id: String,
        content: Vec<VsCodeLmContentPart>,
    },

    /// Tool call part (mirrors `vscode.LanguageModelToolCallPart`).
    #[serde(rename = "tool_call")]
    ToolCall {
        call_id: String,
        name: String,
        input: Value,
    },
}

/// A VSCode LM chat message.
///
/// Mirrors `vscode.LanguageModelChatMessage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsCodeLmChatMessage {
    pub role: VsCodeLmRole,
    pub content: Vec<VsCodeLmContentPart>,
}

// ---------------------------------------------------------------------------
// Conversion functions
// ---------------------------------------------------------------------------

/// Converts Anthropic-style content blocks into VSCode LM chat messages.
///
/// Source: `src/api/transform/vscode-lm-format.ts` — `convertToVsCodeLmMessages`
///
/// # Arguments
/// * `messages` - Slice of `(role, content_blocks)` tuples representing
///   Anthropic-style messages.
///
/// # Returns
/// A vector of [`VsCodeLmChatMessage`] in VSCode LM format.
pub fn convert_to_vscode_lm_messages(
    messages: &[(MessageRole, Vec<ContentBlock>)],
) -> Vec<VsCodeLmChatMessage> {
    let mut vs_code_lm_messages: Vec<VsCodeLmChatMessage> = Vec::new();

    for (role, content) in messages {
        match role {
            MessageRole::User => {
                let (tool_messages, non_tool_messages) =
                    partition_user_content(content);

                let mut content_parts: Vec<VsCodeLmContentPart> = Vec::new();

                // Convert tool messages to ToolResultParts first
                for tool_msg in &tool_messages {
                    if let ContentBlock::ToolResult { tool_use_id, content, .. } = tool_msg {
                        let tool_content_parts: Vec<VsCodeLmContentPart> =
                            convert_tool_result_content(content);
                        content_parts.push(VsCodeLmContentPart::ToolResult {
                            call_id: tool_use_id.clone(),
                            content: tool_content_parts,
                        });
                    }
                }

                // Convert non-tool messages to TextParts after tool messages
                for part in &non_tool_messages {
                    content_parts.push(convert_non_tool_part(part, true));
                }

                if !content_parts.is_empty() {
                    vs_code_lm_messages.push(VsCodeLmChatMessage {
                        role: VsCodeLmRole::User,
                        content: content_parts,
                    });
                }
            }
            MessageRole::Assistant => {
                let (tool_messages, non_tool_messages) =
                    partition_assistant_content(content);

                let mut content_parts: Vec<VsCodeLmContentPart> = Vec::new();

                // Convert non-tool messages to TextParts first
                for part in &non_tool_messages {
                    content_parts.push(convert_non_tool_part(part, false));
                }

                // Convert tool messages to ToolCallParts after text
                for tool_msg in &tool_messages {
                    if let ContentBlock::ToolUse { id, name, input } = tool_msg {
                        content_parts.push(VsCodeLmContentPart::ToolCall {
                            call_id: id.clone(),
                            name: name.clone(),
                            input: as_object_safe(input),
                        });
                    }
                }

                if !content_parts.is_empty() {
                    vs_code_lm_messages.push(VsCodeLmChatMessage {
                        role: VsCodeLmRole::Assistant,
                        content: content_parts,
                    });
                }
            }
        }
    }

    vs_code_lm_messages
}

/// Converts a VSCode LM role to an Anthropic role string.
///
/// Source: `src/api/transform/vscode-lm-format.ts` — `convertToAnthropicRole`
pub fn convert_to_anthropic_role(role: VsCodeLmRole) -> Option<&'static str> {
    match role {
        VsCodeLmRole::Assistant => Some("assistant"),
        VsCodeLmRole::User => Some("user"),
    }
}

/// Extracts the text content from a VSCode LM chat message for token counting.
///
/// Source: `src/api/transform/vscode-lm-format.ts` — `extractTextCountFromMessage`
pub fn extract_text_from_message(message: &VsCodeLmChatMessage) -> String {
    let mut text = String::new();
    for item in &message.content {
        match item {
            VsCodeLmContentPart::Text { value } => {
                text.push_str(value);
            }
            VsCodeLmContentPart::ToolResult { call_id, content } => {
                text.push_str(call_id);
                for part in content {
                    if let VsCodeLmContentPart::Text { value } = part {
                        text.push_str(value);
                    }
                }
            }
            VsCodeLmContentPart::ToolCall {
                name,
                call_id,
                input,
            } => {
                text.push_str(name);
                text.push_str(call_id);
                if !input.is_null()
                    && input.is_object()
                    && input.as_object().map(|o| !o.is_empty()).unwrap_or(false)
                {
                    if let Ok(s) = serde_json::to_string(input) {
                        text.push_str(&s);
                    }
                }
            }
        }
    }
    text
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Safely converts a JSON value into a plain object.
///
/// Source: `src/api/transform/vscode-lm-format.ts` — `asObjectSafe`
fn as_object_safe(value: &Value) -> Value {
    if value.is_null() {
        return serde_json::json!({});
    }
    if value.is_string() {
        if let Ok(parsed) = serde_json::from_str::<Value>(value.as_str().unwrap_or("{}")) {
            if parsed.is_object() {
                return parsed;
            }
        }
        return serde_json::json!({});
    }
    if value.is_object() {
        return value.clone();
    }
    serde_json::json!({})
}

/// Partition user content blocks into tool results and non-tool blocks.
fn partition_user_content(content: &[ContentBlock]) -> (Vec<ContentBlock>, Vec<ContentBlock>) {
    let mut tool_messages = Vec::new();
    let mut non_tool_messages = Vec::new();

    for block in content {
        match block {
            ContentBlock::ToolResult { .. } => tool_messages.push(block.clone()),
            ContentBlock::Text { .. } | ContentBlock::Image { .. } => {
                non_tool_messages.push(block.clone());
            }
            _ => {}
        }
    }

    (tool_messages, non_tool_messages)
}

/// Partition assistant content blocks into tool uses and non-tool blocks.
fn partition_assistant_content(
    content: &[ContentBlock],
) -> (Vec<ContentBlock>, Vec<ContentBlock>) {
    let mut tool_messages = Vec::new();
    let mut non_tool_messages = Vec::new();

    for block in content {
        match block {
            ContentBlock::ToolUse { .. } => tool_messages.push(block.clone()),
            ContentBlock::Text { .. } | ContentBlock::Image { .. } => {
                non_tool_messages.push(block.clone());
            }
            _ => {}
        }
    }

    (tool_messages, non_tool_messages)
}

/// Convert tool result content blocks to VSCode LM content parts.
fn convert_tool_result_content(content: &[ToolResultContent]) -> Vec<VsCodeLmContentPart> {
    content
        .iter()
        .map(|c| match c {
            ToolResultContent::Text { text } => VsCodeLmContentPart::Text {
                value: text.clone(),
            },
            ToolResultContent::Image { .. } => VsCodeLmContentPart::Text {
                value: "[Image content not supported by VSCode LM API]".to_string(),
            },
        })
        .collect()
}

/// Convert a non-tool content block to a VSCode LM content part.
fn convert_non_tool_part(block: &ContentBlock, is_user: bool) -> VsCodeLmContentPart {
    match block {
        ContentBlock::Image { source } => {
            let desc = if is_user {
                format!(
                    "[Image ({}): {} not supported by VSCode LM API]",
                    image_source_type_str(source),
                    image_media_type_str(source)
                )
            } else {
                "[Image generation not supported by VSCode LM API]".to_string()
            };
            VsCodeLmContentPart::Text { value: desc }
        }
        ContentBlock::Text { text } => VsCodeLmContentPart::Text {
            value: text.clone(),
        },
        _ => VsCodeLmContentPart::Text {
            value: String::new(),
        },
    }
}

fn image_source_type_str(source: &ImageSource) -> &str {
    match source {
        ImageSource::Base64 { .. } => "base64",
        ImageSource::Url { .. } => "url",
    }
}

fn image_media_type_str(source: &ImageSource) -> &str {
    match source {
        ImageSource::Base64 { media_type, .. } => media_type,
        ImageSource::Url { .. } => "unknown media-type",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple_user_message() {
        let messages = vec![(
            MessageRole::User,
            vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
        )];
        let result = convert_to_vscode_lm_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, VsCodeLmRole::User);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_convert_simple_assistant_message() {
        let messages = vec![(
            MessageRole::Assistant,
            vec![ContentBlock::Text {
                text: "Hi there".to_string(),
            }],
        )];
        let result = convert_to_vscode_lm_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, VsCodeLmRole::Assistant);
    }

    #[test]
    fn test_convert_tool_use_message() {
        let messages = vec![(
            MessageRole::Assistant,
            vec![
                ContentBlock::Text {
                    text: "Let me read that file.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool_1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "/test.txt"}),
                },
            ],
        )];
        let result = convert_to_vscode_lm_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 2);
        // Text comes first, then tool call
        assert!(matches!(result[0].content[0], VsCodeLmContentPart::Text { .. }));
        assert!(matches!(result[0].content[1], VsCodeLmContentPart::ToolCall { .. }));
    }

    #[test]
    fn test_convert_tool_result_message() {
        let messages = vec![(
            MessageRole::User,
            vec![ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "File contents here".to_string(),
                }],
                is_error: None,
            }],
        )];
        let result = convert_to_vscode_lm_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, VsCodeLmRole::User);
        assert!(matches!(result[0].content[0], VsCodeLmContentPart::ToolResult { .. }));
    }

    #[test]
    fn test_convert_to_anthropic_role() {
        assert_eq!(convert_to_anthropic_role(VsCodeLmRole::Assistant), Some("assistant"));
        assert_eq!(convert_to_anthropic_role(VsCodeLmRole::User), Some("user"));
    }

    #[test]
    fn test_extract_text_from_message() {
        let msg = VsCodeLmChatMessage {
            role: VsCodeLmRole::User,
            content: vec![
                VsCodeLmContentPart::Text {
                    value: "Hello ".to_string(),
                },
                VsCodeLmContentPart::Text {
                    value: "World".to_string(),
                },
            ],
        };
        assert_eq!(extract_text_from_message(&msg), "Hello World");
    }

    #[test]
    fn test_extract_text_from_tool_call() {
        let msg = VsCodeLmChatMessage {
            role: VsCodeLmRole::Assistant,
            content: vec![VsCodeLmContentPart::ToolCall {
                call_id: "call_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test.txt"}),
            }],
        };
        let text = extract_text_from_message(&msg);
        assert!(text.contains("read_file"));
        assert!(text.contains("call_1"));
        assert!(text.contains("/test.txt"));
    }

    #[test]
    fn test_as_object_safe_null() {
        let result = as_object_safe(&Value::Null);
        assert!(result.is_object());
    }

    #[test]
    fn test_as_object_safe_string() {
        let result = as_object_safe(&serde_json::json!("{\"key\": \"value\"}"));
        assert!(result.is_object());
        // This is a string, not valid JSON object, so should return {}
    }

    #[test]
    fn test_as_object_safe_object() {
        let result = as_object_safe(&serde_json::json!({"key": "value"}));
        assert!(result.is_object());
        assert_eq!(result["key"], "value");
    }
}
