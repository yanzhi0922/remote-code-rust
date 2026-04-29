//! # roo-auto-approval
//!
//! Auto-approval system for Roo Code Rust.
//!
//! This crate provides pure-logic, synchronous functions for deciding whether
//! to auto-approve, deny, or ask the user about various operations (commands,
//! tool usage, MCP server calls, follow-up questions, etc.).
//!
//! ## Design Principles
//!
//! - **Pure logic, no I/O**: All configuration is passed as parameters.
//! - **Synchronous**: All functions are synchronous (no async).
//! - **No VS Code API dependency**: Can be used in any context.
//!
//! ## Main Entry Point
//!
//! [`check_auto_approval`] — the primary function that determines whether an
//! operation should be auto-approved, denied, or require user approval.

pub mod approval;
pub mod commands;
pub mod tools;
pub mod types;

// Re-export the main public API at the crate root.
pub use approval::{
    check_auto_approval, is_mcp_tool_always_allowed, AutoApprovalHandler, CheckAutoApprovalParams,
};
pub use commands::{
    contains_dangerous_substitution, find_longest_prefix_match, get_command_decision,
    get_single_command_decision, is_auto_approved_single_command,
    is_auto_denied_single_command, parse_command_chain,
};
pub use tools::{is_read_only_tool_action, is_read_only_tool_name, is_write_tool_action, is_write_tool_name};
pub use types::{
    ApprovalLimitType, AskType, AutoApprovalLimitResult, AutoApprovalState,
    CheckAutoApprovalResult, CommandDecision, McpServer, McpServerUse, McpTool, ToolAction,
};
