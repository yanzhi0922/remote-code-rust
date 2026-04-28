//! Agent adapter implementations.
//!
//! This module contains concrete implementations of the [`AgentAdapter`](crate::AgentAdapter)
//! trait for each supported Agent type. All in-process adapters share the same
//! [`InProcessAdapter`] implementation; each Agent type is exposed as a type alias.

mod in_process;
pub mod remote_claude;
pub mod remote_codex;
pub mod remote_roo;

pub use in_process::InProcessAdapter;
pub use remote_claude::RemoteClaudeAdapter;
pub use remote_codex::RemoteCodexAdapter;
pub use remote_roo::RemoteRooAdapter;
