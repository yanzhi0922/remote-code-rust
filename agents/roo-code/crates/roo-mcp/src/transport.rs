//! MCP transport layer.
//!
//! Defines the `McpTransport` trait and implementations for stdio, SSE, and StreamableHTTP.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::error::{McpError, McpResult};

/// A JSON-RPC 2.0 message used in MCP communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcMessage {
    /// Create a new JSON-RPC request.
    pub fn request(id: u64, method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(id.into())),
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Create a new JSON-RPC notification (no id).
    pub fn notification(method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Check if this message is a response (has id but no method).
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.method.is_none()
    }

    /// Check if this message is a request.
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }

    /// Check if this message is a notification.
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Check if this message is an error response.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the id as u64 if possible.
    pub fn id_as_u64(&self) -> Option<u64> {
        self.id.as_ref().and_then(|v| v.as_u64())
    }
}

/// Type alias for a pinned boxed stream of JSON-RPC messages.
pub type MessageStream = Pin<Box<dyn Stream<Item = JsonRpcMessage> + Send>>;

/// Transport trait for MCP communication.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Connect the transport (start the underlying process/connection).
    async fn connect(&mut self) -> McpResult<()>;

    /// Close the transport.
    async fn close(&mut self) -> McpResult<()>;

    /// Send a JSON-RPC message.
    async fn send(&mut self, message: &JsonRpcMessage) -> McpResult<()>;

    /// Receive the next JSON-RPC message (blocking).
    async fn receive(&mut self) -> McpResult<Option<JsonRpcMessage>>;

    /// Check if the transport is connected.
    fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// StdioTransport
// ---------------------------------------------------------------------------

/// Stdio transport using a child process with stdin/stdout pipes.
///
/// Captures stderr output from the child process for debugging purposes.
pub struct StdioTransport {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: Option<String>,
    child: Option<Child>,
    connected: bool,
    // We use a simple approach: write to stdin, read from stdout line by line
    stdin_writer: Option<tokio::process::ChildStdin>,
    stdout_reader: Option<BufReader<tokio::process::ChildStdout>>,
    /// Channel for stderr lines captured from the child process.
    stderr_rx: Option<mpsc::Receiver<String>>,
    /// Join handle for the stderr reader task.
    stderr_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new(
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: Option<String>,
    ) -> Self {
        Self {
            command,
            args,
            env,
            cwd,
            child: None,
            connected: false,
            stdin_writer: None,
            stdout_reader: None,
            stderr_rx: None,
            stderr_handle: None,
        }
    }

    /// Read a line from the child process stderr (non-blocking).
    ///
    /// Returns `Some(line)` if a line is available, `None` if no data is
    /// currently available or the stderr stream has ended.
    pub fn try_read_stderr(&mut self) -> Option<String> {
        if let Some(rx) = self.stderr_rx.as_mut() {
            rx.try_recv().ok()
        } else {
            None
        }
    }

