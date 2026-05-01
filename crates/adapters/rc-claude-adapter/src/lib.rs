//! # rc-claude-adapter
//!
//! In-process adapter for the Claude agent.
//!
//! This crate provides [`ClaudeInProcessAdapter`] which wraps the
//! [`QueryEngine`](rc_query_engine::QueryEngine) into a unified adapter interface,
//! consistent with [`CodexInProcessAdapter`](rc_codex_adapter::CodexInProcessAdapter)
//! and [`RooInProcessAdapter`](rc_roo_adapter::RooInProcessAdapter).
//!
//! # Architecture
//!
//! Unlike Codex and Roo which have their own independent agent loops, Claude uses
//! the `QueryEngine` — a general-purpose execution engine that supports tool running,
//! context management, streaming, and multi-turn conversations. The adapter wraps
//! this engine to provide a consistent interface.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rc_claude_adapter::ClaudeInProcessAdapter;
//!
//! // The adapter is typically created and used inside the GUI's
//! // run_unified_prompt_with_provider() function.
//! ```

pub use rc_query_engine::QueryEngine as ClaudeInProcessAdapter;

// Re-export commonly used types for convenience.
pub use rc_query_engine::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngineConfig, QueryObserver,
    QueryObserverEvent, ToolRunResult, ToolRunner,
};
