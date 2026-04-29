//! API message builder for constructing conversation history and tool definitions.
//!
//! Builds the messages array and tool definitions sent to the Provider API.
//!
//! Source: `src/core/task/Task.ts` — message building logic in
//! `recursivelyMakeClineRequests` and `presentAssistantMessage`.

use serde_json::{json, Value};
use tracing::debug;

use roo_types::api::{ApiMessage, ContentBlock, ImageSource, MessageRole, ToolResultContent};
use roo_types::mcp::McpServerConnection;
use roo_types::model::ModelInfo;

use crate::stream_parser::ParsedStreamContent;
use crate::tool_dispatcher::ToolExecutionResult;

// ---------------------------------------------------------------------------
// BuildToolsResult
// ---------------------------------------------------------------------------

/// Result of building tools with restrictions.
///
/// Source: `src/core/task/build-tools.ts` — `BuildToolsResult`
pub struct BuildToolsResult {
    /// The tools to pass to the model.
    pub tools: Vec<Value>,
    /// The names of tools that are allowed to be called based on mode restrictions.
    /// Only populated when `include_all_tools_with_restrictions` is true.
    pub allowed_function_names: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// CleanConversationItem
// ---------------------------------------------------------------------------

/// A cleaned conversation history item.
///
/// Source: `src/core/task/Task.ts` — `buildCleanConversationHistory` return type.
/// Can be either a standard API message param or a standalone reasoning item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CleanConversationItem {
    /// A standard message with role and content.
    Message {
        role: String,
        content: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_details: Option<serde_json::Value>,
    },
    /// A standalone reasoning item (OpenAI Native format).
    Reasoning {
        #[serde(rename = "type")]
        item_type: String, // always "reasoning"
        encrypted_content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<serde_json::Value>,
    },
}

// ---------------------------------------------------------------------------
// MessageBuilder
// ---------------------------------------------------------------------------

/// Builds API messages and tool definitions for Provider API calls.
///
/// Responsibilities:
/// 1. Convert conversation history into the `Vec<ApiMessage>` format
/// 2. Build tool definitions in the format expected by the Provider
/// 3. Create user, assistant, and tool-result messages
///
/// Source: `src/core/task/Task.ts` — `recursivelyMakeClineRequests` (message
/// building parts), plus `addToApiConversationHistory` calls scattered
/// throughout the Task class.
pub struct MessageBuilder {
    /// System prompt text (sent separately from messages in most providers).
    system_prompt: String,
    /// Whether the model supports image processing.
    supports_images: bool,
}