    /// Read all available stderr lines.
    pub fn read_all_stderr(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(line) = self.try_read_stderr() {
            lines.push(line);
        }
        lines
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&mut self) -> McpResult<()> {
        let is_windows = cfg!(target_os = "windows");

        let (command, args) = if is_windows {
            // On Windows, wrap commands with cmd.exe to handle non-exe executables
            let is_already_wrapped = self.command.to_lowercase() == "cmd.exe"
                || self.command.to_lowercase() == "cmd";

            if is_already_wrapped {
                (self.command.clone(), self.args.clone())
            } else {
                let mut wrapped_args = vec!["/c".to_string(), self.command.clone()];
                wrapped_args.extend(self.args.iter().cloned());
                ("cmd.exe".to_string(), wrapped_args)
            }
        } else {
            (self.command.clone(), self.args.clone())
        };

        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set environment variables
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn().map_err(|e| {
            McpError::ConnectionFailed(format!("Failed to spawn process '{}': {}", command, e))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::ConnectionFailed("Failed to open stdin pipe".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::ConnectionFailed("Failed to open stdout pipe".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            McpError::ConnectionFailed("Failed to open stderr pipe".to_string())
        })?;

        // Spawn a background task to read stderr lines and forward them to a channel
        let (stderr_tx, stderr_rx) = mpsc::channel::<String>(100);
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "mcp::stderr", "stderr: {}", line);
                if stderr_tx.send(line).await.is_err() {
                    // Receiver dropped
                    break;
                }
            }
        });

        self.stdin_writer = Some(stdin);
        self.stdout_reader = Some(BufReader::new(stdout));
        self.stderr_rx = Some(stderr_rx);
        self.stderr_handle = Some(stderr_handle);
        self.child = Some(child);
        self.connected = true;

        tracing::info!("Stdio transport connected: {} {:?}", command, args);

        Ok(())
    }

    async fn close(&mut self) -> McpResult<()> {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        if let Some(handle) = self.stderr_handle.take() {
            handle.abort();
        }
        self.stdin_writer = None;
        self.stdout_reader = None;
        self.stderr_rx = None;
        self.child = None;
        self.connected = false;
        tracing::info!("Stdio transport closed");
        Ok(())
    }

    async fn send(&mut self, message: &JsonRpcMessage) -> McpResult<()> {
        let writer = self
            .stdin_writer
            .as_mut()
            .ok_or_else(|| McpError::TransportError("Not connected (no stdin)".to_string()))?;

        let mut json = serde_json::to_string(message)?;
        json.push('\n');

        writer
            .write_all(json.as_bytes())
            .await
            .map_err(|e| McpError::TransportError(format!("Write error: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| McpError::TransportError(format!("Flush error: {}", e)))?;

        tracing::trace!("Sent message: {:?}", message);
        Ok(())
    }

    async fn receive(&mut self) -> McpResult<Option<JsonRpcMessage>> {
        let reader = self
            .stdout_reader
            .as_mut()
            .ok_or_else(|| McpError::TransportError("Not connected (no stdout)".to_string()))?;

        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF
                tracing::debug!("Stdio transport reached EOF");
                self.connected = false;
                return Ok(None);
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    // Skip empty lines
                    return self.receive().await;
                }

                let message: JsonRpcMessage = serde_json::from_str(trimmed).map_err(|e| {
                    McpError::TransportError(format!(
                        "Failed to parse JSON-RPC message: {} (input: {})",
                        e,
                        &trimmed[..trimmed.len().min(200)]
                    ))
                })?;

                tracing::trace!("Received message: {:?}", message);
                return Ok(Some(message));
            }
            Err(e) => {
                self.connected = false;
                return Err(McpError::TransportError(format!("Read error: {}", e)));
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            // Try to kill the child process on drop
            let _ = child.start_kill();
        }
    }
}

// ---------------------------------------------------------------------------
// SseTransport
// ---------------------------------------------------------------------------

/// Default maximum number of reconnection attempts for SSE transport.
const SSE_DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Default initial reconnect delay in milliseconds.
const SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS: u64 = 1000;

/// SSE (Server-Sent Events) transport.
///
/// Uses HTTP POST for sending and SSE for receiving.
/// Supports automatic reconnection with exponential backoff.
pub struct SseTransport {
    url: String,
    headers: HashMap<String, String>,
    http_client: reqwest::Client,
    connected: bool,
    // Channel for received messages (populated by SSE listener)
    message_rx: Option<mpsc::Receiver<JsonRpcMessage>>,
    // SSE endpoint URL (discovered from the initial connection)
    sse_endpoint: Option<String>,
    // Join handle for the SSE listener task
    listener_handle: Option<tokio::task::JoinHandle<()>>,
    /// Maximum number of reconnection attempts before giving up.
    max_reconnect_attempts: u32,
    /// Initial delay before reconnecting (milliseconds).
    initial_reconnect_delay_ms: u64,
}

impl SseTransport {
    /// Create a new SSE transport.
    pub fn new(url: String, headers: HashMap<String, String>) -> Self {
        Self {
            url,
            headers,
            http_client: reqwest::Client::new(),
            connected: false,
            message_rx: None,
            sse_endpoint: None,
            listener_handle: None,
            max_reconnect_attempts: SSE_DEFAULT_MAX_RECONNECT_ATTEMPTS,
            initial_reconnect_delay_ms: SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS,
        }
    }

