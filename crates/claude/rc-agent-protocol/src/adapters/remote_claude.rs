//! Remote Claude in-process adapter.
//!
//! [`RemoteClaudeAdapter`] wraps the existing rc-* crates as an in-process Agent.
//! All shared logic lives in [`InProcessAdapter`](super::in_process::InProcessAdapter).

use super::in_process::InProcessAdapter;

/// Remote Claude in-process adapter (type alias for [`InProcessAdapter`]).
///
/// Uses callback functions to interact with the existing rc-* crates,
/// avoiding the need to depend on those crates directly within
/// `rc-agent-protocol`.
///
/// # Example
///
/// ```ignore
/// use rc_agent_protocol::adapters::RemoteClaudeAdapter;
/// use rc_agent_protocol::AgentAdapter;
///
/// let adapter = RemoteClaudeAdapter::new_claude()
///     .with_send_message(|session_id, msg| {
///         // Bridge into rc-* crates here …
///         Ok(vec![])
///     });
/// ```
pub type RemoteClaudeAdapter = InProcessAdapter;
