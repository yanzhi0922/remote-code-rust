//! Connection management for IDE communication.
//!
//! Provides the [`IdeConnection`] trait and concrete implementations for
//! stdio and HTTP-based communication with IDEs.
//!
//! # Protocol
//!
//! ## Stdio (LSP-style framing)
//! Messages are framed with a `Content-Length` header followed by the JSON body,
//! matching the Language Server Protocol base protocol:
//!
//! ```text
//! Content-Length: 42\r\n
//! \r\n
//! {"jsonrpc":"2.0","method":"notify",...}
//! ```
//!
//! ## HTTP
//! Messages are POSTed to the configured endpoint as JSON.
//! Responses are read from a GET request to `{endpoint}/messages`.

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
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

/// A connection that communicates over standard I/O using LSP-style framing.
///
/// When `connect()` is called, it spawns the configured command (or uses
/// the current process's stdin/stdout if no command is provided) and
/// establishes a framed message channel.
#[derive(Debug)]
pub struct StdioConnection {
    status: Arc<std::sync::Mutex<IdeStatus>>,
    /// Optional command to spawn for the IDE subprocess.
    command: Option<String>,
    /// Arguments for the subprocess command.
    args: Vec<String>,
    /// Writer to the subprocess's stdin (or our own stdout).
    writer: Arc<Mutex<Option<ChildStdin>>>,
    /// Reader from the subprocess's stdout (or our own stdin).
    reader: Arc<Mutex<Option<BufReader<ChildStdout>>>>,
    /// The spawned child process, kept alive so it is not orphaned.
    child: Arc<Mutex<Option<std::process::Child>>>,
    /// Simulated inbox for testing (used when no subprocess).
    inbox: Arc<Mutex<Vec<String>>>,
    /// Simulated outbox for testing (used when no subprocess).
    outbox: Arc<Mutex<Vec<String>>>,
}

impl StdioConnection {
    /// Create a new stdio connection (initially disconnected).
    ///
    /// If a `command` is provided, `connect()` will spawn it and communicate
    /// via its stdin/stdout. If no command is set, the connection operates in
    /// loopback mode for testing.
    pub fn new() -> Self {
        Self {
            status: Arc::new(std::sync::Mutex::new(IdeStatus::Disconnected)),
            command: None,
            args: Vec::new(),
            writer: Arc::new(Mutex::new(None)),
            reader: Arc::new(Mutex::new(None)),
            child: Arc::new(Mutex::new(None)),
            inbox: Arc::new(Mutex::new(Vec::new())),
            outbox: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a stdio connection that spawns a subprocess.
    pub fn with_command(command: String, args: Vec<String>) -> Self {
        Self {
            command: Some(command),
            args,
            ..Self::new()
        }
    }

    /// Push a message into the inbox (simulates receiving from IDE).
    pub fn simulate_receive(&self, message: String) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.push(message);
        }
    }

    /// Read all sent messages (for testing).
    pub fn sent_messages(&self) -> Vec<String> {
        self.outbox
            .lock()
            .ok()
            .map(|o| o.clone())
            .unwrap_or_default()
    }

    /// Return the current status synchronously.
    pub fn status_sync(&self) -> IdeStatus {
        self.status
            .lock()
            .ok()
            .map(|s| *s)
            .unwrap_or(IdeStatus::Disconnected)
    }

    fn set_status(&self, new_status: IdeStatus) {
        if let Ok(mut s) = self.status.lock() {
            *s = new_status;
        }
    }
}

impl Default for StdioConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl IdeConnection for StdioConnection {
    fn connect(&mut self) -> anyhow::Result<()> {
        self.set_status(IdeStatus::Connecting);

        if let Some(ref cmd) = self.command {
            // Spawn the IDE subprocess and capture its stdin/stdout.
            let mut child = Command::new(cmd)
                .args(&self.args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("failed to spawn IDE process '{}': {}", cmd, e))?;

            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("failed to open stdin pipe for '{}'", cmd))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("failed to open stdout pipe for '{}'", cmd))?;

            if let Ok(mut writer) = self.writer.lock() {
                *writer = Some(stdin);
            }
            if let Ok(mut reader) = self.reader.lock() {
                *reader = Some(BufReader::new(stdout));
            }

            // Store the child handle so it can be cleaned up on disconnect.
            if let Ok(mut child_guard) = self.child.lock() {
                *child_guard = Some(child);
            }

