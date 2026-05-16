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
            let is_already_wrapped =
                self.command.to_lowercase() == "cmd.exe" || self.command.to_lowercase() == "cmd";

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

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::ConnectionFailed("Failed to open stdin pipe".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::ConnectionFailed("Failed to open stdout pipe".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| McpError::ConnectionFailed("Failed to open stderr pipe".to_string()))?;

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

/// Default maximum delay between reconnection attempts for SSE transport (5 seconds).
/// Matches TS: `ReconnectingEventSource` with `max_retry_time: 5000`.
/// Retries indefinitely but never waits longer than this between attempts.
const SSE_DEFAULT_MAX_RECONNECT_DELAY_MS: u64 = 5000;

/// Default initial reconnect delay in milliseconds (1 second).
const SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS: u64 = 1000;

/// SSE (Server-Sent Events) transport.
///
/// Uses HTTP POST for sending and SSE for receiving.
/// Supports automatic reconnection with exponential backoff.
///
/// ## Endpoint Discovery Protocol
///
/// The MCP SSE protocol works as follows:
/// 1. Connect to the SSE endpoint via GET
/// 2. Listen for an `endpoint` event from the server containing the POST target URL
/// 3. POST JSON-RPC messages to the discovered endpoint URL
///
/// The `endpoint` event format is:
/// ```text
/// event: endpoint
/// data: /mcp?sessionId=xxx
/// ```
/// or with an absolute URL:
/// ```text
/// event: endpoint
/// data: http://server.com/mcp?sessionId=xxx
/// ```
pub struct SseTransport {
    /// The SSE URL to connect to (GET endpoint for receiving events).
    url: String,
    headers: HashMap<String, String>,
    http_client: reqwest::Client,
    connected: bool,
    // Channel for received messages (populated by SSE listener)
    message_rx: Option<mpsc::Receiver<JsonRpcMessage>>,
    // The POST endpoint URL discovered from the `endpoint` SSE event.
    // Messages are sent here instead of to `url`.
    post_endpoint: Option<String>,
    // Join handle for the SSE listener task
    listener_handle: Option<tokio::task::JoinHandle<()>>,
    /// Maximum delay between reconnection attempts (milliseconds).
    /// Matches TS: `max_retry_time: 5000` from `ReconnectingEventSource`.
    /// Retries are indefinite; this caps the wait between attempts.
    max_reconnect_delay_ms: u64,
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
            post_endpoint: None,
            listener_handle: None,
            max_reconnect_delay_ms: SSE_DEFAULT_MAX_RECONNECT_DELAY_MS,
            initial_reconnect_delay_ms: SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS,
        }
    }

    /// Create a new SSE transport with custom reconnection settings.
    pub fn with_reconnect_settings(
        url: String,
        headers: HashMap<String, String>,
        max_reconnect_delay_ms: u64,
        initial_reconnect_delay_ms: u64,
    ) -> Self {
        Self {
            url,
            headers,
            http_client: reqwest::Client::new(),
            connected: false,
            message_rx: None,
            post_endpoint: None,
            listener_handle: None,
            max_reconnect_delay_ms,
            initial_reconnect_delay_ms,
        }
    }

    /// Resolve a potentially relative endpoint URL against the base SSE URL.
    fn resolve_endpoint_url(base_url: &str, endpoint_data: &str) -> String {
        let endpoint_data = endpoint_data.trim();

        // If it's already an absolute URL, use it directly
        if endpoint_data.starts_with("http://") || endpoint_data.starts_with("https://") {
            return endpoint_data.to_string();
        }

        // Parse the base URL and join the relative path
        match url::Url::parse(base_url) {
            Ok(base) => match base.join(endpoint_data) {
                Ok(resolved) => resolved.to_string(),
                Err(e) => {
                    tracing::warn!(
                        "Failed to resolve relative endpoint '{}': {}. Using as-is.",
                        endpoint_data,
                        e
                    );
                    endpoint_data.to_string()
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to parse base URL '{}': {}. Using endpoint as-is.",
                    base_url,
                    e
                );
                endpoint_data.to_string()
            }
        }
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn connect(&mut self) -> McpResult<()> {
        let (msg_tx, msg_rx) = mpsc::channel(100);
        let (endpoint_tx, mut endpoint_rx) = mpsc::channel::<String>(1);
        self.message_rx = Some(msg_rx);

        let sse_url = self.url.clone();
        let client = self.http_client.clone();
        let headers = self.headers.clone();
        let max_reconnect = self.max_reconnect_delay_ms;
        let initial_delay = self.initial_reconnect_delay_ms;

        // Start SSE listener in background; it will signal the discovered endpoint
        let handle = tokio::spawn(async move {
            if let Err(e) = Self::listen_sse_with_reconnect(
                client,
                &sse_url,
                &headers,
                msg_tx,
                Some(endpoint_tx),
                max_reconnect,
                initial_delay,
            )
            .await
            {
                tracing::error!("SSE listener error: {}", e);
            }
        });

        self.listener_handle = Some(handle);

        // Wait for the endpoint discovery event from the server.
        // The MCP SSE protocol requires waiting for an `endpoint` event before
        // the client can POST messages.
        let timeout = std::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout, endpoint_rx.recv()).await {
            Ok(Some(discovered_endpoint)) => {
                // Resolve relative URLs against the base SSE URL
                let resolved = Self::resolve_endpoint_url(&self.url, &discovered_endpoint);
                tracing::info!(
                    "SSE transport discovered POST endpoint: {} (raw: {})",
                    resolved,
                    discovered_endpoint
                );
                self.post_endpoint = Some(resolved);
            }
            Ok(None) => {
                // Channel closed without receiving an endpoint — the listener
                // task ended before sending one. Fall back to posting to the
                // SSE URL itself so the transport still functions for servers
                // that don't follow the discovery protocol.
                tracing::warn!(
                    "SSE transport did not receive an 'endpoint' event. \
                     Falling back to POST to SSE URL: {}",
                    self.url
                );
                self.post_endpoint = Some(self.url.clone());
            }
            Err(_) => {
                // Timed out waiting for endpoint discovery
                tracing::warn!(
                    "SSE transport timed out waiting for endpoint discovery (30s). \
                     Falling back to POST to SSE URL: {}",
                    self.url
                );
                self.post_endpoint = Some(self.url.clone());
            }
        }

        self.connected = true;
        tracing::info!("SSE transport connected to {}", self.url);
        Ok(())
    }

    async fn close(&mut self) -> McpResult<()> {
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }
        self.message_rx = None;
        self.post_endpoint = None;
        self.connected = false;
        tracing::info!("SSE transport closed");
        Ok(())
    }

    async fn send(&mut self, message: &JsonRpcMessage) -> McpResult<()> {
        // POST the message to the discovered endpoint (not the SSE URL)
        let post_url = self.post_endpoint.as_ref().ok_or_else(|| {
            McpError::TransportError(
                "SSE transport not connected: no POST endpoint discovered yet".to_string(),
            )
        })?;

        let mut request = self.http_client.post(post_url);
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        request = request.json(message);

        let response = request
            .send()
            .await
            .map_err(|e| McpError::TransportError(format!("SSE send error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::TransportError(format!(
                "SSE send failed with status {}: {}",
                status, body
            )));
        }

        tracing::trace!("SSE sent message to {}: {:?}", post_url, message);
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
    /// Matches TS: `ReconnectingEventSource` with `max_retry_time: 5000`.
    /// Retries indefinitely with exponential backoff, capped at
    /// `max_reconnect_delay_ms` between attempts.
    ///
    /// The `endpoint_tx` channel is used to signal the discovered POST endpoint
    /// back to the caller. It is only sent to once (on the first successful
    /// endpoint discovery).
    async fn listen_sse_with_reconnect(
        client: reqwest::Client,
        url: &str,
        headers: &HashMap<String, String>,
        tx: mpsc::Sender<JsonRpcMessage>,
        endpoint_tx: Option<mpsc::Sender<String>>,
        max_reconnect_delay_ms: u64,
        initial_delay_ms: u64,
    ) -> McpResult<()> {
        let mut attempt = 0u32;
        // Track whether we have already signaled the endpoint so we only do it once.
        let mut endpoint_signaled = false;
        // Keep the endpoint_tx around; if it was already consumed (sent), that is fine.
        let mut endpoint_tx = endpoint_tx;

        loop {
            let result = Self::listen_sse_once(
                client.clone(),
                url,
                headers,
                &tx,
                &mut endpoint_tx,
                &mut endpoint_signaled,
            )
            .await;

            match result {
                Ok(()) => {
                    // Stream ended normally (EOF) — retry indefinitely with capped backoff.
                    // Matches TS: ReconnectingEventSource retries indefinitely.
                    attempt += 1;

                    let delay_ms =
                        (initial_delay_ms * 2u64.pow(attempt - 1)).min(max_reconnect_delay_ms);
                    tracing::info!(
                        "SSE stream ended. Reconnecting in {}ms (attempt {})",
                        delay_ms,
                        attempt
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                Err(e) => {
                    attempt += 1;

                    let delay_ms =
                        (initial_delay_ms * 2u64.pow(attempt - 1)).min(max_reconnect_delay_ms);
                    tracing::warn!(
                        "SSE error: {}. Reconnecting in {}ms (attempt {})",
                        e,
                        delay_ms,
                        attempt
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
    ///
    /// When an `endpoint` SSE event is received, the discovered URL is sent
    /// through `endpoint_tx` and `endpoint_signaled` is set to `true` so that
    /// subsequent reconnections don't resend it.
    async fn listen_sse_once(
        client: reqwest::Client,
        url: &str,
        headers: &HashMap<String, String>,
        tx: &mpsc::Sender<JsonRpcMessage>,
        endpoint_tx: &mut Option<mpsc::Sender<String>>,
        endpoint_signaled: &mut bool,
    ) -> McpResult<()> {
        let mut request = client.get(url);
        request = request.header("Accept", "text/event-stream");
        request = request.header("Cache-Control", "no-cache");

        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request
            .send()
            .await
            .map_err(|e| McpError::TransportError(format!("SSE connect error: {}", e)))?;

        use eventsource_stream::Eventsource;
        use futures::StreamExt;

        let byte_stream = response.bytes_stream();
        let mut event_stream = byte_stream.eventsource();

        while let Some(event) = event_stream.next().await {
            match event {
                Ok(sse_event) => {
                    // Handle the `endpoint` discovery event from the MCP SSE protocol.
                    // This event tells us where to POST JSON-RPC messages.
                    if sse_event.event == "endpoint" {
                        let endpoint_data = sse_event.data.trim().to_string();
                        if !endpoint_data.is_empty() && !*endpoint_signaled {
                            tracing::info!("SSE received endpoint event: {}", endpoint_data);
                            if let Some(sender) = endpoint_tx.take() {
                                let _ = sender.send(endpoint_data).await;
                                *endpoint_signaled = true;
                            }
                        }
                        continue;
                    }

                    // SSE events with "message" event type (or default) contain JSON-RPC messages
                    if (sse_event.event == "message" || sse_event.event.is_empty())
                        && let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&sse_event.data)
                        && tx.send(msg).await.is_err()
                    {
                        // Receiver dropped
                        return Ok(());
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
///
/// Implements the MCP Streamable HTTP transport as specified in the protocol:
/// - POSTs JSON-RPC messages to the endpoint
/// - Supports both JSON and SSE streaming responses
/// - Manages session IDs via the `Mcp-Session-Id` header
pub struct StreamableHttpTransport {
    url: String,
    headers: HashMap<String, String>,
    http_client: reqwest::Client,
    connected: bool,
    /// The MCP session ID received from the server (sent back on each request).
    session_id: Option<String>,
    // For streaming responses, we buffer incoming messages
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
            session_id: None,
            pending_messages: Vec::new(),
        }
    }
}

#[async_trait]
impl McpTransport for StreamableHttpTransport {
    async fn connect(&mut self) -> McpResult<()> {
        // Verify the endpoint is reachable by sending a lightweight GET request.
        // The server may return 200 (OK) or 405 (Method Not Allowed) — both
        // indicate the server is alive. A connection failure here surfaces
        // immediately instead of on the first send.
        let mut request = self.http_client.get(&self.url);
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request = request.header("Accept", "application/json, text/event-stream");

        match request.send().await {
            Ok(response) => {
                // Extract session ID from the response if present
                if let Some(sid) = response
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|v| v.to_str().ok())
                {
                    self.session_id = Some(sid.to_string());
                    tracing::info!("StreamableHTTP received session ID: {}", sid);
                }
                // Any HTTP response (even 4xx) means the server is reachable
                tracing::debug!(
                    "StreamableHTTP connectivity check: status {}",
                    response.status()
                );
            }
            Err(e) => {
                // If the connectivity check fails, log but don't hard-fail.
                // Some servers may not support GET; we'll verify on the first
                // actual request.
                tracing::warn!(
                    "StreamableHTTP connectivity check failed: {}. \
                     Will attempt to connect on first request.",
                    e
                );
            }
        }

        self.connected = true;
        tracing::info!("StreamableHTTP transport ready for {}", self.url);
        Ok(())
    }

    async fn close(&mut self) -> McpResult<()> {
        self.connected = false;
        self.session_id = None;
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

        // Include the session ID if we have one
        if let Some(ref sid) = self.session_id {
            request = request.header("Mcp-Session-Id", sid.as_str());
        }

        request = request.json(message);

        let response = request
            .send()
            .await
            .map_err(|e| McpError::TransportError(format!("StreamableHTTP send error: {}", e)))?;

        // Update session ID if the server sends a new one
        if let Some(sid) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            let new_sid = sid.to_string();
            if self.session_id.as_ref() != Some(&new_sid) {
                tracing::debug!("StreamableHTTP session ID updated: {}", new_sid);
                self.session_id = Some(new_sid);
            }
        }

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::TransportError(format!(
                "StreamableHTTP send failed with status {}: {}",
                status, body
            )));
        }

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
                    if let Ok(messages) = serde_json::from_str::<Vec<JsonRpcMessage>>(&body) {
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
        let json_str =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
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
        let mut transport = StdioTransport::new("node".to_string(), vec![], HashMap::new(), None);
        // Not connected, so no stderr
        assert!(transport.try_read_stderr().is_none());
        assert!(transport.read_all_stderr().is_empty());
    }

    #[test]
    fn test_sse_transport_creation() {
        let transport = SseTransport::new("http://localhost:8080/sse".to_string(), HashMap::new());
        assert!(!transport.is_connected());
        assert_eq!(
            transport.max_reconnect_delay_ms,
            SSE_DEFAULT_MAX_RECONNECT_DELAY_MS
        );
        assert_eq!(
            transport.initial_reconnect_delay_ms,
            SSE_DEFAULT_INITIAL_RECONNECT_DELAY_MS
        );
    }

    #[test]
    fn test_sse_transport_custom_reconnect_settings() {
        let transport = SseTransport::with_reconnect_settings(
            "http://localhost:8080/sse".to_string(),
            HashMap::new(),
            8000,
            2000,
        );
        assert_eq!(transport.max_reconnect_delay_ms, 8000);
        assert_eq!(transport.initial_reconnect_delay_ms, 2000);
    }

    #[test]
    fn test_streamable_http_transport_creation() {
        let transport =
            StreamableHttpTransport::new("http://localhost:8080/mcp".to_string(), HashMap::new());
        assert!(!transport.is_connected());
    }
}
