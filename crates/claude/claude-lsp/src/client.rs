//! LSP Client for communicating with language servers.
//!
//! Provides a high-level client that handles the LSP lifecycle:
//! initialization, requests, notifications, and shutdown.

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use serde_json::Value;

use crate::types::{LspMessage, LspResponse};

/// Maximum number of buffered outgoing messages before evicting the oldest.
const MAX_OUTGOING_MESSAGES: usize = 1000;
/// Maximum number of cached responses before evicting the oldest.
const MAX_RESPONSES: usize = 1000;

// ---------------------------------------------------------------------------
// LspClient
// ---------------------------------------------------------------------------

/// Status of an LSP client connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientStatus {
    /// Client created but not yet initialized.
    Uninitialized,
    /// Client has sent the initialize request.
    Initializing,
    /// Client is fully initialized and ready.
    Ready,
    /// Client is shutting down.
    ShuttingDown,
    /// Client has been shut down.
    Shutdown,
}

impl std::fmt::Display for ClientStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "uninitialized"),
            Self::Initializing => write!(f, "initializing"),
            Self::Ready => write!(f, "ready"),
            Self::ShuttingDown => write!(f, "shutting_down"),
            Self::Shutdown => write!(f, "shutdown"),
        }
    }
}

/// An LSP client that communicates with a language server.
///
/// In this skeleton implementation, messages are buffered in memory
/// rather than sent over a real transport.
#[derive(Debug)]
pub struct LspClient {
    /// Root URI of the project.
    root_uri: String,
    /// Client name.
    #[allow(dead_code)]
    client_name: String,
    /// Client version.
    #[allow(dead_code)]
    client_version: String,
    /// Current connection status.
    status: RwLock<ClientStatus>,
    /// Next request ID.
    next_id: AtomicU64,
    /// Buffered outgoing messages.
    outgoing: Mutex<Vec<LspMessage>>,
    /// Buffered incoming responses.
    responses: Mutex<HashMap<u64, LspResponse>>,
    /// Server capabilities (populated after initialize).
    server_capabilities: RwLock<Option<Value>>,
}

impl LspClient {
    /// Create a new LSP client for the given root URI.
    #[must_use]
    pub fn new(root_uri: &str) -> Arc<Self> {
        Arc::new(Self {
            root_uri: root_uri.to_string(),
            client_name: "remote-code".to_string(),
            client_version: "0.1.0".to_string(),
            status: RwLock::new(ClientStatus::Uninitialized),
            next_id: AtomicU64::new(1),
            outgoing: Mutex::new(Vec::new()),
            responses: Mutex::new(HashMap::new()),
            server_capabilities: RwLock::new(None),
        })
    }

    /// Create a client with custom name and version.
    #[must_use]
    pub fn with_client_info(root_uri: &str, name: &str, version: &str) -> Arc<Self> {
        Arc::new(Self {
            root_uri: root_uri.to_string(),
            client_name: name.to_string(),
            client_version: version.to_string(),
            status: RwLock::new(ClientStatus::Uninitialized),
            next_id: AtomicU64::new(1),
            outgoing: Mutex::new(Vec::new()),
            responses: Mutex::new(HashMap::new()),
            server_capabilities: RwLock::new(None),
        })
    }

    /// Get the root URI.
    #[must_use]
    pub fn root_uri(&self) -> &str {
        &self.root_uri
    }

    /// Get the current client status.
    #[must_use]
    pub fn status(&self) -> ClientStatus {
        *self.status.read()
    }

    /// Get the server capabilities (if initialized).
    #[must_use]
    pub fn server_capabilities(&self) -> Option<Value> {
        self.server_capabilities.read().clone()
    }

    /// Allocate a new request ID.
    fn next_request_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    // ── Lifecycle ───────────────────────────────────────────────────────────

