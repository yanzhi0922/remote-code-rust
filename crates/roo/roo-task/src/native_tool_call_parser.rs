//! Native tool call parser for OpenAI-style function calling.
//!
//! Faithfully replicates `src/core/assistant-message/NativeToolCallParser.ts` (1078 lines).
//!
//! This module converts native tool call format to ToolUse/McpToolUse format for
//! compatibility with existing tool execution infrastructure.
//!
//! ## Key responsibilities
//!
//! 1. **Raw chunk processing** — converts `tool_call_partial` chunks into start/delta/end events
//! 2. **Streaming tool call accumulation** — accumulates JSON arguments incrementally
//! 3. **Tool call finalization** — parses complete JSON and builds typed nativeArgs
//! 4. **Per-tool nativeArgs construction** — builds typed argument objects for each tool
//! 5. **MCP tool name parsing** — handles dynamic MCP tools (mcp--serverName--toolName)
//!
//! Source: `src/core/assistant-message/NativeToolCallParser.ts`

use std::collections::HashMap;

use serde_json::{json, Value};
use tracing::warn;

use crate::types::{
    AssistantMessageContent, McpToolUse, RawChunkTrackerEntry, StreamingToolCallState, ToolUse,
    ToolCallStreamEvent, normalize_mcp_tool_name, parse_mcp_tool_name,
    is_valid_tool_param, MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR,
};

// ---------------------------------------------------------------------------
// Tool aliases — mirrors TS TOOL_ALIASES from shared/tools.ts
// ---------------------------------------------------------------------------

/// Tool alias resolution map.
///
/// Source: `src/shared/tools.ts` — `TOOL_ALIASES`
const TOOL_ALIASES: &[(&str, &str)] = &[
    ("write_file", "write_to_file"),
    ("search_and_replace", "edit"),
];

/// All valid built-in tool names.
///
/// Source: `packages/types/src/tool.ts` — `toolNames`
const VALID_TOOL_NAMES: &[&str] = &[
    "execute_command",
    "read_file",
    "read_command_output",
    "write_to_file",
    "apply_diff",
    "edit",
    "search_and_replace",
    "search_replace",
    "edit_file",
    "apply_patch",
    "search_files",
    "list_files",
    "use_mcp_tool",
    "access_mcp_resource",
    "ask_followup_question",
    "attempt_completion",
    "switch_mode",
    "new_task",
    "codebase_search",
    "update_todo_list",
    "run_slash_command",
    "skill",
    "generate_image",
    "custom_tool",
];

// ---------------------------------------------------------------------------
// NativeToolCallParser — mirrors TS class NativeToolCallParser
// ---------------------------------------------------------------------------

/// Parser for native tool calls (OpenAI-style function calling).
///
/// Converts native tool call format to ToolUse format for compatibility
/// with existing tool execution infrastructure.
///
/// For tools with refactored parsers (e.g., read_file), this parser provides
/// typed arguments via nativeArgs. Tool-specific handlers should consume
/// nativeArgs directly rather than relying on synthesized legacy params.
///
/// This struct also handles raw tool call chunk processing, converting
/// provider-level raw chunks into start/delta/end events.
///
/// Source: `src/core/assistant-message/NativeToolCallParser.ts`
pub struct NativeToolCallParser {
    /// Streaming state management for argument accumulation (keyed by tool call id).
    ///
    /// Source: TS `NativeToolCallParser.streamingToolCalls`
    streaming_tool_calls: HashMap<String, StreamingToolCallState>,

    /// Raw chunk tracking state (keyed by index from API stream).
    ///
    /// Source: TS `NativeToolCallParser.rawChunkTracker`
    raw_chunk_tracker: HashMap<u32, RawChunkTrackerEntry>,
}

impl NativeToolCallParser {
    /// Create a new parser instance.
    pub fn new() -> Self {
        Self {
            streaming_tool_calls: HashMap::new(),
            raw_chunk_tracker: HashMap::new(),
        }
    }

    // ===================================================================
    // Helper methods
    // ===================================================================

