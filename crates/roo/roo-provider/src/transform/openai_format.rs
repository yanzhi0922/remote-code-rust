//! Converts Anthropic messages to OpenAI Chat Completion format.
//!
//! Derived from `src/api/transform/openai-format.ts` (510 lines).
//! Handles conversion of Anthropic-style content blocks to OpenAI's
//! message format, including tool calls, images, and reasoning details.

use serde_json::{json, Value};

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};

use crate::error::Result;

// ---------------------------------------------------------------------------
// ReasoningDetail (OpenRouter format)
// ---------------------------------------------------------------------------

/// Type for OpenRouter's reasoning detail elements.
/// See: <https://openrouter.ai/docs/use-cases/reasoning-tokens#streaming-response>
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReasoningDetail {
    #[serde(rename = "type")]
    pub detail_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
}

/// Consolidates reasoning_details by grouping by index and type.
///
/// - Filters out corrupted encrypted blocks (missing `data` field)
/// - For text blocks: concatenates text, keeps last signature/id/format
/// - For encrypted blocks: keeps only the last one per index
///
/// Source: `src/api/transform/openai-format.ts` — `consolidateReasoningDetails`
pub fn consolidate_reasoning_details(reasoning_details: &[ReasoningDetail]) -> Vec<ReasoningDetail> {
    if reasoning_details.is_empty() {
        return vec![];
    }

    // Group by index
    let mut grouped_by_index: std::collections::BTreeMap<u64, Vec<&ReasoningDetail>> =
        std::collections::BTreeMap::new();

    for detail in reasoning_details {
        // Drop corrupted encrypted reasoning blocks
        if detail.detail_type == "reasoning.encrypted" && detail.data.is_none() {
            continue;
        }

        let index = detail.index.unwrap_or(0);
        grouped_by_index
            .entry(index)
            .or_default()
            .push(detail);
    }

    let mut consolidated: Vec<ReasoningDetail> = Vec::new();

    for (index, details) in grouped_by_index {
        let mut concatenated_text = String::new();
        let mut concatenated_summary = String::new();
        let mut signature: Option<Value> = None;
        let mut id: Option<Value> = None;
        let mut format = "unknown".to_string();
        let mut detail_type = "reasoning.text".to_string();

        for detail in &details {
            if let Some(ref text) = detail.text {
                concatenated_text.push_str(text);
            }
            if let Some(ref summary) = detail.summary {
                concatenated_summary.push_str(summary);
            }
            if detail.signature.is_some() {
                signature = detail.signature.clone();
            }
            if detail.id.is_some() {
                id = detail.id.clone();
            }
            if let Some(ref f) = detail.format {
                format = f.clone();
            }
            if !detail.detail_type.is_empty() {
                detail_type = detail.detail_type.clone();
            }
        }

        // Create consolidated entry for text
        let has_text = !concatenated_text.is_empty();
        if has_text {
            consolidated.push(ReasoningDetail {
                detail_type: detail_type.clone(),
                text: Some(concatenated_text),
                signature: signature.clone(),
                id: id.clone(),
                format: Some(format.clone()),
                index: Some(index),
                summary: None,
                data: None,
            });
        }

        // Create consolidated entry for summary (used by some providers)
        if !concatenated_summary.is_empty() && !has_text {
            consolidated.push(ReasoningDetail {
                detail_type: detail_type.clone(),
                summary: Some(concatenated_summary),
                signature: signature.clone(),
                id: id.clone(),
                format: Some(format.clone()),
                index: Some(index),
                text: None,
                data: None,
            });
        }

        // For encrypted chunks (data), only keep the last one
        let mut last_data_entry: Option<ReasoningDetail> = None;
        for detail in &details {
            if detail.data.is_some() {
                last_data_entry = Some(ReasoningDetail {
                    detail_type: detail.detail_type.clone(),
                    data: detail.data.clone(),
                    signature: if detail.signature.is_some() { detail.signature.clone() } else { None },
                    id: if detail.id.is_some() { detail.id.clone() } else { None },
                    format: Some(format.clone()),
                    index: Some(index),
                    text: None,
                    summary: None,
                });
            }
        }
        if let Some(entry) = last_data_entry {
            consolidated.push(entry);
        }
    }

    consolidated
}

