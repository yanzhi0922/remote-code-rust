//! Agent adapter implementations.
//!
//! This module contains concrete implementations of the [`AgentAdapter`](crate::AgentAdapter)
//! trait for each supported Agent type.

pub mod codex;
pub mod remote_code;
pub mod roo_code;

pub use codex::CodexAdapter;
pub use remote_code::RemoteCodeAdapter;
pub use roo_code::RooCodeAdapter;
