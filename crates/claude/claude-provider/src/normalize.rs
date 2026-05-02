//! Message normalization for the Anthropic API.
//!
//! Mirrors `normalizeMessagesForAPI()` from
//! `cc-haha/src/utils/messages.ts`.  Ensures the conversation array
//! satisfies the Anthropic API contract:
//!
//! 1. Messages alternate between `"user"` and `"assistant"` roles.
//! 2. Every `tool_use` block has a matching `tool_result` block.
//! 3. Trailing thinking blocks are stripped from the last assistant.
//! 4. Whitespace-only / empty assistant messages are removed.
//! 5. Consecutive same-role messages are merged.

use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Normalize a messages array for the Anthropic Messages API.
///
/// `messages` is a mutable slice of JSON objects, each with a `"role"` field
/// (`"user"` or `"assistant"`) and a `"content"` field (string or array of
/// content blocks).
pub fn normalize_messages_for_api(messages: &mut Vec<Value>) {
    ensure_tool_result_pairing(messages);
    merge_consecutive_same_role(messages);
    filter_orphaned_thinking_only(messages);
    filter_trailing_thinking_from_last_assistant(messages);
    filter_whitespace_only_assistant_messages(messages);
    // Filters may create consecutive same-role messages — re-merge
    merge_consecutive_same_role(messages);
    ensure_non_empty_assistant_content(messages);
}

// ---------------------------------------------------------------------------
// 1. Tool-use / tool-result pairing
// ---------------------------------------------------------------------------

/// Ensure every `tool_use` block in assistant messages has a corresponding
/// `tool_result` block in the following user message.
///
/// If a `tool_result` is missing, a synthetic one is injected.
/// Mirrors `ensureToolResultPairing()` from the TS reference.
fn ensure_tool_result_pairing(messages: &mut Vec<Value>) {
    let mut insertions: Vec<(usize, Value)> = Vec::new();

    for i in 0..messages.len() {
        let msg = &messages[i];
        if msg["role"].as_str() != Some("assistant") {
            continue;
        }

        let Some(blocks) = msg["content"].as_array() else {
            continue;
        };

        let tool_use_ids: Vec<String> = blocks
            .iter()
            .filter_map(|b| {
                if b["type"].as_str() == Some("tool_use") {
                    b["id"].as_str().map(|s| s.to_owned())
                } else {
                    None
                }
            })
            .collect();

        if tool_use_ids.is_empty() {
            continue;
        }

        // Collect tool_result IDs from the next user message
        let mut covered = std::collections::HashSet::new();
        if let Some(next) = messages.get(i + 1) {
            if next["role"].as_str() == Some("user") {
                if let Some(content) = next["content"].as_array() {
                    for block in content {
                        if block["type"].as_str() == Some("tool_result") {
                            if let Some(id) = block["tool_use_id"].as_str() {
                                covered.insert(id.to_owned());
                            }
                        }
                    }
                }
            }
        }

        let missing: Vec<&str> = tool_use_ids
            .iter()
            .filter(|id| !covered.contains(id.as_str()))
            .map(|id| id.as_str())
            .collect();

        if missing.is_empty() {
            continue;
        }

        // Build synthetic tool_result blocks for missing IDs
        let synthetic_blocks: Vec<Value> = missing
            .into_iter()
            .map(|id| {
                json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": "Tool execution was interrupted. Please try again if needed.",
                    "is_error": true
                })
            })
            .collect();

        // Check if the next message is a user message we can merge into
        if let Some(next) = messages.get(i + 1) {
            if next["role"].as_str() == Some("user") {
                // Merge synthetic blocks into existing user message
                let next_msg = &mut messages[i + 1];
                if let Some(content) = next_msg["content"].as_array_mut() {
                    // Prepend synthetic blocks so they appear before other content
                    let mut new_blocks = synthetic_blocks;
                    new_blocks.append(content);
                    next_msg["content"] = Value::Array(new_blocks);
                } else {
                    // Content is a string — convert to array with synthetic blocks
                    let text = next_msg["content"].take();
                    let mut new_blocks = synthetic_blocks;
                    if let Some(t) = text.as_str() {
                        if !t.is_empty() {
                            new_blocks.push(json!({"type": "text", "text": t}));
                        }
                    }
                    next_msg["content"] = Value::Array(new_blocks);
                }
            } else {
                // Insert a new user message with synthetic blocks
                insertions.push((
                    i + 1,
                    json!({
                        "role": "user",
                        "content": synthetic_blocks,
                    }),
                ));
            }
        } else {
            // Last message is assistant with orphaned tool_use — append user msg
            insertions.push((
                i + 1,
                json!({
                    "role": "user",
                    "content": synthetic_blocks,
                }),
            ));
        }
    }

    // Apply insertions in reverse order so indices remain valid
    for (idx, msg) in insertions.into_iter().rev() {
        messages.insert(idx, msg);
    }
}