// ---------------------------------------------------------------------------
// Sanitize Gemini messages
// ---------------------------------------------------------------------------

/// Sanitizes OpenAI messages for Gemini models by filtering reasoning_details
/// to only include entries that match the tool call IDs.
///
/// Source: `src/api/transform/openai-format.ts` — `sanitizeGeminiMessages`
pub fn sanitize_gemini_messages(messages: &[Value], model_id: &str) -> Vec<Value> {
    // Only sanitize for Gemini models
    if !model_id.contains("gemini") {
        return messages.to_vec();
    }

    let mut dropped_tool_call_ids = std::collections::HashSet::new();
    let mut sanitized: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if role == "assistant" {
            let tool_calls = msg.get("tool_calls").and_then(|tc| tc.as_array());
            let reasoning_details = msg.get("reasoning_details").and_then(|rd| rd.as_array());

            if let Some(calls) = tool_calls {
                if !calls.is_empty() {
                    let has_reasoning_details =
                        reasoning_details.is_some() && !reasoning_details.unwrap().is_empty();

                    if !has_reasoning_details {
                        // No reasoning_details at all — drop all tool calls
                        for tc in calls {
                            if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                dropped_tool_call_ids.insert(id.to_string());
                            }
                        }
                        // Keep any textual content, but drop the tool_calls themselves
                        if msg.get("content").and_then(|c| c.as_str()).is_some() {
                            sanitized.push(json!({
                                "role": "assistant",
                                "content": msg["content"]
                            }));
                        }
                        continue;
                    }

                    // Filter reasoning_details to only include entries matching tool call IDs
                    let rd_array = reasoning_details.unwrap();
                    let mut valid_tool_calls: Vec<Value> = Vec::new();
                    let mut valid_reasoning_details: Vec<Value> = Vec::new();

                    for tc in calls {
                        let tc_id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let matching_details: Vec<&Value> = rd_array
                            .iter()
                            .filter(|d| d.get("id").and_then(|i| i.as_str()) == Some(tc_id))
                            .collect();

                        if !matching_details.is_empty() {
                            valid_tool_calls.push(tc.clone());
                            valid_reasoning_details
                                .extend(matching_details.into_iter().cloned());
                        } else if !tc_id.is_empty() {
                            dropped_tool_call_ids.insert(tc_id.to_string());
                        }
                    }

                    // Also include reasoning_details that don't have an id (legacy format)
                    let details_without_id: Vec<&Value> = rd_array
                        .iter()
                        .filter(|d| d.get("id").is_none())
                        .collect();
                    valid_reasoning_details.extend(details_without_id.into_iter().cloned());

                    // Build the sanitized message
                    let content_str = msg
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let mut sanitized_msg = json!({
                        "role": "assistant",
                        "content": content_str
                    });

                    if !valid_reasoning_details.is_empty() {
                        sanitized_msg["reasoning_details"] =
                            serde_json::to_value(consolidate_reasoning_details_from_values(
                                &valid_reasoning_details,
                            ))
                            .unwrap_or(json!([]));
                    }

                    if !valid_tool_calls.is_empty() {
                        sanitized_msg["tool_calls"] = Value::Array(valid_tool_calls);
                    }

                    sanitized.push(sanitized_msg);
                    continue;
                }
            }
        }

        if role == "tool" {
            let tool_call_id = msg
                .get("tool_call_id")
                .and_then(|i| i.as_str())
                .unwrap_or("");
            if dropped_tool_call_ids.contains(tool_call_id) {
                // Skip tool result for dropped tool call
                continue;
            }
        }

        sanitized.push(msg.clone());
    }

    sanitized
}

/// Helper to consolidate reasoning details from JSON Values.
fn consolidate_reasoning_details_from_values(details: &[Value]) -> Vec<ReasoningDetail> {
    details
        .iter()
        .filter_map(|d| serde_json::from_value(d.clone()).ok())
        .collect::<Vec<ReasoningDetail>>()
}

// ---------------------------------------------------------------------------
// Convert to OpenAI messages
// ---------------------------------------------------------------------------