    /// Send the LSP `initialize` request.
    ///
    /// Transitions the client from `Uninitialized` to `Ready`.
    pub fn initialize(&self, name: &str, version: &str) -> Result<u64> {
        {
            let mut status = self.status.write();
            if *status != ClientStatus::Uninitialized {
                anyhow::bail!("Client is not in Uninitialized state (current: {status})");
            }
            *status = ClientStatus::Initializing;
        }

        let id = self.next_request_id();
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": self.root_uri,
            "capabilities": {},
            "clientInfo": {
                "name": name,
                "version": version,
            }
        });

        self.send_request("initialize", Some(params))?;

        // Simulate receiving a response
        let response = LspResponse::success(id, serde_json::json!({"capabilities": {}}));
        {
            let mut responses = self.responses.lock();
            if responses.len() >= MAX_RESPONSES {
                if let Some(&oldest_id) = responses.keys().min() {
                    responses.remove(&oldest_id);
                }
            }
            responses.insert(id, response);
        }

        *self.server_capabilities.write() = Some(serde_json::json!({}));

        // Send initialized notification
        self.send_notification("initialized", Some(serde_json::json!({})))?;

        *self.status.write() = ClientStatus::Ready;
        Ok(id)
    }

    /// Send the LSP `shutdown` request.
    ///
    /// Transitions the client from `Ready` to `Shutdown`.
    pub fn shutdown(&self) -> Result<u64> {
        {
            let status = self.status.read();
            if *status != ClientStatus::Ready {
                anyhow::bail!("Client is not in Ready state (current: {status})");
            }
        }

        *self.status.write() = ClientStatus::ShuttingDown;

        let id = self.send_request("shutdown", None)?;
        *self.status.write() = ClientStatus::Shutdown;
        Ok(id)
    }

    // ── Messaging ───────────────────────────────────────────────────────────

    /// Send an LSP request and return the allocated request ID.
    pub fn send_request(&self, method: &str, params: Option<Value>) -> Result<u64> {
        let id = self.next_request_id();
        let msg = LspMessage::request(id, method, params);
        let mut outgoing = self.outgoing.lock();
        if outgoing.len() >= MAX_OUTGOING_MESSAGES {
            outgoing.remove(0);
        }
        outgoing.push(msg);
        Ok(id)
    }

    /// Send an LSP notification (no response expected).
    pub fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let msg = LspMessage::notification(method, params);
        let mut outgoing = self.outgoing.lock();
        if outgoing.len() >= MAX_OUTGOING_MESSAGES {
            outgoing.remove(0);
        }
        outgoing.push(msg);
        Ok(())
    }

    /// Inject a response for a given request ID (for testing).
    pub fn inject_response(&self, id: u64, response: LspResponse) {
        let mut responses = self.responses.lock();
        if responses.len() >= MAX_RESPONSES {
            if let Some(&oldest_id) = responses.keys().min() {
                responses.remove(&oldest_id);
            }
        }
        responses.insert(id, response);
    }

    /// Get the response for a request ID, if available.
    #[must_use]
    pub fn get_response(&self, id: u64) -> Option<LspResponse> {
        self.responses.lock().remove(&id)
    }

    /// Get all buffered outgoing messages.
    #[must_use]
    pub fn drain_outgoing(&self) -> Vec<LspMessage> {
        self.outgoing.lock().drain(..).collect()
    }

    /// Number of buffered outgoing messages.
    #[must_use]
    pub fn outgoing_count(&self) -> usize {
        self.outgoing.lock().len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_new() {
        let client = LspClient::new("file:///project");
        assert_eq!(client.root_uri(), "file:///project");
        assert_eq!(client.status(), ClientStatus::Uninitialized);
    }

    #[test]
    fn client_with_client_info() {
        let client = LspClient::with_client_info("file:///p", "test", "2.0");
        assert_eq!(client.root_uri(), "file:///p");
    }

    #[test]
    fn client_status_display() {
        assert_eq!(ClientStatus::Uninitialized.to_string(), "uninitialized");
        assert_eq!(ClientStatus::Ready.to_string(), "ready");
        assert_eq!(ClientStatus::Shutdown.to_string(), "shutdown");
    }

    #[test]
    fn client_initialize() {
        let client = LspClient::new("file:///project");
        let id = client.initialize("test", "1.0").expect("init");
        assert!(id > 0);
        assert_eq!(client.status(), ClientStatus::Ready);
        assert!(client.server_capabilities().is_some());
    }

    #[test]
    fn client_initialize_twice_fails() {
        let client = LspClient::new("file:///project");
        client.initialize("test", "1.0").expect("init");
        assert!(client.initialize("test", "1.0").is_err());
    }

    #[test]
    fn client_shutdown() {
        let client = LspClient::new("file:///project");
        client.initialize("test", "1.0").expect("init");
        let id = client.shutdown().expect("shutdown");
        assert!(id > 0);
        assert_eq!(client.status(), ClientStatus::Shutdown);
    }

    #[test]
    fn client_shutdown_without_init_fails() {
        let client = LspClient::new("file:///project");
        assert!(client.shutdown().is_err());
    }

    #[test]
    fn client_send_request() {
        let client = LspClient::new("file:///project");
        let id = client
            .send_request("textDocument/hover", Some(serde_json::json!({})))
            .expect("send");
        assert!(id > 0);
        assert_eq!(client.outgoing_count(), 1);
    }

    #[test]
    fn client_send_notification() {
        let client = LspClient::new("file:///project");
        client
            .send_notification("textDocument/didOpen", Some(serde_json::json!({})))
            .expect("send");
        assert_eq!(client.outgoing_count(), 1);
    }

    #[test]
    fn client_drain_outgoing() {
        let client = LspClient::new("file:///project");
        client.send_notification("n1", None).expect("send");
        client.send_notification("n2", None).expect("send");
        let msgs = client.drain_outgoing();
        assert_eq!(msgs.len(), 2);
        assert_eq!(client.outgoing_count(), 0);
    }

    #[test]
    fn client_inject_and_get_response() {
        let client = LspClient::new("file:///project");
        let resp = LspResponse::success(1, serde_json::json!({"result": true}));
        client.inject_response(1, resp);
        let got = client.get_response(1);
        assert!(got.is_some());
        assert!(got.expect("response").is_success());
    }

    #[test]
    fn client_get_response_missing() {
        let client = LspClient::new("file:///project");
        assert!(client.get_response(999).is_none());
    }

    #[test]
    fn client_initialize_sends_messages() {
        let client = LspClient::new("file:///project");
        client.initialize("test", "1.0").expect("init");
        let msgs = client.drain_outgoing();
        // Should have: initialize request + initialized notification
        assert!(msgs.len() >= 2);
        assert_eq!(msgs[0].method, "initialize");
        assert_eq!(msgs[1].method, "initialized");
    }
}
