//! MiniMax message format conversion.
//!
//! Derived from `src/api/transform/minimax-format.ts`.
//! MiniMax thinking models error when they receive a standalone user message
//! after a `tool_result` block.  This module merges such text (typically
//! `environment_details`) into the preceding `tool_result` to preserve
//! reasoning continuity.

use roo_types::api::{ApiMessage, ContentBlock, MessageRole, ToolResultContent};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Merges text content (like `environment_details`) that follows `tool_result`
/// blocks into the last `tool_result`'s content.
///
/// MiniMax thinking models error when they receive a standalone user message
/// after a `tool_result`.  This function detects user messages that contain
/// **both** `tool_result` and `text` blocks (but no `image` blocks) and merges
/// the text into the last `tool_result`.
///
/// # Key behaviour
/// - User messages with **only** `tool_result` blocks → kept as-is.
/// - User messages with **only** text/image → kept as-is.
/// - User messages with `tool_result` + text (no images) → text is appended to
///   the last `tool_result`'s content.
///
/// Source: `src/api/transform/minimax-format.ts` — `mergeEnvironmentDetailsForMiniMax`
pub fn merge_environment_details_for_minimax(messages: &mut Vec<ApiMessage>) {
    for message in messages.iter_mut() {
        if message.role != MessageRole::User {
            continue;
        }

        // Partition content blocks by type
        let mut has_tool_result = false;
        let mut has_text = false;
        let mut has_image = false;

        for block in &message.content {
            match block {
                ContentBlock::ToolResult { .. } => {
                    has_tool_result = true;
                }
                ContentBlock::Text { .. } => {
                    has_text = true;
                }
                ContentBlock::Image { .. } => {
                    has_image = true;
                }
                _ => {}
            }
        }

        // Only merge when we have tool_result + text but no images
        if !(has_tool_result && has_text && !has_image) {
            continue;
        }

        // Collect text and rebuild content
        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_results: Vec<(String, Vec<ToolResultContent>, Option<bool>)> = Vec::new();
        let mut other_blocks: Vec<ContentBlock> = Vec::new();

        for block in message.content.drain(..) {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text);
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    tool_results.push((tool_use_id, content, is_error));
                }
                other => {
                    other_blocks.push(other);
                }
            }
        }

        if tool_results.is_empty() || text_parts.is_empty() {
            // Shouldn't happen given the checks above, but be safe
            continue;
        }

        let text_content = text_parts.join("\n\n");

        // Merge text into the last tool_result
        let last_idx = tool_results.len() - 1;
        let (id, existing_content, is_error) = tool_results.remove(last_idx);
        let existing_text = extract_tool_result_text(&existing_content);
        let merged = if existing_text.is_empty() {
            text_content
        } else {
            format!("{existing_text}\n\n{text_content}")
        };

        // Rebuild content: tool_results (original) + merged last one + other blocks
        let mut new_content: Vec<ContentBlock> = Vec::new();

        for (tool_use_id, content, is_error) in tool_results {
            new_content.push(ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            });
        }

        new_content.push(ContentBlock::ToolResult {
            tool_use_id: id,
            content: vec![ToolResultContent::Text { text: merged }],
            is_error,
        });

        new_content.extend(other_blocks);
        message.content = new_content;
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

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
    fn test_merges_text_into_tool_result() {
        let mut messages = vec![make_user_message(vec![
            ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "result data".to_string(),
                }],
                is_error: None,
            },
            ContentBlock::Text {
                text: "environment_details here".to_string(),
            },
        ])];

        merge_environment_details_for_minimax(&mut messages);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.len(), 1);

        match &messages[0].content[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    ToolResultContent::Text { text } => {
                        assert!(text.contains("result data"));
                        assert!(text.contains("environment_details here"));
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected tool_result"),
        }
    }

    #[test]
    fn test_keeps_text_only_message() {
        let mut messages = vec![make_user_message(vec![ContentBlock::Text {
            text: "hello".to_string(),
        }])];

        merge_environment_details_for_minimax(&mut messages);

        assert_eq!(messages.len(), 1);
        match &messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected text block"),
        }
    }

    #[test]
    fn test_keeps_tool_result_only_message() {
        let mut messages = vec![make_user_message(vec![ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: vec![ToolResultContent::Text {
                text: "result".to_string(),
            }],
            is_error: None,
        }])];

        merge_environment_details_for_minimax(&mut messages);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.len(), 1);
    }

    #[test]
    fn test_does_not_merge_when_image_present() {
        let mut messages = vec![make_user_message(vec![
            ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "result".to_string(),
                }],
                is_error: None,
            },
            ContentBlock::Text {
                text: "env details".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/img.png".to_string(),
                },
            },
        ])];

        merge_environment_details_for_minimax(&mut messages);

        // Should not merge — image is present
        assert_eq!(messages[0].content.len(), 3);
    }

    #[test]
    fn test_assistant_messages_unchanged() {
        let mut messages = vec![
            make_assistant_message(vec![ContentBlock::Text {
                text: "response".to_string(),
            }]),
            make_user_message(vec![
                ContentBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: vec![ToolResultContent::Text {
                        text: "data".to_string(),
                    }],
                    is_error: None,
                },
                ContentBlock::Text {
                    text: "env".to_string(),
                },
            ]),
        ];

        merge_environment_details_for_minimax(&mut messages);

        // Assistant unchanged
        match &messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "response"),
            _ => panic!("Expected text"),
        }
        // User merged
        assert_eq!(messages[1].content.len(), 1);
    }

    #[test]
    fn test_multiple_tool_results_merges_into_last() {
        let mut messages = vec![make_user_message(vec![
            ContentBlock::ToolResult {
                tool_use_id: "t1".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "first result".to_string(),
                }],
                is_error: None,
            },
            ContentBlock::ToolResult {
                tool_use_id: "t2".to_string(),
                content: vec![ToolResultContent::Text {
                    text: "second result".to_string(),
                }],
                is_error: None,
            },
            ContentBlock::Text {
                text: "env details".to_string(),
            },
        ])];

        merge_environment_details_for_minimax(&mut messages);

        assert_eq!(messages[0].content.len(), 2);

        // First tool_result unchanged
        match &messages[0].content[0] {
            ContentBlock::ToolResult { tool_use_id, .. } => {
                assert_eq!(tool_use_id, "t1");
            }
            _ => panic!("Expected tool_result"),
        }

        // Last tool_result has merged text
        match &messages[0].content[1] {
            ContentBlock::ToolResult {
                tool_use_id, content, ..
            } => {
                assert_eq!(tool_use_id, "t2");
                match &content[0] {
                    ToolResultContent::Text { text } => {
                        assert!(text.contains("second result"));
                        assert!(text.contains("env details"));
                    }
                    _ => panic!("Expected text"),
                }
            }
            _ => panic!("Expected tool_result"),
        }
    }
}