    /// Coerce an optional boolean value from various types.
    ///
    /// Source: TS `NativeToolCallParser.coerceOptionalBoolean()`
    fn coerce_optional_boolean(value: &Value) -> Option<bool> {
        match value {
            Value::Bool(b) => Some(*b),
            Value::String(s) => {
                let lower = s.trim().to_lowercase();
                match lower.as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Coerce an optional number value from various types.
    ///
    /// Source: TS `NativeToolCallParser.coerceOptionalNumber()`
    fn coerce_optional_number(value: &Value) -> Option<f64> {
        match value {
            Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
            Value::String(s) => s.parse::<f64>().ok().filter(|f| f.is_finite()),
            _ => None,
        }
    }

    /// Convert raw file entries from API (with line_ranges) to structured format.
    ///
    /// Handles multiple formats for backward compatibility:
    /// - New tuple format: `{ path: string, line_ranges: [[1, 50], [100, 150]] }`
    /// - Object format: `{ path: string, line_ranges: [{ start: 1, end: 50 }] }`
    /// - Legacy string format: `{ path: string, line_ranges: ["1-50"] }`
    ///
    /// Source: TS `NativeToolCallParser.convertFileEntries()`
    fn convert_file_entries(files: &[Value]) -> Vec<Value> {
        files
            .iter()
            .map(|file| {
                let empty_map = serde_json::Map::new();
                let f = file.as_object().unwrap_or(&empty_map);
                let path = f
                    .get("path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut entry = serde_json::Map::new();
                entry.insert("path".to_string(), Value::String(path));

                if let Some(line_ranges) = f.get("line_ranges").and_then(|lr| lr.as_array()) {
                    let converted: Vec<Value> = line_ranges
                        .iter()
                        .filter_map(|range| {
                            // Handle tuple format: [start, end]
                            if let Some(arr) = range.as_array() {
                                if arr.len() >= 2 {
                                    let start = arr[0].as_f64().unwrap_or(0.0) as u64;
                                    let end = arr[1].as_f64().unwrap_or(0.0) as u64;
                                    return Some(json!({"start": start, "end": end}));
                                }
                            }
                            // Handle object format: { start: number, end: number }
                            if let Some(obj) = range.as_object() {
                                if let (Some(s), Some(e)) = (obj.get("start"), obj.get("end")) {
                                    let start = s.as_f64().unwrap_or(0.0) as u64;
                                    let end = e.as_f64().unwrap_or(0.0) as u64;
                                    return Some(json!({"start": start, "end": end}));
                                }
                            }
                            // Handle legacy string format: "1-50"
                            if let Some(s) = range.as_str() {
                                let re = regex::Regex::new(r"^(\d+)-(\d+)$").ok();
                                if let Some(re) = re {
                                    if let Some(caps) = re.captures(s) {
                                        let start: u64 = caps[1].parse().unwrap_or(0);
                                        let end: u64 = caps[2].parse().unwrap_or(0);
                                        return Some(json!({"start": start, "end": end}));
                                    }
                                }
                            }
                            None
                        })
                        .collect();
                    entry.insert("lineRanges".to_string(), Value::Array(converted));
                }

                Value::Object(entry)
            })
            .collect()
    }

    /// Resolve a tool alias to its canonical name.
    ///
    /// Source: TS `resolveToolAlias()` from `filter-tools-for-mode.ts`
    fn resolve_tool_alias(name: &str) -> &str {
        for (alias, canonical) in TOOL_ALIASES {
            if *alias == name {
                return canonical;
            }
        }
        name
    }

    /// Check if a tool name is a valid built-in tool.
    fn is_valid_tool_name(name: &str) -> bool {
        VALID_TOOL_NAMES.contains(&name)
    }

    // ===================================================================
    // Raw Chunk Processing
    // Source: TS `processRawChunk()`, `processFinishReason()`,
    //         `finalizeRawChunks()`, `clearRawChunkState()`
    // ===================================================================

    /// Process a raw tool call chunk from the API stream.
    ///
    /// Handles tracking, buffering, and emits start/delta/end events.
    /// This is the entry point for providers that emit `tool_call_partial` chunks.
    /// Returns an array of events to be processed by the consumer.
    ///
    /// Source: TS `NativeToolCallParser.processRawChunk()`
    pub fn process_raw_chunk(
        &mut self,
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
    ) -> Vec<ToolCallStreamEvent> {
        let mut events = Vec::new();

        // Initialize new tool call tracking when we receive an id
        let tracked = if let Some(id) = id {
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
    /// Source: TS `NativeToolCallParser.processFinishReason()`
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
    /// Source: TS `NativeToolCallParser.finalizeRawChunks()`
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
    /// Source: TS `NativeToolCallParser.clearRawChunkState()`
    pub fn clear_raw_chunk_state(&mut self) {
        self.raw_chunk_tracker.clear();
    }

    // ===================================================================
    // Streaming Tool Call Accumulation
    // Source: TS `startStreamingToolCall()`, `processStreamingChunk()`,
    //         `finalizeStreamingToolCall()`, `clearAllStreamingToolCalls()`
    // ===================================================================

    /// Start streaming a new tool call.
    ///
    /// Initializes tracking for incremental argument parsing.
    /// Accepts string to support both ToolName and dynamic MCP tools.
    ///
    /// Source: TS `NativeToolCallParser.startStreamingToolCall()`
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

    /// Clear all streaming tool call state.
    ///
    /// Should be called when a new API request starts to prevent memory leaks
    /// from interrupted streams.
    ///
    /// Source: TS `NativeToolCallParser.clearAllStreamingToolCalls()`
    pub fn clear_all_streaming_tool_calls(&mut self) {
        self.streaming_tool_calls.clear();
    }

    /// Check if there are any active streaming tool calls.
    ///
    /// Source: TS `NativeToolCallParser.hasActiveStreamingToolCalls()`
    pub fn has_active_streaming_tool_calls(&self) -> bool {
        !self.streaming_tool_calls.is_empty()
    }

    /// Process a chunk of JSON arguments for a streaming tool call.
    ///
    /// Uses partial JSON parsing to extract values from incomplete JSON immediately.
    /// Returns a partial ToolUse with currently parsed parameters, or None
    /// if the tool call is not tracked or parsing fails.
    ///
    /// Source: TS `NativeToolCallParser.processStreamingChunk()`
    pub fn process_streaming_chunk(&mut self, id: &str, chunk: &str) -> Option<ToolUse> {
        let tool_call = self.streaming_tool_calls.get_mut(id)?;

        // Accumulate the JSON string
        tool_call.arguments_accumulator.push_str(chunk);

        // For dynamic MCP tools, we don't return partial updates - wait for final
        let mcp_prefix = format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR);
        if tool_call.name.starts_with(&mcp_prefix) {
            return None;
        }

        // Try to parse whatever we can from the incomplete JSON
        // In TS this uses partial-json-parser. Here we try a simple approach:
        // try full parse, and if that fails, try to fix common issues.
        let partial_args = match Self::try_parse_partial_json(&tool_call.arguments_accumulator) {
            Some(v) => v,
            None => return None,
        };

        // Resolve tool alias to canonical name
        let resolved_name = Self::resolve_tool_alias(&tool_call.name);
        // Preserve original name if it differs from resolved (i.e., it was an alias)
        let original_name = if tool_call.name != resolved_name {
            Some(tool_call.name.clone())
        } else {
            None
        };

        // Create partial ToolUse with extracted values
        let empty_map = serde_json::Map::new();
        let partial_obj = partial_args.as_object().unwrap_or(&empty_map);
        Self::create_partial_tool_use(
            &tool_call.id,
            resolved_name,
            partial_obj,
            true, // partial
            original_name.as_deref(),
        )
    }

    /// Finalize a streaming tool call.
    ///
    /// Parses the complete JSON and returns the final ToolUse or McpToolUse.
    ///
    /// Source: TS `NativeToolCallParser.finalizeStreamingToolCall()`
    pub fn finalize_streaming_tool_call(&mut self, id: &str) -> Option<AssistantMessageContent> {
        let tool_call = self.streaming_tool_calls.remove(id)?;

        // Parse the complete accumulated JSON via parseToolCall
        let result = self.parse_tool_call(&tool_call.id, &tool_call.name, &tool_call.arguments_accumulator);

        result
    }

    // ===================================================================
    // Partial JSON parsing
    // ===================================================================

    /// Try to parse potentially incomplete JSON.
    ///
    /// This is a simplified version of the TS `partial-json` library.
    /// It tries to parse the string as-is first, then attempts to close
    /// open braces/brackets.
    fn try_parse_partial_json(s: &str) -> Option<Value> {
        // Try full parse first
        if let Ok(v) = serde_json::from_str::<Value>(s) {
            return Some(v);
        }

        // Try to fix incomplete JSON by closing open structures
        let mut fixed = s.to_string();
        let mut open_braces = 0i32;
        let mut open_brackets = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for ch in fixed.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }
            match ch {
                '{' => open_braces += 1,
                '}' => open_braces -= 1,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }

        // Close any open string
        if in_string {
            fixed.push('"');
        }

        // Close open structures
        for _ in 0..open_brackets.max(0) {
            fixed.push(']');
        }
        for _ in 0..open_braces.max(0) {
            fixed.push('}');
        }

        serde_json::from_str::<Value>(&fixed).ok()
    }

    // ===================================================================
    // Tool Call Parsing
    // Source: TS `parseToolCall()`, `parseDynamicMcpTool()`
    // ===================================================================

    /// Convert a native tool call chunk to a ToolUse or McpToolUse object.
    ///
    /// Source: TS `NativeToolCallParser.parseToolCall()`
    pub fn parse_tool_call(
        &self,
        id: &str,
        name: &str,
        arguments: &str,
    ) -> Option<AssistantMessageContent> {
        // Check if this is a dynamic MCP tool (mcp--serverName--toolName)
        // Also handle models that output underscores instead of hyphens
        let mcp_prefix = format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR);
        let normalized_name = normalize_mcp_tool_name(name);

        if normalized_name.starts_with(&mcp_prefix) {
            // Pass the original tool call but with normalized name for parsing
            return self.parse_dynamic_mcp_tool(id, &normalized_name, arguments);
        }

        // Resolve tool alias to canonical name
        let resolved_name = Self::resolve_tool_alias(name);

        // Validate tool name (after alias resolution)
        if !Self::is_valid_tool_name(resolved_name) {
            warn!("Invalid tool name: {} (resolved: {})", name, resolved_name);
            return None;
        }

        // Parse the arguments JSON string
        let args: Value = if arguments.is_empty() {
            json!({})
        } else {
            match serde_json::from_str(arguments) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to parse tool call arguments: {}", e);
                    return None;
                }
            }
        };

        // Build stringified params for display/logging
        let mut params = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                if is_valid_tool_param(key) {
                    let string_value = match value {
                        Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    params.insert(key.clone(), string_value);
                }
            }
        }

        // Build typed nativeArgs for tool execution
        let native_args = Self::build_native_args(resolved_name, &args);
        let mut used_legacy_format = false;

        // Check for legacy format (read_file with files array)
        if resolved_name == "read_file" {
            if let Some(obj) = args.as_object() {
                if obj.contains_key("files") {
                    used_legacy_format = true;
                }
            }
        }

        // Native-only: core tools must always have typed nativeArgs
        if native_args.is_none() {
            warn!(
                "Invalid arguments for tool '{}'. Received: {}",
                resolved_name,
                serde_json::to_string(&args).unwrap_or_default()
            );
            return None;
        }

        let mut result = ToolUse {
            content_type: "tool_use".to_string(),
            name: resolved_name.to_string(),
            params,
            partial: false, // Native tool calls are always complete when yielded
            id: id.to_string(),
            native_args: native_args,
            original_name: if name != resolved_name {
                Some(name.to_string())
            } else {
                None
            },
            used_legacy_format,
        };

        // Track legacy format usage for telemetry
        if used_legacy_format {
            result.used_legacy_format = true;
        }

        Some(AssistantMessageContent::ToolUse(result))
    }

