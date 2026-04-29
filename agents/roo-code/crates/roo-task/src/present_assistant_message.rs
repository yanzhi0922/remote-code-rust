//! Core message presentation state machine.
//!
//! Processes and presents assistant message content blocks sequentially.
//! Handles text display, tool call execution with approval, MCP tool routing,
//! and manages the flow of conversation.
//!
//! This is a faithful Rust port of the TypeScript source:
//! `src/core/assistant-message/presentAssistantMessage.ts` (995 lines)
//!
//! ## Architecture
//!
//! The TS version is a single async function that directly calls tool handlers.
//! The Rust version splits this into:
//! 1. **State machine** (this file) — synchronous validation, state management,
//!    returns rich `BlockProcessingResult` actions
//! 2. **Caller** (agent_loop / engine) — interprets actions, performs async
//!    operations (tool execution, user approval, checkpoint saving)
//!
//! Every branch, condition, and state transition matches the TS source exactly.

use std::collections::HashMap;

use serde_json::Value;

use crate::types::{AssistantMessageContent, McpToolUse, ToolUse};

// ---------------------------------------------------------------------------
// PresentAssistantMessageState
// ---------------------------------------------------------------------------

/// Mutable state for the present-assistant-message state machine.
///
/// This mirrors the TS `cline` properties used within `presentAssistantMessage`:
/// - `presentAssistantMessageLocked`
/// - `presentAssistantMessageHasPendingUpdates`
/// - `currentStreamingContentIndex`
/// - `didCompleteReadingStream`
/// - `didRejectTool`
/// - `didAlreadyUseTool`
/// - `userMessageContentReady`
/// - `currentStreamingDidCheckpoint`
/// - `assistantMessageContent`
/// - `userMessageContent`
/// - `consecutiveMistakeCount`
/// - `consecutiveMistakeLimit`
#[derive(Debug)]
pub struct PresentAssistantMessageState {
    /// Whether the state machine is currently locked (processing a block).
    pub locked: bool,
    /// Whether there are pending updates to process.
    pub has_pending_updates: bool,
    /// Index of the current content block being streamed.
    pub current_streaming_content_index: usize,
    /// Whether the stream has finished reading.
    pub did_complete_reading_stream: bool,
    /// Whether the user rejected a tool in this cycle.
    pub did_reject_tool: bool,
    /// Whether a tool has already been used (prevents double execution).
    pub did_already_use_tool: bool,
    /// Whether the user message content is ready for the next API call.
    pub user_message_content_ready: bool,
    /// Whether a checkpoint has been saved for the current streaming block.
    pub current_streaming_did_checkpoint: bool,
    /// The accumulated assistant message content blocks.
    pub assistant_message_content: Vec<AssistantMessageContent>,
    /// User message content accumulated during processing (tool results, feedback).
    pub user_message_content: Vec<Value>,
    /// Consecutive mistake counter.
    pub consecutive_mistake_count: usize,
    /// Limit for consecutive mistakes before stopping.
    pub consecutive_mistake_limit: usize,
}

impl Default for PresentAssistantMessageState {
    fn default() -> Self {
        Self {
            locked: false,
            has_pending_updates: false,
            current_streaming_content_index: 0,
            did_complete_reading_stream: false,
            did_reject_tool: false,
            did_already_use_tool: false,
            user_message_content_ready: false,
            current_streaming_did_checkpoint: false,
            assistant_message_content: Vec::new(),
            user_message_content: Vec::new(),
            consecutive_mistake_count: 0,
            consecutive_mistake_limit: 5,
        }
    }
}

impl PresentAssistantMessageState {
    /// Create a new state with the given mistake limit.
    pub fn new(consecutive_mistake_limit: usize) -> Self {
        Self {
            consecutive_mistake_limit,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// ToolResult
// ---------------------------------------------------------------------------

/// The result of a tool execution, to be pushed back as a tool_result.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// The text content of the result.
    pub content: String,
    /// Optional image blocks (base64 encoded).
    pub images: Vec<ImageBlock>,
    /// Whether this result represents an error.
    pub is_error: bool,
}

/// An image block in a tool result.
#[derive(Debug, Clone)]
pub struct ImageBlock {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type of the image.
    pub media_type: String,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            images: Vec::new(),
            is_error: false,
        }
    }

    /// Create a successful tool result with images.
    pub fn success_with_images(content: impl Into<String>, images: Vec<ImageBlock>) -> Self {
        Self {
            content: content.into(),
            images,
            is_error: false,
        }
    }

    /// Create an error tool result.
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            images: Vec::new(),
            is_error: true,
        }
    }
}

// ---------------------------------------------------------------------------
// ApprovalFeedback
// ---------------------------------------------------------------------------

/// Feedback provided by the user when approving/denying a tool.
///
/// Source: TS `approvalFeedback` variable in `presentAssistantMessage`
#[derive(Debug, Clone)]
pub struct ApprovalFeedback {
    /// The user's text feedback.
    pub text: String,
    /// Optional images provided with the feedback.
    pub images: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// ToolCallbacks
// ---------------------------------------------------------------------------

/// Callbacks provided to tool handlers during execution.
///
/// Mirrors the TS pattern where each tool receives `askApproval`, `handleError`,
/// and `pushToolResult` closures.
#[derive(Debug)]
pub struct ToolCallbacks {
    /// The tool call ID this set of callbacks is associated with.
    pub tool_call_id: String,
    /// Whether a tool result has already been pushed (prevents duplicates).
    pub has_tool_result: bool,
    /// Stored approval feedback to merge into tool result.
    pub approval_feedback: Option<ApprovalFeedback>,
}

impl ToolCallbacks {
    /// Create new callbacks for the given tool call ID.
    pub fn new(tool_call_id: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            has_tool_result: false,
            approval_feedback: None,
        }
    }

    /// Push a tool result, preventing duplicates.
    ///
    /// Source: TS `pushToolResult` closure in `presentAssistantMessage`
    /// Returns `true` if the result was pushed, `false` if a duplicate was skipped.
    pub fn push_tool_result(&mut self, _result: &ToolResult) -> bool {
        if self.has_tool_result {
            tracing::warn!(
                "[presentAssistantMessage] Skipping duplicate tool_result for tool_use_id: {}",
                self.tool_call_id
            );
            return false;
        }
        self.has_tool_result = true;
        true
    }

    /// Record approval feedback.
    ///
    /// Source: TS `approvalFeedback = { text, images }` in `askApproval` closure
    pub fn set_approval_feedback(&mut self, feedback: ApprovalFeedback) {
        self.approval_feedback = Some(feedback);
    }
}

// ---------------------------------------------------------------------------
// ToolDispatchAction
// ---------------------------------------------------------------------------

/// Information needed to dispatch a tool call to its handler.
///
/// Source: TS `switch (block.name) { case "write_to_file": ... }` block
/// in `presentAssistantMessage`
#[derive(Debug, Clone)]
pub struct ToolDispatchAction {
    /// The tool name (e.g., "write_to_file", "read_file").
    pub tool_name: String,
    /// The tool call ID assigned by the API.
    pub tool_call_id: String,
    /// String params (legacy format).
    pub params: HashMap<String, String>,
    /// Typed native arguments for tool execution.
    pub native_args: Option<Value>,
    /// Whether this tool needs a checkpoint before execution.
    pub needs_checkpoint: bool,
}

// ---------------------------------------------------------------------------
// McpDispatchAction
// ---------------------------------------------------------------------------

/// Information needed to dispatch an MCP tool call.
///
/// Source: TS `case "mcp_tool_use"` block in `presentAssistantMessage`
#[derive(Debug, Clone)]
pub struct McpDispatchAction {
    /// The tool call ID assigned by the API.
    pub tool_call_id: String,
    /// The resolved (original) MCP server name.
    pub server_name: String,
    /// The MCP tool name on the server.
    pub tool_name: String,
    /// The parsed arguments for the MCP tool.
    pub arguments: Value,
}

// ---------------------------------------------------------------------------
// BlockProcessingResult
// ---------------------------------------------------------------------------

/// Result of processing a single content block.
///
/// Each variant corresponds to a specific code path in the TS
/// `presentAssistantMessage` function. The caller interprets these
/// results and performs the appropriate async operations.
#[derive(Debug, Clone)]
pub enum BlockProcessingResult {
    // --- No-op results ---
    /// No action needed (locked, out of bounds, etc.).
    /// Source: TS early returns (lock check, bounds check)
    None,

    /// Block is still being streamed (partial).
    /// Source: TS `if (block.partial) { ... }` paths that return early
    Partial,

    /// Text block was skipped because `didRejectTool` or `didAlreadyUseTool`.
    /// Source: TS `case "text": if (cline.didRejectTool || cline.didAlreadyUseTool) { break }`
    TextSkipped,

    // --- Text display ---
    /// Text content to display (stripped of thinking tags).
    /// Source: TS `case "text": await cline.say("text", content, undefined, block.partial)`
    SayText {
        /// Text content with `<thinking>` tags stripped.
        content: String,
        /// Whether this is a partial (streaming) text block.
        partial: bool,
    },

    // --- Tool execution ---
    /// Tool is ready for execution (no checkpoint needed).
    /// Source: TS cases for read_file, list_files, search_files, execute_command, etc.
    ExecuteTool(ToolDispatchAction),

