//! Prompt-caching strategies for various providers.
//!
//! Derived from four TypeScript modules:
//! - `src/api/transform/caching/anthropic.ts`
//! - `src/api/transform/caching/gemini.ts`
//! - `src/api/transform/caching/vercel-ai-gateway.ts`
//! - `src/api/transform/caching/vertex.ts`
//!
//! Each function adds `cache_control: { "type": "ephemeral" }` markers to
//! strategic positions in the message array so the provider can reuse
//! previously-computed KV-cache entries.

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Anthropic caching
// ---------------------------------------------------------------------------

/// Adds Anthropic-style cache breakpoints to a message array.
///
/// Strategy:
/// 1. The **system** message (index 0) gets `cache_control` on its text part.
/// 2. The **last two user** messages get `cache_control` on their last text
///    part.
///
/// All user messages are first normalised to array-of-parts format so that
/// `cache_control` can be attached to individual parts.
///
/// Source: `src/api/transform/caching/anthropic.ts` — `addCacheBreakpoints`
pub fn apply_anthropic_caching(system_prompt: &str, messages: &mut Vec<Value>) {
    if messages.is_empty() {
        return;
    }

    // Mark system message with cache_control
    messages[0] = json!({
        "role": "system",
        "content": [{ "type": "text", "text": system_prompt, "cache_control": { "type": "ephemeral" } }]
    });

    // Ensure all user messages have content in array format
    for msg in messages.iter_mut() {
        if msg["role"] == "user" && msg["content"].is_string() {
            let text = msg["content"].as_str().unwrap_or("").to_string();
            msg["content"] = json!([{ "type": "text", "text": text }]);
        }
    }

    // Collect indices of user messages
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, msg)| msg["role"] == "user")
        .map(|(i, _)| i)
        .collect();

    // Mark the last two user messages
    for &idx in user_indices.iter().rev().take(2) {
        let msg = &mut messages[idx];
        if let Some(content) = msg["content"].as_array_mut() {
            // Find the last text part
            let last_text_idx = content
                .iter()
                .rposition(|part| part["type"] == "text");

            if let Some(text_idx) = last_text_idx {
                let text_part = &mut content[text_idx];
                if let Some(obj) = text_part.as_object_mut() {
                    obj.insert(
                        "cache_control".to_string(),
                        json!({ "type": "ephemeral" }),
                    );
                }
            } else {
                // No text part — add a placeholder
                content.push(json!({
                    "type": "text",
                    "text": "...",
                    "cache_control": { "type": "ephemeral" }
                }));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Gemini caching
// ---------------------------------------------------------------------------

/// Adds Gemini-style cache breakpoints to a message array.
///
/// Strategy:
/// 1. The **system** message (index 0) gets `cache_control` on its text part.
/// 2. Every **N-th** user message (0-indexed, where N = `frequency`) gets
///    `cache_control` on its last text part.
///
/// Source: `src/api/transform/caching/gemini.ts` — `addCacheBreakpoints`
pub fn apply_gemini_caching(system_prompt: &str, messages: &mut Vec<Value>, frequency: usize) {
    if messages.is_empty() {
        return;
    }

    // Mark system message with cache_control
    messages[0] = json!({
        "role": "system",
        "content": [{ "type": "text", "text": system_prompt, "cache_control": { "type": "ephemeral" } }]
    });

    let mut count = 0usize;

    for msg in messages.iter_mut() {
        if msg["role"] != "user" {
            continue;
        }

        // Ensure content is in array format
        if msg["content"].is_string() {
            let text = msg["content"].as_str().unwrap_or("").to_string();
            msg["content"] = json!([{ "type": "text", "text": text }]);
        }

        // Check if this is the N-th user message (0-indexed: mark when count % frequency == frequency - 1)
        let is_nth = count % frequency == frequency - 1;

        if is_nth {
            if let Some(content) = msg["content"].as_array_mut() {
                let last_text_idx = content.iter().rposition(|part| part["type"] == "text");

                if let Some(text_idx) = last_text_idx {
                    if let Some(obj) = content[text_idx].as_object_mut() {
                        obj.insert(
                            "cache_control".to_string(),
                            json!({ "type": "ephemeral" }),
                        );
                    }
                } else {
                    content.push(json!({
                        "type": "text",
                        "text": "...",
                        "cache_control": { "type": "ephemeral" }
                    }));
                }
            }
        }

        count += 1;
    }
}

// ---------------------------------------------------------------------------
// Vercel AI Gateway caching
// ---------------------------------------------------------------------------

/// Adds Vercel AI Gateway-style cache breakpoints to a message array.
///
/// Strategy:
/// 1. The **system** message (index 0) gets `cache_control` at the **message
///    level** (not on individual parts).
/// 2. The **last two user** messages get `cache_control` on their last text
///    part (only if the text is non-empty).
///
/// Source: `src/api/transform/caching/vercel-ai-gateway.ts` — `addCacheBreakpoints`
pub fn apply_vercel_caching(system_prompt: &str, messages: &mut Vec<Value>) {
    if messages.is_empty() {
        return;
    }

    // Mark system message at the message level
    messages[0] = json!({
        "role": "system",
        "content": system_prompt,
        "cache_control": { "type": "ephemeral" }
    });

    // Find the last two user messages
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, msg)| msg["role"] == "user")
        .map(|(i, _)| i)
        .collect();

    let last_two: Vec<usize> = user_indices.into_iter().rev().take(2).collect();

    for idx in last_two {
        let msg = &mut messages[idx];

        // Convert string content to array if needed
        if msg["content"].is_string() && msg["content"].as_str().unwrap_or("").len() > 0 {
            let text = msg["content"].as_str().unwrap_or("").to_string();
            msg["content"] = json!([{ "type": "text", "text": text }]);
        }

        if let Some(content) = msg["content"].as_array_mut() {
            // Find the last text part with non-empty text
            let last_text_idx = content.iter().rposition(|part| {
                part["type"] == "text"
                    && part
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map_or(false, |t| !t.is_empty())
            });

            if let Some(text_idx) = last_text_idx {
                if let Some(obj) = content[text_idx].as_object_mut() {
                    obj.insert(
                        "cache_control".to_string(),
                        json!({ "type": "ephemeral" }),
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Vertex caching
// ---------------------------------------------------------------------------

/// Adds Vertex-style cache breakpoints to a message array.
///
/// Strategy:
/// Only the **last two user** messages have `cache_control` added to their
/// **last text block**.  The system message is NOT modified (Vertex handles
/// system caching separately).
///
/// This keeps the total number of cached blocks at 3 (1 system + 2 user),
/// within Vertex's 4-block limit.
///
/// Source: `src/api/transform/caching/vertex.ts` — `addCacheBreakpoints`
pub fn apply_vertex_caching(messages: &mut Vec<Value>) {
    // Find indices of user messages
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, msg)| msg["role"] == "user")
        .map(|(i, _)| i)
        .collect();

    // Only cache the last two user messages
    let last_idx = user_indices.last().copied();
    let second_last_idx = if user_indices.len() >= 2 {
        Some(user_indices[user_indices.len() - 2])
    } else {
        None
    };

    let indices_to_cache = [second_last_idx, last_idx];

    for &idx_opt in &indices_to_cache {
        if let Some(idx) = idx_opt {
            let msg = &mut messages[idx];

            // Handle string content
            if msg["content"].is_string() {
                let text = msg["content"].as_str().unwrap_or("").to_string();
                msg["content"] = json!([{
                    "type": "text",
                    "text": text,
                    "cache_control": { "type": "ephemeral" }
                }]);
                continue;
            }

            // Handle array content — find last text block
            if let Some(content) = msg["content"].as_array_mut() {
                let last_text_idx = content
                    .iter()
                    .rposition(|part| part["type"] == "text");

                if let Some(text_idx) = last_text_idx {
                    if let Some(obj) = content[text_idx].as_object_mut() {
                        obj.insert(
                            "cache_control".to_string(),
                            json!({ "type": "ephemeral" }),
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_messages(system: &str, user_texts: &[&str]) -> Vec<Value> {
        let mut msgs = vec![json!({
            "role": "system",
            "content": system
        })];
        for text in user_texts {
            msgs.push(json!({
                "role": "user",
                "content": text
            }));
            msgs.push(json!({
                "role": "assistant",
                "content": "response"
            }));
        }
        msgs
    }

    // --- Anthropic ---

    #[test]
    fn test_anthropic_caches_system_and_last_two_users() {
        let mut msgs = make_messages("sys", &["u1", "u2", "u3"]);
        // Layout: system=0, u1=1, asst=2, u2=3, asst=4, u3=5, asst=6
        apply_anthropic_caching("sys", &mut msgs);

        // System has cache_control
        let sys_content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(sys_content[0]["cache_control"]["type"], "ephemeral");

        // u2 (second-to-last user, index 3) should have cache_control
        let u2_content = msgs[3]["content"].as_array().unwrap();
        assert_eq!(u2_content[0]["cache_control"]["type"], "ephemeral");

        // u3 (last user, index 5) should have cache_control
        let u3_content = msgs[5]["content"].as_array().unwrap();
        assert_eq!(u3_content[0]["cache_control"]["type"], "ephemeral");

        // u1 (first user, index 1) should NOT have cache_control
        let u1_content = msgs[1]["content"].as_array().unwrap();
        assert!(u1_content[0].get("cache_control").is_none());
    }

    #[test]
    fn test_anthropic_empty_messages() {
        let mut msgs: Vec<Value> = vec![];
        apply_anthropic_caching("sys", &mut msgs);
        assert!(msgs.is_empty());
    }

    // --- Gemini ---

    #[test]
    fn test_gemini_caches_every_nth_user() {
        let mut msgs = make_messages("sys", &["u1", "u2", "u3", "u4", "u5"]);
        apply_gemini_caching("sys", &mut msgs, 3);

        // System cached
        let sys_content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(sys_content[0]["cache_control"]["type"], "ephemeral");

        // User messages at indices: 1, 3, 5, 7, 9
        // With frequency=3, mark user indices 2 and 5 (0-indexed count: 2, 5)
        // count=2 → index 5 (u3), count=5 → index 11 (doesn't exist)
        // Wait, let me recalculate: user msgs are at positions 1,3,5,7,9
        // count 0 → pos 1 (u1), count 1 → pos 3 (u2), count 2 → pos 5 (u3)
        // frequency=3: mark when count % 3 == 2 → count=2 (u3), count=5 (no)
        let u3_content = msgs[5]["content"].as_array().unwrap();
        assert_eq!(u3_content[0]["cache_control"]["type"], "ephemeral");
    }

    // --- Vercel ---

    #[test]
    fn test_vercel_caches_system_and_last_two_users() {
        let mut msgs = make_messages("sys", &["u1", "u2"]);
        apply_vercel_caching("sys", &mut msgs);

        // System has message-level cache_control
        assert_eq!(msgs[0]["cache_control"]["type"], "ephemeral");

        // u2 (last user at index 3) should have cache_control
        let u2_content = msgs[3]["content"].as_array().unwrap();
        assert_eq!(u2_content[0]["cache_control"]["type"], "ephemeral");

        // u1 (index 1) should also have cache_control (second-to-last)
        let u1_content = msgs[1]["content"].as_array().unwrap();
        assert_eq!(u1_content[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn test_vercel_skips_empty_text() {
        let mut msgs = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": ""}),
        ];
        apply_vercel_caching("sys", &mut msgs);

        // The empty user message should NOT get cache_control
        let content = msgs[1]["content"].as_str().unwrap();
        assert_eq!(content, "");
    }

    // --- Vertex ---

    #[test]
    fn test_vertex_caches_last_two_users() {
        let mut msgs = make_messages("sys", &["u1", "u2", "u3"]);
        apply_vertex_caching(&mut msgs);

        // System should NOT be modified by vertex caching
        assert!(msgs[0].get("content").unwrap().is_string());

        // u2 (index 3) and u3 (index 5) should have cache_control
        let u2_content = msgs[3]["content"].as_array().unwrap();
        assert_eq!(u2_content[0]["cache_control"]["type"], "ephemeral");

        let u3_content = msgs[5]["content"].as_array().unwrap();
        assert_eq!(u3_content[0]["cache_control"]["type"], "ephemeral");

        // u1 (index 1) should NOT have cache_control
        assert!(msgs[1]["content"].is_string());
    }

    #[test]
    fn test_vertex_single_user() {
        let mut msgs = make_messages("sys", &["u1"]);
        apply_vertex_caching(&mut msgs);

        // Only one user message (index 1)
        let u1_content = msgs[1]["content"].as_array().unwrap();
        assert_eq!(u1_content[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn test_vertex_array_content() {
        let mut msgs = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": [
                {"type": "text", "text": "first"},
                {"type": "text", "text": "second"}
            ]}),
        ];
        apply_vertex_caching(&mut msgs);

        let content = msgs[1]["content"].as_array().unwrap();
        // Only the last text block should have cache_control
        assert!(content[0].get("cache_control").is_none());
        assert_eq!(content[1]["cache_control"]["type"], "ephemeral");
    }
}
