//! Connection management for IDE communication.
//!
//! Provides the [`IdeConnection`] trait and concrete implementations for
//! stdio and HTTP-based communication with IDEs.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Status of an IDE connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeStatus {
    /// Not connected.
    Disconnected,
    /// Connection is being established.
    Connecting,
    /// Performing handshake.
    Handshaking,
    /// Connected and ready to communicate.
    Connected,
    /// Connection is being closed.
    Disconnecting,
    /// Connection failed; will attempt reconnection.
    Reconnecting,
}

impl std::fmt::Display for IdeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdeStatus::Disconnected => write!(f, "disconnected"),
            IdeStatus::Connecting => write!(f, "connecting"),
            IdeStatus::Handshaking => write!(f, "handshaking"),
            IdeStatus::Connected => write!(f, "connected"),
            IdeStatus::Disconnecting => write!(f, "disconnecting"),
            IdeStatus::Reconnecting => write!(f, "reconnecting"),
        }
    }
}

/// Trait for IDE connection transports.
pub trait IdeConnection: Send + Sync + std::fmt::Debug {
    /// Establish the connection.
    fn connect(&mut self) -> anyhow::Result<()>;

    /// Close the connection.
    fn disconnect(&mut self) -> anyhow::Result<()>;

    /// Send a raw string message.
    fn send(&mut self, message: &str) -> anyhow::Result<()>;

    /// Receive a raw string message (blocking).
    fn receive(&mut self) -> anyhow::Result<String>;

    /// Return the current connection status.
    fn status(&self) -> IdeStatus;
}

// ---------------------------------------------------------------------------
// StdioConnection
// ---------------------------------------------------------------------------

/// A connection that communicates over standard I/O (simulated for testing).
#[derive(Debug)]
pub struct StdioConnection {
    status: Arc<RwLock<IdeStatus>>,
    /// Simulated inbox for received messages.
    inbox: Arc<RwLock<Vec<String>>>,
    /// Simulated outbox for sent messages.
    outbox: Arc<RwLock<Vec<String>>>,
}

impl StdioConnection {
    /// Create a new stdio connection (initially disconnected).
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(IdeStatus::Disconnected)),
            inbox: Arc::new(RwLock::new(Vec::new())),
            outbox: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Push a message into the inbox (simulates receiving from IDE).
    pub async fn simulate_receive(&self, message: String) {
        self.inbox.write().await.push(message);
    }

    /// Read all sent messages (for testing).
    pub async fn sent_messages(&self) -> Vec<String> {
        self.outbox.read().await.clone()
    }

    /// Return the current status asynchronously.
    pub async fn status_async(&self) -> IdeStatus {
        *self.status.read().await
    }
}

impl Default for StdioConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl IdeConnection for StdioConnection {
    fn connect(&mut self) -> anyhow::Result<()> {
        // In a real implementation, this would set up stdin/stdout pipes.
        // Here we just update status.
        let status = self.status.clone();
        // We can't use async here, so we use try_write.
        if let Ok(mut s) = status.try_write() {
            *s = IdeStatus::Connected;
        }
        debug!("StdioConnection connected");
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Ok(mut s) = self.status.try_write() {
            *s = IdeStatus::Disconnected;
        }
        debug!("StdioConnection disconnected");
        Ok(())
    }

    fn send(&mut self, message: &str) -> anyhow::Result<()> {
        if let Ok(mut outbox) = self.outbox.try_write() {
            outbox.push(message.to_string());
        }
        debug!(len = message.len(), "StdioConnection sent message");
        Ok(())
    }

    fn receive(&mut self) -> anyhow::Result<String> {
        if let Ok(mut inbox) = self.inbox.try_write()
            && let Some(msg) = inbox.pop()
        {
            return Ok(msg);
        }
        Err(anyhow::anyhow!("No messages available"))
    }

