//! Stream parser for API streaming responses.
//!
//! Parses a stream of [`ApiStreamChunk`] into structured content blocks:
//! accumulated text, complete tool calls, thinking blocks, and usage info.
//!
//! This module precisely replicates the TypeScript `NativeToolCallParser` class
//! which handles:
//! - Raw chunk processing (converting `tool_call_partial` to start/delta/end events)
//! - Streaming tool call argument accumulation (keyed by tool call ID)
//! - Tool call finalization (parsing complete JSON arguments)
//!
//! Source: `src/core/assistant-message/NativeToolCallParser.ts`

use std::collections::HashMap;

use roo_types::api::{ApiStreamChunk, ContentBlock, GroundingSource};

use crate::types::{
    AssistantMessageContent, McpToolUse, RawChunkTrackerEntry, StreamingToolCallState, ToolUse,
    ToolCallStreamEvent, is_mcp_tool_name, normalize_mcp_tool_name, parse_mcp_tool_name,
    is_valid_tool_param,
};

// ---------------------------------------------------------------------------
// ParsedToolCall
// ---------------------------------------------------------------------------

/// A fully accumulated tool call extracted from the stream.
#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    /// Unique tool call ID assigned by the API.
    pub id: String,
    /// Tool name (e.g. "read_file", "write_to_file").
    pub name: String,
    /// Raw JSON arguments string.
    pub arguments: String,
}

impl ParsedToolCall {
    /// Parse the arguments string into a JSON value.
    ///
    /// Returns an empty JSON object `{}` if parsing fails, since the Anthropic
    /// API requires `tool_use.input` to be a valid dictionary (not null).
    pub fn parse_arguments(&self) -> serde_json::Value {
        serde_json::from_str(&self.arguments).unwrap_or(serde_json::Value::Object(Default::default()))
    }
}

// ---------------------------------------------------------------------------
// ThinkingBlock
// ---------------------------------------------------------------------------

/// A thinking / reasoning block extracted from the stream.
#[derive(Debug, Clone)]
pub struct ThinkingBlock {
    /// The thinking text.
    pub text: String,
    /// Optional signature (Anthropic extended thinking).
    pub signature: Option<String>,
}

// ---------------------------------------------------------------------------
// ParsedStreamContent
// ---------------------------------------------------------------------------

/// The aggregated result of parsing a complete stream response.
#[derive(Debug, Clone, Default)]
pub struct ParsedStreamContent {
    /// Accumulated text content.
    pub text: String,
    /// Complete tool calls.
    pub tool_calls: Vec<ParsedToolCall>,
    /// Thinking / reasoning blocks.
    pub thinking_blocks: Vec<ThinkingBlock>,
    /// Token usage (input, output, cache, cost).
    pub usage: Option<StreamUsage>,
    /// Grounding sources (Gemini).
    pub grounding_sources: Vec<GroundingSource>,
    /// Stream error, if any.
    pub error: Option<StreamError>,
}

// ---------------------------------------------------------------------------
// StreamUsage
// ---------------------------------------------------------------------------

/// Token usage information from the stream.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub total_cost: Option<f64>,
}

// ---------------------------------------------------------------------------
// StreamError
// ---------------------------------------------------------------------------

/// An error from the stream.
#[derive(Debug, Clone)]
pub struct StreamError {
    pub error: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// PendingToolCall — internal state for partial tool calls (index-based)
// ---------------------------------------------------------------------------

/// Internal state tracking a partially accumulated tool call (index-based).
#[derive(Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

// ---------------------------------------------------------------------------
// StreamParser — replicates NativeToolCallParser
// ---------------------------------------------------------------------------

/// Parses a stream of [`ApiStreamChunk`] into [`ParsedStreamContent`].
///
/// This struct precisely replicates the TypeScript `NativeToolCallParser` class:
///
/// ## Raw Chunk Processing
/// - [`process_raw_chunk()`] — converts `tool_call_partial` chunks to start/delta/end events
/// - [`process_finish_reason()`] — emits end events when finish_reason is 'tool_calls'
/// - [`finalize_raw_chunks()`] — finalizes any remaining tool calls
/// - [`clear_raw_chunk_state()`] — clears raw chunk tracking state
///
/// ## Streaming Tool Call Accumulation
/// - [`start_streaming_tool_call()`] — starts tracking a streaming tool call
/// - [`process_streaming_chunk()`] — processes a streaming chunk with partial JSON parsing
/// - [`finalize_streaming_tool_call()`] — finalizes a streaming tool call
/// - [`clear_all_streaming_tool_calls()`] — clears all streaming tool call state
///
/// ## Direct Chunk Feeding
/// - [`feed_chunk()`] — feed a single chunk into the parser
/// - [`finalize()`] — finalize the stream and return parsed content
///
/// Source: `src/core/assistant-message/NativeToolCallParser.ts`
pub struct StreamParser {
    // --- Text accumulation ---
    /// Accumulated text content.
    text: String,

    // --- Complete tool calls ---
    /// Complete tool calls (accumulated from ToolCall or ToolCallPartial sequences).
    tool_calls: Vec<ParsedToolCall>,

