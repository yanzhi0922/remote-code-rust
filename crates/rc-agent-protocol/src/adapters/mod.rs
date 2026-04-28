//! Agent adapter implementations.
//!
//! This module contains concrete implementations of the [`AgentAdapter`](crate::AgentAdapter)
//! trait for each supported Agent type.

pub mod remote_claude;
pub mod remote_codex;
pub mod remote_roo;

pub use remote_claude::RemoteClaudeAdapter;
pub use remote_codex::RemoteCodexAdapter;
pub use remote_roo::RemoteRooAdapter;
