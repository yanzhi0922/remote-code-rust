//! Message transformation for condensing.
//!
//! Transforms messages by converting tool blocks to text and injecting synthetic
//! tool results for orphan tool calls.
//!
//! Source: `src/core/condense/index.ts` — `transformMessagesForCondensing`,
//! `injectSyntheticToolResults`

use roo_types::api::{ApiMessage, ContentBlock, MessageRole, ToolResultContent};

use crate::convert::convert_tool_blocks_to_text;

/// Transforms all messages by converting `tool_use` and `tool_result` blocks to
/// text representations.
///
/// This ensures the conversation can be sent for summarization without requiring
/// the `tools` parameter.
///
/// Source: `src/core/condense/index.ts` — `transformMessagesForCondensing`
pub fn transform_messages_for_condensing(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    messages
        .iter()
        .map(|msg| {
            let mut new_msg = msg.clone();
            new_msg.content = convert_tool_blocks_to_text(&msg.content);
            new_msg
        })
        .collect()
}

/// Injects synthetic `tool_result` blocks for orphan `tool_use` calls that don't
/// have matching results.
///
/// This is necessary because some APIs (e.g., OpenAI's Responses API) reject
/// conversations with orphan tool calls. This can happen when the user triggers
/// condense after receiving a `tool_use` (like `attempt_completion`) but before
/// responding to it.
///
/// Source: `src/core/condense/index.ts` — `injectSyntheticToolResults`
pub fn inject_synthetic_tool_results(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    // Find all tool_use IDs in assistant messages
    let mut tool_call_ids = std::collections::HashSet::new();
    // Find all tool_result IDs in user messages
    let mut tool_result_ids = std::collections::HashSet::new();

    for msg in messages {
        if msg.role == MessageRole::Assistant {
            for block in &msg.content {
                if let ContentBlock::ToolUse { id, .. } = block {
                    tool_call_ids.insert(id.clone());
                }
            }
        }
        if msg.role == MessageRole::User {
            for block in &msg.content {
                if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                    tool_result_ids.insert(tool_use_id.clone());
                }
            }
        }
    }

    // Find orphans (tool_calls without matching tool_results)
    let orphan_ids: Vec<String> = tool_call_ids
        .into_iter()
        .filter(|id| !tool_result_ids.contains(id))
        .collect();

    if orphan_ids.is_empty() {
        return messages.to_vec();
    }

    // Inject synthetic tool_results as a new user message
    let synthetic_results: Vec<ContentBlock> = orphan_ids
        .into_iter()
        .map(|id| {
            ContentBlock::ToolResult {
                tool_use_id: id,
                content: vec![ToolResultContent::Text {
                    text: "Context condensation triggered. Tool execution deferred.".to_string(),
                }],
                is_error: None,
            }
        })
        .collect();

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;

    let synthetic_message = ApiMessage {
        role: MessageRole::User,
        content: synthetic_results,
        reasoning: None,
        ts: Some(now_ts),
        truncation_parent: None,
        is_truncation_marker: None,
        truncation_id: None,
        condense_parent: None,
        is_summary: None,
        condense_id: None,
            reasoning_details: None,
    };

    let mut result = messages.to_vec();
    result.push(synthetic_message);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, MessageRole, ToolResultContent};

    fn make_assistant_msg_with_tool(id: &str, name: &str) -> ApiMessage {
        ApiMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input: serde_json::json!({}),
            }],
            reasoning: None,
            ts: Some(1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    fn make_user_msg_with_result(tool_use_id: &str) -> ApiMessage {
        ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: vec![ToolResultContent::Text {
                    text: "result".to_string(),
                }],
                is_error: None,
            }],
            reasoning: None,
            ts: Some(1001.0),
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
    fn test_inject_synthetic_tool_results_no_orphans() {
        let messages = vec![
            make_assistant_msg_with_tool("tool_1", "read_file"),
            make_user_msg_with_result("tool_1"),
        ];
        let result = inject_synthetic_tool_results(&messages);
        assert_eq!(result.len(), 2); // No synthetic message added
    }

    #[test]
    fn test_inject_synthetic_tool_results_with_orphans() {
        let messages = vec![
            make_assistant_msg_with_tool("tool_1", "read_file"),
            make_user_msg_with_result("tool_1"),
            make_assistant_msg_with_tool("tool_2", "write_file"),
            // No result for tool_2
        ];
        let result = inject_synthetic_tool_results(&messages);
        assert_eq!(result.len(), 4); // Synthetic message added
        let last = result.last().unwrap();
        assert_eq!(last.role, MessageRole::User);
        assert!(matches!(&last.content[0], ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "tool_2"));
    }

    #[test]
    fn test_transform_messages_for_condensing() {
        let messages = vec![ApiMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test.txt"}),
            }],
            reasoning: None,
            ts: Some(1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];
        let result = transform_messages_for_condensing(&messages);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].content[0], ContentBlock::Text { text } if text.contains("[Tool Use: read_file]")));
    }
}