    // --- Index-based partial tool call tracking ---
    /// Pending partial tool calls being accumulated (keyed by index).
    /// Used for the `ToolCallPartial` chunk type (index-based tracking).
    pending_tool_calls: HashMap<u64, PendingToolCall>,

    // --- ID-based active tool call tracking (start/delta/end pattern) ---
    /// Active tool calls being accumulated via start/delta/end pattern.
    /// Keyed by tool call ID to support multiple concurrent tool calls.
    ///
    /// Source: TS `NativeToolCallParser.streamingToolCalls`
    active_tool_calls: HashMap<String, PendingToolCall>,

    // --- Raw chunk tracking (NativeToolCallParser.rawChunkTracker) ---
    /// Raw chunk tracking state (keyed by index from API stream).
    ///
    /// Source: TS `NativeToolCallParser.rawChunkTracker`
    raw_chunk_tracker: HashMap<u64, RawChunkTrackerEntry>,

    // --- Streaming tool call state (NativeToolCallParser.streamingToolCalls) ---
    /// Streaming tool call state for argument accumulation (keyed by tool call ID).
    ///
    /// Source: TS `NativeToolCallParser.streamingToolCalls`
    streaming_tool_calls: HashMap<String, StreamingToolCallState>,

    // --- Thinking blocks ---
    /// Thinking / reasoning blocks.
    thinking_blocks: Vec<ThinkingBlock>,
    /// Current thinking text being accumulated.
    current_thinking: Option<String>,
    /// Current thinking signature (set when thinking_complete is received).
    current_thinking_signature: Option<String>,