// ---------------------------------------------------------------------------
// 2. Merge consecutive same-role messages
// ---------------------------------------------------------------------------

/// Merge consecutive messages of the same role into a single message.
/// The API requires strict user/assistant alternation.
fn merge_consecutive_same_role(messages: &mut Vec<Value>) {
    if messages.len() <= 1 {
        return;
    }

    let mut result: Vec<Value> = Vec::with_capacity(messages.len());
    result.push(messages[0].take());

    let mut i = 1;
    while i < messages.len() {
        let current = &mut messages[i];
        let prev_role = result.last().and_then(|m| m["role"].as_str());
        let curr_role = current["role"].as_str();

        if prev_role == curr_role && prev_role.is_some() {
            // Same role — merge content
            let prev = result.last_mut().unwrap();
            let prev_content = prev["content"].take();
            let curr_content = current["content"].take();

            let mut merged = content_to_array(prev_content);
            merged.extend(content_to_array(curr_content));

            // Remove duplicate text blocks (exact duplicates within merged content)
            merged = dedup_content_blocks(&merged);

            prev["content"] = if merged.len() == 1
                && merged[0]["type"].as_str() == Some("text")
            {
                // Single text block — use string form
                merged.remove(0)["content"].take()
            } else {
                Value::Array(merged)
            };
        } else {
            result.push(current.clone());
        }
        i += 1;
    }

    *messages = result;
}

// ---------------------------------------------------------------------------
// 3. Filter orphaned thinking-only assistant messages
// ---------------------------------------------------------------------------

/// Remove assistant messages that contain *only* thinking blocks and no
/// other content.  These are typically artifacts from compaction slicing or
/// failed streaming retries.
fn filter_orphaned_thinking_only(messages: &mut Vec<Value>) {
    messages.retain(|msg| {
        if msg["role"].as_str() != Some("assistant") {
            return true;
        }
        let Some(blocks) = msg["content"].as_array() else {
            // String content is not thinking-only
            return true;
        };
        if blocks.is_empty() {
            return false;
        }
        // Keep if there's any non-thinking block
        blocks.iter().any(|b| {
            let btype = b["type"].as_str().unwrap_or("");
            btype != "thinking" && btype != "redacted_thinking"
        })
    });
}

// ---------------------------------------------------------------------------
// 4. Filter trailing thinking from last assistant
// ---------------------------------------------------------------------------

/// Strip thinking blocks from the very last assistant message.
/// The API rejects trailing thinking blocks with a mismatched signature.
fn filter_trailing_thinking_from_last_assistant(messages: &mut Vec<Value>) {
    // Find the last assistant message
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m["role"].as_str() == Some("assistant"));

    let Some(idx) = last_assistant_idx else {
        return;
    };

    let Some(blocks) = messages[idx]["content"].as_array_mut() else {
        return;
    };

    // Remove trailing thinking/redacted_thinking blocks
    while blocks
        .last()
        .is_some_and(|b| matches!(b["type"].as_str(), Some("thinking" | "redacted_thinking")))
    {
        blocks.pop();
    }

    // If all blocks were removed, leave at least one text block
    if blocks.is_empty() {
        blocks.push(json!({"type": "text", "text": ""}));
    }
}

