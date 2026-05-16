#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
//! # Roo Task Engine
//!
//! Task engine for the Roo Code Rust project.
//!
//! This crate provides:
//! - **Types**: [`TaskState`], [`TaskConfig`], [`TaskResult`], [`TaskError`]
//! - **State machine**: [`StateMachine`] for managing task lifecycle
//! - **Events**: [`TaskEvent`], [`TaskEventEmitter`] for event-driven communication
//! - **Loop control**: [`LoopControl`] for iteration and mistake limits
//! - **Engine**: [`TaskEngine`] orchestrating the full task lifecycle
//! - **Config**: [`validate_config`], [`default_config`] for configuration management
//! - **Stream parser**: [`StreamParser`] for parsing API streaming responses
//! - **Tool dispatcher**: [`ToolDispatcher`] for routing tool calls to handlers
//! - **Message builder**: [`MessageBuilder`] for constructing API messages
//! - **Agent loop**: [`AgentLoop`] for the core agent execution loop

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

pub mod agent_loop;
pub mod ask_say;
pub mod config;
pub mod debug_log;
pub mod engine;
pub mod events;
pub mod loop_control;
pub mod message_builder;
pub mod native_tool_call_parser;
pub mod present_assistant_message;
pub mod state;
pub mod stream_parser;
pub mod task_lifecycle;
pub mod task_manager;
pub mod tool_dispatcher;
pub mod types;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use agent_loop::{AgentLoop, AgentLoopConfig};
pub use ask_say::{AskIgnoredError, AskResponse, AskResult, AskSayHandler, SayOptions};
pub use config::{DEFAULT_MAX_MISTAKES, DEFAULT_MODE, default_config, validate_config};
pub use engine::TaskEngine;
pub use events::{TaskEvent, TaskEventEmitter};
pub use loop_control::LoopControl;
pub use message_builder::MessageBuilder;
pub use native_tool_call_parser::NativeToolCallParser;
pub use state::StateMachine;
pub use stream_parser::{ParsedStreamContent, ParsedToolCall, StreamParser, StreamUsage};
pub use task_lifecycle::{ServiceRefs, TaskLifecycle};
pub use task_manager::TaskManager;
pub use tool_dispatcher::{
    NEW_TASK_SENTINEL, SubtaskConfig, SubtaskResult, ToolContext, ToolDispatcher,
    ToolExecutionResult, ToolHandler, execute_subtask,
};
pub use types::{
    AssistantMessageContent, AttemptResult, DiffStrategy, McpToolUse, RawChunkTrackerEntry,
    StackItem, StreamEvent, StreamingState, StreamingToolCallState, TOOL_PARAM_NAMES, TaskConfig,
    TaskError, TaskResult, TaskState, TextContent, ToolCallStreamEvent, ToolUse, is_mcp_tool_name,
    is_valid_tool_param, normalize_mcp_tool_name, parse_mcp_tool_name,
};

pub use debug_log::{DebugLogger, debug_log, is_debug_log_enabled, set_debug_log_enabled};
pub use present_assistant_message::{
    ApprovalFeedback, BlockProcessingResult, ImageBlock, McpDispatchAction,
    PresentAssistantMessage, PresentAssistantMessageError, PresentAssistantMessageState,
    ToolCallbacks, ToolDispatchAction, ToolResult, format_tool_approved_with_feedback,
    format_tool_denied, format_tool_denied_with_feedback, format_tool_error, format_tool_result,
    is_file_modifying_tool, sanitize_tool_use_id, strip_thinking_tags,
};