    // --- Usage and errors ---
    /// Token usage.
    usage: Option<StreamUsage>,
    /// Grounding sources.
    grounding_sources: Vec<GroundingSource>,
    /// Stream error.
    error: Option<StreamError>,
}

impl StreamParser {
    /// Create a new stream parser.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            tool_calls: Vec::new(),
            pending_tool_calls: HashMap::new(),
            active_tool_calls: HashMap::new(),
            raw_chunk_tracker: HashMap::new(),
            streaming_tool_calls: HashMap::new(),
            thinking_blocks: Vec::new(),
            current_thinking: None,
            current_thinking_signature: None,
            usage: None,
            grounding_sources: Vec::new(),
            error: None,
        }
    }

    // ===================================================================
    // Raw Chunk Processing
    // Source: `NativeToolCallParser.ts` — processRawChunk, processFinishReason,
    //         finalizeRawChunks, clearRawChunkState
    // ===================================================================

    /// Process a raw tool call chunk from the API stream.
    ///
    /// Handles tracking, buffering, and emits start/delta/end events.
    /// This is the entry point for providers that emit `tool_call_partial` chunks.
    ///
    /// Returns an array of events to be processed by the consumer.
    ///
    /// Source: `NativeToolCallParser.ts` — `processRawChunk()`
    pub fn process_raw_chunk(
        &mut self,
        index: u64,
        id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
    ) -> Vec<ToolCallStreamEvent> {
        let mut events = Vec::new();

        let tracked = if let Some(id) = id {
            // Initialize new tool call tracking when we receive an id
            if !self.raw_chunk_tracker.contains_key(&index) {
                let entry = RawChunkTrackerEntry {
                    id: id.to_string(),
                    name: name.unwrap_or("").to_string(),
                    has_started: false,
                    delta_buffer: Vec::new(),
                };
                self.raw_chunk_tracker.insert(index, entry);
            }
            self.raw_chunk_tracker.get_mut(&index)
        } else {
            self.raw_chunk_tracker.get_mut(&index)
        };

        let tracked = match tracked {
            Some(t) => t,
            None => return events,
        };

        // Update name if present in chunk and not yet set
        if let Some(name) = name {
            tracked.name = name.to_string();
        }

        // Emit start event when we have the name
        if !tracked.has_started && !tracked.name.is_empty() {
            events.push(ToolCallStreamEvent::Start {
                id: tracked.id.clone(),
                name: tracked.name.clone(),
            });
            tracked.has_started = true;

            // Flush buffered deltas
            for buffered_delta in tracked.delta_buffer.drain(..) {
                events.push(ToolCallStreamEvent::Delta {
                    id: tracked.id.clone(),
                    delta: buffered_delta,
                });
            }
        }

        // Emit delta event for argument chunks
        if let Some(args) = arguments {
            if tracked.has_started {
                events.push(ToolCallStreamEvent::Delta {
                    id: tracked.id.clone(),
                    delta: args.to_string(),
                });
            } else {
                tracked.delta_buffer.push(args.to_string());
            }
        }

        events
    }

    /// Process stream finish reason.
    ///
    /// Emits end events when `finish_reason` is `"tool_calls"`.
    ///
    /// Source: `NativeToolCallParser.ts` — `processFinishReason()`
    pub fn process_finish_reason(&mut self, finish_reason: Option<&str>) -> Vec<ToolCallStreamEvent> {
        let mut events = Vec::new();

        if finish_reason == Some("tool_calls") && !self.raw_chunk_tracker.is_empty() {
            for (_, tracked) in self.raw_chunk_tracker.iter() {
                events.push(ToolCallStreamEvent::End {
                    id: tracked.id.clone(),
                });
            }
        }

        events
    }

    /// Finalize any remaining tool calls that weren't explicitly ended.
    ///
    /// Should be called at the end of stream processing.
    ///
    /// Source: `NativeToolCallParser.ts` — `finalizeRawChunks()`
    pub fn finalize_raw_chunks(&mut self) -> Vec<ToolCallStreamEvent> {
        let mut events = Vec::new();

        if !self.raw_chunk_tracker.is_empty() {
            for (_, tracked) in self.raw_chunk_tracker.iter() {
                if tracked.has_started {
                    events.push(ToolCallStreamEvent::End {
                        id: tracked.id.clone(),
                    });
                }
            }
            self.raw_chunk_tracker.clear();
        }

        events
    }

    /// Clear all raw chunk tracking state.
    ///
    /// Should be called when a new API request starts.
    ///
    /// Source: `NativeToolCallParser.ts` — `clearRawChunkState()`
    pub fn clear_raw_chunk_state(&mut self) {
        self.raw_chunk_tracker.clear();
    }

    // ===================================================================
    // Streaming Tool Call Accumulation
    // Source: `NativeToolCallParser.ts` — startStreamingToolCall,
    //         processStreamingChunk, finalizeStreamingToolCall,
    //         clearAllStreamingToolCalls
    // ===================================================================

    /// Start streaming a new tool call.
    ///
    /// Initializes tracking for incremental argument parsing.
    /// Accepts string to support both ToolName and dynamic MCP tools.
    ///
    /// Source: `NativeToolCallParser.ts` — `startStreamingToolCall()`
    pub fn start_streaming_tool_call(&mut self, id: &str, name: &str) {
        self.streaming_tool_calls.insert(
            id.to_string(),
            StreamingToolCallState {
                id: id.to_string(),
                name: name.to_string(),
                arguments_accumulator: String::new(),
            },
        );
    }

    /// Process a chunk of JSON arguments for a streaming tool call.
    ///
    /// Uses partial JSON parsing to extract values from incomplete JSON immediately.
    /// Returns a partial [`ToolUse`] with currently parsed parameters, or `None`
    /// if the tool call is not tracked or parsing fails.
    ///
    /// Source: `NativeToolCallParser.ts` — `processStreamingChunk()`
    pub fn process_streaming_chunk(&mut self, id: &str, chunk: &str) -> Option<ToolUse> {
        let tool_call = self.streaming_tool_calls.get_mut(id)?;
        
        // Accumulate the JSON string
        tool_call.arguments_accumulator.push_str(chunk);

        // For dynamic MCP tools, we don't return partial updates - wait for final
        if is_mcp_tool_name(&tool_call.name) {
            return None;
        }

        // Try to parse whatever we can from the incomplete JSON
        // In the TS version, this uses partial-json-parser. Here we use
        // a simpler approach: try full parse, return None if it fails.
        if let Ok(partial_args) = serde_json::from_str::<serde_json::Value>(
            &tool_call.arguments_accumulator,
        ) {
            let empty_map = serde_json::Map::new();
            let partial_obj = partial_args.as_object().unwrap_or(&empty_map);
            
            // Build stringified params
            let mut params = HashMap::new();
            for (key, value) in partial_obj {
                if is_valid_tool_param(key) {
                    let string_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    params.insert(key.clone(), string_value);
                }
            }

            Some(ToolUse {
                content_type: "tool_use".to_string(),
                name: tool_call.name.clone(),
                params,
                partial: true,
                id: tool_call.id.clone(),
                native_args: Some(partial_args),
                original_name: None,
                used_legacy_format: false,
            })
        } else {
            None
        }
    }

    /// Finalize a streaming tool call.
    ///
    /// Parses the complete JSON and returns the final ToolUse or McpToolUse.
    ///
    /// Source: `NativeToolCallParser.ts` — `finalizeStreamingToolCall()`
    pub fn finalize_streaming_tool_call(&mut self, id: &str) -> Option<AssistantMessageContent> {
        let tool_call = self.streaming_tool_calls.remove(id)?;

        // Parse the complete accumulated JSON
        let result = self.parse_tool_call(
            &tool_call.id,
            &tool_call.name,
            &tool_call.arguments_accumulator,
        );

        result
    }

    /// Clear all streaming tool call state.
    ///
    /// Should be called when a new API request starts to prevent memory leaks
    /// from interrupted streams.
    ///
    /// Source: `NativeToolCallParser.ts` — `clearAllStreamingToolCalls()`
    pub fn clear_all_streaming_tool_calls(&mut self) {
        self.streaming_tool_calls.clear();
    }

    /// Check if there are any active streaming tool calls.
    ///
    /// Source: `NativeToolCallParser.ts` — `hasActiveStreamingToolCalls()`
    pub fn has_active_streaming_tool_calls(&self) -> bool {
        !self.streaming_tool_calls.is_empty()
    }

    // ===================================================================
    // Tool Call Parsing
    // Source: `NativeToolCallParser.ts` — parseToolCall, parseDynamicMcpTool
    // ===================================================================

    /// Parse a complete tool call into an AssistantMessageContent.
    ///
    /// Handles both built-in tools and dynamic MCP tools.
    ///
    /// Source: `NativeToolCallParser.ts` — `parseToolCall()`
    pub fn parse_tool_call(
        &self,
        id: &str,
        name: &str,
        arguments: &str,
    ) -> Option<AssistantMessageContent> {
        // Normalize the name (handle mcp__server__tool → mcp--server--tool)
        let normalized_name = normalize_mcp_tool_name(name);

        // Check if this is a dynamic MCP tool
        if is_mcp_tool_name(&normalized_name) {
            return self.parse_dynamic_mcp_tool(id, &normalized_name, arguments);
        }

        // Parse the arguments JSON string
        let args: serde_json::Value = if arguments.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            match serde_json::from_str(arguments) {
                Ok(v) => v,
                Err(_) => return None,
            }
        };

        // Build stringified params for display/logging
        let mut params = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                if is_valid_tool_param(key) {
                    let string_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    params.insert(key.clone(), string_value);
                }
            }
        }

        Some(AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: normalized_name.clone(),
            params,
            partial: false,
            id: id.to_string(),
            native_args: Some(args),
            original_name: if name != normalized_name {
                Some(name.to_string())
            } else {
                None
            },
            used_legacy_format: false,
        }))
    }

    /// Parse a dynamic MCP tool call.
    ///
    /// Source: `NativeToolCallParser.ts` — `parseDynamicMcpTool()`
    pub fn parse_dynamic_mcp_tool(
        &self,
        id: &str,
        name: &str,
        arguments: &str,
    ) -> Option<AssistantMessageContent> {
        let args: serde_json::Value = serde_json::from_str(if arguments.is_empty() { "{}" } else { arguments }).unwrap_or_default();

        let normalized_name = normalize_mcp_tool_name(name);
        let parsed = parse_mcp_tool_name(&normalized_name)?;

        Some(AssistantMessageContent::McpToolUse(McpToolUse {
            content_type: "mcp_tool_use".to_string(),
            name: name.to_string(),
            id: id.to_string(),
            server_name: parsed.0,
            tool_name: parsed.1,
            arguments: args,
            partial: false,
        }))
    }

    // ===================================================================
    // Direct Chunk Feeding
    // Source: Original StreamParser functionality
    // ===================================================================

    /// Feed a single chunk into the parser.
    ///
    /// This method accumulates state incrementally. Call [`finalize()`]
    /// when the stream ends to get the complete parsed content.
    pub fn feed_chunk(&mut self, chunk: &ApiStreamChunk) {
        match chunk {
            ApiStreamChunk::Text { text } => {
                self.text.push_str(text);
            }

            ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                cache_write_tokens,
                cache_read_tokens,
                reasoning_tokens,
                total_cost,
            } => {
                self.usage = Some(StreamUsage {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cache_write_tokens: *cache_write_tokens,
                    cache_read_tokens: *cache_read_tokens,
                    reasoning_tokens: *reasoning_tokens,
                    total_cost: *total_cost,
                });
            }

            ApiStreamChunk::Reasoning { text, signature } => {
                // Accumulate reasoning text
                if let Some(ref mut current) = self.current_thinking {
                    current.push_str(text);
                } else {
                    self.current_thinking = Some(text.clone());
                }
                // If a signature is provided inline, store it
                if let Some(sig) = signature {
                    self.current_thinking_signature = Some(sig.clone());
                }
            }

            ApiStreamChunk::ThinkingComplete { signature } => {
                // Finalize the current thinking block
                if let Some(text) = self.current_thinking.take() {
                    self.thinking_blocks.push(ThinkingBlock {
                        text,
                        signature: Some(signature.clone()),
                    });
                }
                self.current_thinking_signature = None;
            }

            ApiStreamChunk::Grounding { sources } => {
                self.grounding_sources.extend(sources.iter().cloned());
            }

            ApiStreamChunk::ToolCall {
                id,
                name,
                arguments,
            } => {
                self.tool_calls.push(ParsedToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
            }

            ApiStreamChunk::ToolCallStart { id, name } => {
                // Start tracking a new partial tool call using ID-based tracking.
                // Source: TS `NativeToolCallParser.startStreamingToolCall()` —
                // guards against duplicate start events for the same ID.
                if !self.active_tool_calls.contains_key(id) {
                    self.active_tool_calls.insert(
                        id.clone(),
                        PendingToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: String::new(),
                        },
                    );
                }
            }

            ApiStreamChunk::ToolCallDelta { id, delta } => {
                // Append the delta to the correct active tool call by ID.
                if let Some(pending) = self.active_tool_calls.get_mut(id) {
                    pending.arguments.push_str(delta);
                }
            }

            ApiStreamChunk::ToolCallEnd { id } => {
                // Finalize the active tool call by ID and move it to completed.
                if let Some(pending) = self.active_tool_calls.remove(id) {
                    self.tool_calls.push(ParsedToolCall {
                        id: pending.id,
                        name: pending.name,
                        arguments: pending.arguments,
                    });
                }
            }

            ApiStreamChunk::ToolCallPartial {
                index,
                id,
                name,
                arguments,
            } => {
                // Handle partial tool calls from providers that use index-based tracking
                let entry = self
                    .pending_tool_calls
                    .entry(*index)
                    .or_insert_with(|| PendingToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });

                if let Some(id) = id {
                    entry.id = id.clone();
                }
                if let Some(name) = name {
                    entry.name = name.clone();
                }
                if let Some(args) = arguments {
                    entry.arguments.push_str(args);
                }
            }

            ApiStreamChunk::Error { error, message } => {
                self.error = Some(StreamError {
                    error: error.clone(),
                    message: message.clone(),
                });
            }
        }
    }

    /// Finalize the stream and return the parsed content.
    ///
    /// This flushes any pending partial tool calls and the current thinking
    /// block, then returns the complete aggregated content.
    pub fn finalize(mut self) -> ParsedStreamContent {
        // Flush pending partial tool calls (index-based tracking)
        for (_, pending) in self.pending_tool_calls.drain() {
            if !pending.id.is_empty() && !pending.name.is_empty() {
                self.tool_calls.push(ParsedToolCall {
                    id: pending.id,
                    name: pending.name,
                    arguments: pending.arguments,
                });
            }
        }

        // Flush active tool calls (ID-based start/delta/end tracking).
        // Source: TS `NativeToolCallParser.finalizeRawChunks()` — finalizes
        // any streaming tool calls that weren't explicitly ended.
        for (_, pending) in self.active_tool_calls.drain() {
            if !pending.id.is_empty() && !pending.name.is_empty() {
                self.tool_calls.push(ParsedToolCall {
                    id: pending.id,
                    name: pending.name,
                    arguments: pending.arguments,
                });
            }
        }

        // Flush current thinking block (if not finalized by ThinkingComplete)
        if let Some(text) = self.current_thinking.take() {
            self.thinking_blocks.push(ThinkingBlock {
                text,
                signature: self.current_thinking_signature.take(),
            });
        }

        ParsedStreamContent {
            text: self.text,
            tool_calls: self.tool_calls,
            thinking_blocks: self.thinking_blocks,
            usage: self.usage,
            grounding_sources: self.grounding_sources,
            error: self.error,
        }
    }

    /// Get the accumulated text so far (without finalizing).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Check whether any tool calls have been accumulated.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
            || !self.pending_tool_calls.is_empty()
            || !self.active_tool_calls.is_empty()
    }

    /// Get the current usage info (if received).
    pub fn usage(&self) -> Option<&StreamUsage> {
        self.usage.as_ref()
    }

    /// Check whether an error was received.
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper: convert parsed content into API ContentBlocks
// ---------------------------------------------------------------------------

