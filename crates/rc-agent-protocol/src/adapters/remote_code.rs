//! Remote Code in-process adapter.
//!
//! [`RemoteCodeAdapter`] wraps the existing rc-* crates as an in-process Agent.
//! To avoid heavy compile-time dependencies, it communicates with the outside
//! world through **callback functions** (trait objects) that the caller injects
//! via the builder pattern.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use crate::adapter::AgentAdapter;
use crate::events::UnifiedAgentEvent;
use crate::permission::PermissionDecision;
use crate::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

// ---------------------------------------------------------------------------
// Callback type aliases (kept private — the public API uses generics)
// ---------------------------------------------------------------------------

type SendMessageFn =
    Box<dyn Fn(&str, &str) -> anyhow::Result<Vec<UnifiedAgentEvent>> + Send + Sync>;

type CancelFn = Box<dyn Fn(&str) -> anyhow::Result<()> + Send + Sync>;

type ResolvePermissionFn =
    Box<dyn Fn(&str, &str, PermissionDecision) -> anyhow::Result<()> + Send + Sync>;

// ---------------------------------------------------------------------------
// RemoteCodeAdapter
// ---------------------------------------------------------------------------

/// Remote Code in-process adapter.
///
/// Uses callback functions to interact with the existing rc-* crates,
/// avoiding the need to depend on those crates directly within
/// `rc-agent-protocol`.
///
/// # Example
///
/// ```ignore
/// use rc_agent_protocol::adapters::RemoteCodeAdapter;
/// use rc_agent_protocol::AgentAdapter;
///
/// let adapter = RemoteCodeAdapter::new()
///     .with_send_message(|session_id, msg| {
///         // Bridge into rc-* crates here …
///         Ok(vec![])
///     });
/// ```
pub struct RemoteCodeAdapter {
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// Callback invoked by [`send_message`](AgentAdapter::send_message).
    on_send_message: Option<SendMessageFn>,
    /// Callback invoked by [`cancel`](AgentAdapter::cancel).
    on_cancel: Option<CancelFn>,
    /// Callback invoked by [`resolve_permission`](AgentAdapter::resolve_permission).
    on_resolve_permission: Option<ResolvePermissionFn>,
}

// -----
// Construction
// -----

impl RemoteCodeAdapter {
    /// Create a new `RemoteCodeAdapter` in the **Starting** state with no
    /// callbacks configured.
    ///
    /// Use the `with_*` builder methods to attach callbacks, then call
    /// [`start`](AgentAdapter::start) to transition to **Ready**.
    #[must_use]
    pub fn new() -> Self {
        let mut capabilities = std::collections::HashSet::new();
        capabilities.insert(AgentCapability::Streaming);
        capabilities.insert(AgentCapability::ToolUse);
        capabilities.insert(AgentCapability::McpSupport);
        capabilities.insert(AgentCapability::Subtasks);
        capabilities.insert(AgentCapability::Permissions);

        Self {
            info: AgentInfo {
                name: "Remote Code".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            on_send_message: None,
            on_cancel: None,
            on_resolve_permission: None,
        }
    }

    /// Attach a callback that will be invoked when
    /// [`send_message`](AgentAdapter::send_message) is called.
    ///
    /// The callback receives `(session_id, message)` and must return a
    /// `Vec<UnifiedAgentEvent>` representing the Agent's response.
    #[must_use]
    pub fn with_send_message<F>(mut self, f: F) -> Self
    where
        F: Fn(&str, &str) -> anyhow::Result<Vec<UnifiedAgentEvent>> + Send + Sync + 'static,
    {
        self.on_send_message = Some(Box::new(f));
        self
    }

    /// Attach a callback that will be invoked when
    /// [`cancel`](AgentAdapter::cancel) is called.
    ///
    /// The callback receives `session_id`.
    #[must_use]
    pub fn with_cancel<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.on_cancel = Some(Box::new(f));
        self
    }

    /// Attach a callback that will be invoked when
    /// [`resolve_permission`](AgentAdapter::resolve_permission) is called.
    ///
    /// The callback receives `(session_id, request_id, decision)`.
    #[must_use]
    pub fn with_resolve_permission<F>(mut self, f: F) -> Self
    where
        F: Fn(&str, &str, PermissionDecision) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.on_resolve_permission = Some(Box::new(f));
        self
    }
}

impl Default for RemoteCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// -----
// AgentAdapter implementation
// -----

