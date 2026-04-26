//! Codex sub-process adapter — JSON-RPC 2.0 over stdio with NDJSON framing.
//!
//! [`CodexAdapter`] launches an OpenAI Codex binary as a child process and
//! communicates with it over stdin/stdout using **NDJSON** (newline-delimited
//! JSON) where each line is a complete JSON-RPC 2.0 message.
//!
//! # Protocol overview
//!
//! ```text
//! Host → Agent (stdin):  {"jsonrpc":"2.0","method":"...","params":{...},"id":1}\n
//! Agent → Host (stdout): {"jsonrpc":"2.0","method":"...","params":{...}}\n
//! ```
//!
//! # Key differences from [`RooCodeAdapter`](super::roo_code::RooCodeAdapter)
//!
//! - Uses **NDJSON** (newline-delimited JSON) instead of Content-Length framing.
//! - Codex methods: `session/create`, `task/create`, `task/cancel`, `session/delete`.
//! - Codex can send **requests** to the Host (`permission/request`,
//!   `approval/request`) that require a JSON-RPC response.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::adapter::AgentAdapter;
use crate::events::UnifiedAgentEvent;
use crate::permission::PermissionDecision;
use crate::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

// ===========================================================================
// JSON-RPC 2.0 types
// ===========================================================================

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 request (host → agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol version — always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Request identifier.
    pub id: u64,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC 2.0 response (agent → host).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcResponse {
    /// Protocol version — always `"2.0"`.
    pub jsonrpc: String,
    /// Request identifier this response corresponds to.
    pub id: Option<u64>,
    /// Result payload (present on success).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload (present on failure).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 notification (agent → host, no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    /// Protocol version — always `"2.0"`.
    pub jsonrpc: String,
    /// Notification method name.
    pub method: String,
    /// Notification parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

// ===========================================================================
// NDJSON codec
// ===========================================================================

/// NDJSON line codec for reading/writing JSON-RPC messages over stdio.
///
/// Each message is a single JSON object terminated by `\n`.  Empty lines are
/// silently skipped.
pub struct NdjsonCodec;

impl NdjsonCodec {
    /// Write a JSON value as a single line followed by `\n`.
    pub async fn write_line<W: AsyncWriteExt + Unpin>(
        writer: &mut W,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        writer.write_all(line.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Read a single NDJSON line and parse it as a JSON value.
    ///
    /// Empty lines (including lines containing only whitespace) are skipped.
    pub async fn read_line<R: AsyncBufReadExt + Unpin>(
        reader: &mut R,
    ) -> anyhow::Result<serde_json::Value> {
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                anyhow::bail!("stream closed while reading NDJSON line");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue; // skip blank lines
            }
            let value: serde_json::Value = serde_json::from_str(trimmed)?;
            return Ok(value);
        }
    }
}

// ===========================================================================
// Codex notification → UnifiedAgentEvent mapping
// ===========================================================================

/// Convert a Codex JSON-RPC **notification** into a [`UnifiedAgentEvent`].
///
/// The `session_id` is injected by the caller (the adapter tracks it).
pub fn map_codex_notification(
    method: &str,
    params: &serde_json::Value,
    session_id: &str,
) -> Option<UnifiedAgentEvent> {
    match method {
        "task/delta" => Some(UnifiedAgentEvent::MessageDelta {
            session_id: session_id.to_owned(),
            delta: params["delta"].as_str().unwrap_or_default().to_owned(),
        }),

        "task/tool_call" => Some(UnifiedAgentEvent::ToolCallStarted {
            session_id: session_id.to_owned(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            tool_input: params["toolInput"].clone(),
        }),

        "task/tool_result" => Some(UnifiedAgentEvent::ToolCallCompleted {
            session_id: session_id.to_owned(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            result: params["result"].clone(),
        }),

        "task/created" => Some(UnifiedAgentEvent::SubtaskStarted {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            description: params["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        }),

        "task/updated" => Some(UnifiedAgentEvent::SubtaskProgress {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            progress: params["status"].as_str().unwrap_or_default().to_owned(),
        }),

        "task/completed" => Some(UnifiedAgentEvent::SubtaskCompleted {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            result: params["result"].clone(),
        }),

        "task/error" => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_owned(),
            message: params["message"].as_str().unwrap_or_default().to_owned(),
            recoverable: params["recoverable"].as_bool().unwrap_or(true),
        }),

        "session/ready" => Some(UnifiedAgentEvent::Ready),

        _ => {
            debug!(method, "unhandled Codex notification");
            None
        }
    }
}

/// Convert a Codex JSON-RPC **request** (Codex → Host) into a
/// [`UnifiedAgentEvent`].
///
/// The `rpc_id` is the numeric JSON-RPC request id from Codex.  It is
/// stringified and used as the `request_id` in the resulting event so that
/// [`CodexAdapter::resolve_permission`] can map it back when the host
/// responds.
pub fn map_codex_request(
    method: &str,
    params: &serde_json::Value,
    rpc_id: u64,
    session_id: &str,
) -> Option<UnifiedAgentEvent> {
    match method {
        "permission/request" | "approval/request" => Some(UnifiedAgentEvent::PermissionRequest {
            session_id: session_id.to_owned(),
            request_id: rpc_id.to_string(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            input: params
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        }),
        _ => {
            debug!(method, "unhandled Codex request");
            None
        }
    }
}

// ===========================================================================
// CodexAdapter
// ===========================================================================

/// Sub-process adapter for OpenAI Codex.
///
/// Launches the Codex binary configured in [`AgentConfig::binary_path`]
/// and communicates over stdio using JSON-RPC 2.0 with NDJSON framing
/// (one JSON object per newline-terminated line).
pub struct CodexAdapter {
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// Stored configuration (set on `start`).
    config: Option<AgentConfig>,
    /// The child process (if started).
    process: Option<Child>,
    /// Buffered stdin writer for sending NDJSON lines.
    stdin_writer: Option<tokio::io::BufWriter<ChildStdin>>,
    /// Buffered stdout reader (taken by background reader on `send_message`).
    stdout_reader: Option<tokio::io::BufReader<ChildStdout>>,
    /// Monotonic request ID counter.
    next_request_id: AtomicU64,
    /// Event channel sender — the background reader pushes events here.
    event_tx: Option<mpsc::Sender<UnifiedAgentEvent>>,
    /// Current Codex session ID (set during `start` handshake).
    current_session_id: Option<String>,
}

impl CodexAdapter {
    /// Create a new `CodexAdapter` in the **Starting** state.
    #[must_use]
    pub fn new() -> Self {
        let mut capabilities = std::collections::HashSet::new();
        capabilities.insert(AgentCapability::Streaming);
        capabilities.insert(AgentCapability::ToolUse);
        capabilities.insert(AgentCapability::Subtasks);
        capabilities.insert(AgentCapability::Permissions);

        Self {
            info: AgentInfo {
                name: "OpenAI Codex".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            config: None,
            process: None,
            stdin_writer: None,
            stdout_reader: None,
            next_request_id: AtomicU64::new(1),
            event_tx: None,
            current_session_id: None,
        }
    }

    /// Allocate the next JSON-RPC request ID.
    fn next_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request and return the allocated ID.
    ///
    /// Does **not** wait for a response — the background reader dispatches
    /// responses to the appropriate [`oneshot::Sender`].
    #[allow(dead_code)]
    async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<u64> {
        // Allocate ID first to avoid borrowing conflict with stdin_writer.
        let id = self.next_id();

        let writer = self
            .stdin_writer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent not started — no stdin pipe"))?;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        NdjsonCodec::write_line(writer, &request).await?;
        debug!(id, method, "sent JSON-RPC request");
        Ok(id)
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let writer = self
            .stdin_writer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent not started — no stdin pipe"))?;

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        NdjsonCodec::write_line(writer, &notification).await?;
        debug!(method, "sent JSON-RPC notification");
        Ok(())
    }

    /// Send a JSON-RPC **response** (Host → Codex) for an incoming Codex
    /// request such as `permission/request`.
    async fn send_response(&mut self, id: u64, result: serde_json::Value) -> anyhow::Result<()> {
        let writer = self
            .stdin_writer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent not started — no stdin pipe"))?;

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });

        NdjsonCodec::write_line(writer, &response).await?;
        debug!(id, "sent JSON-RPC response");
        Ok(())
    }

    /// Spawn the background reader task that continuously reads NDJSON lines
    /// from the child's stdout and dispatches them to pending request
    /// channels or the event stream.
    fn spawn_reader(
        &mut self,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        let stdout = self
            .stdout_reader
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdout available"))?;

        let (event_tx, event_rx) = mpsc::channel::<UnifiedAgentEvent>(256);
        self.event_tx = Some(event_tx.clone());

        let session_id = self.current_session_id.clone().unwrap_or_default();

        tokio::spawn(async move {
            let mut reader = stdout;
            loop {
                match NdjsonCodec::read_line(&mut reader).await {
                    Ok(msg) => {
                        debug!(msg_len = ?msg.to_string().len(), "received NDJSON message");

                        // 1) Response to a previously-sent request (has id + result/error).
                        if msg.get("id").is_some()
                            && (msg.get("result").is_some() || msg.get("error").is_some())
                        {
                            let id = msg["id"].as_u64().unwrap_or(0);
                            let mut guard =
                                pending.lock().expect("pending requests mutex poisoned");
                            if let Some(sender) = guard.remove(&id) {
                                if sender.send(msg).is_err() {
                                    debug!(id, "response channel already closed");
                                }
                            } else {
                                warn!(id, "received response for unknown request");
                            }
                        }
                        // 2) Incoming request from Codex (has id + method, no result/error).
                        else if msg.get("method").is_some() && msg.get("id").is_some() {
                            let method = msg["method"].as_str().unwrap_or_default();
                            let params = msg
                                .get("params")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let rpc_id = msg["id"].as_u64().unwrap_or(0);

                            if let Some(event) =
                                map_codex_request(method, &params, rpc_id, &session_id)
                                && event_tx.send(event).await.is_err()
                            {
                                debug!("event channel closed, stopping reader");
                                break;
                            }
                        }
                        // 3) Notification from Codex (has method, no id).
                        else if msg.get("method").is_some() {
                            let method = msg["method"].as_str().unwrap_or_default();
                            let params = msg
                                .get("params")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);

                            if let Some(event) =
                                map_codex_notification(method, &params, &session_id)
                                && event_tx.send(event).await.is_err()
                            {
                                debug!("event channel closed, stopping reader");
                                break;
                            }
                        } else {
                            warn!("received unknown JSON-RPC message structure");
                        }
                    }
                    Err(e) => {
                        info!(error = %e, "stdout reader finished");
                        break;
                    }
                }
            }
        });

        Ok(event_rx)
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// AgentAdapter implementation
// ===========================================================================

#[async_trait]
impl AgentAdapter for CodexAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        let binary_path = config
            .binary_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("binary_path is required for CodexAdapter"))?;

        if !binary_path.exists() {
            anyhow::bail!("Codex binary not found at: {}", binary_path.display());
        }

        info!(path = %binary_path.display(), "starting Codex subprocess");

        let mut cmd = Command::new(binary_path);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        if let Some(ref wd) = config.working_dir {
            cmd.current_dir(wd);
        }

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("stdin should be piped");
        let stdout = child.stdout.take().expect("stdout should be piped");

        self.process = Some(child);
        self.stdin_writer = Some(tokio::io::BufWriter::new(stdin));
        self.stdout_reader = Some(tokio::io::BufReader::new(stdout));
        self.status = AgentStatus::Starting;
        self.config = Some(config.clone());

        // ── session/create handshake ──

        let id = self.next_id();
        let create_params = serde_json::json!({
            "clientInfo": {
                "name": "remote-code-rust",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/create",
            "params": create_params,
        });

        NdjsonCodec::write_line(self.stdin_writer.as_mut().expect("stdin"), &request).await?;

        // Read session/create response directly (before background reader).
        let response = NdjsonCodec::read_line(self.stdout_reader.as_mut().expect("stdout")).await?;

        if let Some(err) = response.get("error") {
            anyhow::bail!(
                "session/create failed: code={} message={}",
                err["code"],
                err["message"]
            );
        }

        // Extract session ID from response.
        let session_id = response
            .get("result")
            .and_then(|r| r.get("sessionId"))
            .and_then(|s| s.as_str())
            .unwrap_or("default")
            .to_owned();

        self.current_session_id = Some(session_id);
        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!(
            "CodexAdapter ready (session: {})",
            self.current_session_id.as_deref().unwrap_or("?")
        );
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        if self.status != AgentStatus::Ready && self.status != AgentStatus::Idle {
            anyhow::bail!("adapter not ready (status: {})", self.status);
        }

        self.current_session_id = Some(session_id.to_owned());
        self.status = AgentStatus::Busy;
        self.info.status = AgentStatus::Busy;

        // Shared pending-requests map for the background reader.
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn the background reader for this session.
        let event_rx = self.spawn_reader(pending)?;

        // Send task/create — events arrive as Codex notifications.
        let params = serde_json::json!({
            "sessionId": session_id,
            "prompt": message,
        });
        self.send_notification("task/create", params).await?;

        Ok(event_rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        self.send_notification(
            "task/cancel",
            serde_json::Value::Object(serde_json::Map::new()),
        )
        .await
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let decision_str = match decision {
            PermissionDecision::Allow => "allow",
            PermissionDecision::Deny => "deny",
            PermissionDecision::AllowAll => "allowAll",
        };

        // `request_id` is the stringified JSON-RPC id from the Codex
        // permission request.
        let rpc_id: u64 = request_id.parse().unwrap_or(0);

        let result = serde_json::json!({
            "decision": decision_str,
        });

        self.send_response(rpc_id, result).await?;
        debug!(request_id, "resolved Codex permission request");
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("CodexAdapter stopping");

        // Try to send session/delete.
        if self.stdin_writer.is_some() {
            let _ = self
                .send_notification("session/delete", serde_json::Value::Null)
                .await;
        }

        // Give the child a chance to exit gracefully, then kill.
        if let Some(ref mut child) = self.process {
            match tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await {
                Ok(Ok(status)) => {
                    info!(status = %status, "child exited");
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "error waiting for child");
                }
                Err(_) => {
                    warn!("child did not exit in time, killing");
                    let _ = child.kill().await;
                }
            }
        }

        self.process = None;
        self.stdin_writer = None;
        self.stdout_reader = None;
        self.event_tx = None;
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        if matches!(self.status, AgentStatus::Stopped | AgentStatus::Error) {
            return false;
        }
        self.process.is_some()
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── NDJSON codec tests ──

    #[tokio::test]
    async fn ndjson_write_read_roundtrip() {
        let (tx, rx) = tokio::io::duplex(1024);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        let value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/create",
            "params": {"clientInfo": {"name": "test"}},
            "id": 1
        });

        NdjsonCodec::write_line(&mut writer, &value)
            .await
            .expect("write");
        let received = NdjsonCodec::read_line(&mut reader).await.expect("read");

        assert_eq!(received, value);
    }

    #[tokio::test]
    async fn ndjson_write_read_multiple_messages() {
        let (tx, rx) = tokio::io::duplex(4096);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        let messages = [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"session/create","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","method":"task/delta","params":{"delta":"hello"}}),
            serde_json::json!({"jsonrpc":"2.0","method":"session/ready","params":{}}),
        ];

        for msg in &messages {
            NdjsonCodec::write_line(&mut writer, msg)
                .await
                .expect("write");
        }

        for expected in &messages {
            let received = NdjsonCodec::read_line(&mut reader).await.expect("read");
            assert_eq!(&received, expected);
        }
    }

    #[tokio::test]
    async fn ndjson_skips_empty_lines() {
        let (tx, rx) = tokio::io::duplex(1024);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        // Write a valid message, then an empty line, then another message.
        let msg1 = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"test","params":{}});
        let msg2 = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"test2","params":{}});

        NdjsonCodec::write_line(&mut writer, &msg1)
            .await
            .expect("write1");
        writer.write_all(b"\n").await.expect("write empty line");
        NdjsonCodec::write_line(&mut writer, &msg2)
            .await
            .expect("write2");

        let received1 = NdjsonCodec::read_line(&mut reader).await.expect("read1");
        let received2 = NdjsonCodec::read_line(&mut reader).await.expect("read2");
        assert_eq!(received1, msg1);
        assert_eq!(received2, msg2);
    }

    // ── JSON-RPC serialization tests ──

    #[test]
    fn jsonrpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "session/create".to_owned(),
            params: serde_json::json!({"clientInfo": {"name": "test"}}),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "session/create");
        assert_eq!(parsed["params"]["clientInfo"]["name"], "test");
    }

    #[test]
    fn jsonrpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"sessionId":"sess-abc"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");

        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(result["sessionId"], "sess-abc");
    }

    #[test]
    fn jsonrpc_response_error_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");

        assert_eq!(response.id, Some(2));
        assert!(response.result.is_none());
        let err = response.error.expect("should have error");
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn jsonrpc_notification_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"task/delta","params":{"delta":"Hello"}}"#;
        let notification: JsonRpcNotification = serde_json::from_str(json).expect("deserialize");

        assert_eq!(notification.method, "task/delta");
        assert_eq!(notification.params["delta"], "Hello");
    }

    // ── Codex notification mapping tests ──

    #[test]
    fn codex_notification_mapping_task_delta() {
        let params = serde_json::json!({"delta": "Hello, "});
        let event = map_codex_notification("task/delta", &params, "sess-1");
        match event {
            Some(UnifiedAgentEvent::MessageDelta { session_id, delta }) => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(delta, "Hello, ");
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_tool_call() {
        let params = serde_json::json!({
            "toolName": "read_file",
            "toolInput": {"path": "/tmp/test.rs"}
        });
        let event = map_codex_notification("task/tool_call", &params, "sess-2");
        match event {
            Some(UnifiedAgentEvent::ToolCallStarted {
                session_id,
                tool_name,
                tool_input,
            }) => {
                assert_eq!(session_id, "sess-2");
                assert_eq!(tool_name, "read_file");
                assert_eq!(tool_input["path"], "/tmp/test.rs");
            }
            other => panic!("expected ToolCallStarted, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_tool_result() {
        let params = serde_json::json!({
            "toolName": "bash",
            "result": {"stdout": "ok"}
        });
        let event = map_codex_notification("task/tool_result", &params, "sess-3");
        match event {
            Some(UnifiedAgentEvent::ToolCallCompleted {
                session_id,
                tool_name,
                result,
            }) => {
                assert_eq!(session_id, "sess-3");
                assert_eq!(tool_name, "bash");
                assert_eq!(result["stdout"], "ok");
            }
            other => panic!("expected ToolCallCompleted, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_created() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "description": "refactor module"
        });
        let event = map_codex_notification("task/created", &params, "sess-4");
        match event {
            Some(UnifiedAgentEvent::SubtaskStarted {
                session_id,
                task_id,
                description,
            }) => {
                assert_eq!(session_id, "sess-4");
                assert_eq!(task_id, "task-1");
                assert_eq!(description, "refactor module");
            }
            other => panic!("expected SubtaskStarted, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_updated() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "status": "running"
        });
        let event = map_codex_notification("task/updated", &params, "sess-5");
        match event {
            Some(UnifiedAgentEvent::SubtaskProgress {
                session_id,
                task_id,
                progress,
            }) => {
                assert_eq!(session_id, "sess-5");
                assert_eq!(task_id, "task-1");
                assert_eq!(progress, "running");
            }
            other => panic!("expected SubtaskProgress, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_completed() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "result": {"success": true}
        });
        let event = map_codex_notification("task/completed", &params, "sess-6");
        match event {
            Some(UnifiedAgentEvent::SubtaskCompleted {
                session_id,
                task_id,
                result,
            }) => {
                assert_eq!(session_id, "sess-6");
                assert_eq!(task_id, "task-1");
                assert_eq!(result["success"], true);
            }
            other => panic!("expected SubtaskCompleted, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_task_error() {
        let params = serde_json::json!({
            "message": "rate limited",
            "recoverable": true
        });
        let event = map_codex_notification("task/error", &params, "sess-7");
        match event {
            Some(UnifiedAgentEvent::Error {
                session_id,
                message,
                recoverable,
            }) => {
                assert_eq!(session_id, "sess-7");
                assert_eq!(message, "rate limited");
                assert!(recoverable);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_session_ready() {
        let params = serde_json::json!({});
        let event = map_codex_notification("session/ready", &params, "sess-8");
        match event {
            Some(UnifiedAgentEvent::Ready) => {}
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn codex_notification_mapping_unknown_returns_none() {
        let params = serde_json::json!({});
        let event = map_codex_notification("codex/unknownMethod", &params, "sess-9");
        assert!(event.is_none());
    }

    // ── Codex permission request mapping tests ──

    #[test]
    fn codex_permission_request_mapping() {
        let params = serde_json::json!({
            "toolName": "write_file",
            "input": {"path": "/etc/hosts"}
        });
        let event = map_codex_request("permission/request", &params, 42, "sess-10");
        match event {
            Some(UnifiedAgentEvent::PermissionRequest {
                session_id,
                request_id,
                tool_name,
                input,
            }) => {
                assert_eq!(session_id, "sess-10");
                assert_eq!(request_id, "42");
                assert_eq!(tool_name, "write_file");
                assert_eq!(input["path"], "/etc/hosts");
            }
            other => panic!("expected PermissionRequest, got {other:?}"),
        }
    }

    #[test]
    fn codex_approval_request_mapping() {
        let params = serde_json::json!({
            "toolName": "bash",
            "input": {"command": "rm -rf /tmp/test"}
        });
        let event = map_codex_request("approval/request", &params, 99, "sess-11");
        match event {
            Some(UnifiedAgentEvent::PermissionRequest {
                session_id,
                request_id,
                tool_name,
                input,
            }) => {
                assert_eq!(session_id, "sess-11");
                assert_eq!(request_id, "99");
                assert_eq!(tool_name, "bash");
                assert_eq!(input["command"], "rm -rf /tmp/test");
            }
            other => panic!("expected PermissionRequest, got {other:?}"),
        }
    }

    #[test]
    fn codex_request_mapping_unknown_returns_none() {
        let params = serde_json::json!({});
        let event = map_codex_request("codex/unknownRequest", &params, 1, "sess-12");
        assert!(event.is_none());
    }

    // ── Adapter lifecycle tests ──

    #[test]
    fn new_adapter_has_codex_type() {
        let adapter = CodexAdapter::new();
        assert_eq!(adapter.agent_type(), AgentType::Codex);
    }

    #[test]
    fn new_adapter_is_starting() {
        let adapter = CodexAdapter::new();
        assert_eq!(adapter.status, AgentStatus::Starting);
        assert_eq!(adapter.info().status, AgentStatus::Starting);
    }

    #[test]
    fn new_adapter_info_has_name() {
        let adapter = CodexAdapter::new();
        assert_eq!(adapter.info().name, "OpenAI Codex");
    }

    #[test]
    fn new_adapter_has_all_capabilities() {
        let adapter = CodexAdapter::new();
        let info = adapter.info();
        assert!(info.capabilities.contains(&AgentCapability::Streaming));
        assert!(info.capabilities.contains(&AgentCapability::ToolUse));
        assert!(info.capabilities.contains(&AgentCapability::Subtasks));
        assert!(info.capabilities.contains(&AgentCapability::Permissions));
        assert_eq!(info.capabilities.len(), 4);
    }

    #[tokio::test]
    async fn start_without_binary_returns_error() {
        let mut adapter = CodexAdapter::new();
        let config = AgentConfig {
            agent_type: AgentType::Codex,
            binary_path: None,
            args: vec![],
            env: vec![],
            working_dir: None,
            model: None,
            provider: None,
            api_key: None,
            base_url: None,
        };
        let result = adapter.start(&config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("binary_path is required"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn start_with_nonexistent_binary_returns_error() {
        let mut adapter = CodexAdapter::new();
        let config = AgentConfig {
            agent_type: AgentType::Codex,
            binary_path: Some(std::path::PathBuf::from("/nonexistent/codex-binary")),
            args: vec![],
            env: vec![],
            working_dir: None,
            model: None,
            provider: None,
            api_key: None,
            base_url: None,
        };
        let result = adapter.start(&config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("binary not found"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn stop_when_not_started_is_ok() {
        let mut adapter = CodexAdapter::new();
        let result = adapter.stop().await;
        assert!(result.is_ok());
        assert_eq!(adapter.status, AgentStatus::Stopped);
        assert_eq!(adapter.info().status, AgentStatus::Stopped);
    }

    #[test]
    fn default_equals_new() {
        let new_adapter = CodexAdapter::new();
        let default_adapter = CodexAdapter::default();
        assert_eq!(new_adapter.agent_type(), default_adapter.agent_type());
        assert_eq!(new_adapter.status, default_adapter.status);
    }

    #[test]
    fn is_alive_when_not_started_is_false() {
        let adapter = CodexAdapter::new();
        assert!(!adapter.is_alive());
    }

    #[test]
    fn next_id_is_monotonic() {
        let adapter = CodexAdapter::new();
        let id1 = adapter.next_id();
        let id2 = adapter.next_id();
        let id3 = adapter.next_id();
        assert!(id2 > id1);
        assert!(id3 > id2);
    }
}