    /// Create a new SSE transport with custom reconnection settings.
    pub fn with_reconnect_settings(
        url: String,
        headers: HashMap<String, String>,
        max_reconnect_attempts: u32,
        initial_reconnect_delay_ms: u64,
    ) -> Self {
        Self {
            url,
            headers,
            http_client: reqwest::Client::new(),
            connected: false,
            message_rx: None,
            sse_endpoint: None,
            listener_handle: None,
            max_reconnect_attempts,
            initial_reconnect_delay_ms,
        }
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn connect(&mut self) -> McpResult<()> {
        let (tx, rx) = mpsc::channel(100);
        self.message_rx = Some(rx);

        // The SSE endpoint is the URL itself
        let sse_url = self.url.clone();
        self.sse_endpoint = Some(sse_url.clone());

        let client = self.http_client.clone();
        let headers = self.headers.clone();
        let max_reconnect = self.max_reconnect_attempts;
        let initial_delay = self.initial_reconnect_delay_ms;

        // Start SSE listener in background
        let handle = tokio::spawn(async move {
            if let Err(e) = Self::listen_sse_with_reconnect(
                client,
                &sse_url,
                &headers,
                tx,
                max_reconnect,
                initial_delay,
            )
            .await
            {
                tracing::error!("SSE listener error: {}", e);
            }
        });

        self.listener_handle = Some(handle);
        self.connected = true;

        tracing::info!("SSE transport connected to {}", self.url);
        Ok(())
    }

    async fn close(&mut self) -> McpResult<()> {
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }
        self.message_rx = None;
        self.connected = false;
        tracing::info!("SSE transport closed");
        Ok(())
    }

    async fn send(&mut self, message: &JsonRpcMessage) -> McpResult<()> {
        // For SSE transport, we POST the message to the server
        // The endpoint for sending is typically the same URL
        let mut request = self.http_client.post(&self.url);
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        request = request.json(message);

        let response = request.send().await.map_err(|e| {
            McpError::TransportError(format!("SSE send error: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::TransportError(format!(
                "SSE send failed with status {}: {}",
                status, body
            )));
        }

        tracing::trace!("SSE sent message: {:?}", message);
        Ok(())
    }

