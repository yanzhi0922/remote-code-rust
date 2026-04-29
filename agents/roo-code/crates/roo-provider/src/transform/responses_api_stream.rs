//! Async stream processing for OpenAI Responses API events.
//!
//! Derived from `src/api/transform/responses-api-stream.ts`.
//!
//! Processes Responses API stream events and yields [`ApiStreamChunk`]s.
//! This is a shared utility for providers that use OpenAI's Responses API
//! (`POST /v1/responses` with `stream: true`).
//!
//! # Event types handled
//! - Text deltas (`response.output_text.delta`, `response.text.delta`)
//! - Reasoning deltas (`response.reasoning_text.delta`, etc.)
//! - Tool/function calls (`response.output_item.done` with `function_call` type)
//! - Function call argument deltas (`response.function_call_arguments.delta`, etc.)
//! - Usage data (`response.completed`, `response.done`)

use futures::Stream;
use serde_json::Value;

use roo_types::api::ApiStreamChunk;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Stream processing
// ---------------------------------------------------------------------------

/// Processes a stream of Responses API JSON events into [`ApiStreamChunk`]s.
///
/// This is the Rust equivalent of the TypeScript `processResponsesApiStream`
/// async generator. It reads raw JSON events from the input stream and
/// converts them into standardized API stream chunks.
///
/// # Arguments
/// * `stream` - A stream of raw JSON Values (SSE event payloads)
/// * `normalize_usage` - A function that converts a raw usage JSON object into
///   an optional [`ApiStreamChunk::Usage`]
///
/// # Example
/// ```rust,ignore
/// use roo_provider::transform::responses_api_stream::process_responses_api_stream;
///
/// let chunks: Vec<ApiStreamChunk> = process_responses_api_stream(
///     event_stream,
///     |usage| normalize_usage(usage),
/// ).await?;
/// ```
pub async fn process_responses_api_stream<S>(
    stream: S,
    normalize_usage: impl Fn(&Value) -> Option<ApiStreamChunk>,
) -> Result<Vec<ApiStreamChunk>>
where
    S: Stream<Item = Result<Value>>,
{
    use futures::StreamExt;

    let mut output = Vec::new();
    let mut stream = Box::pin(stream);

    while let Some(item) = stream.next().await {
        let event = item?;

        if let Some(chunk) = process_single_event(&event, &normalize_usage) {
            output.push(chunk);
        }
    }

    Ok(output)
}