impl ParsedStreamContent {
    /// Convert the parsed content into a vector of [`ContentBlock`] suitable
    /// for adding to the API conversation history as an assistant message.
    pub fn to_content_blocks(&self) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();

        // Thinking blocks
        for thinking in &self.thinking_blocks {
            blocks.push(ContentBlock::Thinking {
                thinking: thinking.text.clone(),
                signature: thinking.signature.clone().unwrap_or_default(),
            });
        }

        // Text block (if non-empty)
        if !self.text.is_empty() {
            blocks.push(ContentBlock::Text {
                text: self.text.clone(),
            });
        }

        // Tool use blocks
        for tc in &self.tool_calls {
            blocks.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: tc.parse_arguments(),
            });
        }

        blocks
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Text accumulation tests ---

    #[test]
    fn test_parser_text_accumulation() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Text {
            text: "Hello".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::Text {
            text: " world".into(),
        });

        assert_eq!(parser.text(), "Hello world");

        let content = parser.finalize();
        assert_eq!(content.text, "Hello world");
        assert!(content.tool_calls.is_empty());
    }

    // --- Complete tool call tests ---

    #[test]
    fn test_parser_complete_tool_call() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Text {
            text: "I'll read that file.".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCall {
            id: "call_123".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"src/main.rs"}"#.into(),
        });

        let content = parser.finalize();
        assert_eq!(content.text, "I'll read that file.");
        assert_eq!(content.tool_calls.len(), 1);
        assert_eq!(content.tool_calls[0].id, "call_123");
        assert_eq!(content.tool_calls[0].name, "read_file");
        assert_eq!(
            content.tool_calls[0].parse_arguments(),
            serde_json::json!({"path": "src/main.rs"})
        );
    }

    // --- Partial tool call (index-based) tests ---

    #[test]
    fn test_parser_partial_tool_call() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 0,
            id: Some("call_1".into()),
            name: Some("write_to_file".into()),
            arguments: None,
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 0,
            id: None,
            name: None,
            arguments: Some(r#"{"path":"test.rs","#.into()),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 0,
            id: None,
            name: None,
            arguments: Some(r#""content":"hello"}"#.into()),
        });

        assert!(parser.has_tool_calls());

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 1);
        assert_eq!(content.tool_calls[0].name, "write_to_file");
        assert_eq!(
            content.tool_calls[0].arguments,
            r#"{"path":"test.rs","content":"hello"}"#
        );
    }

    // --- Start/Delta/End tool call tests ---

    #[test]
    fn test_parser_start_delta_end_tool_call() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_1".into(),
            name: "apply_diff".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallDelta {
            id: "tc_1".into(),
            delta: r#"{"path":"a.rs"}"#.into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallEnd {
            id: "tc_1".into(),
        });

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 1);
        assert_eq!(content.tool_calls[0].name, "apply_diff");
        assert_eq!(content.tool_calls[0].arguments, r#"{"path":"a.rs"}"#);
    }

    // --- Multiple concurrent tool calls tests ---

    #[test]
    fn test_parser_multiple_tool_calls() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 0,
            id: Some("call_a".into()),
            name: Some("read_file".into()),
            arguments: None,
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 1,
            id: Some("call_b".into()),
            name: Some("list_files".into()),
            arguments: None,
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 0,
            id: None,
            name: None,
            arguments: Some(r#"{"path":"a.rs"}"#.into()),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallPartial {
            index: 1,
            id: None,
            name: None,
            arguments: Some(r#"{"path":".","recursive":true}"#.into()),
        });

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 2);

        let names: Vec<&str> = content.tool_calls.iter().map(|tc| tc.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"list_files"));
    }

    #[test]
    fn test_parser_multiple_concurrent_start_delta_end() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_a".into(),
            name: "read_file".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_b".into(),
            name: "list_files".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallDelta {
            id: "tc_a".into(),
            delta: r#"{"path":"a.rs"}"#.into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallDelta {
            id: "tc_b".into(),
            delta: r#"{"path":".","recursive":true}"#.into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallEnd {
            id: "tc_a".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallEnd {
            id: "tc_b".into(),
        });

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 2);

        for tc in &content.tool_calls {
            if tc.name == "read_file" {
                assert_eq!(tc.id, "tc_a");
                assert_eq!(tc.arguments, r#"{"path":"a.rs"}"#);
            } else if tc.name == "list_files" {
                assert_eq!(tc.id, "tc_b");
                assert_eq!(tc.arguments, r#"{"path":".","recursive":true}"#);
            }
        }
    }

    // --- Duplicate start ignored test ---

    #[test]
    fn test_parser_duplicate_tool_call_start_ignored() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_1".into(),
            name: "read_file".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_1".into(),
            name: "read_file".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallDelta {
            id: "tc_1".into(),
            delta: r#"{"path":"x.rs"}"#.into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallEnd {
            id: "tc_1".into(),
        });

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 1);
        assert_eq!(content.tool_calls[0].arguments, r#"{"path":"x.rs"}"#);
    }

    // --- Unfinalized active tool call flushed test ---

    #[test]
    fn test_parser_unfinalized_active_tool_call_flushed() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::ToolCallStart {
            id: "tc_unfinished".into(),
            name: "write_to_file".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCallDelta {
            id: "tc_unfinished".into(),
            delta: r#"{"path":"a.rs"}"#.into(),
        });
        // No ToolCallEnd — stream ended prematurely

        let content = parser.finalize();
        assert_eq!(content.tool_calls.len(), 1);
        assert_eq!(content.tool_calls[0].name, "write_to_file");
        assert_eq!(content.tool_calls[0].id, "tc_unfinished");
    }

    // --- Usage tests ---

    #[test]
    fn test_parser_usage() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_write_tokens: Some(100),
            cache_read_tokens: Some(50),
            reasoning_tokens: None,
            total_cost: Some(0.05),
        });

        let content = parser.finalize();
        let usage = content.usage.unwrap();
        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.cache_write_tokens, Some(100));
        assert_eq!(usage.cache_read_tokens, Some(50));
        assert_eq!(usage.total_cost, Some(0.05));
    }

    // --- Thinking block tests ---

    #[test]
    fn test_parser_thinking_blocks() {
        let mut parser = StreamParser::new();

        parser.feed_chunk(&ApiStreamChunk::Reasoning {
            text: "Let me think".into(),
            signature: None,
        });
        parser.feed_chunk(&ApiStreamChunk::Reasoning {
            text: " about this...".into(),
            signature: None,
        });
        parser.feed_chunk(&ApiStreamChunk::ThinkingComplete {
            signature: "sig_123".into(),
        });

        let content = parser.finalize();
        assert_eq!(content.thinking_blocks.len(), 1);
        assert_eq!(content.thinking_blocks[0].text, "Let me think about this...");
        assert_eq!(
            content.thinking_blocks[0].signature,
            Some("sig_123".to_string())
        );
    }

    #[test]
    fn test_parser_thinking_without_complete() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Reasoning {
            text: "Partial thought".into(),
            signature: None,
        });

        let content = parser.finalize();
        assert_eq!(content.thinking_blocks.len(), 1);
        assert_eq!(content.thinking_blocks[0].text, "Partial thought");
        assert!(content.thinking_blocks[0].signature.is_none());
    }

    // --- Error tests ---

    #[test]
    fn test_parser_error() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Error {
            error: "rate_limit".into(),
            message: "Rate limit exceeded".into(),
        });

        assert!(parser.has_error());
        let content = parser.finalize();
        assert!(content.error.is_some());
        assert_eq!(content.error.unwrap().error, "rate_limit");
    }

    // --- Grounding tests ---

    #[test]
    fn test_parser_grounding() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Grounding {
            sources: vec![
                GroundingSource {
                    title: Some("Doc".into()),
                    url: Some("https://example.com".into()),
                    snippet: None,
                },
            ],
        });

        let content = parser.finalize();
        assert_eq!(content.grounding_sources.len(), 1);
    }

    // --- to_content_blocks tests ---

    #[test]
    fn test_to_content_blocks() {
        let mut parser = StreamParser::new();
        parser.feed_chunk(&ApiStreamChunk::Reasoning {
            text: "hmm".into(),
            signature: None,
        });
        parser.feed_chunk(&ApiStreamChunk::ThinkingComplete {
            signature: "sig".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::Text {
            text: "Hello".into(),
        });
        parser.feed_chunk(&ApiStreamChunk::ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"x.rs"}"#.into(),
        });

        let content = parser.finalize();
        let blocks = content.to_content_blocks();

        assert_eq!(blocks.len(), 3);

        match &blocks[0] {
            ContentBlock::Thinking { thinking, signature } => {
                assert_eq!(thinking, "hmm");
                assert_eq!(signature, "sig");
            }
            _ => panic!("Expected Thinking block"),
        }

        match &blocks[1] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text block"),
        }

        match &blocks[2] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "c1");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "x.rs");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    // --- Empty stream test ---

    #[test]
    fn test_parser_empty_stream() {
        let parser = StreamParser::new();
        let content = parser.finalize();
        assert!(content.text.is_empty());
        assert!(content.tool_calls.is_empty());
        assert!(content.thinking_blocks.is_empty());
        assert!(content.usage.is_none());
        assert!(content.error.is_none());
    }

    // --- Invalid JSON arguments test ---

    #[test]
    fn test_parse_arguments_invalid_json() {
        let tc = ParsedToolCall {
            id: "c1".into(),
            name: "test".into(),
            arguments: "not json".into(),
        };
        // Should return empty object {} instead of null (P0-1 fix)
        assert_eq!(tc.parse_arguments(), serde_json::json!({}));
    }

    // ===================================================================
    // NativeToolCallParser-specific tests
    // ===================================================================

    // --- process_raw_chunk tests ---

    #[test]
    fn test_process_raw_chunk_start_and_delta() {
        let mut parser = StreamParser::new();

        // First chunk: id + name
        let events = parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::Start { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected Start event"),
        }

        // Second chunk: arguments delta
        let events = parser.process_raw_chunk(0, None, None, Some(r#"{"path":"x.rs"}"#));
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, r#"{"path":"x.rs"}"#);
            }
            _ => panic!("Expected Delta event"),
        }
    }

    #[test]
    fn test_process_raw_chunk_buffered_deltas() {
        let mut parser = StreamParser::new();

        // First chunk: id only (no name yet)
        let events = parser.process_raw_chunk(0, Some("call_1"), None, Some(r#"{"path""#));
        assert!(events.is_empty()); // No events until we have a name

        // Second chunk: name arrives, should flush buffered delta
        let events = parser.process_raw_chunk(0, None, Some("read_file"), Some(r#":"x.rs"}"#));
        assert_eq!(events.len(), 3); // Start + buffered Delta + new Delta
        match &events[0] {
            ToolCallStreamEvent::Start { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected Start event"),
        }
        // Buffered delta from first chunk
        match &events[1] {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, r#"{"path""#);
            }
            _ => panic!("Expected buffered Delta event"),
        }
        // New delta from second chunk
        match &events[2] {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, r#":"x.rs"}"#);
            }
            _ => panic!("Expected new Delta event"),
        }
    }

    #[test]
    fn test_process_raw_chunk_no_id_returns_empty() {
        let mut parser = StreamParser::new();

        // No id, no prior tracking
        let events = parser.process_raw_chunk(0, None, Some("read_file"), None);
        assert!(events.is_empty());
    }

    // --- process_finish_reason tests ---

    #[test]
    fn test_process_finish_reason_tool_calls() {
        let mut parser = StreamParser::new();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        parser.process_raw_chunk(1, Some("call_2"), Some("list_files"), None);

        let events = parser.process_finish_reason(Some("tool_calls"));
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_process_finish_reason_not_tool_calls() {
        let mut parser = StreamParser::new();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);

        let events = parser.process_finish_reason(Some("stop"));
        assert!(events.is_empty());
    }

    // --- finalize_raw_chunks tests ---

    #[test]
    fn test_finalize_raw_chunks() {
        let mut parser = StreamParser::new();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), Some(r#"{"path":"x.rs"}"#));

        let events = parser.finalize_raw_chunks();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::End { id } => {
                assert_eq!(id, "call_1");
            }
            _ => panic!("Expected End event"),
        }

        // Raw chunk state should be cleared
        assert!(parser.raw_chunk_tracker.is_empty());
    }

    // --- clear_raw_chunk_state tests ---

    #[test]
    fn test_clear_raw_chunk_state() {
        let mut parser = StreamParser::new();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        assert!(!parser.raw_chunk_tracker.is_empty());

        parser.clear_raw_chunk_state();
        assert!(parser.raw_chunk_tracker.is_empty());
    }

    // --- Streaming tool call tests ---

    #[test]
    fn test_start_streaming_tool_call() {
        let mut parser = StreamParser::new();

        parser.start_streaming_tool_call("tc_1", "read_file");
        assert!(parser.has_active_streaming_tool_calls());

        parser.clear_all_streaming_tool_calls();
        assert!(!parser.has_active_streaming_tool_calls());
    }

    #[test]
    fn test_process_streaming_chunk() {
        let mut parser = StreamParser::new();
        parser.start_streaming_tool_call("tc_1", "read_file");

        // First chunk: partial JSON
        let result = parser.process_streaming_chunk("tc_1", r#"{"path":"src/main.rs"}"#);
        assert!(result.is_some());
        let tool_use = result.unwrap();
        assert_eq!(tool_use.name, "read_file");
        assert!(tool_use.partial);
        assert_eq!(tool_use.params.get("path").unwrap(), "src/main.rs");
    }

    #[test]
    fn test_process_streaming_chunk_mcp_returns_none() {
        let mut parser = StreamParser::new();
        parser.start_streaming_tool_call("tc_1", "mcp--server--tool");

        let result = parser.process_streaming_chunk("tc_1", r#"{"key":"value"}"#);
        assert!(result.is_none()); // MCP tools don't return partial updates
    }

    #[test]
    fn test_process_streaming_chunk_unknown_id_returns_none() {
        let mut parser = StreamParser::new();

        let result = parser.process_streaming_chunk("unknown_id", r#"{}"#);
        assert!(result.is_none());
    }

    #[test]
    fn test_finalize_streaming_tool_call() {
        let mut parser = StreamParser::new();
        parser.start_streaming_tool_call("tc_1", "read_file");
        parser.process_streaming_chunk("tc_1", r#"{"path":"src/main.rs"}"#);

        let result = parser.finalize_streaming_tool_call("tc_1");
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(!tu.partial);
                assert_eq!(tu.id, "tc_1");
            }
            _ => panic!("Expected ToolUse"),
        }

        // Should be removed from streaming state
        assert!(!parser.has_active_streaming_tool_calls());
    }

    #[test]
    fn test_finalize_streaming_tool_call_mcp() {
        let mut parser = StreamParser::new();
        parser.start_streaming_tool_call("tc_1", "mcp--server--tool");
        parser.process_streaming_chunk("tc_1", r#"{"key":"value"}"#);

        let result = parser.finalize_streaming_tool_call("tc_1");
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.server_name, "server");
                assert_eq!(mtu.tool_name, "tool");
                assert_eq!(mtu.id, "tc_1");
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    // --- parse_tool_call tests ---

    #[test]
    fn test_parse_tool_call_builtin() {
        let parser = StreamParser::new();
        let result = parser.parse_tool_call("call_1", "read_file", r#"{"path":"x.rs"}"#);
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(!tu.partial);
                assert_eq!(tu.id, "call_1");
                assert_eq!(tu.params.get("path").unwrap(), "x.rs");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_mcp() {
        let parser = StreamParser::new();
        let result = parser.parse_tool_call("call_1", "mcp--server--tool", r#"{"key":"value"}"#);
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.server_name, "server");
                assert_eq!(mtu.tool_name, "tool");
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_invalid_json() {
        let parser = StreamParser::new();
        let result = parser.parse_tool_call("call_1", "read_file", "not json");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_tool_call_empty_arguments() {
        let parser = StreamParser::new();
        let result = parser.parse_tool_call("call_1", "read_file", "");
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(tu.params.is_empty());
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    // --- parse_dynamic_mcp_tool tests ---

    #[test]
    fn test_parse_dynamic_mcp_tool() {
        let parser = StreamParser::new();
        let result = parser.parse_dynamic_mcp_tool("call_1", "mcp--serverName--toolName", r#"{"arg":"val"}"#);
        assert!(result.is_some());

        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.name, "mcp--serverName--toolName");
                assert_eq!(mtu.server_name, "serverName");
                assert_eq!(mtu.tool_name, "toolName");
                assert_eq!(mtu.id, "call_1");
                assert_eq!(mtu.arguments["arg"], "val");
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    #[test]
    fn test_parse_dynamic_mcp_tool_invalid_name() {
        let parser = StreamParser::new();
        let result = parser.parse_dynamic_mcp_tool("call_1", "mcp--", "{}");
        assert!(result.is_none()); // No server name or tool name
    }
}