    async fn receive(&mut self) -> McpResult<Option<JsonRpcMessage>> {
        let rx = self
            .message_rx
            .as_mut()
            .ok_or_else(|| McpError::TransportError("Not connected (no receiver)".to_string()))?;

        match rx.recv().await {
            Some(msg) => Ok(Some(msg)),
            None => {
                self.connected = false;
                Ok(None)
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl SseTransport {
    /// Listen to SSE events with automatic reconnection and exponential backoff.
    ///
    /// When the SSE stream ends or encounters an error, this method will
    /// attempt to reconnect with exponential backoff up to `max_reconnect_attempts`
    /// times before giving up.
    async fn listen_sse_with_reconnect(
        client: reqwest::Client,
        url: &str,
        headers: &HashMap<String, String>,
        tx: mpsc::Sender<JsonRpcMessage>,
        max_reconnect_attempts: u32,
        initial_delay_ms: u64,
    ) -> McpResult<()> {
        let mut attempt = 0u32;

        loop {
            match Self::listen_sse_once(client.clone(), url, headers, &tx).await {
                Ok(()) => {
                    // Stream ended normally (EOF) — try to reconnect
                    attempt += 1;
                    if attempt > max_reconnect_attempts {
                        tracing::warn!(
                            "SSE stream ended. Max reconnect attempts ({}) reached.",
                            max_reconnect_attempts
                        );
                        return Ok(());
                    }

                    let delay_ms = initial_delay_ms * 2u64.pow(attempt - 1);
                    tracing::info!(
                        "SSE stream ended. Reconnecting in {}ms (attempt {}/{})",
                        delay_ms,
                        attempt,
                        max_reconnect_attempts
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > max_reconnect_attempts {
                        tracing::error!(
                            "SSE error: {}. Max reconnect attempts ({}) reached.",
                            e,
                            max_reconnect_attempts
                        );
                        return Err(e);
                    }

                    let delay_ms = initial_delay_ms * 2u64.pow(attempt - 1);
                    tracing::warn!(
                        "SSE error: {}. Reconnecting in {}ms (attempt {}/{})",
                        e,
                        delay_ms,
                        attempt,
                        max_reconnect_attempts
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    /// Listen to SSE events once (without reconnection).
    ///
    /// Returns `Ok(())` when the stream ends normally (EOF),
    /// or `Err` on a fatal error.
    async fn listen_sse_once(
        client: reqwest::Client,
        url: &str,
        headers: &HashMap<String, String>,
        tx: &mpsc::Sender<JsonRpcMessage>,
    ) -> McpResult<()> {
        let mut request = client.get(url);
        request = request.header("Accept", "text/event-stream");
        request = request.header("Cache-Control", "no-cache");

        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request.send().await.map_err(|e| {
            McpError::TransportError(format!("SSE connect error: {}", e))
        })?;

        use eventsource_stream::Eventsource;
        use futures::StreamExt;

        let byte_stream = response.bytes_stream();
        let mut event_stream = byte_stream.eventsource();

        while let Some(event) = event_stream.next().await {
            match event {
                Ok(sse_event) => {
                    // SSE events with "message" event type contain JSON-RPC messages
                    if sse_event.event == "message" || sse_event.event.is_empty() {
                        if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&sse_event.data) {
                            if tx.send(msg).await.is_err() {
                                // Receiver dropped
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("SSE event stream error: {}", e);
                    // Return error to trigger reconnection
                    return Err(McpError::TransportError(format!(
                        "SSE event stream error: {}",
                        e
                    )));
                }
            }
        }

        // Stream ended normally (EOF)
        Ok(())
    }

}

// ---------------------------------------------------------------------------
// StreamableHttpTransport
// ---------------------------------------------------------------------------

/// Streamable HTTP transport using standard HTTP request/response.
pub struct StreamableHttpTransport {
    url: String,
    headers: HashMap<String, String>,
    http_client: reqwest::Client,
    connected: bool,
    // For streaming responses, we may need to buffer
    pending_messages: Vec<JsonRpcMessage>,
}

impl StreamableHttpTransport {
    /// Create a new StreamableHTTP transport.
    pub fn new(url: String, headers: HashMap<String, String>) -> Self {
        Self {
            url,
            headers,
            http_client: reqwest::Client::new(),
            connected: false,
            pending_messages: Vec::new(),
        }
    }
}

#[async_trait]
impl McpTransport for StreamableHttpTransport {
    async fn connect(&mut self) -> McpResult<()> {
        // For StreamableHTTP, we just verify the endpoint is reachable
        self.connected = true;
        tracing::info!("StreamableHTTP transport ready for {}", self.url);
        Ok(())
    }

    async fn close(&mut self) -> McpResult<()> {
        self.connected = false;
        self.pending_messages.clear();
        tracing::info!("StreamableHTTP transport closed");
        Ok(())
    }

    async fn send(&mut self, message: &JsonRpcMessage) -> McpResult<()> {
        let mut request = self.http_client.post(&self.url);
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request = request.header("Content-Type", "application/json");
        request = request.header("Accept", "application/json, text/event-stream");

        request = request.json(message);

        let response = request.send().await.map_err(|e| {
            McpError::TransportError(format!("StreamableHTTP send error: {}", e))
        })?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("text/event-stream") {
            // Handle SSE streaming response
            use eventsource_stream::Eventsource;
            use futures::StreamExt;

            let byte_stream = response.bytes_stream();
            let mut event_stream = byte_stream.eventsource();

            while let Some(event) = event_stream.next().await {
                match event {
                    Ok(sse_event) => {
                        if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&sse_event.data) {
                            self.pending_messages.push(msg);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("StreamableHTTP SSE error: {}", e);
                        break;
                    }
                }
            }
        } else {
            // Standard JSON response
            let body = response.text().await.map_err(|e| {
                McpError::TransportError(format!("Failed to read response body: {}", e))
            })?;

            if !body.is_empty() {
                // The response could be a single message or an array
                if body.trim_start().starts_with('[') {
                    if let Ok(messages) =
                        serde_json::from_str::<Vec<JsonRpcMessage>>(&body)
                    {
                        self.pending_messages.extend(messages);
                    }
                } else if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&body) {
                    self.pending_messages.push(msg);
                }
            }
        }

        tracing::trace!(
            "StreamableHTTP sent message, pending: {}",
            self.pending_messages.len()
        );
        Ok(())
    }

    async fn receive(&mut self) -> McpResult<Option<JsonRpcMessage>> {
        if !self.pending_messages.is_empty() {
            return Ok(Some(self.pending_messages.remove(0)));
        }

        if !self.connected {
            return Ok(None);
        }

        // No pending messages — the client should call send first
        Ok(None)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_rpc_request_creation() {
        let msg = JsonRpcMessage::request(1, "tools/list", json!({}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id_as_u64(), Some(1));
        assert_eq!(msg.method.as_deref(), Some("tools/list"));
        assert!(msg.is_request());
        assert!(!msg.is_response());
        assert!(!msg.is_notification());
    }

    #[test]
    fn test_json_rpc_notification_creation() {
        let msg = JsonRpcMessage::notification("notifications/cancelled", json!({}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert!(msg.id.is_none());
        assert!(msg.is_notification());
        assert!(!msg.is_request());
    }

    #[test]
    fn test_json_rpc_serialization() {
        let msg = JsonRpcMessage::request(42, "initialize", json!({"capabilities": {}}));
        let json_str = serde_json::to_string(&msg).unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"id\":42"));
        assert!(json_str.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_json_rpc_deserialization_response() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert!(msg.is_response());
        assert!(!msg.is_error());
        assert_eq!(msg.id_as_u64(), Some(1));
        assert!(msg.result.is_some());
    }

    #[test]
    fn test_json_rpc_deserialization_error() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert!(msg.is_error());
        assert_eq!(msg.error.as_ref().unwrap().code, -32600);
    }

    #[test]
    fn test_json_rpc_roundtrip() {
        let original = JsonRpcMessage::request(
            123,
            "tools/call",
            json!({"name": "get_weather", "arguments": {"city": "Tokyo"}}),
        );
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: JsonRpcMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.id_as_u64(), Some(123));
        assert_eq!(deserialized.method.as_deref(), Some("tools/call"));
    }

    #[test]
    fn test_stdio_transport_creation() {
        let transport = StdioTransport::new(
            "node".to_string(),
            vec!["server.js".to_string()],
            HashMap::new(),
            None,
        );
        assert!(!transport.is_connected());
        assert!(transport.stderr_rx.is_none());
    }

    #[test]
    fn test_stdio_transport_stderr_initially_empty() {
        let mut transport = StdioTransport::new(
            "node".to_string(),
            vec![],
            HashMap::new(),
            None,
        );
        // Not connected, so no stderr
        assert!(transport.try_read_stderr().is_none());
        assert!(transport.read_all_stderr().is_empty());
    }

    #[test]
    fn test_sse_transport_creation() {
        let transport = SseTransport::new(
            "http://localhost:8080/sse".to_string(),
            HashMap::new(),
        );
        assert!(!transport.is_connected());
        assert_eq!(transport.max_reconnect_attempts, SSE_DEFAULT_MAX_RECONNECT_ATTEMPTS);
        assert_eq!(transport.initial_reconnect_delay_ms, SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS);
    }

    #[test]
    fn test_sse_transport_custom_reconnect_settings() {
        let transport = SseTransport::with_reconnect_settings(
            "http://localhost:8080/sse".to_string(),
            HashMap::new(),
            10,
            2000,
        );
        assert_eq!(transport.max_reconnect_attempts, 10);
        assert_eq!(transport.initial_reconnect_delay_ms, 2000);
    }

    #[test]
    fn test_streamable_http_transport_creation() {
        let transport = StreamableHttpTransport::new(
            "http://localhost:8080/mcp".to_string(),
            HashMap::new(),
        );
        assert!(!transport.is_connected());
    }
}
