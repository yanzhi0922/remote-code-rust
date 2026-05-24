//! P2P peer session management — list, invite, accept, leave peer sessions.
//!
//! Native Rust implementation of `claude-code-rev/src/bridge/peerSessions.ts`.
//! Maps remote agent instances to local mailbox-based coordination.

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A peer session — direct connection to another agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSession {
    /// Unique session identifier.
    pub session_id: Uuid,
    /// Peer display name.
    pub peer_name: String,
    /// Peer agent type.
    pub peer_type: String,
    /// Session status.
    pub status: PeerSessionStatus,
    /// When the session was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Transport endpoint (WS URL for relay, direct IP for P2P).
    pub endpoint: Option<String>,
}

/// Status of a peer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerSessionStatus {
    Pending,
    Active,
    Disconnected,
}

/// Manager for peer-to-peer agent collaboration sessions.
pub struct PeerSessionManager {
    sessions: HashMap<Uuid, PeerSession>,
}

impl PeerSessionManager {
    /// Create a new empty peer session manager.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// List all active peer sessions.
    pub fn list(&self) -> Vec<&PeerSession> {
        self.sessions.values().collect()
    }

    /// Get a specific session by ID.
    pub fn get(&self, session_id: Uuid) -> Option<&PeerSession> {
        self.sessions.get(&session_id)
    }

    /// Invite a peer to start a session.
    pub fn invite(&mut self, peer_name: String, peer_type: String) -> PeerSession {
        let session = PeerSession {
            session_id: Uuid::new_v4(),
            peer_name,
            peer_type,
            status: PeerSessionStatus::Pending,
            created_at: chrono::Utc::now(),
            endpoint: None,
        };
        let id = session.session_id;
        self.sessions.insert(id, session);
        self.sessions.get(&id).cloned().unwrap()
    }

    /// Accept an incoming peer session invitation.
    pub fn accept(&mut self, session_id: Uuid, endpoint: Option<String>) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("peer session not found: {session_id}"))?;
        session.status = PeerSessionStatus::Active;
        session.endpoint = endpoint;
        Ok(())
    }

    /// Disconnect a peer session.
    pub fn disconnect(&mut self, session_id: Uuid) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("peer session not found: {session_id}"))?;
        session.status = PeerSessionStatus::Disconnected;
        Ok(())
    }

    /// Remove a peer session entirely.
    pub fn remove(&mut self, session_id: Uuid) -> Option<PeerSession> {
        self.sessions.remove(&session_id)
    }

    /// Number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.status == PeerSessionStatus::Active)
            .count()
    }
}

impl Default for PeerSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager() {
        let mgr = PeerSessionManager::new();
        assert_eq!(mgr.list().len(), 0);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn invite_creates_pending_session() {
        let mut mgr = PeerSessionManager::new();
        let session = mgr.invite("peer-1".into(), "claude".into());
        assert_eq!(session.status, PeerSessionStatus::Pending);
        assert_eq!(session.peer_name, "peer-1");
        assert_eq!(mgr.list().len(), 1);
    }

    #[test]
    fn accept_activates_session() {
        let mut mgr = PeerSessionManager::new();
        let session = mgr.invite("peer-1".into(), "claude".into());
        mgr.accept(session.session_id, Some("ws://localhost:9000".into()))
            .unwrap();
        let s = mgr.get(session.session_id).unwrap();
        assert_eq!(s.status, PeerSessionStatus::Active);
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn disconnect_marks_disconnected() {
        let mut mgr = PeerSessionManager::new();
        let session = mgr.invite("peer-1".into(), "claude".into());
        mgr.disconnect(session.session_id).unwrap();
        let s = mgr.get(session.session_id).unwrap();
        assert_eq!(s.status, PeerSessionStatus::Disconnected);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn remove_deletes_session() {
        let mut mgr = PeerSessionManager::new();
        let session = mgr.invite("peer-1".into(), "claude".into());
        assert!(mgr.remove(session.session_id).is_some());
        assert_eq!(mgr.list().len(), 0);
    }

    #[test]
    fn accept_unknown_session_errors() {
        let mut mgr = PeerSessionManager::new();
        assert!(mgr.accept(Uuid::new_v4(), None).is_err());
    }
}
