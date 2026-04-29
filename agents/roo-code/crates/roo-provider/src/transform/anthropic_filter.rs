//! Filters non-Anthropic content blocks from messages.
//!
//! Derived from `src/api/transform/anthropic-filter.ts`.
//! Uses an allowlist approach — only blocks with recognized Anthropic types are kept.

use roo_types::api::{ApiMessage, ContentBlock};

/// Set of content block types that are valid for the Anthropic API.
/// Only these types will be passed through to the API.
/// See: <https://docs.anthropic.com/en/api/messages>
pub const VALID_ANTHROPIC_BLOCK_TYPES: &[&str] = &[
    "text",
    "image",
    "tool_use",
    "tool_result",
    "thinking",
    "redacted_thinking",
    "document",
];

/// Returns true if the given block type is valid for the Anthropic API.
fn is_valid_anthropic_block(block: &ContentBlock) -> bool {
    matches!(
        block,
        ContentBlock::Text { .. }
            | ContentBlock::Image { .. }
            | ContentBlock::ToolUse { .. }
            | ContentBlock::ToolResult { .. }
            | ContentBlock::Thinking { .. }
            | ContentBlock::RedactedThinking { .. }
    )
}

/// Filters out non-Anthropic content blocks from messages before sending to
/// Anthropic/Vertex API.
///
/// Uses an allowlist approach — only blocks with recognized Anthropic types are kept.
/// This automatically filters out:
/// - Internal "reasoning" blocks (Roo Code's internal representation)
/// - Gemini's "thoughtSignature" blocks (encrypted reasoning continuity tokens)
/// - Any other unknown block types
///
/// Source: `src/api/transform/anthropic-filter.ts` — `filterNonAnthropicBlocks`
pub fn filter_non_anthropic_blocks(messages: Vec<ApiMessage>) -> Vec<ApiMessage> {
    messages
        .into_iter()
        .filter_map(|message| {
            let filtered_content: Vec<ContentBlock> = message
                .content
                .into_iter()
                .filter(is_valid_anthropic_block)
                .collect();

            // If all content was filtered out, skip this message entirely
            if filtered_content.is_empty() {
                return None;
            }

            Some(ApiMessage {
                content: filtered_content,
                ..message
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ImageSource, MessageRole, ToolResultContent};

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

    #[test]
    fn test_filters_empty_messages() {
        let messages = vec![make_message(MessageRole::User, vec![])];
        let result = filter_non_anthropic_blocks(messages);
        assert!(result.is_empty());
    }

    #[test]
    fn test_keeps_valid_blocks() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![ContentBlock::Text {
                text: "hello".to_string(),
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_keeps_tool_use_blocks() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "test.rs"}),
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_keeps_tool_result_blocks() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "result".to_string(),
                }],
                is_error: None,
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_keeps_thinking_blocks() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![ContentBlock::Thinking {
                thinking: "deep thoughts".to_string(),
                signature: "sig123".to_string(),
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_keeps_redacted_thinking_blocks() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![ContentBlock::RedactedThinking {
                data: "encrypted_data".to_string(),
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_keeps_image_blocks() {
        let messages = vec![make_message(
            MessageRole::User,
            vec![ContentBlock::Image {
                source: ImageSource::Base64 {
                    data: "iVBORw0KGgo=".to_string(),
                    media_type: "image/png".to_string(),
                },
            }],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 1);
    }

    #[test]
    fn test_preserves_message_fields() {
        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: "hello".to_string(),
            }],
            reasoning: Some("some reasoning".to_string()),
            ts: Some(12345.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result[0].reasoning, Some("some reasoning".to_string()));
        assert_eq!(result[0].ts, Some(12345.0));
    }

    #[test]
    fn test_mixed_valid_and_invalid_keeps_valid() {
        let messages = vec![make_message(
            MessageRole::Assistant,
            vec![
                ContentBlock::Text {
                    text: "response".to_string(),
                },
                ContentBlock::Thinking {
                    thinking: "thoughts".to_string(),
                    signature: "sig".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "test".to_string(),
                    input: serde_json::json!({}),
                },
            ],
        )];
        let result = filter_non_anthropic_blocks(messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 3);
    }
}