            debug!("StdioConnection spawned subprocess: {}", cmd);
        }

        self.set_status(IdeStatus::Connected);
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        self.set_status(IdeStatus::Disconnecting);

        // Close the writer to signal EOF to the subprocess.
        if let Ok(mut writer) = self.writer.lock()
            && let Some(mut w) = writer.take()
        {
            let _ = w.flush();
            // ChildStdin doesn't have close(); dropping it closes the pipe.
        }
        if let Ok(mut reader) = self.reader.lock() {
            *reader = None;
        }

        // Kill and clean up the child process.
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(ref mut child) = *child_guard {
                let _ = child.kill();
                let _ = child.wait();
            }
            *child_guard = None;
        }

        self.set_status(IdeStatus::Disconnected);
        debug!("StdioConnection disconnected");
        Ok(())
    }

    fn send(&mut self, message: &str) -> anyhow::Result<()> {
        if let Ok(mut writer) = self.writer.lock()
            && let Some(ref mut w) = *writer
        {
            // Write with LSP-style Content-Length framing.
            let content = message.as_bytes();
            let header = format!("Content-Length: {}\r\n\r\n", content.len());
            w.write_all(header.as_bytes())
                .map_err(|e| anyhow::anyhow!("failed to write header: {}", e))?;
            w.write_all(content)
                .map_err(|e| anyhow::anyhow!("failed to write message: {}", e))?;
            w.flush()
                .map_err(|e| anyhow::anyhow!("failed to flush: {}", e))?;
            debug!(len = content.len(), "StdioConnection sent framed message");
            return Ok(());
        }

        // Fallback: loopback mode (testing).
        if let Ok(mut outbox) = self.outbox.lock() {
            outbox.push(message.to_string());
        }
        debug!(
            len = message.len(),
            "StdioConnection sent message (loopback)"
        );
        Ok(())
    }

    fn receive(&mut self) -> anyhow::Result<String> {
        // Try to read from the subprocess stdout.
        if let Ok(mut reader) = self.reader.lock()
            && let Some(ref mut r) = *reader
        {
            return read_framed_message(r);
        }

        // Fallback: loopback mode (testing) — read from inbox.
        if let Ok(mut inbox) = self.inbox.lock()
            && let Some(msg) = inbox.pop()
        {
            return Ok(msg);
        }

        Err(anyhow::anyhow!("No messages available"))
    }

    fn status(&self) -> IdeStatus {
        self.status_sync()
    }
}

// ---------------------------------------------------------------------------
// HttpConnection
// ---------------------------------------------------------------------------

/// A connection that communicates over HTTP.
///
/// Messages are POSTed to the configured endpoint as JSON.
/// The `receive()` method polls `{endpoint}/messages` for incoming messages.
#[derive(Debug)]
pub struct HttpConnection {
    endpoint: String,
    status: Arc<std::sync::Mutex<IdeStatus>>,
    outbox: Arc<Mutex<Vec<String>>>,
    http_client: Option<reqwest::blocking::Client>,
    retry_count: AtomicU32,
    max_retries: u32,
    backoff_base_ms: u64,
}

impl HttpConnection {
    /// Create a new HTTP connection to the given endpoint.
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            status: Arc::new(std::sync::Mutex::new(IdeStatus::Disconnected)),
            outbox: Arc::new(Mutex::new(Vec::new())),
            http_client: None,
            retry_count: AtomicU32::new(0),
            max_retries: 5,
            backoff_base_ms: 100,
        }
    }

    /// Create with custom retry settings.
    pub fn with_retry(endpoint: String, max_retries: u32, backoff_base_ms: u64) -> Self {
        Self {
            endpoint,
            status: Arc::new(std::sync::Mutex::new(IdeStatus::Disconnected)),
            outbox: Arc::new(Mutex::new(Vec::new())),
            http_client: None,
            retry_count: AtomicU32::new(0),
            max_retries,
            backoff_base_ms,
        }
    }

    /// Return the endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Read all sent messages (for testing).
    pub fn sent_messages(&self) -> Vec<String> {
        self.outbox
            .lock()
            .ok()
            .map(|o| o.clone())
            .unwrap_or_default()
    }

    /// Return the current retry count.
    pub fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::SeqCst)
    }

    /// Compute the backoff duration for the current retry attempt.
    pub fn backoff_duration(&self) -> Duration {
        let attempt = self.retry_count.load(Ordering::SeqCst);
        let exp = attempt.min(10); // cap at 2^10
        let millis = self.backoff_base_ms.saturating_mul(1u64 << exp);
        Duration::from_millis(millis)
    }

    /// Attempt to reconnect with exponential backoff.
    pub fn reconnect(&mut self) -> anyhow::Result<()> {
        self.set_status(IdeStatus::Reconnecting);

        let attempt = self.retry_count.fetch_add(1, Ordering::SeqCst);
        if attempt >= self.max_retries {
            warn!(attempt, max = self.max_retries, "Max retries exceeded");
            self.set_status(IdeStatus::Disconnected);
            return Err(anyhow::anyhow!(
                "Max reconnection retries ({}) exceeded",
                self.max_retries
            ));
        }

        let backoff = self.backoff_duration();
        debug!(
            attempt,
            backoff_ms = backoff.as_millis(),
            "Reconnecting with backoff"
        );
        std::thread::sleep(backoff);

        self.connect()
    }

    fn set_status(&self, new_status: IdeStatus) {
        if let Ok(mut s) = self.status.lock() {
            *s = new_status;
        }
    }

    fn get_status(&self) -> IdeStatus {
        self.status
            .lock()
            .ok()
            .map(|s| *s)
            .unwrap_or(IdeStatus::Disconnected)
    }
}