impl MessageBuilder {
    /// Create a new message builder with the given system prompt.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            supports_images: false,
        }
    }

    /// Create a message builder with image support flag.
    pub fn with_images_support(mut self, supports: bool) -> Self {
        self.supports_images = supports;
        self
    }

    /// Get the system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Whether the model supports images.
    pub fn supports_images(&self) -> bool {
        self.supports_images
    }

    // -----------------------------------------------------------------------
    // Build tool definitions
    // -----------------------------------------------------------------------

    /// Build tool definitions in the format expected by the Provider API.
    ///
    /// Returns a vector of JSON values, each representing a tool definition
    /// in OpenAI Function Calling format.
    ///
    /// Source: `src/core/task/Task.ts` — tool definition building in
    /// `recursivelyMakeClineRequests`
    pub fn build_tool_definitions(&self) -> Vec<Value> {
        self.build_tool_definitions_with_options(None, &[], None, None, &[])
    }

    /// Build tool definitions with mode-based restrictions.
    ///
    /// Source: `src/core/task/build-tools.ts` — `buildNativeToolsArrayWithRestrictions()`
    ///
    /// This method:
    /// 1. Gets native tool definitions
    /// 2. Filters tools based on the current mode (e.g., "code", "architect", "ask")
    /// 3. Removes tools that are in the `disabled_tools` list
    /// 4. Optionally computes `allowed_function_names` for providers like Gemini
    ///    that support function call restrictions via `allowedFunctionNames`
    /// 5. Merges MCP tools from connected servers
    pub fn build_tool_definitions_with_options(
        &self,
        mode: Option<&str>,
        custom_modes: &[roo_types::mode::ModeConfig],
        disabled_tools: Option<&[String]>,
        experiments: Option<&std::collections::HashMap<String, bool>>,
        mcp_servers: &[McpServerConnection],
    ) -> Vec<Value> {
        let options = roo_tools::definition::NativeToolsOptions {
            supports_images: self.supports_images,
        };
        let native_tools = roo_tools::definition::get_native_tools(options);

        // Apply mode-based filtering
        let filter_settings = roo_tools::filter::FilterSettings {
            todo_list_enabled: true,
            disabled_tools: disabled_tools
                .map(|d| d.to_vec())
                .unwrap_or_default(),
            model_info: None,
            codebase_search_enabled: true,
            mcp_resources_available: true,
        };

        let filtered_tools = roo_tools::filter::filter_native_tools_for_mode(
            &native_tools,
            mode,
            custom_modes,
            experiments,
            &filter_settings,
        );

        // Convert native tools to JSON
        let mut tools_json: Vec<Value> = filtered_tools
            .into_iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                })
            })
            .collect();

        // Merge MCP tools
        let mcp_tools = get_mcp_server_tools(mcp_servers);
        tools_json.extend(mcp_tools);

        tools_json
    }

    /// Build tool definitions with restrictions, returning both tools and allowed function names.
    ///
    /// Source: `src/core/task/build-tools.ts` — `buildNativeToolsArrayWithRestrictions()`
    ///
    /// When `include_all_tools_with_restrictions` is true, returns ALL tools but also
    /// provides the list of allowed tool names for use with `allowedFunctionNames`.
    pub fn build_tool_definitions_with_restrictions(
        &self,
        mode: Option<&str>,
        custom_modes: &[roo_types::mode::ModeConfig],
        disabled_tools: Option<&[String]>,
        experiments: Option<&std::collections::HashMap<String, bool>>,
        include_all_tools_with_restrictions: bool,
        mcp_servers: &[McpServerConnection],
    ) -> BuildToolsResult {
        let options = roo_tools::definition::NativeToolsOptions {
            supports_images: self.supports_images,
        };
        let native_tools = roo_tools::definition::get_native_tools(options);

        let filter_settings = roo_tools::filter::FilterSettings {
            todo_list_enabled: true,
            disabled_tools: disabled_tools
                .map(|d| d.to_vec())
                .unwrap_or_default(),
            model_info: None,
            codebase_search_enabled: true,
            mcp_resources_available: true,
        };

        let filtered_tools = roo_tools::filter::filter_native_tools_for_mode(
            &native_tools,
            mode,
            custom_modes,
            experiments,
            &filter_settings,
        );

        // Get MCP tools
        let mcp_tools = get_mcp_server_tools(mcp_servers);

        if include_all_tools_with_restrictions {
            // Combine filtered native + MCP for allowed names
            let mut allowed_function_names: Vec<String> = filtered_tools
                .iter()
                .map(|t| roo_tools::groups::resolve_tool_alias(&t.name).to_string())
                .collect();
            // Add MCP tool names to allowed list
            allowed_function_names.extend(
                mcp_servers
                    .iter()
                    .flat_map(|s| s.tools.iter())
                    .filter(|t| t.enabled_for_prompt)
                    .map(|t| t.name.clone()),
            );

            let all_native_json: Vec<Value> = native_tools
                .into_iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    })
                })
                .collect();

            // Combine ALL tools (unfiltered native + all MCP)
            let mut all_tools: Vec<Value> = all_native_json;
            all_tools.extend(mcp_tools);

            BuildToolsResult {
                tools: all_tools,
                allowed_function_names: Some(allowed_function_names),
            }
        } else {
            // Default: return only filtered tools + MCP tools
            let mut tools_json: Vec<Value> = filtered_tools
                .into_iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    })
                })
                .collect();
            tools_json.extend(mcp_tools);

            BuildToolsResult {
                tools: tools_json,
                allowed_function_names: None,
            }
        }
    }

    // -----------------------------------------------------------------------
    // Build messages
    // -----------------------------------------------------------------------

    /// Build the complete messages array for an API call.
    ///
    /// Takes the existing conversation history and optionally appends a new
    /// user message. The system prompt is handled separately by the provider.
    ///
    /// Source: `src/core/task/Task.ts` — `recursivelyMakeClineRequests` builds
    /// messages from `apiConversationHistory` before each API call.
    pub fn build_api_messages(
        &self,
        history: &[ApiMessage],
        user_message: Option<&str>,
        images: &[String],
    ) -> Vec<ApiMessage> {
        let mut messages: Vec<ApiMessage> = history.to_vec();

        // Append a new user message if provided
        if let Some(text) = user_message {
            let msg = Self::create_user_message(text, images);
            messages.push(msg);
        }

        debug!(
            "Built {} API messages (history: {}, new user msg: {})",
            messages.len(),
            history.len(),
            user_message.is_some(),
        );

        messages
    }

    // -----------------------------------------------------------------------
    // Clean conversation history
    // -----------------------------------------------------------------------

    /// Build a clean conversation history suitable for sending to the API.
    ///
    /// Processes raw `ApiMessage`s and produces a cleaned list that:
    /// - Strips plain-text reasoning blocks (stored for history, not sent to API)
    /// - Preserves encrypted reasoning blocks (sent as separate reasoning items)
    /// - Handles `reasoning_details` format (OpenRouter/Gemini 3 etc.)
    /// - Converts content arrays to the appropriate format for the API
    ///
    /// Source: `src/core/task/Task.ts` — `buildCleanConversationHistory()`
    pub fn build_clean_conversation_history(
        messages: &[ApiMessage],
        preserve_reasoning: bool,
    ) -> Vec<CleanConversationItem> {
        let mut clean: Vec<CleanConversationItem> = Vec::new();

        for msg in messages {
            // Standalone reasoning: send encrypted, skip plain text
            // In the TS source, messages with type === "reasoning" are handled specially.
            // In our Rust ApiMessage, reasoning is indicated by the `reasoning` field
            // and `reasoning_details` field.
            if let Some(ref reasoning) = msg.reasoning {
                if !reasoning.is_empty() {
                    // Check for encrypted reasoning content
                    // The encrypted content is stored in the reasoning field
                    if reasoning.starts_with("enc_") || reasoning.contains("encrypted_content") {
                        clean.push(CleanConversationItem::Reasoning {
                            item_type: "reasoning".to_string(),
                            encrypted_content: reasoning.clone(),
                            id: msg.truncation_id.clone(),
                            summary: None,
                        });
                        continue;
                    }
                }
            }

            // Handle assistant messages with potential reasoning content
            if msg.role == MessageRole::Assistant {
                let raw_content = &msg.content;

                // Check if this message has reasoning_details (OpenRouter format)
                if let Some(ref details) = msg.reasoning_details {
                    if !details.is_empty() {
                        // Build the assistant message with reasoning_details
                        let content_value = content_blocks_to_api_value(raw_content);
                        clean.push(CleanConversationItem::Message {
                            role: "assistant".to_string(),
                            content: content_value,
                            reasoning_details: Some(serde_json::Value::Array(details.clone())),
                        });
                        continue;
                    }
                }

                // Check for embedded reasoning as first content block
                if let Some(first_block) = raw_content.first() {
                    // Check for encrypted reasoning block
                    if let ContentBlock::Text { text } = first_block {
                        if text.starts_with("{\"type\":\"reasoning\"") {
                            // Try to parse as encrypted reasoning
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                                if parsed.get("type").and_then(|v| v.as_str()) == Some("reasoning") {
                                    if let Some(enc) = parsed.get("encrypted_content").and_then(|v| v.as_str()) {
                                        // Send as separate reasoning item
                                        clean.push(CleanConversationItem::Reasoning {
                                            item_type: "reasoning".to_string(),
                                            encrypted_content: enc.to_string(),
                                            id: parsed.get("id").and_then(|v| v.as_str()).map(String::from),
                                            summary: parsed.get("summary").cloned(),
                                        });

                                        // Send assistant message without reasoning
                                        let rest: Vec<&ContentBlock> = raw_content.iter().skip(1).collect();
                                        let assistant_content =
                                            content_refs_to_api_value(&rest);
                                        clean.push(CleanConversationItem::Message {
                                            role: "assistant".to_string(),
                                            content: assistant_content,
                                            reasoning_details: None,
                                        });
                                        continue;
                                    }
                                }
                            }
                        }

                        // Check for plain text reasoning block
                        if text.starts_with("{\"type\":\"reasoning\",\"text\":") {
                            if !preserve_reasoning {
                                // Strip reasoning out - stored for history only
                                let rest: Vec<&ContentBlock> = raw_content.iter().skip(1).collect();
                                let assistant_content = content_refs_to_api_value(&rest);
                                clean.push(CleanConversationItem::Message {
                                    role: "assistant".to_string(),
                                    content: assistant_content,
                                    reasoning_details: None,
                                });
                                continue;
                            } else {
                                // Include reasoning block in content sent to API
                                let content_value = content_blocks_to_api_value(raw_content);
                                clean.push(CleanConversationItem::Message {
                                    role: "assistant".to_string(),
                                    content: content_value,
                                    reasoning_details: None,
                                });
                                continue;
                            }
                        }
                    }
                }
            }

            // Default path for regular messages (no embedded reasoning)
            if matches!(msg.role, MessageRole::User | MessageRole::Assistant) {
                let content_value = content_blocks_to_api_value(&msg.content);
                clean.push(CleanConversationItem::Message {
                    role: match msg.role {
                        MessageRole::User => "user".to_string(),
                        MessageRole::Assistant => "assistant".to_string(),
                    },
                    content: content_value,
                    reasoning_details: None,
                });
            }
        }

        clean
    }

    // -----------------------------------------------------------------------
    // Image cleaning
    // -----------------------------------------------------------------------

    /// Remove image blocks from messages if the model does not support images.
    ///
    /// This is a convenience wrapper around
    /// `roo_provider::transform::image_cleaning::maybe_remove_image_blocks`.
    ///
    /// Source: `src/api/transform/image-cleaning.ts` — `maybeRemoveImageBlocks`
    pub fn maybe_remove_image_blocks(
        messages: Vec<ApiMessage>,
        model_info: &ModelInfo,
    ) -> Vec<ApiMessage> {
        roo_provider::transform::image_cleaning::maybe_remove_image_blocks(messages, model_info)
    }

    // -----------------------------------------------------------------------
    // Message constructors
    // -----------------------------------------------------------------------

    /// Create a user message with text and optional images.
    ///
    /// Source: `src/core/task/Task.ts` — initial user message construction
    /// in `startTask` / `resumeTask`
    pub fn create_user_message(text: &str, images: &[String]) -> ApiMessage {
        let mut content: Vec<ContentBlock> = Vec::new();

        // Add text block
        if !text.is_empty() {
            content.push(ContentBlock::Text {
                text: text.to_string(),
            });
        }

        // Add image blocks
        for image_data in images {
            content.push(ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: guess_media_type(image_data),
                    data: image_data.clone(),
                },
            });
        }

        ApiMessage {
            role: MessageRole::User,
            content,
            reasoning: None,
            ts: Some(chrono::Utc::now().timestamp_millis() as f64 / 1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    /// Create an assistant message from parsed stream content.
    ///
    /// Converts the accumulated `ParsedStreamContent` into an `ApiMessage`
    /// with the appropriate content blocks (thinking, text, tool_use).
    ///
    /// Source: `src/core/task/Task.ts` — `presentAssistantMessage` saves the
    /// assistant response to `apiConversationHistory`
    pub fn create_assistant_message(parsed: &ParsedStreamContent) -> ApiMessage {
        let content = parsed.to_content_blocks();

        ApiMessage {
            role: MessageRole::Assistant,
            content,
            reasoning: None,
            ts: Some(chrono::Utc::now().timestamp_millis() as f64 / 1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    /// Create a tool result message.
    ///
    /// Converts a `ToolExecutionResult` into an `ApiMessage` with a
    /// `ToolResult` content block.
    ///
    /// Source: `src/core/task/Task.ts` — tool result is added to
    /// `apiConversationHistory` after each tool execution
    pub fn create_tool_result_message(
        tool_use_id: &str,
        result: &ToolExecutionResult,
    ) -> ApiMessage {
        let mut content: Vec<ToolResultContent> = vec![ToolResultContent::Text {
            text: result.text.clone(),
        }];

        // Add images if present
        if let Some(ref images) = result.images {
            for image_data in images {
                content.push(ToolResultContent::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/png".to_string(),
                        data: image_data.clone(),
                    },
                });
            }
        }

        let is_error = if result.is_error {
            Some(true)
        } else {
            None
        };

        ApiMessage {
            role: MessageRole::User, // Tool results are sent as "user" role
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content,
                is_error,
            }],
            reasoning: None,
            ts: Some(chrono::Utc::now().timestamp_millis() as f64 / 1000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }
}

// ---------------------------------------------------------------------------
// MCP Tool Helpers
// ---------------------------------------------------------------------------

/// Get MCP tools from all connected servers as tool definition JSON values.
///
/// Source: `src/core/task/build-tools.ts` — `getMcpServerTools()`
pub fn get_mcp_server_tools(mcp_servers: &[McpServerConnection]) -> Vec<Value> {
    let mut tools = Vec::new();

    for server in mcp_servers {
        for tool in &server.tools {
            // Skip tools that are not enabled for prompt
            if !tool.enabled_for_prompt {
                continue;
            }

            tools.push(json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description.clone().unwrap_or_default(),
                    "parameters": tool.input_schema.clone().unwrap_or(json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    })),
                }
            }));
        }
    }

    tools
}