    /// Parse dynamic MCP tools (named mcp--serverName--toolName).
    ///
    /// These are generated dynamically by getMcpServerTools() and are returned
    /// as McpToolUse objects that preserve the original tool name.
    ///
    /// Source: TS `NativeToolCallParser.parseDynamicMcpTool()`
    pub fn parse_dynamic_mcp_tool(
        &self,
        id: &str,
        name: &str,
        arguments: &str,
    ) -> Option<AssistantMessageContent> {
        // Parse the arguments - these are the actual tool arguments passed directly
        let args: Value = serde_json::from_str(if arguments.is_empty() {
            "{}"
        } else {
            arguments
        })
        .unwrap_or_default();

        // Normalize the tool name to handle models that output underscores instead of hyphens
        let normalized_name = normalize_mcp_tool_name(name);

        // Extract server_name and tool_name from the tool name itself
        let parsed = parse_mcp_tool_name(&normalized_name)?;
        let (server_name, tool_name) = parsed;

        Some(AssistantMessageContent::McpToolUse(McpToolUse {
            content_type: "mcp_tool_use".to_string(),
            name: name.to_string(),
            id: id.to_string(),
            server_name,
            tool_name,
            arguments: args,
            partial: false,
        }))
    }

    // ===================================================================
    // Per-tool nativeArgs construction
    // Source: TS `parseToolCall()` switch statement (L728-995)
    //         and `createPartialToolUse()` switch statement (L397-642)
    // ===================================================================

