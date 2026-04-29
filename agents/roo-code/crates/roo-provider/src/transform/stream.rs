//! Generic stream processing utilities for API responses.
//!
//! Derived from `src/api/transform/stream.ts`.
//!
//! Provides stream types, helpers, and processing utilities used across
//! all provider implementations for handling streaming API responses.

use futures::Stream;
use serde_json::Value;

use roo_types::api::ApiStreamChunk;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Stream helpers
// ---------------------------------------------------------------------------

/// Collects all chunks from an API stream into a vector.
///
/// This is a convenience function for tests and non-streaming use cases
/// where the full response needs to be materialized.
///
/// # Arguments
/// * `stream` - A stream of `Result<ApiStreamChunk>` items
///
/// # Returns
/// A vector of all successfully parsed chunks, or an error on the first failure.
pub async fn collect_stream<S>(stream: S) -> Result<Vec<ApiStreamChunk>>
where
    S: Stream<Item = Result<ApiStreamChunk>>,
{
    use futures::StreamExt;
    let mut output = Vec::new();
    let mut pin = Box::pin(stream);
    while let Some(item) = pin.next().await {
        output.push(item?);
    }
    Ok(output)
}

/// Extracts all text content from a stream of API chunks.
///
/// Concatenates all [`ApiStreamChunk::Text`] chunks into a single string.
pub fn extract_text_from_chunks(chunks: &[ApiStreamChunk]) -> String {
    chunks
        .iter()
        .filter_map(|chunk| match chunk {
            ApiStreamChunk::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Extracts all reasoning content from a stream of API chunks.
///
/// Concatenates all [`ApiStreamChunk::Reasoning`] chunks into a single string.
pub fn extract_reasoning_from_chunks(chunks: &[ApiStreamChunk]) -> String {
    chunks
        .iter()
        .filter_map(|chunk| match chunk {
            ApiStreamChunk::Reasoning { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Extracts all tool calls from a stream of API chunks.
///
/// Returns a vector of `(id, name, arguments)` tuples from all
/// [`ApiStreamChunk::ToolCall`] chunks.
pub fn extract_tool_calls_from_chunks(chunks: &[ApiStreamChunk]) -> Vec<(String, String, String)> {
    chunks
        .iter()
        .filter_map(|chunk| match chunk {
            ApiStreamChunk::ToolCall {
                id,
                name,
                arguments,
            } => Some((id.clone(), name.clone(), arguments.clone())),
            _ => None,
        })
        .collect()
}

/// Extracts the first usage chunk from a stream of API chunks.
///
/// Returns `None` if no usage information was found.
pub fn extract_usage_from_chunks(chunks: &[ApiStreamChunk]) -> Option<&ApiStreamChunk> {
    chunks.iter().find(|chunk| matches!(chunk, ApiStreamChunk::Usage { .. }))
}

/// Processes a stream of raw SSE JSON events into API stream chunks.
///
/// This is a generic processor that handles the common SSE event patterns
/// used by most OpenAI-compatible providers.
///
/// # Arguments
/// * `stream` - Stream of raw JSON event payloads
/// * `processor` - Function that converts each JSON event into optional chunks
pub async fn process_json_stream<S, F>(
    stream: S,
    processor: F,
) -> Result<Vec<ApiStreamChunk>>
where
    S: Stream<Item = Result<Value>>,
    F: Fn(&Value) -> Option<ApiStreamChunk>,
{
    use futures::StreamExt;
    let mut output = Vec::new();
    let mut pin = Box::pin(stream);
    while let Some(item) = pin.next().await {
        let event = item?;
        if let Some(chunk) = processor(&event) {
            output.push(chunk);
        }
    }
    Ok(output)
}

/// Merges partial tool call chunks into complete tool calls.
///
/// Takes a slice of chunks that may contain [`ApiStreamChunk::ToolCallPartial`]
/// entries and merges them into complete [`ApiStreamChunk::ToolCall`] entries.
/// This replicates the behavior of `NativeToolCallParser` in the TS source.
pub fn merge_partial_tool_calls(chunks: &[ApiStreamChunk]) -> Vec<ApiStreamChunk> {
    use std::collections::HashMap;

    let mut result: Vec<ApiStreamChunk> = Vec::new();
    let mut pending: HashMap<String, (String, String, String)> = HashMap::new(); // id -> (id, name, args_buffer)

    for chunk in chunks {
        match chunk {
            ApiStreamChunk::ToolCallPartial {
                id,
                name,
                arguments,
                ..
            } => {
                if let Some(call_id) = id {
                    let entry = pending.entry(call_id.clone()).or_insert_with(|| {
                        (call_id.clone(), String::new(), String::new())
                    });
                    if let Some(n) = name {
                        entry.1 = n.clone();
                    }
                    if let Some(args) = arguments {
                        entry.2.push_str(args);
                    }
                }
            }
            ApiStreamChunk::ToolCall { .. } => {
                result.push(chunk.clone());
            }
            _ => {
                result.push(chunk.clone());
            }
        }
    }

    // Flush any remaining partial calls as complete calls
    for (_key, (id, name, args)) in pending {
        if !id.is_empty() && !name.is_empty() {
            result.push(ApiStreamChunk::ToolCall {
                id,
                name,
                arguments: args,
            });
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

    #[test]
    fn test_extract_text_from_chunks() {
        let chunks = vec![
            ApiStreamChunk::Text {
                text: "Hello, ".to_string(),
            },
            ApiStreamChunk::Text {
                text: "world!".to_string(),
            },
            ApiStreamChunk::Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_write_tokens: None,
                cache_read_tokens: None,
                reasoning_tokens: None,
                total_cost: None,
            },
        ];
        assert_eq!(extract_text_from_chunks(&chunks), "Hello, world!");
    }

    #[test]
    fn test_extract_reasoning_from_chunks() {
        let chunks = vec![
            ApiStreamChunk::Reasoning {
                text: "Step 1: ".to_string(),
                signature: None,
            },
            ApiStreamChunk::Reasoning {
                text: "Analyze...".to_string(),
                signature: None,
            },
            ApiStreamChunk::Text {
                text: "Result".to_string(),
            },
        ];
        assert_eq!(extract_reasoning_from_chunks(&chunks), "Step 1: Analyze...");
    }

    #[test]
    fn test_extract_tool_calls_from_chunks() {
        let chunks = vec![
            ApiStreamChunk::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
            ApiStreamChunk::Text {
                text: "done".to_string(),
            },
            ApiStreamChunk::ToolCall {
                id: "call_2".to_string(),
                name: "write_file".to_string(),
                arguments: "{\"path\":\"/a\"}".to_string(),
            },
        ];
        let calls = extract_tool_calls_from_chunks(&chunks);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, "read_file");
        assert_eq!(calls[1].1, "write_file");
    }

    #[test]
    fn test_extract_usage_from_chunks() {
        let chunks = vec![
            ApiStreamChunk::Text {
                text: "hi".to_string(),
            },
            ApiStreamChunk::Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_write_tokens: None,
                cache_read_tokens: None,
                reasoning_tokens: None,
                total_cost: None,
            },
        ];
        let usage = extract_usage_from_chunks(&chunks);
        assert!(usage.is_some());
    }

    #[test]
    fn test_merge_partial_tool_calls() {
        let chunks = vec![
            ApiStreamChunk::ToolCallPartial {
                index: 0,
                id: Some("call_1".to_string()),
                name: Some("search".to_string()),
                arguments: Some("{\"q\":".to_string()),
            },
            ApiStreamChunk::ToolCallPartial {
                index: 0,
                id: Some("call_1".to_string()),
                name: None,
                arguments: Some("\"test\"}".to_string()),
            },
            ApiStreamChunk::Text {
                text: "result".to_string(),
            },
        ];
        let merged = merge_partial_tool_calls(&chunks);
        assert_eq!(merged.len(), 2); // 1 ToolCall + 1 Text
        let tool_call = merged.iter().find(|c| matches!(c, ApiStreamChunk::ToolCall { .. }));
        assert!(tool_call.is_some());
        if let Some(ApiStreamChunk::ToolCall { arguments, .. }) = tool_call {
            assert_eq!(arguments, "{\"q\":\"test\"}");
        }
    }

    #[test]
    fn test_extract_text_empty_chunks() {
        let chunks: Vec<ApiStreamChunk> = vec![];
        assert_eq!(extract_text_from_chunks(&chunks), "");
    }

    #[test]
    fn test_extract_usage_no_usage() {
        let chunks = vec![ApiStreamChunk::Text {
            text: "no usage".to_string(),
        }];
        assert!(extract_usage_from_chunks(&chunks).is_none());
    }
}