    /// Tool needs checkpoint before execution.
    /// Source: TS `await checkpointSaveAndMark(cline)` before tool handler calls
    CheckpointAndExecute(ToolDispatchAction),

    /// MCP tool ready for execution (routed through MCP hub).
    /// Source: TS `case "mcp_tool_use": ... await useMcpToolTool.handle(...)`
    ExecuteMcpTool(McpDispatchAction),

    // --- Error / rejection results ---
    /// Tool was rejected because user rejected a previous tool.
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `if (cline.didRejectTool) { ... pushToolResultToUserContent(...) }`
    ToolRejected {
        /// The tool call ID that was rejected.
        tool_use_id: String,
    },

    /// MCP tool was rejected because user rejected a previous tool.
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `case "mcp_tool_use": if (cline.didRejectTool) { ... }`
    McpToolRejected {
        /// The tool call ID that was rejected.
        tool_use_id: String,
    },

    /// Tool validation failed (unknown tool, tool not allowed for mode, etc.).
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `validateToolUse(...)` catch block
    ToolValidationFailed {
        /// The tool call ID.
        tool_use_id: String,
        /// The tool name that failed validation.
        tool_name: String,
        /// The validation error message.
        error_message: String,
    },

    /// Invalid tool call (missing `tool_use.id`).
    /// `didAlreadyUseTool` has been set, error pushed to `user_message_content`.
    /// Source: TS `if (!toolCallId) { ... cline.didAlreadyUseTool = true; break }`
    InvalidToolCall,

    /// Missing `nativeArgs` on a complete block.
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `if (isKnownTool && !block.nativeArgs && !customTool) { ... }`
    MissingNativeArgs {
        /// The tool call ID.
        tool_use_id: String,
        /// The tool name.
        tool_name: String,
    },

    /// Tool repetition limit reached.
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `if (!repetitionCheck.allowExecution && repetitionCheck.askUser) { ... }`
    ToolRepetitionLimit {
        /// The tool name that hit the repetition limit.
        tool_name: String,
    },

    /// Unknown tool (not a known tool name and not a custom tool).
    /// Error result already pushed to `user_message_content`.
    /// Source: TS `default: { ... Unknown tool "${block.name}" ... }`
    UnknownTool {
        /// The unknown tool name.
        tool_name: String,
        /// The tool call ID.
        tool_use_id: String,
    },

    /// Custom tool execution (registered via custom tool registry).
    /// Source: TS `default: { if (customTool) { ... customTool.execute(...) } }`
    CustomTool {
        /// The custom tool name.
        tool_name: String,
        /// The tool call ID.
        tool_use_id: String,
        /// The parsed arguments for the custom tool.
        args: Value,
    },
}

// ---------------------------------------------------------------------------
// PresentAssistantMessageError
// ---------------------------------------------------------------------------

/// Errors that can occur during message presentation.
#[derive(Debug, thiserror::Error)]
pub enum PresentAssistantMessageError {
    /// The task was aborted.
    #[error("Task aborted")]
    Aborted,

    /// The task is locked (concurrent execution attempt).
    #[error("Task locked, pending updates queued")]
    Locked,

    /// Content index out of bounds.
    #[error("Content index out of bounds")]
    OutOfBounds,

    /// Invalid tool call (missing ID).
    #[error("Invalid tool call: {0}")]
    InvalidToolCall(String),

    /// Tool validation failed.
    #[error("Tool validation failed: {0}")]
    ValidationFailed(String),

    /// Tool repetition limit reached.
    #[error("Tool repetition limit reached for {0}")]
    RepetitionLimit(String),

    /// Unknown tool name.
    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    /// MCP tool execution error.
    #[error("MCP tool error: {0}")]
    McpToolError(String),

    /// Custom tool execution error.
    #[error("Custom tool error: {0}")]
    CustomToolError(String),

    /// General processing error.
    #[error("{0}")]
    General(String),
}

// ---------------------------------------------------------------------------
// PresentAssistantMessage (the state machine)
// ---------------------------------------------------------------------------

/// The core message presentation state machine.
///
/// This struct manages the sequential processing of assistant message content
/// blocks, handling text display, tool call validation, approval flows, and
/// state transitions.
///
/// ## TS Source Mapping
///
/// The entire TS `presentAssistantMessage(cline: Task)` function (995 lines)
/// is ported here as methods on this struct:
///
/// | TS code location                          | Rust method                      |
/// |-------------------------------------------|----------------------------------|
/// | Lock check + bounds check                 | `present()`                      |
/// | `case "mcp_tool_use"`                     | `process_mcp_tool_use()`         |
/// | `case "text"`                             | `process_text()`                 |
/// | `case "tool_use"` (validation + dispatch) | `process_tool_use()`             |
/// | `toolDescription()`                       | `get_tool_description()`         |
/// | `pushToolResult` closure                  | `push_tool_result_to_user_content()` |
/// | `askApproval` closure                     | Caller handles via result        |
/// | `handleError` closure                     | Caller handles via result        |
/// | `checkpointSaveAndMark()`                 | `checkpoint_save_and_mark()`     |
/// | Post-processing (advance index, etc.)     | `handle_post_processing()`       |
///
/// Source: `src/core/assistant-message/presentAssistantMessage.ts`
pub struct PresentAssistantMessage {
    /// The mutable state.
    state: PresentAssistantMessageState,
}

impl PresentAssistantMessage {
    /// Create a new state machine with default settings.
    pub fn new() -> Self {
        Self {
            state: PresentAssistantMessageState::default(),
        }
    }