    /// Build typed native arguments for a tool call.
    ///
    /// Each case validates the minimum required parameters and constructs a properly
    /// typed nativeArgs object. If validation fails, returns None.
    ///
    /// Source: TS `parseToolCall()` — nativeArgs construction switch
    fn build_native_args(resolved_name: &str, args: &Value) -> Option<Value> {
        let obj = args.as_object();

        match resolved_name {
            // -----------------------------------------------------------------
            // read_file
            // -----------------------------------------------------------------
            "read_file" => {
                // Check for legacy format first: { files: [...] }
                if let Some(obj) = obj {
                    if let Some(files_val) = obj.get("files") {
                        let files_array = Self::extract_files_array(files_val);
                        if let Some(files) = files_array {
                            if !files.is_empty() {
                                return Some(json!({
                                    "files": Self::convert_file_entries(&files),
                                    "_legacyFormat": true,
                                }));
                            }
                        }
                    }
                }
                // New format: { path: "...", mode: "..." }
                if let Some(obj) = obj {
                    if let Some(path) = obj.get("path") {
                        let indentation = if let Some(indent) = obj.get("indentation") {
                            if indent.is_object() {
                                Some(json!({
                                    "anchor_line": indent.get("anchor_line").and_then(|v| Self::coerce_optional_number(v)),
                                    "max_levels": indent.get("max_levels").and_then(|v| Self::coerce_optional_number(v)),
                                    "max_lines": indent.get("max_lines").and_then(|v| Self::coerce_optional_number(v)),
                                    "include_siblings": indent.get("include_siblings").and_then(|v| Self::coerce_optional_boolean(v)),
                                    "include_header": indent.get("include_header").and_then(|v| Self::coerce_optional_boolean(v)),
                                }))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        return Some(json!({
                            "path": path,
                            "mode": obj.get("mode"),
                            "offset": obj.get("offset").and_then(|v| Self::coerce_optional_number(v)),
                            "limit": obj.get("limit").and_then(|v| Self::coerce_optional_number(v)),
                            "indentation": indentation,
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // attempt_completion
            // -----------------------------------------------------------------
            "attempt_completion" => {
                if let Some(obj) = obj {
                    if obj.contains_key("result") {
                        return Some(json!({
                            "result": obj.get("result"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // execute_command
            // -----------------------------------------------------------------
            "execute_command" => {
                if let Some(obj) = obj {
                    if obj.contains_key("command") {
                        return Some(json!({
                            "command": obj.get("command"),
                            "cwd": obj.get("cwd"),
                            "timeout": obj.get("timeout"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // apply_diff
            // -----------------------------------------------------------------
            "apply_diff" => {
                if let Some(obj) = obj {
                    if obj.contains_key("path") && obj.contains_key("diff") {
                        return Some(json!({
                            "path": obj.get("path"),
                            "diff": obj.get("diff"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // edit / search_and_replace (alias)
            // -----------------------------------------------------------------
            "edit" | "search_and_replace" => {
                if let Some(obj) = obj {
                    if obj.contains_key("file_path")
                        && obj.contains_key("old_string")
                        && obj.contains_key("new_string")
                    {
                        return Some(json!({
                            "file_path": obj.get("file_path"),
                            "old_string": obj.get("old_string"),
                            "new_string": obj.get("new_string"),
                            "replace_all": obj.get("replace_all").and_then(|v| Self::coerce_optional_boolean(v)),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // ask_followup_question
            // -----------------------------------------------------------------
            "ask_followup_question" => {
                if let Some(obj) = obj {
                    if obj.contains_key("question") && obj.contains_key("follow_up") {
                        return Some(json!({
                            "question": obj.get("question"),
                            "follow_up": obj.get("follow_up"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // codebase_search
            // -----------------------------------------------------------------
            "codebase_search" => {
                if let Some(obj) = obj {
                    if obj.contains_key("query") {
                        return Some(json!({
                            "query": obj.get("query"),
                            "path": obj.get("path"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // generate_image
            // -----------------------------------------------------------------
            "generate_image" => {
                if let Some(obj) = obj {
                    if obj.contains_key("prompt") && obj.contains_key("path") {
                        return Some(json!({
                            "prompt": obj.get("prompt"),
                            "path": obj.get("path"),
                            "image": obj.get("image"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // run_slash_command
            // -----------------------------------------------------------------
            "run_slash_command" => {
                if let Some(obj) = obj {
                    if obj.contains_key("command") {
                        return Some(json!({
                            "command": obj.get("command"),
                            "args": obj.get("args"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // skill
            // -----------------------------------------------------------------
            "skill" => {
                if let Some(obj) = obj {
                    if obj.contains_key("skill") {
                        return Some(json!({
                            "skill": obj.get("skill"),
                            "args": obj.get("args"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // search_files
            // -----------------------------------------------------------------
            "search_files" => {
                if let Some(obj) = obj {
                    if obj.contains_key("path") && obj.contains_key("regex") {
                        return Some(json!({
                            "path": obj.get("path"),
                            "regex": obj.get("regex"),
                            "file_pattern": obj.get("file_pattern"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // switch_mode
            // -----------------------------------------------------------------
            "switch_mode" => {
                if let Some(obj) = obj {
                    if obj.contains_key("mode_slug") && obj.contains_key("reason") {
                        return Some(json!({
                            "mode_slug": obj.get("mode_slug"),
                            "reason": obj.get("reason"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // update_todo_list
            // -----------------------------------------------------------------
            "update_todo_list" => {
                if let Some(obj) = obj {
                    if obj.contains_key("todos") {
                        return Some(json!({
                            "todos": obj.get("todos"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // read_command_output
            // -----------------------------------------------------------------
            "read_command_output" => {
                if let Some(obj) = obj {
                    if obj.contains_key("artifact_id") {
                        return Some(json!({
                            "artifact_id": obj.get("artifact_id"),
                            "search": obj.get("search"),
                            "offset": obj.get("offset"),
                            "limit": obj.get("limit"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // write_to_file
            // -----------------------------------------------------------------
            "write_to_file" => {
                if let Some(obj) = obj {
                    if obj.contains_key("path") && obj.contains_key("content") {
                        return Some(json!({
                            "path": obj.get("path"),
                            "content": obj.get("content"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // use_mcp_tool
            // -----------------------------------------------------------------
            "use_mcp_tool" => {
                if let Some(obj) = obj {
                    if obj.contains_key("server_name") && obj.contains_key("tool_name") {
                        return Some(json!({
                            "server_name": obj.get("server_name"),
                            "tool_name": obj.get("tool_name"),
                            "arguments": obj.get("arguments"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // access_mcp_resource
            // -----------------------------------------------------------------
            "access_mcp_resource" => {
                if let Some(obj) = obj {
                    if obj.contains_key("server_name") && obj.contains_key("uri") {
                        return Some(json!({
                            "server_name": obj.get("server_name"),
                            "uri": obj.get("uri"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // apply_patch
            // -----------------------------------------------------------------
            "apply_patch" => {
                if let Some(obj) = obj {
                    if obj.contains_key("patch") {
                        return Some(json!({
                            "patch": obj.get("patch"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // search_replace (legacy alias)
            // -----------------------------------------------------------------
            "search_replace" => {
                if let Some(obj) = obj {
                    if obj.contains_key("file_path")
                        && obj.contains_key("old_string")
                        && obj.contains_key("new_string")
                    {
                        return Some(json!({
                            "file_path": obj.get("file_path"),
                            "old_string": obj.get("old_string"),
                            "new_string": obj.get("new_string"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // edit_file
            // -----------------------------------------------------------------
            "edit_file" => {
                if let Some(obj) = obj {
                    if obj.contains_key("file_path")
                        && obj.contains_key("old_string")
                        && obj.contains_key("new_string")
                    {
                        return Some(json!({
                            "file_path": obj.get("file_path"),
                            "old_string": obj.get("old_string"),
                            "new_string": obj.get("new_string"),
                            "expected_replacements": obj.get("expected_replacements"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // list_files
            // -----------------------------------------------------------------
            "list_files" => {
                if let Some(obj) = obj {
                    if obj.contains_key("path") {
                        return Some(json!({
                            "path": obj.get("path"),
                            "recursive": obj.get("recursive").and_then(|v| Self::coerce_optional_boolean(v)),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // new_task
            // -----------------------------------------------------------------
            "new_task" => {
                if let Some(obj) = obj {
                    if obj.contains_key("mode") && obj.contains_key("message") {
                        return Some(json!({
                            "mode": obj.get("mode"),
                            "message": obj.get("message"),
                            "todos": obj.get("todos"),
                        }));
                    }
                }
                None
            }

            // -----------------------------------------------------------------
            // Default: unknown tool
            // -----------------------------------------------------------------
            _ => {
                // For custom tools, pass through args as-is
                Some(args.clone())
            }
        }
    }

    /// Extract a files array from a value, handling double-stringified cases.
    ///
    /// Source: TS `parseToolCall()` — read_file legacy format handling
    fn extract_files_array(files_val: &Value) -> Option<Vec<Value>> {
        // Handle array directly
        if let Some(arr) = files_val.as_array() {
            if !arr.is_empty() {
                return Some(arr.clone());
            }
        }
        // Handle double-stringified case: files is a string containing JSON array
        if let Some(s) = files_val.as_str() {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                if let Some(arr) = parsed.as_array() {
                    if !arr.is_empty() {
                        return Some(arr.clone());
                    }
                }
            }
        }
        None
    }

    // ===================================================================
    // Partial ToolUse Creation
    // Source: TS `createPartialToolUse()`
    // ===================================================================

    /// Create a partial ToolUse from currently parsed arguments.
    ///
    /// Used during streaming to show progress.
    ///
    /// Source: TS `NativeToolCallParser.createPartialToolUse()`
    fn create_partial_tool_use(
        id: &str,
        name: &str,
        partial_args: &serde_json::Map<String, Value>,
        partial: bool,
        original_name: Option<&str>,
    ) -> Option<ToolUse> {
        // Build stringified params for display/partial-progress UI
        let mut params = HashMap::new();
        for (key, value) in partial_args {
            if is_valid_tool_param(key) {
                let string_value = match value {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                params.insert(key.clone(), string_value);
            }
        }

        // Build partial nativeArgs based on what we have so far
        let native_args = Self::build_partial_native_args(name, partial_args);
        let mut used_legacy_format = false;

        // Check for legacy format
        if name == "read_file" && partial_args.contains_key("files") {
            used_legacy_format = true;
        }

        let mut result = ToolUse {
            content_type: "tool_use".to_string(),
            name: name.to_string(),
            params,
            partial,
            id: id.to_string(),
            native_args,
            original_name: original_name.map(|s| s.to_string()),
            used_legacy_format,
        };

        if used_legacy_format {
            result.used_legacy_format = true;
        }

        Some(result)
    }

    /// Build partial nativeArgs for streaming progress display.
    ///
    /// Source: TS `NativeToolCallParser.createPartialToolUse()` — switch statement
    fn build_partial_native_args(
        name: &str,
        partial_args: &serde_json::Map<String, Value>,
    ) -> Option<Value> {
        match name {
            // -----------------------------------------------------------------
            // read_file
            // -----------------------------------------------------------------
            "read_file" => {
                // Check for legacy format first: { files: [...] }
                if let Some(files_val) = partial_args.get("files") {
                    let files_array = Self::extract_files_array(files_val);
                    if let Some(files) = files_array {
                        if !files.is_empty() {
                            return Some(json!({
                                "files": Self::convert_file_entries(&files),
                                "_legacyFormat": true,
                            }));
                        }
                    }
                }
                // New format: { path: "...", mode: "..." }
                if partial_args.contains_key("path") {
                    let indentation = if let Some(indent) = partial_args.get("indentation") {
                        if indent.is_object() {
                            Some(json!({
                                "anchor_line": indent.get("anchor_line").and_then(|v| Self::coerce_optional_number(v)),
                                "max_levels": indent.get("max_levels").and_then(|v| Self::coerce_optional_number(v)),
                                "max_lines": indent.get("max_lines").and_then(|v| Self::coerce_optional_number(v)),
                                "include_siblings": indent.get("include_siblings").and_then(|v| Self::coerce_optional_boolean(v)),
                                "include_header": indent.get("include_header").and_then(|v| Self::coerce_optional_boolean(v)),
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    return Some(json!({
                        "path": partial_args.get("path"),
                        "mode": partial_args.get("mode"),
                        "offset": partial_args.get("offset").and_then(|v| Self::coerce_optional_number(v)),
                        "limit": partial_args.get("limit").and_then(|v| Self::coerce_optional_number(v)),
                        "indentation": indentation,
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // attempt_completion
            // -----------------------------------------------------------------
            "attempt_completion" => {
                if partial_args.contains_key("result") {
                    return Some(json!({
                        "result": partial_args.get("result"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // execute_command
            // -----------------------------------------------------------------
            "execute_command" => {
                if partial_args.contains_key("command") {
                    return Some(json!({
                        "command": partial_args.get("command"),
                        "cwd": partial_args.get("cwd"),
                        "timeout": partial_args.get("timeout"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // write_to_file
            // -----------------------------------------------------------------
            "write_to_file" => {
                if partial_args.contains_key("path") || partial_args.contains_key("content") {
                    return Some(json!({
                        "path": partial_args.get("path"),
                        "content": partial_args.get("content"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // ask_followup_question
            // -----------------------------------------------------------------
            "ask_followup_question" => {
                if partial_args.contains_key("question") || partial_args.contains_key("follow_up") {
                    return Some(json!({
                        "question": partial_args.get("question"),
                        "follow_up": partial_args.get("follow_up"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // apply_diff
            // -----------------------------------------------------------------
            "apply_diff" => {
                if partial_args.contains_key("path") || partial_args.contains_key("diff") {
                    return Some(json!({
                        "path": partial_args.get("path"),
                        "diff": partial_args.get("diff"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // codebase_search
            // -----------------------------------------------------------------
            "codebase_search" => {
                if partial_args.contains_key("query") {
                    return Some(json!({
                        "query": partial_args.get("query"),
                        "path": partial_args.get("path"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // generate_image
            // -----------------------------------------------------------------
            "generate_image" => {
                if partial_args.contains_key("prompt") || partial_args.contains_key("path") {
                    return Some(json!({
                        "prompt": partial_args.get("prompt"),
                        "path": partial_args.get("path"),
                        "image": partial_args.get("image"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // run_slash_command
            // -----------------------------------------------------------------
            "run_slash_command" => {
                if partial_args.contains_key("command") {
                    return Some(json!({
                        "command": partial_args.get("command"),
                        "args": partial_args.get("args"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // skill
            // -----------------------------------------------------------------
            "skill" => {
                if partial_args.contains_key("skill") {
                    return Some(json!({
                        "skill": partial_args.get("skill"),
                        "args": partial_args.get("args"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // search_files
            // -----------------------------------------------------------------
            "search_files" => {
                if partial_args.contains_key("path") || partial_args.contains_key("regex") {
                    return Some(json!({
                        "path": partial_args.get("path"),
                        "regex": partial_args.get("regex"),
                        "file_pattern": partial_args.get("file_pattern"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // switch_mode
            // -----------------------------------------------------------------
            "switch_mode" => {
                if partial_args.contains_key("mode_slug") || partial_args.contains_key("reason") {
                    return Some(json!({
                        "mode_slug": partial_args.get("mode_slug"),
                        "reason": partial_args.get("reason"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // update_todo_list
            // -----------------------------------------------------------------
            "update_todo_list" => {
                if partial_args.contains_key("todos") {
                    return Some(json!({
                        "todos": partial_args.get("todos"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // use_mcp_tool
            // -----------------------------------------------------------------
            "use_mcp_tool" => {
                if partial_args.contains_key("server_name") || partial_args.contains_key("tool_name") {
                    return Some(json!({
                        "server_name": partial_args.get("server_name"),
                        "tool_name": partial_args.get("tool_name"),
                        "arguments": partial_args.get("arguments"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // apply_patch
            // -----------------------------------------------------------------
            "apply_patch" => {
                if partial_args.contains_key("patch") {
                    return Some(json!({
                        "patch": partial_args.get("patch"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // search_replace (legacy)
            // -----------------------------------------------------------------
            "search_replace" => {
                if partial_args.contains_key("file_path")
                    || partial_args.contains_key("old_string")
                    || partial_args.contains_key("new_string")
                {
                    return Some(json!({
                        "file_path": partial_args.get("file_path"),
                        "old_string": partial_args.get("old_string"),
                        "new_string": partial_args.get("new_string"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // edit / search_and_replace (alias)
            // -----------------------------------------------------------------
            "edit" | "search_and_replace" => {
                if partial_args.contains_key("file_path")
                    || partial_args.contains_key("old_string")
                    || partial_args.contains_key("new_string")
                {
                    return Some(json!({
                        "file_path": partial_args.get("file_path"),
                        "old_string": partial_args.get("old_string"),
                        "new_string": partial_args.get("new_string"),
                        "replace_all": partial_args.get("replace_all").and_then(|v| Self::coerce_optional_boolean(v)),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // edit_file
            // -----------------------------------------------------------------
            "edit_file" => {
                if partial_args.contains_key("file_path")
                    || partial_args.contains_key("old_string")
                    || partial_args.contains_key("new_string")
                {
                    return Some(json!({
                        "file_path": partial_args.get("file_path"),
                        "old_string": partial_args.get("old_string"),
                        "new_string": partial_args.get("new_string"),
                        "expected_replacements": partial_args.get("expected_replacements"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // list_files
            // -----------------------------------------------------------------
            "list_files" => {
                if partial_args.contains_key("path") {
                    return Some(json!({
                        "path": partial_args.get("path"),
                        "recursive": partial_args.get("recursive").and_then(|v| Self::coerce_optional_boolean(v)),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // new_task
            // -----------------------------------------------------------------
            "new_task" => {
                if partial_args.contains_key("mode") || partial_args.contains_key("message") {
                    return Some(json!({
                        "mode": partial_args.get("mode"),
                        "message": partial_args.get("message"),
                        "todos": partial_args.get("todos"),
                    }));
                }
                None
            }

            // -----------------------------------------------------------------
            // Default
            // -----------------------------------------------------------------
            _ => None,
        }
    }
}

impl Default for NativeToolCallParser {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helper to create a parser ---
    fn new_parser() -> NativeToolCallParser {
        NativeToolCallParser::new()
    }

    // ===================================================================
    // Raw chunk processing tests
    // ===================================================================

    #[test]
    fn test_process_raw_chunk_initializes_tracking() {
        let mut parser = new_parser();
        let events = parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::Start { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected Start event"),
        }
    }

    #[test]
    fn test_process_raw_chunk_buffers_before_name() {
        let mut parser = new_parser();
        // First chunk: id but no name
        let events = parser.process_raw_chunk(0, Some("call_1"), None, Some("{\"pat"));
        assert!(events.is_empty()); // No name yet, so no start event

        // Second chunk: name arrives
        let events = parser.process_raw_chunk(0, None, Some("read_file"), None);
        // Should emit Start + buffered delta
        assert_eq!(events.len(), 2);
        match &events[0] {
            ToolCallStreamEvent::Start { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected Start event"),
        }
        match &events[1] {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, "{\"pat");
            }
            _ => panic!("Expected Delta event"),
        }
    }

    #[test]
    fn test_process_raw_chunk_emits_deltas() {
        let mut parser = new_parser();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        let events = parser.process_raw_chunk(0, None, None, Some("{\"path\":"));
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::Delta { id, delta } => {
                assert_eq!(id, "call_1");
                assert_eq!(delta, "{\"path\":");
            }
            _ => panic!("Expected Delta event"),
        }
    }

    #[test]
    fn test_process_finish_reason() {
        let mut parser = new_parser();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        let events = parser.process_finish_reason(Some("tool_calls"));
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::End { id } => {
                assert_eq!(id, "call_1");
            }
            _ => panic!("Expected End event"),
        }
    }

    #[test]
    fn test_process_finish_reason_not_tool_calls() {
        let mut parser = new_parser();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        let events = parser.process_finish_reason(Some("stop"));
        assert!(events.is_empty());
    }

    #[test]
    fn test_finalize_raw_chunks() {
        let mut parser = new_parser();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        let events = parser.finalize_raw_chunks();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ToolCallStreamEvent::End { id } => {
                assert_eq!(id, "call_1");
            }
            _ => panic!("Expected End event"),
        }
        // Should be cleared now
        assert!(parser.raw_chunk_tracker.is_empty());
    }

    #[test]
    fn test_clear_raw_chunk_state() {
        let mut parser = new_parser();
        parser.process_raw_chunk(0, Some("call_1"), Some("read_file"), None);
        parser.clear_raw_chunk_state();
        assert!(parser.raw_chunk_tracker.is_empty());
    }

    // ===================================================================
    // Streaming tool call tests
    // ===================================================================

    #[test]
    fn test_start_streaming_tool_call() {
        let mut parser = new_parser();
        parser.start_streaming_tool_call("call_1", "read_file");
        assert!(parser.has_active_streaming_tool_calls());
        assert!(parser.streaming_tool_calls.contains_key("call_1"));
    }

    #[test]
    fn test_clear_all_streaming_tool_calls() {
        let mut parser = new_parser();
        parser.start_streaming_tool_call("call_1", "read_file");
        parser.clear_all_streaming_tool_calls();
        assert!(!parser.has_active_streaming_tool_calls());
    }

    #[test]
    fn test_process_streaming_chunk_accumulates() {
        let mut parser = new_parser();
        parser.start_streaming_tool_call("call_1", "read_file");

        // First chunk - incomplete JSON
        let result = parser.process_streaming_chunk("call_1", "{\"path\":");
        assert!(result.is_none()); // Can't parse incomplete JSON

        // Second chunk - complete JSON
        let result = parser.process_streaming_chunk("call_1", " \"test.rs\"}");
        assert!(result.is_some());
        let tool_use = result.unwrap();
        assert_eq!(tool_use.name, "read_file");
        assert!(tool_use.partial);
    }

    #[test]
    fn test_process_streaming_chunk_mcp_returns_none() {
        let mut parser = new_parser();
        parser.start_streaming_tool_call("call_1", "mcp--server--tool");
        let result = parser.process_streaming_chunk("call_1", "{\"arg\":");
        assert!(result.is_none());
    }

    #[test]
    fn test_finalize_streaming_tool_call() {
        let mut parser = new_parser();
        parser.start_streaming_tool_call("call_1", "read_file");
        parser.process_streaming_chunk("call_1", "{\"path\": \"test.rs\"}");

        let result = parser.finalize_streaming_tool_call("call_1");
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(!tu.partial);
                assert_eq!(tu.id, "call_1");
            }
            _ => panic!("Expected ToolUse"),
        }
        // Should be removed from streaming state
        assert!(!parser.has_active_streaming_tool_calls());
    }

    // ===================================================================
    // parse_tool_call tests
    // ===================================================================

    #[test]
    fn test_parse_tool_call_read_file() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "read_file",
            r#"{"path": "test.rs", "mode": "slice"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(!tu.partial);
                assert_eq!(tu.id, "call_1");
                assert!(tu.native_args.is_some());
                let args = tu.native_args.unwrap();
                assert_eq!(args["path"], "test.rs");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_execute_command() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "execute_command",
            r#"{"command": "cargo build"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "execute_command");
                assert!(tu.native_args.is_some());
                let args = tu.native_args.unwrap();
                assert_eq!(args["command"], "cargo build");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_write_to_file() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "write_to_file",
            r#"{"path": "test.rs", "content": "fn main() {}"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "write_to_file");
                assert!(tu.native_args.is_some());
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_invalid_name() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "nonexistent_tool",
            r#"{"arg": "value"}"#,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_tool_call_empty_arguments() {
        let parser = new_parser();
        let result = parser.parse_tool_call("call_1", "list_files", "");
        // list_files requires path, so native_args should be None → returns None
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_tool_call_list_files() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "list_files",
            r#"{"path": ".", "recursive": true}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "list_files");
                assert!(tu.native_args.is_some());
                let args = tu.native_args.unwrap();
                assert_eq!(args["path"], ".");
                assert_eq!(args["recursive"], true);
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    // ===================================================================
    // Alias resolution tests
    // ===================================================================

    #[test]
    fn test_resolve_tool_alias_write_file() {
        assert_eq!(NativeToolCallParser::resolve_tool_alias("write_file"), "write_to_file");
    }

    #[test]
    fn test_resolve_tool_alias_search_and_replace() {
        assert_eq!(NativeToolCallParser::resolve_tool_alias("search_and_replace"), "edit");
    }

    #[test]
    fn test_resolve_tool_alias_no_alias() {
        assert_eq!(NativeToolCallParser::resolve_tool_alias("read_file"), "read_file");
    }

    // ===================================================================
    // MCP tool parsing tests
    // ===================================================================

    #[test]
    fn test_parse_dynamic_mcp_tool() {
        let parser = new_parser();
        let result = parser.parse_dynamic_mcp_tool(
            "call_1",
            "mcp--myServer--myTool",
            r#"{"arg1": "value1"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.name, "mcp--myServer--myTool");
                assert_eq!(mtu.server_name, "myServer");
                assert_eq!(mtu.tool_name, "myTool");
                assert_eq!(mtu.id, "call_1");
                assert!(!mtu.partial);
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    #[test]
    fn test_parse_dynamic_mcp_tool_normalizes_underscores() {
        let parser = new_parser();
        let result = parser.parse_dynamic_mcp_tool(
            "call_1",
            "mcp__myServer__myTool",
            r#"{"arg1": "value1"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.server_name, "myServer");
                assert_eq!(mtu.tool_name, "myTool");
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_mcp_tool() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "mcp--myServer--myTool",
            r#"{"arg1": "value1"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::McpToolUse(mtu) => {
                assert_eq!(mtu.server_name, "myServer");
                assert_eq!(mtu.tool_name, "myTool");
            }
            _ => panic!("Expected McpToolUse"),
        }
    }

    // ===================================================================
    // Helper method tests
    // ===================================================================

    #[test]
    fn test_coerce_optional_boolean() {
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!(true)), Some(true));
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!(false)), Some(false));
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!("true")), Some(true));
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!("false")), Some(false));
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!("yes")), None);
        assert_eq!(NativeToolCallParser::coerce_optional_boolean(&json!(42)), None);
    }

    #[test]
    fn test_coerce_optional_number() {
        assert_eq!(NativeToolCallParser::coerce_optional_number(&json!(42)), Some(42.0));
        assert_eq!(NativeToolCallParser::coerce_optional_number(&json!("42")), Some(42.0));
        assert_eq!(NativeToolCallParser::coerce_optional_number(&json!("abc")), None);
        assert_eq!(NativeToolCallParser::coerce_optional_number(&json!(true)), None);
    }

    #[test]
    fn test_convert_file_entries_tuple_format() {
        let files = vec![json!({"path": "test.rs", "line_ranges": [[1, 50], [100, 150]]})];
        let result = NativeToolCallParser::convert_file_entries(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["path"], "test.rs");
        let ranges = result[0]["lineRanges"].as_array().unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0]["start"], 1);
        assert_eq!(ranges[0]["end"], 50);
    }

    #[test]
    fn test_convert_file_entries_object_format() {
        let files = vec![json!({"path": "test.rs", "line_ranges": [{"start": 1, "end": 50}]})];
        let result = NativeToolCallParser::convert_file_entries(&files);
        let ranges = result[0]["lineRanges"].as_array().unwrap();
        assert_eq!(ranges[0]["start"], 1);
        assert_eq!(ranges[0]["end"], 50);
    }

    #[test]
    fn test_convert_file_entries_string_format() {
        let files = vec![json!({"path": "test.rs", "line_ranges": ["1-50"]})];
        let result = NativeToolCallParser::convert_file_entries(&files);
        let ranges = result[0]["lineRanges"].as_array().unwrap();
        assert_eq!(ranges[0]["start"], 1);
        assert_eq!(ranges[0]["end"], 50);
    }

    // ===================================================================
    // read_file legacy format tests
    // ===================================================================

    #[test]
    fn test_parse_tool_call_read_file_legacy_format() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "read_file",
            r#"{"files": [{"path": "test.rs", "line_ranges": [[1, 50]]}]}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "read_file");
                assert!(tu.used_legacy_format);
                assert!(tu.native_args.is_some());
                let args = tu.native_args.unwrap();
                assert!(args.get("files").is_some());
                assert_eq!(args["_legacyFormat"], true);
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_tool_call_read_file_legacy_double_stringified() {
        let parser = new_parser();
        let inner = serde_json::to_string(&json!([{"path": "test.rs"}])).unwrap();
        let args = json!({"files": inner}).to_string();
        let result = parser.parse_tool_call("call_1", "read_file", &args);
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert!(tu.used_legacy_format);
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    // ===================================================================
    // Alias resolution in parse_tool_call
    // ===================================================================

    #[test]
    fn test_parse_tool_call_resolves_alias() {
        let parser = new_parser();
        let result = parser.parse_tool_call(
            "call_1",
            "write_file",
            r#"{"path": "test.rs", "content": "hello"}"#,
        );
        assert!(result.is_some());
        match result.unwrap() {
            AssistantMessageContent::ToolUse(tu) => {
                assert_eq!(tu.name, "write_to_file"); // resolved alias
                assert_eq!(tu.original_name.as_deref(), Some("write_file"));
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    // ===================================================================
    // Partial JSON parsing tests
    // ===================================================================

    #[test]
    fn test_try_parse_partial_json_complete() {
        let result = NativeToolCallParser::try_parse_partial_json(r#"{"path": "test.rs"}"#);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["path"], "test.rs");
    }

    #[test]
    fn test_try_parse_partial_json_incomplete_object() {
        let result = NativeToolCallParser::try_parse_partial_json(r#"{"path": "test.rs"#);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["path"], "test.rs");
    }

    #[test]
    fn test_try_parse_partial_json_incomplete_nested() {
        let result = NativeToolCallParser::try_parse_partial_json(r#"{"path": "test.rs", "indentation": {"anchor_line": 42"#);
        assert!(result.is_some());
        let v = result.unwrap();
        assert_eq!(v["path"], "test.rs");
        assert_eq!(v["indentation"]["anchor_line"], 42);
    }

    // ===================================================================
    // Multiple tool calls test
    // ===================================================================

    #[test]
    fn test_multiple_concurrent_tool_calls() {
        let mut parser = new_parser();

        // Start two tool calls
        parser.start_streaming_tool_call("call_1", "read_file");
        parser.start_streaming_tool_call("call_2", "write_to_file");

        // Feed chunks to both
        parser.process_streaming_chunk("call_1", r#"{"path":"a.rs"}"#);
        parser.process_streaming_chunk("call_2", r#"{"path":"b.rs","content":"x"}"#);

        // Finalize both
        let r1 = parser.finalize_streaming_tool_call("call_1");
        let r2 = parser.finalize_streaming_tool_call("call_2");

        assert!(r1.is_some());
        assert!(r2.is_some());

        match r1.unwrap() {
            AssistantMessageContent::ToolUse(tu) => assert_eq!(tu.name, "read_file"),
            _ => panic!("Expected ToolUse"),
        }
        match r2.unwrap() {
            AssistantMessageContent::ToolUse(tu) => assert_eq!(tu.name, "write_to_file"),
            _ => panic!("Expected ToolUse"),
        }
    }

    // ===================================================================
    // All tool types test
    // ===================================================================

    #[test]
    fn test_parse_all_tool_types() {
        let parser = new_parser();

        let test_cases = vec![
            ("execute_command", r#"{"command": "ls"}"#, true),
            ("read_file", r#"{"path": "test.rs"}"#, true),
            ("read_command_output", r#"{"artifact_id": "abc"}"#, true),
            ("write_to_file", r#"{"path": "test.rs", "content": "hi"}"#, true),
            ("apply_diff", r#"{"path": "test.rs", "diff": "--- a\n+++ b"}"#, true),
            ("edit", r#"{"file_path":"a","old_string":"b","new_string":"c"}"#, true),
            ("search_and_replace", r#"{"file_path":"a","old_string":"b","new_string":"c"}"#, true),
            ("edit_file", r#"{"file_path":"a","old_string":"b","new_string":"c"}"#, true),
            ("apply_patch", r#"{"patch": "--- a\n+++ b"}"#, true),
            ("search_files", r#"{"path": ".", "regex": "TODO"}"#, true),
            ("list_files", r#"{"path": "."}"#, true),
            ("use_mcp_tool", r#"{"server_name":"s","tool_name":"t"}"#, true),
            ("access_mcp_resource", r#"{"server_name":"s","uri":"u"}"#, true),
            ("ask_followup_question", r#"{"question":"q","follow_up":[]}"#, true),
            ("attempt_completion", r#"{"result": "done"}"#, true),
            ("switch_mode", r#"{"mode_slug":"code","reason":"r"}"#, true),
            ("new_task", r#"{"mode":"code","message":"m"}"#, true),
            ("codebase_search", r#"{"query": "test"}"#, true),
            ("update_todo_list", r#"{"todos": []}"#, true),
            ("run_slash_command", r#"{"command": "init"}"#, true),
            ("skill", r#"{"skill": "test"}"#, true),
            ("generate_image", r#"{"prompt":"p","path":"p.png"}"#, true),
        ];

        for (name, args, should_succeed) in test_cases {
            let result = parser.parse_tool_call("call_1", name, args);
            assert_eq!(
                result.is_some(),
                should_succeed,
                "Tool '{}' should have {}",
                name,
                if should_succeed { "succeeded" } else { "failed" }
            );
        }
    }
}
