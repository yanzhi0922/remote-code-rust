use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use claude_provider::ConversationBackend;
use claude_query_engine::config::ToolRunner;
use claude_session::SessionStore;
use parking_lot::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ServerConfig;
use crate::ws::protocol::ServerMessage;

/// Maximum number of concurrent active sessions to prevent resource exhaustion.
const MAX_ACTIVE_SESSIONS: usize = 1000;

/// An active session with at least one connected WS client.
pub struct ActiveSession {
    /// Broadcast sender for ServerMessage events to all WS clients.
    pub event_tx: broadcast::Sender<ServerMessage>,
    /// Handle to the running query task (if any).
    pub query_task: Option<tokio::task::JoinHandle<()>>,
    /// Interrupt flag shared with the running query.
    pub interrupted: Arc<AtomicBool>,
}

/// Shared server state injected into all handlers via axum State.
#[derive(Clone)]
pub struct ServerState {
    pub config: ServerConfig,
    pub session_store: Arc<SessionStore>,
    pub active_sessions: Arc<RwLock<BTreeMap<Uuid, ActiveSession>>>,
    /// Shared conversation backend (LLM provider).
    pub backend: Arc<dyn ConversationBackend>,
    /// Shared tool runner.
    pub tool_runner: Arc<dyn ToolRunner>,
}

impl ServerState {
    pub fn new(
        config: ServerConfig,
        session_store: SessionStore,
        backend: Arc<dyn ConversationBackend>,
        tool_runner: Arc<dyn ToolRunner>,
    ) -> Self {
        Self {
            config,
            session_store: Arc::new(session_store),
            active_sessions: Arc::new(RwLock::new(BTreeMap::new())),
            backend,
            tool_runner,
        }
    }

    /// Get or create an ActiveSession entry, returning a broadcast receiver.
    ///
    /// Returns an error if the number of active sessions would exceed
    /// `MAX_ACTIVE_SESSIONS`.
    pub fn ensure_active_session(
        &self,
        session_id: Uuid,
    ) -> Result<broadcast::Receiver<ServerMessage>, &'static str> {
        let mut sessions = self.active_sessions.write();
        // Check the limit before inserting a new entry. If the session already
        // exists, we can always return it (it doesn't add to the count).
        let already_exists = sessions.contains_key(&session_id);
        if !already_exists && sessions.len() >= MAX_ACTIVE_SESSIONS {
            return Err("too many active sessions");
        }
        let active = sessions.entry(session_id).or_insert_with(|| {
            let (tx, _) = broadcast::channel(self.config.event_buffer_size);
            ActiveSession {
                event_tx: tx,
                query_task: None,
                interrupted: Arc::new(AtomicBool::new(false)),
            }
        });
        Ok(active.event_tx.subscribe())
    }

    /// Broadcast a ServerMessage to all WS clients subscribed to a session.
    pub fn broadcast_to_session(&self, session_id: Uuid, msg: ServerMessage) {
        let sessions = self.active_sessions.read();
        if let Some(active) = sessions.get(&session_id) {
            let _ = active.event_tx.send(msg);
        }
    }

    /// Remove an active session entry.
    pub fn remove_active_session(&self, session_id: Uuid) {
        self.active_sessions.write().remove(&session_id);
    }
}