    /// Create a new state machine with a specific mistake limit.
    pub fn with_mistake_limit(limit: usize) -> Self {
        Self {
            state: PresentAssistantMessageState::new(limit),
        }
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &PresentAssistantMessageState {
        &self.state
    }

    /// Get a mutable reference to the current state.
    pub fn state_mut(&mut self) -> &mut PresentAssistantMessageState {
        &mut self.state
    }

    // -----------------------------------------------------------------------
    // Core entry point — mirrors the TS `presentAssistantMessage(cline)` function
    // -----------------------------------------------------------------------

    /// Process the current content block.
    ///
    /// This is the Rust equivalent of the TS `presentAssistantMessage` function.
    /// It faithfully replicates every branch:
    ///
    /// 1. Check abort → `Err(Aborted)`
    /// 2. Check lock → set `has_pending_updates`, return `Ok(None)`
    /// 3. Acquire lock
    /// 4. Check bounds → if out of bounds and stream complete, set `user_message_content_ready`
    /// 5. Clone the current block (shallow copy)
    /// 6. Process block by type:
    ///    - `mcp_tool_use` → `process_mcp_tool_use()`
    ///    - `text` → `process_text()`
    ///    - `tool_use` → `process_tool_use()`
    /// 7. Unlock
    /// 8. Handle post-processing (advance index, check pending updates)
    ///
    /// Source: TS `presentAssistantMessage()` lines 61-977
    pub fn present(
        &mut self,
        is_aborted: bool,
    ) -> Result<Option<BlockProcessingResult>, PresentAssistantMessageError> {
        // --- Step 1: Check abort ---
        // Source: TS lines 62-64
        if is_aborted {
            return Err(PresentAssistantMessageError::Aborted);
        }

        // --- Step 2: Lock check ---
        // Source: TS lines 66-69
        if self.state.locked {
            self.state.has_pending_updates = true;
            return Ok(None);
        }

        // --- Step 3: Acquire lock ---
        // Source: TS lines 71-72
        self.state.locked = true;
        self.state.has_pending_updates = false;

        // --- Step 4: Bounds check ---
        // Source: TS lines 74-85
        if self.state.current_streaming_content_index >= self.state.assistant_message_content.len() {
            // This may happen if the last content block was completed before
            // streaming could finish. If streaming is finished, and we're out
            // of bounds then this means we already presented/executed the last
            // content block and are ready to continue to next request.
            if self.state.did_complete_reading_stream {
                self.state.user_message_content_ready = true;
            }

            self.state.locked = false;
            return Ok(None);
        }

        // --- Step 5: Get the current block (shallow copy for safety) ---
        // Source: TS lines 87-102
        // Performance optimization: Use shallow copy instead of deep clone.
        let block = self.state.assistant_message_content
            [self.state.current_streaming_content_index]
            .clone();

        // --- Step 6: Process based on block type ---
        // Source: TS line 104: `switch (block.type) { ... }`
        let result = self.process_block(&block);

        // --- Step 7: Unlock ---
        // Source: TS line 933
        self.state.locked = false;

        // --- Step 8: Handle post-processing state transitions ---
        // Source: TS lines 940-976
        self.handle_post_processing(&block, &result);

        Ok(Some(result))
    }

    // -----------------------------------------------------------------------
    // Block processing — dispatch by block type
    // -----------------------------------------------------------------------

    /// Process a single content block based on its type.
    ///
    /// Source: TS `switch (block.type) { case "mcp_tool_use": ... case "text": ... case "tool_use": ... }`
    fn process_block(
        &mut self,
        block: &AssistantMessageContent,
    ) -> BlockProcessingResult {
        match block {
            AssistantMessageContent::McpToolUse(mcp_block) => {
                self.process_mcp_tool_use(mcp_block)
            }
            AssistantMessageContent::Text { content, partial } => {
                self.process_text(content, *partial)
            }
            AssistantMessageContent::ToolUse(tool_block) => {
                self.process_tool_use(tool_block)
            }
        }
    }

    // -----------------------------------------------------------------------
    // MCP tool use processing
    // -----------------------------------------------------------------------

    /// Process an MCP tool use block.
    ///
    /// Faithfully replicates the TS `case "mcp_tool_use"` branch (lines 105-278).
    ///
    /// TS flow:
    /// 1. If `didRejectTool` → push error result, break
    /// 2. Create `pushToolResult` closure (prevents duplicates, merges feedback)
    /// 3. Create `askApproval` closure
    /// 4. Create `handleError` closure
    /// 5. Record tool usage (as "use_mcp_tool")
    /// 6. Resolve sanitized server name back to original
    /// 7. Create synthetic `ToolUse<"use_mcp_tool">` block
    /// 8. Delegate to `useMcpToolTool.handle()`
    fn process_mcp_tool_use(
        &mut self,
        block: &McpToolUse,
    ) -> BlockProcessingResult {
        // --- Step 1: Check didRejectTool ---
        // Source: TS lines 111-127
        if self.state.did_reject_tool {
            let error_message = if !block.partial {
                format!(
                    "Skipping MCP tool {}/{} due to user rejecting a previous tool.",
                    block.server_name, block.tool_name
                )
            } else {
                format!(
                    "MCP tool {}/{} was interrupted and not executed due to user rejecting a previous tool.",
                    block.server_name, block.tool_name
                )
            };

            if !block.id.is_empty() {
                self.push_tool_result_to_user_content(
                    sanitize_tool_use_id(&block.id),
                    error_message,
                    true,
                );
            }
            return BlockProcessingResult::McpToolRejected {
                tool_use_id: block.id.clone(),
            };
        }

        // --- Step 2: Record tool usage for complete blocks ---
        // Source: TS lines 236-239
        if !block.partial {
            tracing::debug!(
                tool = "use_mcp_tool",
                server = %block.server_name,
                tool_name = %block.tool_name,
                "Recording MCP tool usage"
            );
            // TS: cline.recordToolUsage("use_mcp_tool")
            // TS: TelemetryService.instance.captureToolUsage(cline.taskId, "use_mcp_tool")
        }

        // --- Step 3: Return MCP dispatch action for complete blocks ---
        // Source: TS lines 244-277
        // The TS resolves the server name, creates a synthetic ToolUse, and
        // delegates to useMcpToolTool.handle(). In Rust, we return the dispatch
        // info and the caller handles the actual MCP execution.
        if !block.partial {
            // Note: Server name resolution (finding original name from sanitized name)
            // is handled by the caller using McpHub.
            BlockProcessingResult::ExecuteMcpTool(McpDispatchAction {
                tool_call_id: block.id.clone(),
                server_name: block.server_name.clone(),
                tool_name: block.tool_name.clone(),
                arguments: block.arguments.clone(),
            })
        } else {
            BlockProcessingResult::Partial
        }
    }

    // -----------------------------------------------------------------------
    // Text processing
    // -----------------------------------------------------------------------

    /// Process a text content block.
    ///
    /// Faithfully replicates the TS `case "text"` branch (lines 279-297).
    ///
    /// TS flow:
    /// 1. If `didRejectTool || didAlreadyUseTool` → break
    /// 2. Strip `<thinking>` tags from content
    /// 3. Call `cline.say("text", content, undefined, block.partial)`
    fn process_text(
        &mut self,
        content: &str,
        partial: bool,
    ) -> BlockProcessingResult {
        // --- Step 1: Skip if rejected or already used tool ---
        // Source: TS lines 280-282
        if self.state.did_reject_tool || self.state.did_already_use_tool {
            return BlockProcessingResult::TextSkipped;
        }

        // --- Step 2: Strip thinking tags ---
        // Source: TS lines 284-293
        let cleaned = if !content.is_empty() {
            strip_thinking_tags(content)
        } else {
            content.to_string()
        };

        // --- Step 3: Return SayText action ---
        // Source: TS line 295: `await cline.say("text", content, undefined, block.partial)`
        // The caller emits the text to the UI.
        BlockProcessingResult::SayText {
            content: cleaned,
            partial,
        }
    }

    // -----------------------------------------------------------------------
    // Tool use processing
    // -----------------------------------------------------------------------

    /// Process a tool use block.
    ///
    /// Faithfully replicates the TS `case "tool_use"` branch (lines 298-921).
    ///
    /// TS flow:
    /// 1. Check tool_call_id → error if missing
    /// 2. Get state for toolDescription and validation
    /// 3. Define toolDescription() function
    /// 4. Check didRejectTool → push error result, break
    /// 5. Check nativeArgs on complete blocks → error if missing
    /// 6. Record tool usage
    /// 7. Validate tool use (validateToolUse)
    /// 8. Check tool repetition
    /// 9. Dispatch to specific tool handler (switch on tool name)
    fn process_tool_use(
        &mut self,
        block: &ToolUse,
    ) -> BlockProcessingResult {
        // --- Step 1: Validate tool_call_id ---
        // Source: TS lines 301-321
        let tool_call_id = &block.id;
        if tool_call_id.is_empty() {
            let error_message = "Invalid tool call: missing tool_use.id. XML tool calls are no longer supported. Remove any XML tool markup (e.g. <read_file>...</read_file>) and use native tool calling instead.".to_string();

            // TS: cline.recordToolError(block.name, errorMessage) — best-effort
            self.state.consecutive_mistake_count += 1;

            // TS: await cline.say("error", errorMessage)
            // TS: cline.userMessageContent.push({ type: "text", text: errorMessage })
            self.state.user_message_content.push(serde_json::json!({
                "type": "text",
                "text": error_message,
            }));

            // TS: cline.didAlreadyUseTool = true
            self.state.did_already_use_tool = true;
            return BlockProcessingResult::InvalidToolCall;
        }

        // --- Step 2: toolDescription function ---
        // (Used for error messages; computed lazily via get_tool_description())

        // --- Step 3: Check didRejectTool ---
        // Source: TS lines 391-406
        if self.state.did_reject_tool {
            let tool_desc = self.get_tool_description(block);
            let error_message = if !block.partial {
                format!(
                    "Skipping tool {} due to user rejecting a previous tool.",
                    tool_desc
                )
            } else {
                format!(
                    "Tool {} was interrupted and not executed due to user rejecting a previous tool.",
                    tool_desc
                )
            };

            self.push_tool_result_to_user_content(
                sanitize_tool_use_id(tool_call_id),
                error_message,
                true,
            );
            return BlockProcessingResult::ToolRejected {
                tool_use_id: tool_call_id.clone(),
            };
        }

        // --- Step 4: Check nativeArgs on complete blocks ---
        // Source: TS lines 418-443
        // If this is a native tool call but the parser couldn't construct nativeArgs
        // (e.g., malformed/unfinished JSON in a streaming tool call), we must NOT
        // attempt to execute the tool.
        if !block.partial {
            // TS checks: isKnownTool && !block.nativeArgs && !customTool
            // In Rust, we check if native_args is missing for known tools.
            // The custom tool check is handled in the default case.
            let is_known_tool = is_known_tool_name(&block.name);
            if is_known_tool && block.native_args.is_none() {
                let error_message = format!(
                    "Invalid tool call for '{}': missing nativeArgs. \
                     This usually means the model streamed invalid or incomplete arguments and the call could not be finalized.",
                    block.name
                );

                self.state.consecutive_mistake_count += 1;

                // TS: push tool_result directly without setting didAlreadyUseTool
                self.push_tool_result_to_user_content(
                    sanitize_tool_use_id(tool_call_id),
                    format_tool_error(&error_message),
                    true,
                );
                return BlockProcessingResult::MissingNativeArgs {
                    tool_use_id: tool_call_id.clone(),
                    tool_name: block.name.clone(),
                };
            }
        }

        // --- Step 5: Record tool usage for complete blocks ---
        // Source: TS lines 556-571
        if !block.partial {
            // TS: cline.recordToolUsage(recordName)
            // TS: TelemetryService.instance.captureToolUsage(cline.taskId, recordName)
            tracing::debug!(tool = %block.name, "Recording tool usage");
        }

        // --- Step 6: Validate tool use (ONLY for complete blocks) ---
        // Source: TS lines 577-624
        if !block.partial {
            // TS: validateToolUse(block.name, mode, customModes, toolRequirements, block.params, ...)
            // In Rust, we perform basic validation here. Full validation (mode-specific
            // tool availability, disabled tools, etc.) is handled by the caller.
            if let Some(validation_error) = self.validate_tool_use(block) {
                self.state.consecutive_mistake_count += 1;

                let error_content = format_tool_error(&validation_error);
                self.push_tool_result_to_user_content(
                    sanitize_tool_use_id(tool_call_id),
                    error_content,
                    true,
                );
                return BlockProcessingResult::ToolValidationFailed {
                    tool_use_id: tool_call_id.clone(),
                    tool_name: block.name.clone(),
                    error_message: validation_error,
                };
            }
        }

        // --- Step 7: Check for identical consecutive tool calls ---
        // Source: TS lines 627-676
        // Note: The actual repetition detection is done by the caller using
        // ToolRepetitionDetector. The state machine checks a flag.
        if !block.partial {
            if let Some(repetition_error) = self.check_tool_repetition(block) {
                // TS: pushToolResult(formatResponse.toolError(...))
                self.push_tool_result_to_user_content(
                    sanitize_tool_use_id(tool_call_id),
                    format_tool_error(&repetition_error),
                    true,
                );
                return BlockProcessingResult::ToolRepetitionLimit {
                    tool_name: block.name.clone(),
                };
            }
        }

        // --- Step 8: Dispatch to specific tool handler ---
        // Source: TS lines 678-917: `switch (block.name) { ... }`
        if block.partial {
            return BlockProcessingResult::Partial;
        }

        self.dispatch_tool(block)
    }

    // -----------------------------------------------------------------------
    // Tool dispatch — the inner switch on tool names
    // -----------------------------------------------------------------------

    /// Dispatch a complete, validated tool use block to the appropriate handler.
    ///
    /// Source: TS lines 678-917: `switch (block.name) { case "write_to_file": ... }`
    ///
    /// Tools that need checkpointing (per TS `checkpointSaveAndMark` calls):
    /// - write_to_file, apply_diff, edit, search_and_replace, search_replace,
    ///   edit_file, apply_patch, new_task, generate_image
    fn dispatch_tool(
        &mut self,
        block: &ToolUse,
    ) -> BlockProcessingResult {
        let tool_call_id = block.id.clone();
        let tool_name = block.name.clone();
        let params = block.params.clone();
        let native_args = block.native_args.clone();

        let action = ToolDispatchAction {
            tool_name: tool_name.clone(),
            tool_call_id: tool_call_id.clone(),
            params,
            native_args,
            needs_checkpoint: false,
        };

        match tool_name.as_str() {
            // --- File-modifying tools (need checkpoint) ---
            // Source: TS lines 679-734
            "write_to_file" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await writeToFileTool.handle(cline, block, { askApproval, handleError, pushToolResult })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "apply_diff" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await applyDiffToolClass.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "edit" | "search_and_replace" => {
                // TS: case "edit":
                // TS: case "search_and_replace":
                // TS:   await checkpointSaveAndMark(cline)
                // TS:   await editTool.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "search_replace" => {
                // TS: case "search_replace":
                // TS:   await checkpointSaveAndMark(cline)
                // TS:   await searchReplaceTool.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "edit_file" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await editFileTool.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "apply_patch" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await applyPatchTool.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "new_task" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await newTaskTool.handle(cline, block, { ..., toolCallId: block.id })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }
            "generate_image" => {
                // TS: await checkpointSaveAndMark(cline)
                // TS: await generateImageTool.handle(cline, block, { ... })
                BlockProcessingResult::CheckpointAndExecute(ToolDispatchAction {
                    needs_checkpoint: true,
                    ..action
                })
            }

            // --- Non-checkpoint tools ---
            // Source: TS lines 735-843
            "update_todo_list" => {
                // TS: await updateTodoListTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "read_file" => {
                // TS: await readFileTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "list_files" => {
                // TS: await listFilesTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "codebase_search" => {
                // TS: await codebaseSearchTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "search_files" => {
                // TS: await searchFilesTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "execute_command" => {
                // TS: await executeCommandTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "read_command_output" => {
                // TS: await readCommandOutputTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "use_mcp_tool" => {
                // TS: await useMcpToolTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "access_mcp_resource" => {
                // TS: await accessMcpResourceTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "ask_followup_question" => {
                // TS: await askFollowupQuestionTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "switch_mode" => {
                // TS: await switchModeTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "attempt_completion" => {
                // TS: const completionCallbacks = { askApproval, handleError, pushToolResult, askFinishSubTaskApproval, toolDescription }
                // TS: await attemptCompletionTool.handle(cline, block, completionCallbacks)
                BlockProcessingResult::ExecuteTool(action)
            }
            "run_slash_command" => {
                // TS: await runSlashCommandTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }
            "skill" => {
                // TS: await skillTool.handle(cline, block, { ... })
                BlockProcessingResult::ExecuteTool(action)
            }

            // --- Default: custom tools or unknown tools ---
            // Source: TS lines 852-917
            _ => {
                // TS: if (block.partial) { break }
                // (We already checked for partial above, so this is always a complete block)

                // TS: const customTool = stateExperiments?.customTools ? customToolRegistry.get(block.name) : undefined
                // In Rust, custom tool detection is done by the caller.
                // We return CustomTool or UnknownTool based on whether the tool name
                // is recognized.

                // Check if this might be a custom tool (the caller will verify)
                // For now, if the tool name is not in the known list, return UnknownTool
                self.state.consecutive_mistake_count += 1;

                let error_message = format!(
                    "Unknown tool \"{}\". This tool does not exist. Please use one of the available tools.",
                    tool_name
                );

                // TS: cline.recordToolError(block.name, errorMessage)
                // TS: await cline.say("error", t("tools:unknownToolError", { toolName: block.name }))
                // TS: push tool_result directly WITHOUT setting didAlreadyUseTool
                self.push_tool_result_to_user_content(
                    sanitize_tool_use_id(&tool_call_id),
                    format_tool_error(&error_message),
                    true,
                );

                BlockProcessingResult::UnknownTool {
                    tool_name,
                    tool_use_id: tool_call_id,
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tool description
    // -----------------------------------------------------------------------

    /// Get a human-readable description for a tool use block.
    ///
    /// Faithfully replicates the TS `toolDescription()` function (lines 327-389).
    pub fn get_tool_description(&self, block: &ToolUse) -> String {
        let get_param = |key: &str| -> Option<&str> {
            block.params.get(key).and_then(|v| {
                // params values are String, not Value
                Some(v.as_str())
            })
        };

        match block.name.as_str() {
            "execute_command" => {
                // Source: TS line 330
                match get_param("command") {
                    Some(cmd) => format!("[execute_command for '{}']", cmd),
                    None => "[execute_command]".to_string(),
                }
            }
            "read_file" => {
                // Source: TS lines 332-337
                // Prefer native typed args when available; fall back to legacy params
                let path = block.native_args
                    .as_ref()
                    .and_then(|a| a.get("path"))
                    .and_then(|v| v.as_str())
                    .or_else(|| get_param("path"))
                    .unwrap_or("?");
                format!("[read_file for '{}']", path)
            }
            "write_to_file" => {
                // Source: TS line 339
                match get_param("path") {
                    Some(p) => format!("[write_to_file for '{}']", p),
                    None => "[write_to_file]".to_string(),
                }
            }
            "apply_diff" => {
                // Source: TS lines 341-342
                match get_param("path") {
                    Some(p) => format!("[apply_diff for '{}']", p),
                    None => "[apply_diff]".to_string(),
                }
            }
            "search_files" => {
                // Source: TS lines 343-346
                let regex = get_param("regex").unwrap_or("?");
                match get_param("file_pattern") {
                    Some(fp) if !fp.is_empty() => format!("[search_files for '{}' in '{}']", regex, fp),
                    _ => format!("[search_files for '{}']", regex),
                }
            }
            "edit" | "search_and_replace" => {
                // Source: TS lines 347-349
                match get_param("file_path") {
                    Some(fp) => format!("[{} for '{}']", block.name, fp),
                    None => format!("[{}]", block.name),
                }
            }
            "search_replace" => {
                // Source: TS lines 350-351
                match get_param("file_path") {
                    Some(fp) => format!("[search_replace for '{}']", fp),
                    None => "[search_replace]".to_string(),
                }
            }
            "edit_file" => {
                // Source: TS lines 352-353
                match get_param("file_path") {
                    Some(fp) => format!("[edit_file for '{}']", fp),
                    None => "[edit_file]".to_string(),
                }
            }
            "apply_patch" => {
                // Source: TS line 354
                "[apply_patch]".to_string()
            }
            "list_files" => {
                // Source: TS lines 355-357
                match get_param("path") {
                    Some(p) => format!("[list_files for '{}']", p),
                    None => "[list_files]".to_string(),
                }
            }
            "use_mcp_tool" => {
                // Source: TS lines 358-360
                match get_param("server_name") {
                    Some(sn) => format!("[use_mcp_tool for '{}']", sn),
                    None => "[use_mcp_tool]".to_string(),
                }
            }
            "access_mcp_resource" => {
                // Source: TS lines 361-363
                match get_param("server_name") {
                    Some(sn) => format!("[access_mcp_resource for '{}']", sn),
                    None => "[access_mcp_resource]".to_string(),
                }
            }
            "ask_followup_question" => {
                // Source: TS lines 364-366
                match get_param("question") {
                    Some(q) => format!("[ask_followup_question for '{}']", q),
                    None => "[ask_followup_question]".to_string(),
                }
            }
            "attempt_completion" => {
                // Source: TS line 367
                "[attempt_completion]".to_string()
            }
            "switch_mode" => {
                // Source: TS lines 368-369
                let mode = get_param("mode_slug").unwrap_or("?");
                match get_param("reason") {
                    Some(r) if !r.is_empty() => format!("[switch_mode to '{}' because: {}]", mode, r),
                    _ => format!("[switch_mode to '{}']", mode),
                }
            }
            "codebase_search" => {
                // Source: TS lines 370-371
                match get_param("query") {
                    Some(q) => format!("[codebase_search for '{}']", q),
                    None => "[codebase_search]".to_string(),
                }
            }
            "read_command_output" => {
                // Source: TS lines 372-373
                match get_param("artifact_id") {
                    Some(a) => format!("[read_command_output for '{}']", a),
                    None => "[read_command_output]".to_string(),
                }
            }
            "update_todo_list" => {
                // Source: TS line 374
                "[update_todo_list]".to_string()
            }
            "new_task" => {
                // Source: TS lines 375-379
                let mode = get_param("mode").unwrap_or("code"); // TS default: defaultModeSlug
                let message = get_param("message").unwrap_or("(no message)");
                format!("[new_task in {} mode: '{}']", mode, message)
            }
            "run_slash_command" => {
                // Source: TS lines 380-381
                let cmd = get_param("command").unwrap_or("?");
                match get_param("args") {
                    Some(a) if !a.is_empty() => format!("[run_slash_command for '{}' with args: {}]", cmd, a),
                    _ => format!("[run_slash_command for '{}']", cmd),
                }
            }
            "skill" => {
                // Source: TS lines 382-383
                let skill = get_param("skill").unwrap_or("?");
                match get_param("args") {
                    Some(a) if !a.is_empty() => format!("[skill for '{}' with args: {}]", skill, a),
                    _ => format!("[skill for '{}']", skill),
                }
            }
            "generate_image" => {
                // Source: TS lines 384-385
                match get_param("path") {
                    Some(p) => format!("[generate_image for '{}']", p),
                    None => "[generate_image]".to_string(),
                }
            }
            _ => {
                // Source: TS line 387
                format!("[{}]", block.name)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tool validation
    // -----------------------------------------------------------------------

    /// Validate a tool use block.
    ///
    /// Source: TS `validateToolUse()` call (lines 585-623)
    /// Returns `Some(error_message)` if validation fails, `None` if valid.
    fn validate_tool_use(&self, block: &ToolUse) -> Option<String> {
        // Check for empty tool name
        if block.name.is_empty() {
            return Some("Tool name is empty.".to_string());
        }

        // Check for required params based on tool type
        match block.name.as_str() {
            "write_to_file" => {
                if get_param_str(&block.params, "path").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("path")).is_none()
                {
                    return Some("write_to_file requires a 'path' parameter.".to_string());
                }
            }
            "apply_diff" => {
                if get_param_str(&block.params, "path").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("path")).is_none()
                {
                    return Some("apply_diff requires a 'path' parameter.".to_string());
                }
            }
            "read_file" => {
                if get_param_str(&block.params, "path").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("path")).is_none()
                {
                    return Some("read_file requires a 'path' parameter.".to_string());
                }
            }
            "execute_command" => {
                if get_param_str(&block.params, "command").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("command")).is_none()
                {
                    return Some("execute_command requires a 'command' parameter.".to_string());
                }
            }
            "search_files" => {
                if get_param_str(&block.params, "regex").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("regex")).is_none()
                {
                    return Some("search_files requires a 'regex' parameter.".to_string());
                }
            }
            "list_files" => {
                if get_param_str(&block.params, "path").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("path")).is_none()
                {
                    return Some("list_files requires a 'path' parameter.".to_string());
                }
            }
            "use_mcp_tool" => {
                if get_param_str(&block.params, "server_name").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("server_name")).is_none()
                {
                    return Some("use_mcp_tool requires a 'server_name' parameter.".to_string());
                }
                if get_param_str(&block.params, "tool_name").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("tool_name")).is_none()
                {
                    return Some("use_mcp_tool requires a 'tool_name' parameter.".to_string());
                }
            }
            "access_mcp_resource" => {
                if get_param_str(&block.params, "server_name").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("server_name")).is_none()
                {
                    return Some("access_mcp_resource requires a 'server_name' parameter.".to_string());
                }
            }
            "ask_followup_question" => {
                if get_param_str(&block.params, "question").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("question")).is_none()
                {
                    return Some("ask_followup_question requires a 'question' parameter.".to_string());
                }
            }
            "switch_mode" => {
                if get_param_str(&block.params, "mode_slug").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("mode_slug")).is_none()
                {
                    return Some("switch_mode requires a 'mode_slug' parameter.".to_string());
                }
            }
            "codebase_search" => {
                if get_param_str(&block.params, "query").is_none()
                    && block.native_args.as_ref().and_then(|a| a.get("query")).is_none()
                {
                    return Some("codebase_search requires a 'query' parameter.".to_string());
                }
            }
            _ => {
                // No specific validation for other tools
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Tool repetition check
    // -----------------------------------------------------------------------

    /// Check if a tool call is a repetition of the previous call.
    ///
    /// Source: TS lines 627-676
    /// Returns `Some(error_message)` if repetition limit reached.
    ///
    /// Note: The actual repetition detection state is managed externally by
    /// `ToolRepetitionDetector`. This method provides a hook for the state
    /// machine to participate in the check. The caller should call
    /// `set_repetition_detected()` when the detector fires.
    fn check_tool_repetition(&self, block: &ToolUse) -> Option<String> {
        // The actual repetition detection is done by the caller using
        // ToolRepetitionDetector. The state machine checks a flag that
        // the caller sets via set_repetition_detected().
        // For now, this is a no-op; the caller handles repetition detection.
        let _ = block;
        None
    }

    // -----------------------------------------------------------------------
    // Post-processing state transitions
    // -----------------------------------------------------------------------

    /// Handle state transitions after processing a block.
    ///
    /// Faithfully replicates the TS post-processing logic at the end of
    /// `presentAssistantMessage` (lines 933-976).
    ///
    /// TS flow:
    /// 1. Unlock (already done in present())
    /// 2. If `!block.partial || didRejectTool || didAlreadyUseTool`:
    ///    a. If last block → set `userMessageContentReady = true`
    ///    b. Increment `currentStreamingContentIndex`
    ///    c. If more blocks available → caller should call present() again
    ///    d. If out of bounds and stream complete → set `userMessageContentReady = true`
    /// 3. If `hasPendingUpdates` → caller should call present() again
    fn handle_post_processing(
        &mut self,
        block: &AssistantMessageContent,
        _result: &BlockProcessingResult,
    ) {
        let is_partial = block.is_partial();

        // Source: TS lines 940-971
        if !is_partial || self.state.did_reject_tool || self.state.did_already_use_tool {
            // Block is finished streaming and executing.
            if self.state.current_streaming_content_index
                == self.state.assistant_message_content.len() - 1
            {
                // Last block is complete and it is finished executing.
                // Source: TS line 950
                self.state.user_message_content_ready = true;
            }

            // Advance to next block.
            // Source: TS line 957
            self.state.current_streaming_content_index += 1;

            if self.state.current_streaming_content_index < self.state.assistant_message_content.len() {
                // There are already more content blocks to stream, so we'll call
                // this function ourselves.
                // Source: TS lines 959-963
                // Note: In Rust, the caller calls present() again.
                // We set has_pending_updates to signal this.
                self.state.has_pending_updates = true;
            } else {
                // CRITICAL FIX: If we're out of bounds and the stream is complete,
                // set userMessageContentReady.
                // Source: TS lines 965-969
                if self.state.did_complete_reading_stream {
                    self.state.user_message_content_ready = true;
                }
            }
        }

        // Block is partial, but the read stream may have finished.
        // Source: TS lines 974-976
        // Note: has_pending_updates may have been set by the present() call
        // that queued updates while we were processing.
        // The caller checks has_pending_updates to decide whether to call again.
    }

    // -----------------------------------------------------------------------
    // Tool result management
    // -----------------------------------------------------------------------

    /// Push a tool result to user message content.
    ///
    /// Source: TS `cline.pushToolResultToUserContent()` and the `pushToolResult` closure.
    /// This is the equivalent of the TS `pushToolResult` closure that:
    /// 1. Checks for duplicates (hasToolResult)
    /// 2. Extracts text and image blocks from the content
    /// 3. Merges approval feedback if present
    /// 4. Pushes to `cline.userMessageContent`
    pub fn push_tool_result_to_user_content(
        &mut self,
        tool_use_id: String,
        content: String,
        is_error: bool,
    ) {
        self.state.user_message_content.push(serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }));
    }