// ---------------------------------------------------------------------------
// 5. Filter whitespace-only assistant messages
// ---------------------------------------------------------------------------

/// Remove assistant messages whose content is entirely whitespace text.
fn filter_whitespace_only_assistant_messages(messages: &mut Vec<Value>) {
    let mut i = 0;
    while i < messages.len() {
        if messages[i]["role"].as_str() == Some("assistant") {
            if is_whitespace_only_assistant(&messages[i]) {
                messages.remove(i);
                continue;
            }
        }
        i += 1;
    }
}

fn is_whitespace_only_assistant(msg: &Value) -> bool {
    let Some(blocks) = msg["content"].as_array() else {
        // String content
        return msg["content"]
            .as_str()
            .is_some_and(|s| s.trim().is_empty());
    };

    if blocks.is_empty() {
        return true;
    }

    // Check if all blocks are whitespace-only text or thinking
    blocks.iter().all(|b| match b["type"].as_str() {
        Some("text") => b["text"].as_str().is_some_and(|s| s.trim().is_empty()),
        Some("thinking" | "redacted_thinking") => true,
        _ => false,
    })
}

// ---------------------------------------------------------------------------
// 6. Ensure non-empty assistant content
// ---------------------------------------------------------------------------

/// Guarantee every assistant message has at least one content block.
fn ensure_non_empty_assistant_content(messages: &mut Vec<Value>) {
    for msg in messages.iter_mut() {
        if msg["role"].as_str() != Some("assistant") {
            continue;
        }

        match &mut msg["content"] {
            Value::String(s) if s.is_empty() => {
                *msg = json!({
                    "role": "assistant",
                    "content": [{"type": "text", "text": ""}]
                });
            }
            Value::Array(blocks) if blocks.is_empty() => {
                blocks.push(json!({"type": "text", "text": ""}));
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert message content (string or array) to an array of content blocks.
fn content_to_array(content: Value) -> Vec<Value> {
    match content {
        Value::String(s) => {
            if s.is_empty() {
                Vec::new()
            } else {
                vec![json!({"type": "text", "text": s})]
            }
        }
        Value::Array(blocks) => blocks,
        _ => Vec::new(),
    }
}

/// Remove duplicate content blocks (exact text/tool_use duplicates).
fn dedup_content_blocks(blocks: &[Value]) -> Vec<Value> {
    let mut seen_text = std::collections::HashSet::new();
    let mut seen_tool_result = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(blocks.len());

    for block in blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(text) = block["text"].as_str() {
                    if !seen_text.contains(text) {
                        seen_text.insert(text.to_owned());
                        result.push(block.clone());
                    }
                } else {
                    result.push(block.clone());
                }
            }
            Some("tool_result") => {
                if let Some(id) = block["tool_use_id"].as_str() {
                    if !seen_tool_result.contains(id) {
                        seen_tool_result.insert(id.to_owned());
                        result.push(block.clone());
                    }
                } else {
                    result.push(block.clone());
                }
            }
            _ => {
                result.push(block.clone());
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_result_pairing_orphaned_tool_use() {
        let mut messages = vec![
            json!({"role": "assistant", "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "tool-1", "name": "Read", "input": {"path": "/foo"}}
            ]}),
            json!({"role": "user", "content": "next message"}),
        ];
        ensure_tool_result_pairing(&mut messages);
        // Should have injected synthetic tool_result
        assert_eq!(messages[1]["role"], "user");
        let content = messages[1]["content"].as_array().unwrap();
        assert!(content
            .iter()
            .any(|b| b["type"].as_str() == Some("tool_result") && b["tool_use_id"].as_str() == Some("tool-1")));
    }

    #[test]
    fn test_tool_result_pairing_already_paired() {
        let mut messages = vec![
            json!({"role": "assistant", "content": [
                {"type": "tool_use", "id": "tool-1", "name": "Read", "input": {}}
            ]}),
            json!({"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "tool-1", "content": "ok"}
            ]}),
        ];
        ensure_tool_result_pairing(&mut messages);
        // No insertion needed
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_merge_consecutive_user_messages() {
        let mut messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "user", "content": "world"}),
        ];
        merge_consecutive_same_role(&mut messages);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn test_merge_consecutive_assistant_messages() {
        let mut messages = vec![
            json!({"role": "assistant", "content": [
                {"type": "text", "text": "part1"}
            ]}),
            json!({"role": "assistant", "content": [
                {"type": "text", "text": "part2"}
            ]}),
        ];
        merge_consecutive_same_role(&mut messages);
        assert_eq!(messages.len(), 1);
        let content = messages[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
    }

    #[test]
    fn test_no_merge_different_roles() {
        let mut messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "hi"}),
            json!({"role": "user", "content": "bye"}),
        ];
        merge_consecutive_same_role(&mut messages);
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_filter_orphaned_thinking_only() {
        let mut messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "abc"}
            ]}),
            json!({"role": "user", "content": "world"}),
        ];
        filter_orphaned_thinking_only(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn test_keep_assistant_with_mixed_content() {
        let mut messages = vec![
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "abc"},
                {"type": "text", "text": "result"}
            ]}),
        ];
        filter_orphaned_thinking_only(&mut messages);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_filter_trailing_thinking() {
        let mut messages = vec![json!({
            "role": "assistant",
            "content": [
                {"type": "text", "text": "hello"},
                {"type": "thinking", "thinking": "hmm", "signature": "abc"},
                {"type": "redacted_thinking", "data": "xyz"}
            ]
        })];
        filter_trailing_thinking_from_last_assistant(&mut messages);
        let content = messages[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
    }

    #[test]
    fn test_filter_whitespace_only_assistant() {
        let mut messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "   "}),
            json!({"role": "user", "content": "world"}),
        ];
        filter_whitespace_only_assistant_messages(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn test_ensure_non_empty_assistant() {
        let mut messages = vec![json!({
            "role": "assistant",
            "content": []
        })];
        ensure_non_empty_assistant_content(&mut messages);
        let content = messages[0]["content"].as_array().unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_full_normalization_pipeline() {
        let mut messages = vec![
            json!({"role": "user", "content": "read file"}),
            json!({"role": "assistant", "content": [
                {"type": "tool_use", "id": "t1", "name": "Read", "input": {"path": "/x"}}
            ]}),
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "s1"}
            ]}),
            json!({"role": "user", "content": "thanks"}),
            json!({"role": "assistant", "content": "   "}),
            json!({"role": "user", "content": "bye"}),
        ];
        normalize_messages_for_api(&mut messages);

        // Should have: user, assistant(with tool_use), user(with synthetic tool_result + "thanks" merged), user("bye")
        // After orphan thinking filter removes the thinking-only assistant
        // After whitespace filter removes "   " assistant
        // After merge: consecutive users are merged

        // Verify role alternation
        for i in 1..messages.len() {
            assert_ne!(
                messages[i]["role"].as_str(),
                messages[i - 1]["role"].as_str(),
                "Consecutive messages at index {} have same role",
                i
            );
        }

        // Verify tool_use has a matching tool_result
        let tool_use_ids: Vec<&str> = messages
            .iter()
            .flat_map(|m| m["content"].as_array().into_iter().flatten())
            .filter(|b| b["type"].as_str() == Some("tool_use"))
            .filter_map(|b| b["id"].as_str())
            .collect();
        let tool_result_ids: Vec<&str> = messages
            .iter()
            .flat_map(|m| m["content"].as_array().into_iter().flatten())
            .filter(|b| b["type"].as_str() == Some("tool_result"))
            .filter_map(|b| b["tool_use_id"].as_str())
            .collect();
        for id in &tool_use_ids {
            assert!(
                tool_result_ids.contains(id),
                "Missing tool_result for tool_use id {}",
                id
            );
        }
    }
}