// ---------------------------------------------------------------------------
// Content Conversion Helpers
// ---------------------------------------------------------------------------

/// Convert content blocks to a value suitable for the API.
///
/// For a single text block, returns a string.
/// For multiple blocks, returns an array.
/// For empty content, returns an empty string.
fn content_blocks_to_api_value(blocks: &[ContentBlock]) -> serde_json::Value {
    if blocks.is_empty() {
        return serde_json::Value::String(String::new());
    }

    if blocks.len() == 1 {
        if let ContentBlock::Text { ref text } = blocks[0] {
            return serde_json::Value::String(text.clone());
        }
    }

    // Multiple blocks or non-text single block: return as array
    serde_json::to_value(blocks).unwrap_or(serde_json::Value::Array(Vec::new()))
}

/// Convert content block references to a value suitable for the API.
fn content_refs_to_api_value(blocks: &[&ContentBlock]) -> serde_json::Value {
    if blocks.is_empty() {
        return serde_json::Value::String(String::new());
    }

    if blocks.len() == 1 {
        if let ContentBlock::Text { text } = blocks[0] {
            return serde_json::Value::String(text.clone());
        }
    }

    serde_json::to_value(blocks).unwrap_or(serde_json::Value::Array(Vec::new()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Guess the media type from base64 image data or file extension.
///
/// If the data starts with a known image signature, return that type.
/// Otherwise default to "image/png".
fn guess_media_type(data: &str) -> String {
    // Check for common base64-encoded image signatures
    // JPEG: starts with /9j/
    if data.starts_with("/9j/") {
        return "image/jpeg".to_string();
    }
    // PNG: starts with iVBOR
    if data.starts_with("iVBOR") {
        return "image/png".to_string();
    }
    // GIF: starts with R0lGOD
    if data.starts_with("R0lGOD") {
        return "image/gif".to_string();
    }
    // WebP: starts with UklGR
    if data.starts_with("UklGR") {
        return "image/webp".to_string();
    }
    // SVG: might start with PHN2Zw (base64 of <svg)
    if data.starts_with("PHN2Zw") {
        return "image/svg+xml".to_string();
    }

    // Default
    "image/png".to_string()
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new(String::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_builder_new() {
        let builder = MessageBuilder::new("You are a helpful assistant.");
        assert_eq!(builder.system_prompt(), "You are a helpful assistant.");
        assert!(!builder.supports_images);
    }

    #[test]
    fn test_message_builder_with_images_support() {
        let builder = MessageBuilder::new("test").with_images_support(true);
        assert!(builder.supports_images);
    }

    #[test]
    fn test_message_builder_default() {
        let builder = MessageBuilder::default();
        assert!(builder.system_prompt().is_empty());
    }

    #[test]
    fn test_build_tool_definitions() {
        let builder = MessageBuilder::new("test");

        // Default (no mode filter) returns code-mode-filtered tools
        let tools = builder.build_tool_definitions();

        // Code mode filters some tools; verify we get a reasonable set
        assert!(!tools.is_empty(), "Should have at least some tools");

        // Each tool should have the expected structure
        for tool in &tools {
            assert_eq!(tool["type"], "function");
            assert!(tool["function"]["name"].is_string());
            assert!(tool["function"]["description"].is_string());
            assert!(tool["function"]["parameters"].is_object());
        }

        // Check some known tools that should be in code mode
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"read_file"), "Code mode should have read_file");
        assert!(names.contains(&"write_to_file"), "Code mode should have write_to_file");
        assert!(names.contains(&"apply_diff"), "Code mode should have apply_diff");
    }

    #[test]
    fn test_create_user_message_text_only() {
        let msg = MessageBuilder::create_user_message("Hello, world!", &[]);

        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content.len(), 1);

        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected Text block"),
        }

        assert!(msg.ts.is_some());
    }

    #[test]
    fn test_create_user_message_with_images() {
        let images = vec!["iVBORw0KGgoAAAANSUhEUg==".to_string()]; // PNG-like
        let msg = MessageBuilder::create_user_message("See this image", &images);

        assert_eq!(msg.content.len(), 2); // text + image

        match &msg.content[1] {
            ContentBlock::Image { source } => match source {
                ImageSource::Base64 { media_type, data } => {
                    assert_eq!(media_type, "image/png");
                    assert_eq!(data, "iVBORw0KGgoAAAANSUhEUg==");
                }
                _ => panic!("Expected Base64 image source"),
            },
            _ => panic!("Expected Image block"),
        }
    }

    #[test]
    fn test_create_user_message_empty_text_with_images() {
        let images = vec!["/9j/4AAQSkZJRg==".to_string()]; // JPEG-like
        let msg = MessageBuilder::create_user_message("", &images);

        // No text block since text is empty, only image
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::Image { .. } => {}
            _ => panic!("Expected Image block"),
        }
    }

    #[test]
    fn test_create_assistant_message() {
        let mut parser = crate::stream_parser::StreamParser::new();
        parser.feed_chunk(&roo_types::api::ApiStreamChunk::Text {
            text: "I'll help you.".into(),
        });
        parser.feed_chunk(&roo_types::api::ApiStreamChunk::ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"main.rs"}"#.into(),
        });

        let content = parser.finalize();
        let msg = MessageBuilder::create_assistant_message(&content);

        assert_eq!(msg.role, MessageRole::Assistant);
        // to_content_blocks() produces: Text + ToolUse = 2 blocks
        assert_eq!(msg.content.len(), 2);
    }

    #[test]
    fn test_create_tool_result_message_success() {
        let result = ToolExecutionResult::success("File contents here");
        let msg = MessageBuilder::create_tool_result_message("call_1", &result);

        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content.len(), 1);

        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "call_1");
                assert!(is_error.is_none());
                assert_eq!(content.len(), 1);
                match &content[0] {
                    ToolResultContent::Text { text } => {
                        assert_eq!(text, "File contents here");
                    }
                    _ => panic!("Expected Text content"),
                }
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_create_tool_result_message_error() {
        let result = ToolExecutionResult::error("File not found");
        let msg = MessageBuilder::create_tool_result_message("call_2", &result);

        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "call_2");
                assert_eq!(*is_error, Some(true));
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_create_tool_result_message_with_images() {
        let result =
            ToolExecutionResult::success_with_images("screenshot", vec!["base64data".into()]);
        let msg = MessageBuilder::create_tool_result_message("call_3", &result);

        match &msg.content[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert_eq!(content.len(), 2); // text + image
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_build_api_messages_empty_history() {
        let builder = MessageBuilder::new("test");
        let messages = builder.build_api_messages(&[], None, &[]);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_build_api_messages_with_history() {
        let builder = MessageBuilder::new("test");
        let history = vec![
            MessageBuilder::create_user_message("Hello", &[]),
            MessageBuilder::create_assistant_message(&{
                let mut p = crate::stream_parser::StreamParser::new();
                p.feed_chunk(&roo_types::api::ApiStreamChunk::Text {
                    text: "Hi there".into(),
                });
                p.finalize()
            }),
        ];

        let messages = builder.build_api_messages(&history, None, &[]);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_build_api_messages_with_new_user_message() {
        let builder = MessageBuilder::new("test");
        let history = vec![MessageBuilder::create_user_message("Hello", &[])];
        let messages = builder.build_api_messages(&history, Some("Follow up"), &[]);

        assert_eq!(messages.len(), 2);
        match &messages[1].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Follow up"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_guess_media_type() {
        assert_eq!(guess_media_type("/9j/abc"), "image/jpeg");
        assert_eq!(guess_media_type("iVBORabc"), "image/png");
        assert_eq!(guess_media_type("R0lGODabc"), "image/gif");
        assert_eq!(guess_media_type("UklGRabc"), "image/webp");
        assert_eq!(guess_media_type("PHN2Zwabc"), "image/svg+xml");
        assert_eq!(guess_media_type("unknown"), "image/png");
    }

    #[test]
    fn test_build_clean_conversation_history_basic() {
        let messages = vec![
            MessageBuilder::create_user_message("Hello", &[]),
            MessageBuilder::create_assistant_message(&{
                let mut p = crate::stream_parser::StreamParser::new();
                p.feed_chunk(&roo_types::api::ApiStreamChunk::Text {
                    text: "Hi there".into(),
                });
                p.finalize()
            }),
        ];

        let clean = MessageBuilder::build_clean_conversation_history(&messages, false);
        assert_eq!(clean.len(), 2);

        // First should be user message
        match &clean[0] {
            CleanConversationItem::Message { role, .. } => {
                assert_eq!(role, "user");
            }
            _ => panic!("Expected Message item"),
        }

        // Second should be assistant message
        match &clean[1] {
            CleanConversationItem::Message { role, .. } => {
                assert_eq!(role, "assistant");
            }
            _ => panic!("Expected Message item"),
        }
    }

    #[test]
    fn test_build_clean_conversation_history_empty() {
        let clean = MessageBuilder::build_clean_conversation_history(&[], false);
        assert!(clean.is_empty());
    }

    #[test]
    fn test_maybe_remove_image_blocks_no_support() {
        let model_info = ModelInfo {
            supports_images: Some(false),
            ..Default::default()
        };

        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: vec![
                ContentBlock::Text {
                    text: "Look at this".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image.png".to_string(),
                    },
                },
            ],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];

        let result = MessageBuilder::maybe_remove_image_blocks(messages, &model_info);
        assert_eq!(result.len(), 1);
        // Image should be replaced with text
        assert_eq!(result[0].content.len(), 2);
        match &result[0].content[1] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[Referenced image in conversation]");
            }
            _ => panic!("Expected Text block replacing image"),
        }
    }

    #[test]
    fn test_get_mcp_server_tools_empty() {
        let tools = get_mcp_server_tools(&[]);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_build_tool_definitions_with_restrictions() {
        let builder = MessageBuilder::new("test");

        let result =
            builder.build_tool_definitions_with_restrictions(None, &[], None, None, false, &[]);

        assert!(!result.tools.is_empty());
        assert!(result.allowed_function_names.is_none());

        // With include_all flag
        let result_all =
            builder.build_tool_definitions_with_restrictions(None, &[], None, None, true, &[]);
        assert!(!result_all.tools.is_empty());
        assert!(result_all.allowed_function_names.is_some());
    }
}
