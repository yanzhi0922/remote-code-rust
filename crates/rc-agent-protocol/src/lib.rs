//! # rc-agent-protocol
//!
//! Unified Agent protocol layer for the multi-agent architecture.
//!
//! This crate defines the common types, events, and traits that all Agent
//! adapters must implement, enabling seamless integration of:
//! - **Remote Code** (in-process, direct crate calls)
//! - **Roo Code** (sub-process, JSON-RPC 2.0 + Content-Length framing)
//! - **OpenAI Codex** (sub-process, JSON-RPC v2 + line-delimited JSON)

pub mod adapter;
pub mod adapters;
pub mod error;
pub mod events;
pub mod permission;
pub mod router;
pub mod types;

// Re-export core types at crate root for convenience.
pub use adapter::AgentAdapter;
pub use adapters::{CodexAdapter, RemoteCodeAdapter, RooCodeAdapter};
pub use error::AgentProtocolError;
pub use events::{AgentResult, ToolCallInfo, UnifiedAgentEvent, UsageInfo};
pub use permission::{PermissionDecision, PermissionRequest};
pub use router::AgentRouter;
pub use types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};