#[async_trait]
impl AgentAdapter for RemoteCodeAdapter {
    async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
        info!("RemoteCodeAdapter starting");
        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        // #22: The callback is a synchronous blocking function. This means the
        // entire agent computation runs on the current tokio task, blocking
        // other tasks on the same thread. For short-lived operations this is
        // acceptable, but long-running agent calls should use
        // `tokio::task::spawn_blocking` to avoid starving the runtime.
        // This is a known limitation that will be addressed when the callback
        // signature is changed to async in a future refactor.
        let callback = self
            .on_send_message
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("send_message callback not configured"))?;

        let events = callback(session_id, message)?;

        // Create a bounded channel; 256 is generous for a single response.
        let (tx, rx) = mpsc::channel(256);

        // Send all events into the channel.  If the receiver is dropped early
        // we simply stop sending — no need to propagate an error.
        for event in events {
            if tx.send(event).await.is_err() {
                break;
            }
        }

        Ok(rx)
    }

    async fn cancel(&mut self, session_id: &str) -> anyhow::Result<()> {
        let callback = self
            .on_cancel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("cancel callback not configured"))?;

        callback(session_id)
    }

    async fn resolve_permission(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let callback = self
            .on_resolve_permission
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("resolve_permission callback not configured"))?;

        callback(session_id, request_id, decision)
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("RemoteCodeAdapter stopping");
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        !matches!(self.status, AgentStatus::Stopped | AgentStatus::Error)
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteCode
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentConfig;

    /// Helper: a minimal config for tests.
    fn test_config() -> AgentConfig {
        AgentConfig {
            agent_type: AgentType::RemoteCode,
            binary_path: None,
            args: vec![],
            env: vec![],
            working_dir: None,
            model: None,
            provider: None,
            api_key: None,
            base_url: None,
        }
    }

    #[test]
    fn new_adapter_has_remote_code_type() {
        let adapter = RemoteCodeAdapter::new();
        assert_eq!(adapter.agent_type(), AgentType::RemoteCode);
    }

    #[tokio::test]
    async fn start_sets_status_to_ready() {
        let mut adapter = RemoteCodeAdapter::new();
        assert_eq!(adapter.status, AgentStatus::Starting);

        adapter.start(&test_config()).await.unwrap();
        assert_eq!(adapter.status, AgentStatus::Ready);
        assert_eq!(adapter.info().status, AgentStatus::Ready);
    }

    #[tokio::test]
    async fn send_message_without_callback_returns_error() {
        let mut adapter = RemoteCodeAdapter::new();
        adapter.start(&test_config()).await.unwrap();

        let result = adapter.send_message("sess-1", "hello").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("send_message callback not configured"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn send_message_with_callback_returns_events() {
        let adapter = RemoteCodeAdapter::new().with_send_message(|_sid, msg| {
            Ok(vec![
                UnifiedAgentEvent::MessageDelta {
                    session_id: "sess-1".into(),
                    delta: msg.into(),
                },
                UnifiedAgentEvent::Completed {
                    session_id: "sess-1".into(),
                    result: crate::events::AgentResult {
                        response_text: msg.into(),
                        tool_calls: vec![],
                        usage: crate::events::UsageInfo::default(),
                        cost: None,
                    },
                },
            ])
        });

        let mut adapter = adapter;
        adapter.start(&test_config()).await.unwrap();

        let mut rx = adapter.send_message("sess-1", "hello world").await.unwrap();

        // First event: MessageDelta
        let ev1 = rx.recv().await.expect("should receive first event");
        assert!(matches!(ev1, UnifiedAgentEvent::MessageDelta { .. }));

        // Second event: Completed
        let ev2 = rx.recv().await.expect("should receive second event");
        assert!(matches!(ev2, UnifiedAgentEvent::Completed { .. }));

        // No more events.
        let ev3 = rx.recv().await;
        assert!(ev3.is_none(), "channel should be closed");
    }

    #[tokio::test]
    async fn cancel_without_callback_returns_error() {
        let mut adapter = RemoteCodeAdapter::new();
        adapter.start(&test_config()).await.unwrap();

        let result = adapter.cancel("sess-1").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("cancel callback not configured"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn cancel_with_callback_succeeds() {
        let adapter = RemoteCodeAdapter::new().with_cancel(|_sid| Ok(()));

        let mut adapter = adapter;
        adapter.start(&test_config()).await.unwrap();

        let result = adapter.cancel("sess-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn stop_sets_status_to_stopped() {
        let mut adapter = RemoteCodeAdapter::new();
        adapter.start(&test_config()).await.unwrap();
        assert_eq!(adapter.status, AgentStatus::Ready);

        adapter.stop().await.unwrap();
        assert_eq!(adapter.status, AgentStatus::Stopped);
        assert_eq!(adapter.info().status, AgentStatus::Stopped);
    }

    #[test]
    fn is_alive_reflects_status() {
        let mut adapter = RemoteCodeAdapter::new();
        // Starting → alive
        assert!(adapter.is_alive());

        // Simulate Ready
        adapter.status = AgentStatus::Ready;
        assert!(adapter.is_alive());

        // Simulate Busy
        adapter.status = AgentStatus::Busy;
        assert!(adapter.is_alive());

        // Simulate Idle
        adapter.status = AgentStatus::Idle;
        assert!(adapter.is_alive());

        // Simulate Stopped
        adapter.status = AgentStatus::Stopped;
        assert!(!adapter.is_alive());

        // Simulate Error
        adapter.status = AgentStatus::Error;
        assert!(!adapter.is_alive());
    }

    #[test]
    fn builder_pattern_works() {
        let adapter = RemoteCodeAdapter::new()
            .with_send_message(|_sid, _msg| Ok(vec![]))
            .with_cancel(|_sid| Ok(()))
            .with_resolve_permission(|_sid, _rid, _dec| Ok(()));

        assert_eq!(adapter.agent_type(), AgentType::RemoteCode);
        assert!(adapter.on_send_message.is_some());
        assert!(adapter.on_cancel.is_some());
        assert!(adapter.on_resolve_permission.is_some());
    }

    #[test]
    fn default_equals_new() {
        let new_adapter = RemoteCodeAdapter::new();
        let default_adapter = RemoteCodeAdapter::default();
        assert_eq!(new_adapter.agent_type(), default_adapter.agent_type());
        assert_eq!(new_adapter.status, default_adapter.status);
    }

    #[test]
    fn info_has_all_capabilities() {
        let adapter = RemoteCodeAdapter::new();
        let info = adapter.info();

        assert_eq!(info.name, "Remote Code");
        assert!(info.capabilities.contains(&AgentCapability::Streaming));
        assert!(info.capabilities.contains(&AgentCapability::ToolUse));
        assert!(info.capabilities.contains(&AgentCapability::McpSupport));
        assert!(info.capabilities.contains(&AgentCapability::Subtasks));
        assert!(info.capabilities.contains(&AgentCapability::Permissions));
        assert_eq!(info.capabilities.len(), 5);
    }
}