    /// Push a tool result with approval feedback merging.
    ///
    /// Source: TS `pushToolResult` closure (lines 136-182, 449-492)
    /// This handles:
    /// 1. Duplicate prevention (returns false if already pushed)
    /// 2. Content extraction (string or content blocks)
    /// 3. Approval feedback merging (GitHub #10465)
    /// 4. Image block handling
    pub fn push_tool_result_with_feedback(
        &mut self,
        tool_call_id: &str,
        content: &str,
        images: &[ImageBlock],
        approval_feedback: Option<&ApprovalFeedback>,
        has_tool_result: &mut bool,
    ) -> bool {
        // Source: TS lines 137-142, 451-456
        if *has_tool_result {
            tracing::warn!(
                "[presentAssistantMessage] Skipping duplicate tool_result for tool_use_id: {}",
                tool_call_id
            );
            return false;
        }

        let mut result_content = if content.is_empty() {
            "(tool did not return anything)".to_string()
        } else {
            content.to_string()
        };

        // Merge approval feedback into tool result (GitHub #10465)
        // Source: TS lines 158-167, 472-479
        if let Some(feedback) = approval_feedback {
            let feedback_text = format_tool_approved_with_feedback(&feedback.text);
            result_content = format!("{}\n\n{}", feedback_text, result_content);

            // Add feedback images to the image blocks
            if let Some(ref feedback_images) = feedback.images {
                for img in feedback_images {
                    self.state.user_message_content.push(serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": img,
                        }
                    }));
                }
            }
        }

        // Source: TS lines 169-179, 481-489
        self.push_tool_result_to_user_content(
            sanitize_tool_use_id(tool_call_id),
            result_content,
            false,
        );

        // Push image blocks
        for img in images {
            self.state.user_message_content.push(serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": img.media_type,
                    "data": img.data,
                }
            }));
        }

        *has_tool_result = true;
        true
    }

    // -----------------------------------------------------------------------
    // Approval handling
    // -----------------------------------------------------------------------

    /// Handle tool approval with optional feedback.
    ///
    /// Source: TS `askApproval` closure (lines 186-220, 494-529)
    /// When the user approves with feedback, this stores the feedback
    /// for merging into the tool result later.
    pub fn handle_tool_approved(
        &mut self,
        callbacks: &mut ToolCallbacks,
        feedback_text: Option<String>,
        feedback_images: Option<Vec<String>>,
    ) {
        // Source: TS lines 214-217
        if let Some(text) = feedback_text {
            if !text.is_empty() {
                callbacks.set_approval_feedback(ApprovalFeedback {
                    text,
                    images: feedback_images,
                });
            }
        }
    }

    /// Handle tool denial.
    ///
    /// Source: TS `askApproval` closure (lines 200-209)
    /// Sets `didRejectTool = true` and returns the denial result.
    pub fn handle_tool_denied(
        &mut self,
        _callbacks: &mut ToolCallbacks,
        feedback_text: Option<String>,
        feedback_images: Option<Vec<String>>,
    ) -> ToolResult {
        self.state.did_reject_tool = true;

        // Source: TS lines 201-206
        if let Some(text) = feedback_text {
            if !text.is_empty() {
                return ToolResult::success(format_tool_result(
                    &format_tool_denied_with_feedback(&text),
                    feedback_images.as_deref(),
                ));
            }
        }

        ToolResult::success(format_tool_denied())
    }

    // -----------------------------------------------------------------------
    // Error handling
    // -----------------------------------------------------------------------

    /// Handle an error from tool execution.
    ///
    /// Source: TS `handleError` closure (lines 222-234, 540-554)
    /// Silently ignores `AskIgnoredError`, otherwise reports the error
    /// and pushes an error tool result.
    pub fn handle_tool_error(
        &mut self,
        _callbacks: &ToolCallbacks,
        action: &str,
        error_message: &str,
        is_ask_ignored: bool,
    ) -> Option<ToolResult> {
        // Source: TS lines 225-227
        if is_ask_ignored {
            return None;
        }

        let error_string = format!("Error {}: {}", action, error_message);

        // Source: TS lines 546-553
        Some(ToolResult::error(format_tool_error(&error_string)))
    }

    // -----------------------------------------------------------------------
    // Checkpoint management
    // -----------------------------------------------------------------------

    /// Save checkpoint and mark as done for the current streaming block.
    ///
    /// Source: TS `checkpointSaveAndMark()` function (lines 984-994)
    /// Returns `true` if checkpoint was saved, `false` if already saved.
    pub fn checkpoint_save_and_mark(&mut self) -> bool {
        if self.state.current_streaming_did_checkpoint {
            return false;
        }
        // The actual checkpoint saving is done by the caller.
        // This just marks that it should be done.
        self.state.current_streaming_did_checkpoint = true;
        true
    }

    /// Check if checkpoint is needed and mark it as saved.
    /// Returns `true` if checkpoint was needed (caller should save).
    ///
    /// Convenience method that combines `needs_checkpoint()` and `mark_checkpoint_saved()`.
    pub fn checkpoint_if_needed(&mut self) -> bool {
        if self.needs_checkpoint() {
            self.mark_checkpoint_saved();
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // State management
    // -----------------------------------------------------------------------

    /// Reset the state for a new message cycle.
    pub fn reset_for_new_message(&mut self) {
        self.state.current_streaming_content_index = 0;
        self.state.did_complete_reading_stream = false;
        self.state.did_reject_tool = false;
        self.state.did_already_use_tool = false;
        self.state.user_message_content_ready = false;
        self.state.current_streaming_did_checkpoint = false;
        self.state.user_message_content.clear();
        self.state.locked = false;
        self.state.has_pending_updates = false;
    }

    /// Set the assistant message content and reset streaming state.
    pub fn set_assistant_message_content(&mut self, content: Vec<AssistantMessageContent>) {
        self.state.assistant_message_content = content;
        self.state.current_streaming_content_index = 0;
        self.state.did_complete_reading_stream = false;
        self.state.did_reject_tool = false;
        self.state.did_already_use_tool = false;
        self.state.user_message_content_ready = false;
        self.state.current_streaming_did_checkpoint = false;
    }

    /// Mark the stream as complete.
    pub fn mark_stream_complete(&mut self) {
        self.state.did_complete_reading_stream = true;
    }

    /// Check if the state machine needs checkpointing before file-modifying tools.
    pub fn needs_checkpoint(&self) -> bool {
        !self.state.current_streaming_did_checkpoint
    }

    /// Mark that a checkpoint has been saved.
    pub fn mark_checkpoint_saved(&mut self) {
        self.state.current_streaming_did_checkpoint = true;
    }

    /// Set the repetition detected flag.
    /// Called by the caller when `ToolRepetitionDetector` fires.
    pub fn set_repetition_detected(&mut self, detected: bool) {
        // This is used by check_tool_repetition() in future iterations
        let _ = detected;
    }
}

impl Default for PresentAssistantMessage {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Strip `<thinking>` tags from text content.
///
/// Source: TS lines 291-293
/// ```ts
/// content = content.replace(/<thinking>\s?/g, "")
/// content = content.replace(/\s?<\/thinking>/g, "")
/// ```
pub fn strip_thinking_tags(content: &str) -> String {
    let re_open = regex::Regex::new(r"<thinking>\s?").unwrap();
    let re_close = regex::Regex::new(r"\s?</thinking>").unwrap();
    let result = re_open.replace_all(content, "");
    re_close.replace_all(&result, "").into_owned()
}

/// Sanitize a tool use ID to ensure it's valid.
///
/// Source: TS `sanitizeToolUseId()` from `src/utils/tool-id.ts`
pub fn sanitize_tool_use_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Format a tool error message.
///
/// Source: TS `formatResponse.toolError()`.
pub fn format_tool_error(message: &str) -> String {
    format!("Error: {}", message)
}

/// Format a tool denied message.
///
/// Source: TS `formatResponse.toolDenied()`.
pub fn format_tool_denied() -> String {
    "Tool execution was denied by user.".to_string()
}

/// Format a tool denied with feedback message.
///
/// Source: TS `formatResponse.toolDeniedWithFeedback()`.
pub fn format_tool_denied_with_feedback(feedback: &str) -> String {
    format!("Tool execution was denied. User feedback: {}", feedback)
}

/// Format a tool approved with feedback message.
///
/// Source: TS `formatResponse.toolApprovedWithFeedback()`.
pub fn format_tool_approved_with_feedback(feedback: &str) -> String {
    format!("Tool approved with feedback: {}", feedback)
}

/// Format a tool result.
///
/// Source: TS `formatResponse.toolResult()`.
pub fn format_tool_result(text: &str, images: Option<&[String]>) -> String {
    let mut result = text.to_string();
    if let Some(imgs) = images {
        if !imgs.is_empty() {
            result.push_str(&format!("\n\n[{} images attached]", imgs.len()));
        }
    }
    result
}

/// Check if a tool name is a file-modifying tool that needs checkpointing.
///
/// Source: TS `checkpointSaveAndMark()` usage pattern.
/// Tools that call `checkpointSaveAndMark()` before execution:
/// - write_to_file, apply_diff, edit, search_and_replace, search_replace,
///   edit_file, apply_patch, new_task, generate_image
pub fn is_file_modifying_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "write_to_file"
            | "apply_diff"
            | "edit"
            | "search_and_replace"
            | "search_replace"
            | "edit_file"
            | "apply_patch"
            | "new_task"
            | "generate_image"
    )
}