/// Options for converting Anthropic messages to OpenAI format.
///
/// Source: `src/api/transform/openai-format.ts` — `ConvertToOpenAiMessagesOptions`
pub struct ConvertToOpenAiMessagesOptions {
    /// Optional function to normalize tool call IDs for providers with strict
    /// ID requirements.
    pub normalize_tool_call_id: Option<Box<dyn Fn(&str) -> String + Send + Sync>>,
    /// If true, merge text content after tool_results into the last tool message
    /// instead of creating a separate user message.
    pub merge_tool_result_text: bool,
}

impl Default for ConvertToOpenAiMessagesOptions {
    fn default() -> Self {
        Self {
            normalize_tool_call_id: None,
            merge_tool_result_text: false,
        }
    }
}

/// Converts Anthropic-style messages to OpenAI Chat Completion message format.
///
/// Source: `src/api/transform/openai-format.ts` — `convertToOpenAiMessages`
pub fn convert_to_openai_messages(
    anthropic_messages: &[ApiMessage],
    options: Option<&ConvertToOpenAiMessagesOptions>,
) -> Result<Vec<Value>> {
    let mut openai_messages: Vec<Value> = Vec::new();

    let normalize_id = |id: &str| -> String {
        if let Some(ref f) = options.and_then(|o| o.normalize_tool_call_id.as_ref()) {
            f(id)
        } else {
            id.to_string()
        }
    };

    let merge_tool_result_text = options
        .map(|o| o.merge_tool_result_text)
        .unwrap_or(false);

    for anthropic_message in anthropic_messages {
        match anthropic_message.role {
            MessageRole::User => {
                let mut non_tool_messages: Vec<&ContentBlock> = Vec::new();
                let mut tool_messages: Vec<&ContentBlock> = Vec::new();

                for block in &anthropic_message.content {
                    match block {
                        ContentBlock::ToolResult { .. } => {
                            tool_messages.push(block);
                        }
                        ContentBlock::Text { .. } | ContentBlock::Image { .. } => {
                            non_tool_messages.push(block);
                        }
                        _ => {}
                    }
                }

                // Process tool result messages FIRST since they must follow the tool use messages
                for tool_message in &tool_messages {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = tool_message
                    {
                        // Map tool result content to a single string
                        let content_str = content
                            .iter()
                            .map(|part| match part {
                                ToolResultContent::Text { text } => text.clone(),
                                ToolResultContent::Image { .. } => {
                                    "(see following user message for image)".to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        openai_messages.push(json!({
                            "role": "tool",
                            "tool_call_id": normalize_id(tool_use_id),
                            // Use "(empty)" placeholder for empty content to satisfy providers like Gemini
                            "content": if content_str.is_empty() { "(empty)".to_string() } else { content_str }
                        }));
                    }
                }

                // Process non-tool messages
                // Filter out empty text blocks to prevent errors from Gemini
                let filtered_non_tool: Vec<&ContentBlock> = non_tool_messages
                    .into_iter()
                    .filter(|part| match part {
                        ContentBlock::Image { .. } => true,
                        ContentBlock::Text { text } => !text.is_empty(),
                        _ => false,
                    })
                    .collect();

                if !filtered_non_tool.is_empty() {
                    // Check if we should merge text into the last tool message
                    let has_only_text_content = filtered_non_tool
                        .iter()
                        .all(|p| matches!(p, ContentBlock::Text { .. }));
                    let has_tool_messages = !tool_messages.is_empty();
                    let should_merge_into_tool_message =
                        merge_tool_result_text && has_tool_messages && has_only_text_content;

                    if should_merge_into_tool_message {
                        // Merge text content into the last tool message
                        if let Some(last_msg) = openai_messages.last_mut() {
                            if last_msg.get("role").and_then(|r| r.as_str()) == Some("tool") {
                                let additional_text: String = filtered_non_tool
                                    .iter()
                                    .filter_map(|p| {
                                        if let ContentBlock::Text { text } = p {
                                            Some(text.as_str())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                let existing_content = last_msg
                                    .get("content")
                                    .and_then(|c| c.as_str())
                                    .unwrap_or("");
                                last_msg["content"] =
                                    Value::String(format!("{existing_content}\n\n{additional_text}"));
                            }
                        }
                    } else {
                        // Standard behavior: add user message with text/image content
                        let content: Vec<Value> = filtered_non_tool
                            .iter()
                            .map(|part| match part {
                                ContentBlock::Image { source } => match source {
                                    ImageSource::Base64 { data, media_type } => json!({
                                        "type": "image_url",
                                        "image_url": { "url": format!("data:{media_type};base64,{data}") }
                                    }),
                                    ImageSource::Url { url } => json!({
                                        "type": "image_url",
                                        "image_url": { "url": url }
                                    }),
                                },
                                ContentBlock::Text { text } => json!({
                                    "type": "text",
                                    "text": text
                                }),
                                _ => json!(null),
                            })
                            .filter(|v| !v.is_null())
                            .collect();

                        openai_messages.push(json!({
                            "role": "user",
                            "content": content
                        }));
                    }
                }
            }
            MessageRole::Assistant => {
                let mut non_tool_messages: Vec<&ContentBlock> = Vec::new();
                let mut tool_messages: Vec<&ContentBlock> = Vec::new();

                for block in &anthropic_message.content {
                    match block {
                        ContentBlock::ToolUse { .. } => {
                            tool_messages.push(block);
                        }
                        ContentBlock::Text { .. } | ContentBlock::Image { .. } => {
                            non_tool_messages.push(block);
                        }
                        _ => {}
                    }
                }

                // Process non-tool messages
                let content: Option<String> = if non_tool_messages.is_empty() {
                    None
                } else {
                    Some(
                        non_tool_messages
                            .iter()
                            .map(|part| match part {
                                ContentBlock::Text { text } => text.as_str(),
                                ContentBlock::Image { .. } => "", // impossible as assistant cannot send images
                                _ => "",
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                };

                // Process tool use messages
                let tool_calls: Vec<Value> = tool_messages
                    .iter()
                    .filter_map(|tool_message| {
                        if let ContentBlock::ToolUse { id, name, input } = tool_message {
                            Some(json!({
                                "id": normalize_id(id),
                                "type": "function",
                                "function": {
                                    "name": name,
                                    // JSON string
                                    "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string())
                                }
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Build message with reasoning_details BEFORE tool_calls to preserve
                // the order expected by providers like Roo. Property order matters
                // when sending messages back to some APIs.
                let mut base_message = json!({
                    "role": "assistant",
                    // Use empty string instead of undefined for providers like Gemini
                    // that require every message to have content in the "parts" field
                    "content": content.unwrap_or_default()
                });

                // Pass through reasoning_details to preserve the original shape from the API.
                // The `id` field is stripped from openai-responses-v1 blocks (see map_reasoning_details).
                if let Some(ref details) = anthropic_message.reasoning_details {
                    if let Some(mapped) = map_reasoning_details(details) {
                        base_message["reasoning_details"] = Value::Array(mapped);
                    }
                }

                // Cannot be an empty array. API expects minimum length 1
                if !tool_calls.is_empty() {
                    base_message["tool_calls"] = Value::Array(tool_calls);
                }

                openai_messages.push(base_message);
            }
        }
    }

    Ok(openai_messages)
}

// ---------------------------------------------------------------------------
// Reasoning detail helpers
// ---------------------------------------------------------------------------

/// Strips the `id` field from `openai-responses-v1` reasoning detail blocks.
///
/// OpenAI's Responses API requires `store: true` to persist reasoning blocks.
/// Since we manage conversation state client-side, we don't use `store: true`,
/// and sending back the `id` field causes a 404 error.
///
/// Source: `src/api/transform/openai-format.ts` — `mapReasoningDetails`
pub fn map_reasoning_details(details: &[Value]) -> Option<Vec<Value>> {
    if details.is_empty() {
        return None;
    }

    let mapped: Vec<Value> = details
        .iter()
        .map(|detail| {
            // Strip `id` from openai-responses-v1 blocks
            if detail.get("format").and_then(|f| f.as_str()) == Some("openai-responses-v1")
                && detail.get("id").is_some()
            {
                let mut obj = detail
                    .as_object()
                    .cloned()
                    .expect("detail should be an object");
                obj.remove("id");
                Value::Object(obj)
            } else {
                detail.clone()
            }
        })
        .collect();

    Some(mapped)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ContentBlock, ImageSource, MessageRole, ToolResultContent};

    // -- Helper builders -------------------------------------------------------

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

    fn image_block(data: &str, media_type: &str) -> ContentBlock {
        ContentBlock::Image {
            source: ImageSource::Base64 {
                data: data.to_string(),
                media_type: media_type.to_string(),
            },
        }
    }

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

    // -- convert_to_openai_messages tests --------------------------------------

    #[test]
    fn test_simple_user_message() {
        let messages = vec![make_user_message(vec![text_block("Hello")])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello");
    }

    #[test]
    fn test_simple_assistant_message() {
        let messages = vec![make_assistant_message(vec![text_block("Hi there!")])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"], "Hi there!");
    }

    #[test]
    fn test_assistant_with_tool_calls() {
        let messages = vec![make_assistant_message(vec![
            text_block("Let me check."),
            tool_use_block("call_123", "read_file", r#"{"path":"test.rs"}"#),
        ])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"], "Let me check.");

        let tool_calls = result[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_123");
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "read_file");
    }

    #[test]
    fn test_user_with_tool_result() {
        let messages = vec![make_user_message(vec![
            tool_result_block("call_123", "file contents here"),
            text_block("Additional info"),
        ])];
        let result = convert_to_openai_messages(&messages, None).unwrap();

        // Should have: tool message + user message
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "tool");
        assert_eq!(result[0]["tool_call_id"], "call_123");
        assert_eq!(result[0]["content"], "file contents here");

        assert_eq!(result[1]["role"], "user");
        let content = result[1]["content"].as_array().unwrap();
        assert_eq!(content[0]["text"], "Additional info");
    }

    #[test]
    fn test_empty_tool_result_gets_placeholder() {
        let messages = vec![make_user_message(vec![ContentBlock::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: vec![],
            is_error: None,
        }])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        assert_eq!(result[0]["content"], "(empty)");
    }

    #[test]
    fn test_empty_text_blocks_filtered() {
        let messages = vec![make_user_message(vec![
            text_block(""),
            text_block("visible"),
            text_block(""),
        ])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        let content = result[0]["content"].as_array().unwrap();
        // Only non-empty text blocks should be included
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["text"], "visible");
    }

    #[test]
    fn test_image_conversion() {
        let messages = vec![make_user_message(vec![
            text_block("See this"),
            image_block("iVBORw0KGgo=", "image/png"),
        ])];
        let result = convert_to_openai_messages(&messages, None).unwrap();
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[1]["type"], "image_url");
        assert!(content[1]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_merge_tool_result_text_option() {
        let messages = vec![make_user_message(vec![
            tool_result_block("call_1", "result"),
            text_block("env details"),
        ])];
        let options = ConvertToOpenAiMessagesOptions {
            normalize_tool_call_id: None,
            merge_tool_result_text: true,
        };
        let result = convert_to_openai_messages(&messages, Some(&options)).unwrap();

        // Should merge text into the tool message
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
        let content = result[0]["content"].as_str().unwrap();
        assert!(content.contains("result"));
        assert!(content.contains("env details"));
    }

    #[test]
    fn test_normalize_tool_call_id() {
        let messages = vec![make_user_message(vec![ContentBlock::ToolResult {
            tool_use_id: "original_id".to_string(),
            content: vec![ToolResultContent::Text {
                text: "result".to_string(),
            }],
            is_error: None,
        }])];
        let options = ConvertToOpenAiMessagesOptions {
            normalize_tool_call_id: Some(Box::new(|id| format!("norm_{id}"))),
            merge_tool_result_text: false,
        };
        let result = convert_to_openai_messages(&messages, Some(&options)).unwrap();
        assert_eq!(result[0]["tool_call_id"], "norm_original_id");
    }

    // -- consolidate_reasoning_details tests -----------------------------------

    #[test]
    fn test_consolidate_empty() {
        let result = consolidate_reasoning_details(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_consolidate_drops_corrupted_encrypted() {
        let details = vec![ReasoningDetail {
            detail_type: "reasoning.encrypted".to_string(),
            text: None,
            summary: None,
            data: None, // missing data → corrupted
            signature: None,
            id: None,
            format: None,
            index: Some(0),
        }];
        let result = consolidate_reasoning_details(&details);
        assert!(result.is_empty());
    }

    #[test]
    fn test_consolidate_text_blocks() {
        let details = vec![
            ReasoningDetail {
                detail_type: "reasoning.text".to_string(),
                text: Some("Hello ".to_string()),
                summary: None,
                data: None,
                signature: None,
                id: None,
                format: Some("anthropic-claude-v1".to_string()),
                index: Some(0),
            },
            ReasoningDetail {
                detail_type: "reasoning.text".to_string(),
                text: Some("World".to_string()),
                summary: None,
                data: None,
                signature: None,
                id: None,
                format: Some("anthropic-claude-v1".to_string()),
                index: Some(0),
            },
        ];
        let result = consolidate_reasoning_details(&details);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text.as_deref(), Some("Hello World"));
        assert_eq!(result[0].format.as_deref(), Some("anthropic-claude-v1"));
    }

    #[test]
    fn test_consolidate_encrypted_keeps_last() {
        let details = vec![
            ReasoningDetail {
                detail_type: "reasoning.encrypted".to_string(),
                text: None,
                summary: None,
                data: Some("data1".to_string()),
                signature: None,
                id: None,
                format: None,
                index: Some(0),
            },
            ReasoningDetail {
                detail_type: "reasoning.encrypted".to_string(),
                text: None,
                summary: None,
                data: Some("data2".to_string()),
                signature: None,
                id: None,
                format: None,
                index: Some(0),
            },
        ];
        let result = consolidate_reasoning_details(&details);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].data.as_deref(), Some("data2"));
    }

    // -- sanitize_gemini_messages tests ----------------------------------------

    #[test]
    fn test_sanitize_non_gemini_unchanged() {
        let messages = vec![json!({"role": "user", "content": "hello"})];
        let result = sanitize_gemini_messages(&messages, "gpt-4");
        assert_eq!(result, messages);
    }

    #[test]
    fn test_sanitize_gemini_drops_tool_calls_without_reasoning() {
        let messages = vec![json!({
            "role": "assistant",
            "content": "text",
            "tool_calls": [{"id": "tc1", "function": {"name": "test"}}]
        })];
        let result = sanitize_gemini_messages(&messages, "gemini-2.5-pro");
        assert_eq!(result.len(), 1);
        assert!(result[0].get("tool_calls").is_none());
        assert_eq!(result[0]["content"], "text");
    }

    #[test]
    fn test_sanitize_gemini_keeps_matched_tool_calls() {
        let messages = vec![json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{"id": "tc1", "function": {"name": "test"}}],
            "reasoning_details": [{"type": "reasoning.text", "text": "thinking", "id": "tc1"}]
        })];
        let result = sanitize_gemini_messages(&messages, "gemini-2.5-pro");
        assert_eq!(result.len(), 1);
        assert!(result[0].get("tool_calls").is_some());
    }

    #[test]
    fn test_sanitize_gemini_drops_orphaned_tool_results() {
        let messages = vec![
            json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{"id": "tc1", "function": {"name": "test"}}]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "tc1",
                "content": "result"
            }),
        ];
        let result = sanitize_gemini_messages(&messages, "gemini-2.5-pro");
        // The tool result for tc1 should be dropped since tc1 was dropped
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
    }

    // -- map_reasoning_details tests -------------------------------------------

    #[test]
    fn test_map_reasoning_details_empty() {
        assert!(map_reasoning_details(&[]).is_none());
    }

    #[test]
    fn test_map_reasoning_details_strips_openai_responses_v1_id() {
        let details = vec![json!({
            "type": "reasoning.text",
            "text": "thinking",
            "format": "openai-responses-v1",
            "id": "rs_abc123"
        })];
        let result = map_reasoning_details(&details).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].get("id").is_none());
        assert_eq!(result[0]["text"], "thinking");
    }

    #[test]
    fn test_map_reasoning_details_preserves_non_openai_format() {
        let details = vec![json!({
            "type": "reasoning.text",
            "text": "thinking",
            "format": "anthropic-claude-v1",
            "id": "rs_abc123"
        })];
        let result = map_reasoning_details(&details).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["id"], "rs_abc123");
    }
}
