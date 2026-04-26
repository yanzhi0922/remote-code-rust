//! Agent router — manages multiple Agent instances and routes messages.
//!
//! The [`AgentRouter`] holds a mapping from session IDs to Agent adapters and
//! dispatches incoming messages to the correct adapter. It can create adapters
//! based on [`AgentType`] via the [`create_adapter`](AgentRouter::create_adapter)
//! factory method.

use std::collections::HashMap;

use tracing::warn;

use crate::adapter::AgentAdapter;
use crate::adapters::{CodexAdapter, RemoteCodeAdapter, RooCodeAdapter};
use crate::error::AgentProtocolError;
use crate::events::UnifiedAgentEvent;
use crate::permission::PermissionDecision;
use crate::types::{AgentConfig, AgentType};

/// Manages multiple Agent instances and routes messages by session ID.
///
/// This is a simplified framework — actual adapter instantiation (Remote Code,
/// Roo Code, Codex) will be implemented in subsequent phases.
pub struct AgentRouter {
    /// Session ID → Agent adapter.
    adapters: HashMap<String, Box<dyn AgentAdapter>>,
}

impl AgentRouter {
    /// Create a new, empty router.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Register a pre-built adapter under the given session ID.
    pub fn register(&mut self, session_id: String, adapter: Box<dyn AgentAdapter>) {
        self.adapters.insert(session_id, adapter);
    }

    /// Create an adapter based on the [`AgentType`] specified in `config`.
    ///
    /// Currently only [`AgentType::RemoteCode`] is supported; other types
    /// will return an error.
    pub fn create_adapter(config: &AgentConfig) -> anyhow::Result<Box<dyn AgentAdapter>> {
        match config.agent_type {
            AgentType::RemoteCode => {
                let adapter = RemoteCodeAdapter::new();
                Ok(Box::new(adapter))
            }
            AgentType::RooCode => {
                let adapter = RooCodeAdapter::new();
                Ok(Box::new(adapter))
            }
            AgentType::Codex => {
                let adapter = CodexAdapter::new();
                Ok(Box::new(adapter))
            }
        }
    }

    /// Convenience method: create an adapter from `config`, start it, and
    /// register it under `session_id`.
    pub async fn create_and_register(
        &mut self,
        session_id: String,
        config: &AgentConfig,
    ) -> anyhow::Result<()> {
        let mut adapter = Self::create_adapter(config)?;
        adapter.start(config).await?;
        self.adapters.insert(session_id, adapter);
        Ok(())
    }

    /// Send a message to the Agent bound to `session_id`.
    ///
    /// Returns a receiver that yields [`UnifiedAgentEvent`]s from the Agent.
    pub async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<UnifiedAgentEvent>> {
        let adapter =
            self.adapters
                .get_mut(session_id)
                .ok_or_else(|| AgentProtocolError::ProtocolError {
                    message: format!("no adapter found for session {session_id}"),
                })?;
        adapter.send_message(session_id, message).await
    }

    /// Cancel the current operation for the given session.
    pub async fn cancel(&mut self, session_id: &str) -> anyhow::Result<()> {
        let adapter =
            self.adapters
                .get_mut(session_id)
                .ok_or_else(|| AgentProtocolError::ProtocolError {
                    message: format!("no adapter found for session {session_id}"),
                })?;
        adapter.cancel(session_id).await
    }

    /// Resolve a permission request for the given session.
    pub async fn resolve_permission(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let adapter =
            self.adapters
                .get_mut(session_id)
                .ok_or_else(|| AgentProtocolError::ProtocolError {
                    message: format!("no adapter found for session {session_id}"),
                })?;
        adapter
            .resolve_permission(session_id, request_id, decision)
            .await
    }

    /// Close and remove the session, stopping the underlying Agent.
    pub async fn close_session(&mut self, session_id: &str) -> anyhow::Result<()> {
        if let Some(mut adapter) = self.adapters.remove(session_id) {
            adapter.stop().await?;
        } else {
            warn!(session_id, "attempted to close unknown session");
        }
        Ok(())
    }

    /// Returns the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.adapters.len()
    }

    /// Returns `true` if the router has an adapter for the given session.
    pub fn has_session(&self, session_id: &str) -> bool {
        self.adapters.contains_key(session_id)
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_new_is_empty() {
        let router = AgentRouter::new();
        assert_eq!(router.session_count(), 0);
        assert!(!router.has_session("nonexistent"));
    }

    #[test]
    fn router_default() {
        let router = AgentRouter::default();
        assert_eq!(router.session_count(), 0);
    }

    #[tokio::test]
    async fn router_send_message_unknown_session_fails() {
        let mut router = AgentRouter::new();
        let result = router.send_message("unknown", "hello").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn router_cancel_unknown_session_fails() {
        let mut router = AgentRouter::new();
        let result = router.cancel("unknown").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn router_close_unknown_session_ok() {
        let mut router = AgentRouter::new();
        // Closing a non-existent session should not error.
        let result = router.close_session("unknown").await;
        assert!(result.is_ok());
    }
}
