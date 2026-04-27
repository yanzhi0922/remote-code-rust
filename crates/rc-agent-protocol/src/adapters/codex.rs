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
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::adapter::AgentAdapter;
use crate::events::UnifiedAgentEvent;
use crate::permission::PermissionDecision;
use crate::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

// ===========================================================================
// NDJSON codec
// ===========================================================================

/// Maximum allowed line length in bytes (10 MiB).
///
/// Lines exceeding this size are rejected to prevent unbounded memory
/// allocation when a misbehaving agent sends extremely long lines.
const MAX_LINE_LENGTH: usize = 10 * 1024 * 1024;

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
        if line.len() > MAX_LINE_LENGTH {
            anyhow::bail!(
                "NDJSON line length {} exceeds maximum allowed size ({MAX_LINE_LENGTH} bytes)",
                line.len()
            );
        }
        writer.write_all(line.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Read a single NDJSON line and parse it as a JSON value.
    ///
    /// Empty lines (including lines containing only whitespace) are skipped.
    ///
    /// # Errors
    ///
    /// Returns an error if a line exceeds [`MAX_LINE_LENGTH`], the stream
    /// closes, or the line is not valid JSON.
    pub async fn read_line<R: AsyncBufReadExt + Unpin>(
        reader: &mut R,
    ) -> anyhow::Result<serde_json::Value> {
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                anyhow::bail!("stream closed while reading NDJSON line");
            }
            // #27: Reject oversized lines.
            if line.len() > MAX_LINE_LENGTH {
                anyhow::bail!(
                    "NDJSON line length {} exceeds maximum allowed size ({MAX_LINE_LENGTH} bytes)",
                    line.len()
                );
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

        // #23: Map task/completed to Completed event (not just SubtaskCompleted).
        "task/completed" => {
            let response_text = params["result"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let result = crate::events::AgentResult {
                response_text,
                tool_calls: vec![],
                usage: crate::events::UsageInfo::default(),
                cost: params["cost"].as_f64(),
            };
            Some(UnifiedAgentEvent::Completed {
                session_id: session_id.to_owned(),
                result,
            })
        }

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
    /// Monotonic request ID counter.
    next_request_id: AtomicU64,
    /// Shared event sender — the background reader pushes events here.
    /// Swapped on each `send_message` call to provide a fresh receiver.
    event_tx: Arc<tokio::sync::Mutex<mpsc::Sender<UnifiedAgentEvent>>>,
    /// Shared pending-request map for routing JSON-RPC responses.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    /// Current Codex session ID (set during `start` handshake).
    current_session_id: Option<String>,
    /// Background reader task handle.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
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

        let (event_tx, _) = mpsc::channel(256);

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
            next_request_id: AtomicU64::new(1),
            event_tx: Arc::new(tokio::sync::Mutex::new(event_tx)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            current_session_id: None,
            reader_handle: None,
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

    /// Spawn the background reader task that persists for the adapter's
    /// lifetime, reading NDJSON lines from the child's stdout.
    ///
    /// Events are sent through a shared sender that can be swapped on each
    /// `send_message` call, enabling multi-turn conversations without
    /// re-spawning the reader.
    fn spawn_background_reader(
        stdout: tokio::io::BufReader<ChildStdout>,
        event_tx: Arc<tokio::sync::Mutex<mpsc::Sender<UnifiedAgentEvent>>>,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
        session_id: String,
    ) -> tokio::task::JoinHandle<()> {
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
                            let mut guard = match pending.lock() {
                                Ok(g) => g,
                                Err(e) => {
                                    // #2: Mutex poisoned — log and stop instead of panicking.
                                    error!("pending requests mutex poisoned: {e}");
                                    break;
                                }
                            };
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

                            let tx = event_tx.lock().await;
                            if let Some(event) =
                                map_codex_request(method, &params, rpc_id, &session_id)
                                && tx.send(event).await.is_err()
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

                            let tx = event_tx.lock().await;
                            if let Some(event) =
                                map_codex_notification(method, &params, &session_id)
                                && tx.send(event).await.is_err()
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
        })
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
        self.status = AgentStatus::Starting;
        self.config = Some(config.clone());

        // Perform handshake using a local buffered reader.
        let mut stdout_reader = tokio::io::BufReader::new(stdout);

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

        // #16: Wrap handshake write in a 30-second timeout.
        let write_fut = NdjsonCodec::write_line(self.stdin_writer.as_mut().expect("stdin"), &request);
        tokio::time::timeout(std::time::Duration::from_secs(30), write_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending session/create (30s)"))??;

        // Read session/create response directly (before background reader) with timeout.
        let read_fut = NdjsonCodec::read_line(&mut stdout_reader);
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), read_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout reading session/create response (30s)"))??;

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

        // Spawn the persistent background reader.  It reads from stdout for
        // the entire lifetime of the adapter, routing events through a shared
        // sender that `send_message` swaps on each call.
        self.reader_handle = Some(Self::spawn_background_reader(
            stdout_reader,
            self.event_tx.clone(),
            self.pending.clone(),
            self.current_session_id.clone().unwrap_or_default(),
        ));

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

        // Create a fresh event channel and swap the shared sender so that
        // the persistent background reader routes new events here.
        let (new_tx, new_rx) = mpsc::channel(256);
        {
            let mut guard = self.event_tx.lock().await;
            *guard = new_tx;
        }

        // Send task/create — events arrive as Codex notifications.
        let params = serde_json::json!({
            "sessionId": session_id,
            "prompt": message,
        });

        // #16: Wrap in a 30-second timeout.
        let send_fut = self.send_notification("task/create", params);
        tokio::time::timeout(std::time::Duration::from_secs(30), send_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending task/create (30s)"))??;

        Ok(new_rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        // #16: Wrap in a 10-second timeout.
        let cancel_fut = self.send_notification(
            "task/cancel",
            serde_json::Value::Object(serde_json::Map::new()),
        );
        tokio::time::timeout(std::time::Duration::from_secs(10), cancel_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending task/cancel (10s)"))?
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

        // #14: Parse request_id strictly — return error on failure instead of
        // silently using 0 which would route the response to the wrong request.
        let rpc_id: u64 = request_id.parse().map_err(|_| {
            anyhow::anyhow!(
                "invalid request_id '{request_id}': expected a numeric JSON-RPC id"
            )
        })?;

        let result = serde_json::json!({
            "decision": decision_str,
        });

        self.send_response(rpc_id, result).await?;
        debug!(request_id, "resolved Codex permission request");
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("CodexAdapter stopping");

        // Abort the background reader task.
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

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
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        if matches!(self.status, AgentStatus::Stopped | AgentStatus::Error) {
            return false;
        }
        // #17: Check actual process state instead of just self.process.is_some().
        // Since is_alive() takes &self (not &mut self), we cannot call try_wait()
        // here. We rely on the status field being updated when the process exits
        // (via stop() or error handling). A more precise implementation would
        // require interior mutability.
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
        // #23: task/completed now maps to Completed (not SubtaskCompleted).
        match event {
            Some(UnifiedAgentEvent::Completed { session_id, result }) => {
                assert_eq!(session_id, "sess-6");
                assert_eq!(result.response_text, "");
            }
            other => panic!("expected Completed, got {other:?}"),
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
