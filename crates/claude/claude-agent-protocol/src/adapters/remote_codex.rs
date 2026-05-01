//! Remote Codex in-process adapter.
//!
//! [`RemoteCodexAdapter`] wraps the existing rc-* crates as an in-process Agent.
//! All shared logic lives in [`InProcessAdapter`](super::in_process::InProcessAdapter).

use super::in_process::InProcessAdapter;

/// Remote Codex in-process adapter (type alias for [`InProcessAdapter`]).
///
/// Uses callback functions to interact with the existing rc-* crates,
/// avoiding the need to depend on those crates directly within
/// `rc-agent-protocol`.
///
/// # Example
///
/// ```ignore
/// use claude_agent_protocol::adapters::RemoteCodexAdapter;
/// use claude_agent_protocol::AgentAdapter;
///
/// let adapter = RemoteCodexAdapter::new_codex()
///     .with_send_message(|session_id, msg| {
///         // Bridge into rc-* crates here …
///         Ok(vec![])
///     });
/// ```
pub type RemoteCodexAdapter = InProcessAdapter;