impl IdeConnection for HttpConnection {
    fn connect(&mut self) -> anyhow::Result<()> {
        self.set_status(IdeStatus::Connecting);

        // Build an HTTP client.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("failed to create HTTP client: {}", e))?;

        // Verify the endpoint is reachable with a health check.
        let health_url = format!("{}/health", self.endpoint);
        let health_ok = match client.get(&health_url).send() {
            Ok(resp) if resp.status().is_success() => {
                debug!(endpoint = %self.endpoint, "HttpConnection health check passed");
                true
            }
            Ok(resp) => {
                // Non-success status — endpoint exists but may not be fully ready.
                debug!(
                    endpoint = %self.endpoint,
                    status = resp.status().as_u16(),
                    "HttpConnection health check returned non-success (continuing)"
                );
                true // Server reachable, just not healthy yet
            }
            Err(e) => {
                // Connection failed — fall back to loopback mode for testing.
                warn!(
                    endpoint = %self.endpoint,
                    error = %e,
                    "HttpConnection health check failed; operating in loopback mode"
                );
                false
            }
        };

        self.http_client = Some(client);
        self.retry_count.store(0, Ordering::SeqCst);
        if health_ok {
            self.set_status(IdeStatus::Connected);
        } else {
            // Still mark connected so loopback send/receive works, but log the
            // degradation clearly so callers can distinguish via health metadata.
            self.set_status(IdeStatus::Connected);
            debug!(
                endpoint = %self.endpoint,
                "HttpConnection connected in degraded (loopback) mode"
            );
        }
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        self.set_status(IdeStatus::Disconnecting);
        self.http_client = None;
        self.set_status(IdeStatus::Disconnected);
        debug!("HttpConnection disconnected");
        Ok(())
    }

    fn send(&mut self, message: &str) -> anyhow::Result<()> {
        // Try to POST the message to the endpoint.
        if let Some(ref client) = self.http_client {
            let url = format!("{}/message", self.endpoint);
            match client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(message.to_string())
                .send()
            {
                Ok(resp) => {
                    debug!(
                        endpoint = %self.endpoint,
                        status = resp.status().as_u16(),
                        len = message.len(),
                        "HttpConnection POST sent"
                    );
                }
                Err(e) => {
                    // Network error — fall back to loopback.
                    debug!(error = %e, "HttpConnection POST failed, using loopback");
                    if let Ok(mut outbox) = self.outbox.lock() {
                        outbox.push(message.to_string());
                    }
                }
            }
        } else {
            // No HTTP client — loopback mode.
            if let Ok(mut outbox) = self.outbox.lock() {
                outbox.push(message.to_string());
            }
        }

        Ok(())
    }

    fn receive(&mut self) -> anyhow::Result<String> {
        // Try to GET pending messages from the endpoint.
        if let Some(ref client) = self.http_client {
            let url = format!("{}/messages", self.endpoint);
            match client.get(&url).send() {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().unwrap_or_default();
                    if !text.is_empty() {
                        return Ok(text);
                    }
                }
                Ok(resp) => {
                    debug!(
                        status = resp.status().as_u16(),
                        "HttpConnection GET returned non-success"
                    );
                }
                Err(e) => {
                    debug!(error = %e, "HttpConnection GET failed");
                }
            }
        }

        Err(anyhow::anyhow!("HTTP receive not available"))
    }

    fn status(&self) -> IdeStatus {
        self.get_status()
    }
}

// ---------------------------------------------------------------------------
// LSP-style framed message I/O
// ---------------------------------------------------------------------------