/// Process a single Responses API event into an optional [`ApiStreamChunk`].
///
/// This handles all the event type matching logic from the TS source:
/// - Text content deltas
/// - Reasoning deltas
/// - Output item events (completed function calls)
/// - Function call argument deltas
/// - Completion events (usage)
fn process_single_event(
    event: &Value,
    normalize_usage: &impl Fn(&Value) -> Option<ApiStreamChunk>,
) -> Option<ApiStreamChunk> {
    let event_type = event["type"].as_str().unwrap_or("");

    // Text content deltas
    if event_type == "response.output_text.delta" || event_type == "response.text.delta" {
        if let Some(delta) = event["delta"].as_str() {
            if !delta.is_empty() {
                return Some(ApiStreamChunk::Text {
                    text: delta.to_string(),
                });
            }
        }
        return None;
    }

    // Reasoning deltas
    if event_type == "response.reasoning_text.delta"
        || event_type == "response.reasoning.delta"
        || event_type == "response.reasoning_summary_text.delta"
        || event_type == "response.reasoning_summary.delta"
    {
        if let Some(delta) = event["delta"].as_str() {
            if !delta.is_empty() {
                return Some(ApiStreamChunk::Reasoning {
                    text: delta.to_string(),
                    signature: None,
                });
            }
        }
        return None;
    }

    // Output item events — handle completed function calls
    if event_type == "response.output_item.done" {
        let item = &event["item"];
        let item_type = item["type"].as_str().unwrap_or("");

        if item_type == "function_call" || item_type == "tool_call" {
            let call_id = item
                .get("call_id")
                .or_else(|| item.get("tool_call_id"))
                .or_else(|| item.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let name = item
                .get("name")
                .or_else(|| item.get("function").and_then(|f| f.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let args_raw = item
                .get("arguments")
                .or_else(|| item.get("function").and_then(|f| f.get("arguments")))
                .or_else(|| item.get("input"));

            let args = match args_raw {
                Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
                Some(v) if v.is_object() => v.to_string(),
                _ => String::new(),
            };

            if !call_id.is_empty() && !name.is_empty() {
                return Some(ApiStreamChunk::ToolCall {
                    id: call_id.to_string(),
                    name: name.to_string(),
                    arguments: args,
                });
            }
        }
        return None;
    }

    // Function call argument deltas (for streaming tool calls)
    if event_type == "response.function_call_arguments.delta"
        || event_type == "response.tool_call_arguments.delta"
    {
        let call_id = event
            .get("call_id")
            .or_else(|| event.get("tool_call_id"))
            .or_else(|| event.get("id"))
            .or_else(|| event.get("item_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let name = event
            .get("name")
            .or_else(|| event.get("function_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let delta = event
            .get("delta")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let index = event["index"].as_u64().unwrap_or(0);

        if !call_id.is_empty() {
            return Some(ApiStreamChunk::ToolCallPartial {
                index,
                id: Some(call_id.to_string()),
                name: if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                },
                arguments: if delta.is_empty() {
                    None
                } else {
                    Some(delta.to_string())
                },
            });
        }
        return None;
    }

    // Completion events — extract usage
    if event_type == "response.completed" || event_type == "response.done" {
        let usage = event
            .get("response")
            .and_then(|r| r.get("usage"))
            .or_else(|| event.get("usage"));

        if let Some(usage_data) = usage {
            return normalize_usage(usage_data);
        }
        return None;
    }

    None
}

// ---------------------------------------------------------------------------
// Usage normalizer factory
// ---------------------------------------------------------------------------

/// Creates a standard usage normalizer for providers with per-token pricing.
///
/// Extracts input/output tokens, cache tokens, reasoning tokens, and computes cost.
///
/// # Arguments
/// * `calculate_cost` - Optional function to compute total cost from token counts.
///
/// # Example
/// ```rust,ignore
/// let normalizer = create_usage_normalizer(None);
/// let chunk = normalizer(&usage_json);
/// ```
pub fn create_usage_normalizer(
    calculate_cost: Option<fn(u64, u64, u64) -> f64>,
) -> impl Fn(&Value) -> Option<ApiStreamChunk> {
    move |usage: &Value| -> Option<ApiStreamChunk> {
        if usage.is_null() {
            return None;
        }

        let input_details = usage
            .get("input_tokens_details")
            .or_else(|| usage.get("prompt_tokens_details"));

        let cached_tokens = input_details
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let input_tokens = usage
            .get("input_tokens")
            .or_else(|| usage.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let output_tokens = usage
            .get("output_tokens")
            .or_else(|| usage.get("completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let cache_read_tokens = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(cached_tokens);

        let cache_write_tokens = usage
            .get("cache_creation_input_tokens")
            .or_else(|| usage.get("cache_write_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let reasoning_tokens = usage
            .get("output_tokens_details")
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(|v| v.as_u64());

        let total_cost = calculate_cost.map(|f| f(input_tokens, output_tokens, cache_read_tokens));

        Some(ApiStreamChunk::Usage {
            input_tokens,
            output_tokens,
            cache_write_tokens: if cache_write_tokens > 0 {
                Some(cache_write_tokens)
            } else {
                None
            },
            cache_read_tokens: if cache_read_tokens > 0 {
                Some(cache_read_tokens)
            } else {
                None
            },
            reasoning_tokens,
            total_cost,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_text_delta_event() {
        let event = json!({
            "type": "response.output_text.delta",
            "delta": "Hello, world!"
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Text { text }) => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected Text chunk"),
        }
    }

    #[test]
    fn test_text_delta_alternate_type() {
        let event = json!({
            "type": "response.text.delta",
            "delta": "Alternate text"
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Text { text }) => assert_eq!(text, "Alternate text"),
            _ => panic!("Expected Text chunk"),
        }
    }

    #[test]
    fn test_reasoning_delta_event() {
        let event = json!({
            "type": "response.reasoning_text.delta",
            "delta": "Thinking..."
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Reasoning { text, signature }) => {
                assert_eq!(text, "Thinking...");
                assert!(signature.is_none());
            }
            _ => panic!("Expected Reasoning chunk"),
        }
    }

    #[test]
    fn test_reasoning_summary_delta_event() {
        let event = json!({
            "type": "response.reasoning_summary_text.delta",
            "delta": "Summary..."
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Reasoning { text, .. }) => assert_eq!(text, "Summary..."),
            _ => panic!("Expected Reasoning chunk"),
        }
    }

    #[test]
    fn test_function_call_event() {
        let event = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "call_id": "call_123",
                "name": "read_file",
                "arguments": "{\"path\": \"/test.txt\"}"
            }
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::ToolCall {
                id,
                name,
                arguments,
            }) => {
                assert_eq!(id, "call_123");
                assert_eq!(name, "read_file");
                assert_eq!(arguments, "{\"path\": \"/test.txt\"}");
            }
            _ => panic!("Expected ToolCall chunk"),
        }
    }

    #[test]
    fn test_function_call_with_object_args() {
        let event = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "call_id": "call_456",
                "name": "write_file",
                "input": {"path": "/out.txt", "content": "hello"}
            }
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::ToolCall { arguments, .. }) => {
                // Arguments should be serialized JSON
                let parsed: Value = serde_json::from_str(&arguments).unwrap();
                assert_eq!(parsed["path"], "/out.txt");
            }
            _ => panic!("Expected ToolCall chunk"),
        }
    }

    #[test]
    fn test_tool_call_partial_event() {
        let event = json!({
            "type": "response.function_call_arguments.delta",
            "call_id": "call_789",
            "name": "search",
            "delta": "{\"qu",
            "index": 0
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::ToolCallPartial {
                index,
                id,
                name,
                arguments,
            }) => {
                assert_eq!(index, 0);
                assert_eq!(id, Some("call_789".to_string()));
                assert_eq!(name, Some("search".to_string()));
                assert_eq!(arguments, Some("{\"qu".to_string()));
            }
            _ => panic!("Expected ToolCallPartial chunk"),
        }
    }

    #[test]
    fn test_usage_event() {
        let event = json!({
            "type": "response.completed",
            "response": {
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "input_tokens_details": {
                        "cached_tokens": 30
                    }
                }
            }
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                ..
            }) => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 50);
                assert_eq!(cache_read_tokens, Some(30));
            }
            _ => panic!("Expected Usage chunk"),
        }
    }

    #[test]
    fn test_usage_event_with_cost() {
        let event = json!({
            "type": "response.done",
            "usage": {
                "input_tokens": 200,
                "output_tokens": 100
            }
        });
        let normalizer = create_usage_normalizer(Some(|input, output, _cache| {
            (input as f64) * 0.00001 + (output as f64) * 0.00003
        }));
        let result = process_single_event(&event, &normalizer);
        match result {
            Some(ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                total_cost,
                ..
            }) => {
                assert_eq!(input_tokens, 200);
                assert_eq!(output_tokens, 100);
                assert!(total_cost.is_some());
                let cost = total_cost.unwrap();
                assert!((cost - 0.005).abs() < 0.0001);
            }
            _ => panic!("Expected Usage chunk"),
        }
    }

    #[test]
    fn test_empty_delta_ignored() {
        let event = json!({
            "type": "response.output_text.delta",
            "delta": ""
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        assert!(result.is_none());
    }

    #[test]
    fn test_unknown_event_type_ignored() {
        let event = json!({
            "type": "response.unknown_event",
            "data": "something"
        });
        let normalizer = create_usage_normalizer(None);
        let result = process_single_event(&event, &normalizer);
        assert!(result.is_none());
    }

    #[test]
    fn test_create_usage_normalizer_null_usage() {
        let normalizer = create_usage_normalizer(None);
        let result = normalizer(&Value::Null);
        assert!(result.is_none());
    }

    #[test]
    fn test_create_usage_normalizer_with_reasoning_tokens() {
        let usage = json!({
            "input_tokens": 500,
            "output_tokens": 200,
            "output_tokens_details": {
                "reasoning_tokens": 150
            }
        });
        let normalizer = create_usage_normalizer(None);
        let result = normalizer(&usage);
        match result {
            Some(ApiStreamChunk::Usage {
                reasoning_tokens, ..
            }) => {
                assert_eq!(reasoning_tokens, Some(150));
            }
            _ => panic!("Expected Usage chunk with reasoning tokens"),
        }
    }
}