    fn status(&self) -> IdeStatus {
        self.status.try_read().map(|s| *s).unwrap_or(IdeStatus::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// HttpConnection
// ---------------------------------------------------------------------------

/// A connection that communicates over HTTP (simulated for testing).
#[derive(Debug)]
pub struct HttpConnection {
    endpoint: String,
    status: Arc<RwLock<IdeStatus>>,
    outbox: Arc<RwLock<Vec<String>>>,
    retry_count: AtomicU64,
    max_retries: u32,
    backoff_base_ms: u64,
}

impl HttpConnection {
    /// Create a new HTTP connection to the given endpoint.
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            status: Arc::new(RwLock::new(IdeStatus::Disconnected)),
            outbox: Arc::new(RwLock::new(Vec::new())),
            retry_count: AtomicU64::new(0),
            max_retries: 5,
            backoff_base_ms: 100,
        }
    }

    /// Create with custom retry settings.
    pub fn with_retry(endpoint: String, max_retries: u32, backoff_base_ms: u64) -> Self {
        Self {
            endpoint,
            status: Arc::new(RwLock::new(IdeStatus::Disconnected)),
            outbox: Arc::new(RwLock::new(Vec::new())),
            retry_count: AtomicU64::new(0),
            max_retries,
            backoff_base_ms,
        }
    }

    /// Return the endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Read all sent messages (for testing).
    pub async fn sent_messages(&self) -> Vec<String> {
        self.outbox.read().await.clone()
    }

    /// Return the current retry count.
    pub fn retry_count(&self) -> u64 {
        self.retry_count.load(Ordering::SeqCst)
    }

    /// Compute the backoff duration for the current retry attempt.
    pub fn backoff_duration(&self) -> Duration {
        let attempt = self.retry_count.load(Ordering::SeqCst);
        let millis = self.backoff_base_ms * 2u64.pow(u32::try_from(attempt).unwrap_or(0));
        Duration::from_millis(millis)
    }

    /// Attempt to reconnect with exponential backoff.
    pub async fn reconnect(&mut self) -> anyhow::Result<()> {
        if let Ok(mut s) = self.status.try_write() {
            *s = IdeStatus::Reconnecting;
        }

        let attempt = self.retry_count.fetch_add(1, Ordering::SeqCst);
        if attempt >= self.max_retries as u64 {
            warn!(attempt, max = self.max_retries, "Max retries exceeded");
            if let Ok(mut s) = self.status.try_write() {
                *s = IdeStatus::Disconnected;
            }
            return Err(anyhow::anyhow!("Max reconnection retries ({}) exceeded", self.max_retries));
        }

        let backoff = self.backoff_duration();
        debug!(attempt, backoff_ms = backoff.as_millis(), "Reconnecting with backoff");
        tokio::time::sleep(backoff).await;

        self.connect()
    }
}

impl IdeConnection for HttpConnection {
    fn connect(&mut self) -> anyhow::Result<()> {
        // Simulated: in a real impl this would make an HTTP handshake.
        if let Ok(mut s) = self.status.try_write() {
            *s = IdeStatus::Connected;
        }
        self.retry_count.store(0, Ordering::SeqCst);
        debug!(endpoint = %self.endpoint, "HttpConnection connected");
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Ok(mut s) = self.status.try_write() {
            *s = IdeStatus::Disconnected;
        }
        debug!("HttpConnection disconnected");
        Ok(())
    }

    fn send(&mut self, message: &str) -> anyhow::Result<()> {
        if let Ok(mut outbox) = self.outbox.try_write() {
            outbox.push(message.to_string());
        }
        debug!(endpoint = %self.endpoint, len = message.len(), "HttpConnection sent");
        Ok(())
    }

    fn receive(&mut self) -> anyhow::Result<String> {
        // HTTP is request-response; receiving would be the response body.
        Err(anyhow::anyhow!("HTTP receive not supported in simulation"))
    }