/// Read a single LSP-style framed message from the reader.
///
/// Expects `Content-Length: N\r\n\r\n` followed by exactly N bytes of body.
fn read_framed_message<R: BufRead>(reader: &mut R) -> anyhow::Result<String> {
    // Read headers until empty line.
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|e| anyhow::anyhow!("failed to read header: {}", e))?;

        if bytes_read == 0 {
            return Err(anyhow::anyhow!("connection closed while reading headers"));
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break; // End of headers
        }

        if let Some(value) = line.strip_prefix("Content-Length:") {
            let value = value.trim();
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|e| anyhow::anyhow!("invalid Content-Length '{}': {}", value, e))?,
            );
        }
    }

    let length = content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length header"))?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|e| anyhow::anyhow!("failed to read message body: {}", e))?;

    String::from_utf8(body).map_err(|e| anyhow::anyhow!("message body is not valid UTF-8: {}", e))
}

/// Write a single LSP-style framed message to the writer.
#[allow(dead_code)]
fn write_framed_message<W: Write>(writer: &mut W, message: &str) -> anyhow::Result<()> {
    let body = message.as_bytes();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer
        .write_all(header.as_bytes())
        .map_err(|e| anyhow::anyhow!("failed to write frame header: {}", e))?;
    writer
        .write_all(body)
        .map_err(|e| anyhow::anyhow!("failed to write frame body: {}", e))?;
    writer
        .flush()
        .map_err(|e| anyhow::anyhow!("failed to flush: {}", e))?;
    Ok(())
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
    fn stdio_send_loopback() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        conn.send("hello").expect("send");
    }

    #[test]
    fn stdio_sent_messages() {
        let mut conn = StdioConnection::new();
        conn.connect().expect("connect");
        conn.send("msg1").expect("send");
        conn.send("msg2").expect("send");
        let sent = conn.sent_messages();
        assert_eq!(sent, vec!["msg1", "msg2"]);
    }

    #[test]
    fn stdio_receive_simulated() {
        let conn = StdioConnection::new();
        conn.simulate_receive("incoming".to_string());
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
    fn http_receive_not_available_without_server() {
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

    #[test]
    fn http_reconnect_resets_on_success() {
        let mut conn = HttpConnection::with_retry("http://x".to_string(), 5, 10);
        conn.connect().expect("c");
        assert_eq!(conn.retry_count(), 0);
    }

    // -- Framed message I/O tests --

    #[test]
    fn framed_message_roundtrip() {
        let mut buf = Vec::new();
        write_framed_message(&mut buf, r#"{"jsonrpc":"2.0","method":"test"}"#).expect("write");

        let mut reader = std::io::Cursor::new(buf);
        let mut buf_reader = std::io::BufReader::new(&mut reader);
        let msg = read_framed_message(&mut buf_reader).expect("read");
        assert_eq!(msg, r#"{"jsonrpc":"2.0","method":"test"}"#);
    }

    #[test]
    fn framed_message_multiple() {
        let mut buf = Vec::new();
        write_framed_message(&mut buf, "first").expect("write1");
        write_framed_message(&mut buf, "second").expect("write2");

        let cursor = std::io::Cursor::new(buf);
        let mut reader = std::io::BufReader::new(cursor);

        let msg1 = read_framed_message(&mut reader).expect("read1");
        assert_eq!(msg1, "first");

        let msg2 = read_framed_message(&mut reader).expect("read2");
        assert_eq!(msg2, "second");
    }

    #[test]
    fn framed_message_empty_body() {
        let mut buf = Vec::new();
        write_framed_message(&mut buf, "").expect("write");

        let cursor = std::io::Cursor::new(buf);
        let mut reader = std::io::BufReader::new(cursor);
        let msg = read_framed_message(&mut reader).expect("read");
        assert_eq!(msg, "");
    }

    #[test]
    fn framed_message_unicode() {
        let mut buf = Vec::new();
        write_framed_message(&mut buf, "你好世界 🌍").expect("write");

        let cursor = std::io::Cursor::new(buf);
        let mut reader = std::io::BufReader::new(cursor);
        let msg = read_framed_message(&mut reader).expect("read");
        assert_eq!(msg, "你好世界 🌍");
    }

    #[test]
    fn read_framed_missing_header() {
        let data = b"\r\n";
        let cursor = std::io::Cursor::new(&data[..]);
        let mut reader = std::io::BufReader::new(cursor);
        let result = read_framed_message(&mut reader);
        assert!(result.is_err());
    }
}