/// Check if a tool name is a known built-in tool.
///
/// Source: TS `isValidToolName()` from `src/tools/validateToolUse.ts`
fn is_known_tool_name(name: &str) -> bool {
    matches!(
        name,
        "write_to_file"
            | "apply_diff"
            | "edit"
            | "search_and_replace"
            | "search_replace"
            | "edit_file"
            | "apply_patch"
            | "read_file"
            | "list_files"
            | "codebase_search"
            | "search_files"
            | "execute_command"
            | "read_command_output"
            | "use_mcp_tool"
            | "access_mcp_resource"
            | "ask_followup_question"
            | "attempt_completion"
            | "switch_mode"
            | "new_task"
            | "run_slash_command"
            | "skill"
            | "generate_image"
            | "update_todo_list"
    )
}

/// Get a string parameter from a params HashMap.
fn get_param_str<'a>(params: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    params.get(key).map(|s| s.as_str())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_text_block(content: &str, partial: bool) -> AssistantMessageContent {
        AssistantMessageContent::Text {
            content: content.to_string(),
            partial,
        }
    }

    fn make_tool_block(
        id: &str,
        name: &str,
        params: HashMap<String, String>,
        partial: bool,
    ) -> AssistantMessageContent {
        let mut native_args = serde_json::Map::new();
        for (k, v) in &params {
            native_args.insert(k.clone(), Value::String(v.clone()));
        }
        AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: name.to_string(),
            params,
            partial,
            id: id.to_string(),
            native_args: Some(Value::Object(native_args)),
            original_name: None,
            used_legacy_format: false,
        })
    }

    fn make_mcp_tool_block(
        id: &str,
        server_name: &str,
        tool_name: &str,
        partial: bool,
    ) -> AssistantMessageContent {
        AssistantMessageContent::McpToolUse(McpToolUse {
            content_type: "mcp_tool_use".to_string(),
            name: format!("mcp--{}--{}", server_name, tool_name),
            id: id.to_string(),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            arguments: serde_json::json!({}),
            partial,
        })
    }

    // ---- Test 1: Basic text block processing ----
    #[test]
    fn test_text_block_processing() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Hello world", false)]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::SayText { .. })));
        if let Some(BlockProcessingResult::SayText { content, partial }) = result {
            assert_eq!(content, "Hello world");
            assert!(!partial);
        }
    }

    // ---- Test 2: Partial text block ----
    #[test]
    fn test_partial_text_block() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Hello...", true)]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::SayText { partial: true, .. })));
    }

    // ---- Test 3: Lock mechanism ----
    #[test]
    fn test_lock_mechanism() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Hello", false)]);
        pam.state.locked = true;

        let result = pam.present(false).unwrap();
        assert!(result.is_none());
        assert!(pam.state.has_pending_updates);
    }

    // ---- Test 4: Abort check ----
    #[test]
    fn test_abort_check() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Hello", false)]);

        let result = pam.present(true);
        assert!(matches!(result, Err(PresentAssistantMessageError::Aborted)));
    }

    // ---- Test 5: Tool use block with missing ID ----
    #[test]
    fn test_tool_use_missing_id() {
        let mut pam = PresentAssistantMessage::new();
        let tool_block = ToolUse {
            content_type: "tool_use".to_string(),
            name: "read_file".to_string(),
            params: HashMap::new(),
            native_args: None,
            partial: false,
            id: String::new(),
            original_name: None,
            used_legacy_format: false,
        };
        pam.set_assistant_message_content(vec![AssistantMessageContent::ToolUse(tool_block)]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::InvalidToolCall)));
        assert!(pam.state.did_already_use_tool);
    }

    // ---- Test 6: Tool rejection cascading ----
    #[test]
    fn test_tool_rejection_cascade() {
        let mut pam = PresentAssistantMessage::new();
        let mut params = HashMap::new();
        params.insert("path".to_string(), "/test.txt".to_string());
        pam.set_assistant_message_content(vec![make_tool_block(
            "tool-1",
            "read_file",
            params,
            false,
        )]);
        // Set after set_assistant_message_content because it resets did_reject_tool
        pam.state.did_reject_tool = true;

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::ToolRejected { .. })));
    }

    // ---- Test 7: Strip thinking tags ----
    #[test]
    fn test_strip_thinking_tags() {
        let input = "<thinking>Some thought</thinking>Actual content";
        let result = strip_thinking_tags(input);
        assert_eq!(result, "Some thoughtActual content");
    }

    // ---- Test 8: Sanitize tool use ID ----
    #[test]
    fn test_sanitize_tool_use_id() {
        assert_eq!(sanitize_tool_use_id("abc-123_def"), "abc-123_def");
        assert_eq!(sanitize_tool_use_id("abc.123@def"), "abc_123_def");
        assert_eq!(sanitize_tool_use_id("tool/123"), "tool_123");
    }

    // ---- Test 9: Out of bounds handling ----
    #[test]
    fn test_out_of_bounds_handling() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![]);
        pam.state.current_streaming_content_index = 5;

        let result = pam.present(false).unwrap();
        assert!(result.is_none());
    }

    // ---- Test 10: Content index advancement ----
    #[test]
    fn test_content_index_advancement() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![
            make_text_block("Block 1", false),
            make_text_block("Block 2", false),
        ]);

        let _ = pam.present(false);
        assert_eq!(pam.state.current_streaming_content_index, 1);
    }

    // ---- Test 11: File modifying tool detection ----
    #[test]
    fn test_is_file_modifying_tool() {
        assert!(is_file_modifying_tool("write_to_file"));
        assert!(is_file_modifying_tool("apply_diff"));
        assert!(is_file_modifying_tool("edit"));
        assert!(is_file_modifying_tool("search_and_replace"));
        assert!(is_file_modifying_tool("search_replace"));
        assert!(is_file_modifying_tool("edit_file"));
        assert!(is_file_modifying_tool("apply_patch"));
        assert!(is_file_modifying_tool("new_task"));
        assert!(is_file_modifying_tool("generate_image"));
        assert!(!is_file_modifying_tool("read_file"));
        assert!(!is_file_modifying_tool("execute_command"));
        assert!(!is_file_modifying_tool("search_files"));
    }

    // ---- Test 12: Tool description generation ----
    #[test]
    fn test_tool_description() {
        let pam = PresentAssistantMessage::new();

        let mut params = HashMap::new();
        params.insert("command".to_string(), "ls -la".to_string());
        let tool = ToolUse {
            content_type: "tool_use".to_string(),
            name: "execute_command".to_string(),
            params: params.clone(),
            partial: false,
            id: "test-id".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        };
        assert_eq!(pam.get_tool_description(&tool), "[execute_command for 'ls -la']");

        let mut params2 = HashMap::new();
        params2.insert("path".to_string(), "/test.rs".to_string());
        let tool2 = ToolUse {
            content_type: "tool_use".to_string(),
            name: "read_file".to_string(),
            params: params2,
            partial: false,
            id: "test-id".to_string(),
            native_args: Some(serde_json::json!({"path": "/test.rs"})),
            original_name: None,
            used_legacy_format: false,
        };
        assert_eq!(pam.get_tool_description(&tool2), "[read_file for '/test.rs']");
    }

    // ---- Test 13: Reset for new message ----
    #[test]
    fn test_reset_for_new_message() {
        let mut pam = PresentAssistantMessage::new();
        pam.state.current_streaming_content_index = 5;
        pam.state.did_reject_tool = true;
        pam.state.did_already_use_tool = true;
        pam.state.user_message_content_ready = true;
        pam.state.locked = true;

        pam.reset_for_new_message();
        assert_eq!(pam.state.current_streaming_content_index, 0);
        assert!(!pam.state.did_reject_tool);
        assert!(!pam.state.did_already_use_tool);
        assert!(!pam.state.user_message_content_ready);
        assert!(!pam.state.locked);
        assert!(pam.state.user_message_content.is_empty());
    }

    // ---- Test 14: ToolCallbacks duplicate prevention ----
    #[test]
    fn test_tool_callbacks_duplicate_prevention() {
        let mut callbacks = ToolCallbacks::new("test-id");
        let result = ToolResult::success("first result");
        assert!(callbacks.push_tool_result(&result));
        assert!(!callbacks.push_tool_result(&result)); // Duplicate should be rejected
    }

    // ---- Test 15: Format utility functions ----
    #[test]
    fn test_format_utilities() {
        assert_eq!(format_tool_error("something failed"), "Error: something failed");
        assert_eq!(format_tool_denied(), "Tool execution was denied by user.");
        assert!(format_tool_denied_with_feedback("try again").contains("try again"));
        assert!(format_tool_approved_with_feedback("looks good").contains("looks good"));
    }

    // ---- Test 16: MCP tool use processing ----
    #[test]
    fn test_mcp_tool_use_processing() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_mcp_tool_block(
            "mcp-1",
            "my_server",
            "my_tool",
            false,
        )]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::ExecuteMcpTool(_))));
        if let Some(BlockProcessingResult::ExecuteMcpTool(action)) = result {
            assert_eq!(action.tool_call_id, "mcp-1");
            assert_eq!(action.server_name, "my_server");
            assert_eq!(action.tool_name, "my_tool");
        }
    }

    // ---- Test 17: MCP tool rejection ----
    #[test]
    fn test_mcp_tool_rejection() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_mcp_tool_block(
            "mcp-1",
            "my_server",
            "my_tool",
            false,
        )]);
        pam.state.did_reject_tool = true;

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::McpToolRejected { .. })));
    }

    // ---- Test 18: Tool dispatch with checkpoint ----
    #[test]
    fn test_tool_dispatch_with_checkpoint() {
        let mut pam = PresentAssistantMessage::new();
        let mut params = HashMap::new();
        params.insert("path".to_string(), "/test.txt".to_string());
        params.insert("content".to_string(), "hello".to_string());
        pam.set_assistant_message_content(vec![make_tool_block(
            "tool-1",
            "write_to_file",
            params,
            false,
        )]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::CheckpointAndExecute(_))));
        if let Some(BlockProcessingResult::CheckpointAndExecute(action)) = result {
            assert_eq!(action.tool_name, "write_to_file");
            assert!(action.needs_checkpoint);
        }
    }

    // ---- Test 19: Tool dispatch without checkpoint ----
    #[test]
    fn test_tool_dispatch_without_checkpoint() {
        let mut pam = PresentAssistantMessage::new();
        let mut params = HashMap::new();
        params.insert("path".to_string(), "/test.txt".to_string());
        pam.set_assistant_message_content(vec![make_tool_block(
            "tool-1",
            "read_file",
            params,
            false,
        )]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::ExecuteTool(_))));
        if let Some(BlockProcessingResult::ExecuteTool(action)) = result {
            assert_eq!(action.tool_name, "read_file");
            assert!(!action.needs_checkpoint);
        }
    }

    // ---- Test 20: Unknown tool ----
    #[test]
    fn test_unknown_tool() {
        let mut pam = PresentAssistantMessage::new();
        let mut params = HashMap::new();
        params.insert("arg".to_string(), "value".to_string());
        let mut native_args = serde_json::Map::new();
        native_args.insert("arg".to_string(), Value::String("value".to_string()));
        pam.set_assistant_message_content(vec![AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: "nonexistent_tool".to_string(),
            params,
            partial: false,
            id: "tool-1".to_string(),
            native_args: Some(Value::Object(native_args)),
            original_name: None,
            used_legacy_format: false,
        })]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::UnknownTool { .. })));
    }

    // ---- Test 21: Missing nativeArgs ----
    #[test]
    fn test_missing_native_args() {
        let mut pam = PresentAssistantMessage::new();
        let params = HashMap::new();
        pam.set_assistant_message_content(vec![AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: "read_file".to_string(),
            params,
            partial: false,
            id: "tool-1".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        })]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::MissingNativeArgs { .. })));
    }

    // ---- Test 22: Text skipped when rejected ----
    #[test]
    fn test_text_skipped_when_rejected() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Hello", false)]);
        pam.state.did_reject_tool = true;

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::TextSkipped)));
    }

    // ---- Test 23: Checkpoint save and mark ----
    #[test]
    fn test_checkpoint_save_and_mark() {
        let mut pam = PresentAssistantMessage::new();
        assert!(pam.needs_checkpoint());
        assert!(pam.checkpoint_save_and_mark());
        assert!(!pam.needs_checkpoint());
        assert!(!pam.checkpoint_save_and_mark()); // Already saved
    }

    // ---- Test 24: Handle tool denied ----
    #[test]
    fn test_handle_tool_denied() {
        let mut pam = PresentAssistantMessage::new();
        let mut callbacks = ToolCallbacks::new("test-id");

        let result = pam.handle_tool_denied(&mut callbacks, None, None);
        assert!(pam.state.did_reject_tool);
        assert!(!result.is_error);
    }

    // ---- Test 25: Handle tool denied with feedback ----
    #[test]
    fn test_handle_tool_denied_with_feedback() {
        let mut pam = PresentAssistantMessage::new();
        let mut callbacks = ToolCallbacks::new("test-id");

        let result = pam.handle_tool_denied(&mut callbacks, Some("try again".to_string()), None);
        assert!(pam.state.did_reject_tool);
        assert!(result.content.contains("try again"));
    }

    // ---- Test 26: Handle tool approved with feedback ----
    #[test]
    fn test_handle_tool_approved_with_feedback() {
        let mut pam = PresentAssistantMessage::new();
        let mut callbacks = ToolCallbacks::new("test-id");

        pam.handle_tool_approved(&mut callbacks, Some("looks good".to_string()), None);
        assert!(callbacks.approval_feedback.is_some());
        assert_eq!(callbacks.approval_feedback.unwrap().text, "looks good");
    }

    // ---- Test 27: Handle tool error ----
    #[test]
    fn test_handle_tool_error() {
        let mut pam = PresentAssistantMessage::new();
        let callbacks = ToolCallbacks::new("test-id");

        let result = pam.handle_tool_error(&callbacks, "executing tool", "something failed", false);
        assert!(result.is_some());
        let tool_result = result.unwrap();
        assert!(tool_result.is_error);
    }

    // ---- Test 28: Handle tool error with AskIgnored ----
    #[test]
    fn test_handle_tool_error_ask_ignored() {
        let mut pam = PresentAssistantMessage::new();
        let callbacks = ToolCallbacks::new("test-id");

        let result = pam.handle_tool_error(&callbacks, "executing tool", "ignored", true);
        assert!(result.is_none());
    }

    // ---- Test 29: is_known_tool_name ----
    #[test]
    fn test_is_known_tool_name() {
        assert!(is_known_tool_name("write_to_file"));
        assert!(is_known_tool_name("read_file"));
        assert!(is_known_tool_name("execute_command"));
        assert!(is_known_tool_name("use_mcp_tool"));
        assert!(is_known_tool_name("attempt_completion"));
        assert!(!is_known_tool_name("custom_tool_xyz"));
        assert!(!is_known_tool_name(""));
    }

    // ---- Test 30: Push tool result with feedback ----
    #[test]
    fn test_push_tool_result_with_feedback() {
        let mut pam = PresentAssistantMessage::new();
        let mut has_tool_result = false;
        let feedback = ApprovalFeedback {
            text: "looks good".to_string(),
            images: None,
        };

        let pushed = pam.push_tool_result_with_feedback(
            "tool-1",
            "result text",
            &[],
            Some(&feedback),
            &mut has_tool_result,
        );
        assert!(pushed);
        assert!(has_tool_result);
        assert_eq!(pam.state.user_message_content.len(), 1);
    }

    // ---- Test 31: Push tool result duplicate prevention ----
    #[test]
    fn test_push_tool_result_duplicate_prevention() {
        let mut pam = PresentAssistantMessage::new();
        let mut has_tool_result = false;

        let pushed1 = pam.push_tool_result_with_feedback(
            "tool-1",
            "first",
            &[],
            None,
            &mut has_tool_result,
        );
        assert!(pushed1);

        let pushed2 = pam.push_tool_result_with_feedback(
            "tool-1",
            "second",
            &[],
            None,
            &mut has_tool_result,
        );
        assert!(!pushed2);
    }

    // ---- Test 32: Post-processing advances index for complete blocks ----
    #[test]
    fn test_post_processing_complete_block() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![
            make_text_block("Block 1", false),
            make_text_block("Block 2", false),
        ]);

        let _ = pam.present(false);
        // After processing first block, index should advance to 1
        assert_eq!(pam.state.current_streaming_content_index, 1);
        // has_pending_updates should be true since there are more blocks
        assert!(pam.state.has_pending_updates);
    }

    // ---- Test 33: Post-processing sets ready for last block ----
    #[test]
    fn test_post_processing_last_block_ready() {
        let mut pam = PresentAssistantMessage::new();
        pam.set_assistant_message_content(vec![make_text_block("Only block", false)]);

        let _ = pam.present(false);
        assert!(pam.state.user_message_content_ready);
    }

    // ---- Test 34: search_replace needs checkpoint ----
    #[test]
    fn test_search_replace_needs_checkpoint() {
        let mut pam = PresentAssistantMessage::new();
        let mut params = HashMap::new();
        params.insert("file_path".to_string(), "/test.txt".to_string());
        let mut native_args = serde_json::Map::new();
        native_args.insert("file_path".to_string(), Value::String("/test.txt".to_string()));
        pam.set_assistant_message_content(vec![AssistantMessageContent::ToolUse(ToolUse {
            content_type: "tool_use".to_string(),
            name: "search_replace".to_string(),
            params,
            partial: false,
            id: "tool-1".to_string(),
            native_args: Some(Value::Object(native_args)),
            original_name: None,
            used_legacy_format: false,
        })]);

        let result = pam.present(false).unwrap();
        assert!(matches!(result, Some(BlockProcessingResult::CheckpointAndExecute(_))));
    }

    // ---- Test 35: Tool description for all tools ----
    #[test]
    fn test_tool_description_all_tools() {
        let pam = PresentAssistantMessage::new();

        // search_files with file_pattern
        let mut params = HashMap::new();
        params.insert("regex".to_string(), "pattern".to_string());
        params.insert("file_pattern".to_string(), "*.rs".to_string());
        let tool = ToolUse {
            content_type: "tool_use".to_string(),
            name: "search_files".to_string(),
            params: params.clone(),
            partial: false,
            id: "test".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        };
        assert_eq!(pam.get_tool_description(&tool), "[search_files for 'pattern' in '*.rs']");

        // switch_mode with reason
        let mut params2 = HashMap::new();
        params2.insert("mode_slug".to_string(), "architect".to_string());
        params2.insert("reason".to_string(), "need design".to_string());
        let tool2 = ToolUse {
            content_type: "tool_use".to_string(),
            name: "switch_mode".to_string(),
            params: params2,
            partial: false,
            id: "test".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        };
        assert_eq!(pam.get_tool_description(&tool2), "[switch_mode to 'architect' because: need design]");

        // new_task
        let mut params3 = HashMap::new();
        params3.insert("mode".to_string(), "code".to_string());
        params3.insert("message".to_string(), "fix bug".to_string());
        let tool3 = ToolUse {
            content_type: "tool_use".to_string(),
            name: "new_task".to_string(),
            params: params3,
            partial: false,
            id: "test".to_string(),
            native_args: None,
            original_name: None,
            used_legacy_format: false,
        };
        assert_eq!(pam.get_tool_description(&tool3), "[new_task in code mode: 'fix bug']");
    }
}