    fn status(&self) -> IdeStatus {
        self.status.try_read().map(|s| *s).unwrap_or(IdeStatus::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- IdeStatus tests --

    #[test]
    fn ide_status_display() {
        assert_eq!(IdeStatus::Connected.to_string(), "connected");
        assert_eq!(IdeStatus::Disconnected.to_string(), "disconnected");
        assert_eq!(IdeStatus::Reconnecting.to_string(), "reconnecting");
    }

    #[test]
    fn ide_status_serde() {
        let s = IdeStatus::Handshaking;
        let json = serde_json::to_string(&s).expect("s");
        assert_eq!(json, "\"handshaking\"");
        let back: IdeStatus = serde_json::from_str(&json).expect("d");
        assert_eq!(back, IdeStatus::Handshaking);
    }

    // -- StdioConnection tests --

    #[test]
    fn stdio_new_is_disconnected() {
        let conn = StdioConnection::new();
        assert_eq!(conn.status(), IdeStatus::Disconnected);
    }

    #[test]
    fn stdio_connect() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        assert_eq!(conn.status(), IdeStatus::Connected);
    }

    #[test]
    fn stdio_disconnect() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        conn.disconnect().expect("disconnect");
        assert_eq!(conn.status(), IdeStatus::Disconnected);
    }

    #[test]
    fn stdio_send() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        conn.send("hello").expect("send");
    }

    #[tokio::test]
    async fn stdio_sent_messages() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        conn.send("msg1").expect("send");
        conn.send("msg2").expect("send");
        let sent = conn.sent_messages().await;
        assert_eq!(sent, vec!["msg1", "msg2"]);
    }

    #[tokio::test]
    async fn stdio_receive_simulated() {
        let conn = StdioConnection::new();
        conn.simulate_receive("incoming".to_string()).await;
        let mut conn = conn;
        let msg = conn.receive().expect("receive");
        assert_eq!(msg, "incoming");
    }

    #[test]
    fn stdio_receive_empty_fails() {
        let mut conn = StdioConnection::new();
        let result = conn.receive();
        assert!(result.is_err());
    }

    #[test]
    fn stdio_default() {
        let conn = StdioConnection::default();
        assert_eq!(conn.status(), IdeStatus::Disconnected);
    }

    // -- HttpConnection tests --

    #[test]
    fn http_new_is_disconnected() {
        let conn = HttpConnection::new("http://localhost:8080".to_string());
        assert_eq!(conn.status(), IdeStatus::Disconnected);
    }

    #[test]
    fn http_connect() {
        let mut conn = HttpConnection::new("http://localhost:8080".to_string());
        conn.connect().expect("connect");
        assert_eq!(conn.status(), IdeStatus::Connected);
    }

    #[test]
    fn http_disconnect() {
        let mut conn = HttpConnection::new("http://localhost:8080".to_string());
        conn.connect().expect("connect");
        conn.disconnect().expect("disconnect");
        assert_eq!(conn.status(), IdeStatus::Disconnected);
    }

    #[test]
    fn http_send() {
        let mut conn = HttpConnection::new("http://localhost:8080".to_string());
        conn.connect().expect("connect");
        conn.send("data").expect("send");
    }

    #[test]
    fn http_receive_not_supported() {
        let mut conn = HttpConnection::new("http://localhost:8080".to_string());
        conn.connect().expect("connect");
        assert!(conn.receive().is_err());
    }

    #[test]
    fn http_endpoint() {
        let conn = HttpConnection::new("http://localhost:9999".to_string());
        assert_eq!(conn.endpoint(), "http://localhost:9999");
    }

    #[test]
    fn http_backoff_increases() {
        let conn = HttpConnection::with_retry("http://x".to_string(), 5, 100);
        assert_eq!(conn.backoff_duration(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn http_reconnect_resets_on_success() {
        let mut conn = HttpConnection::with_retry("http://x".to_string(), 5, 10);
        conn.connect().expect("c");
        assert_eq!(conn.retry_count(), 0);
    }
}
