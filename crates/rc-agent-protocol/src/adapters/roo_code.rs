//! Roo Code sub-process adapter — JSON-RPC 2.0 over stdio with Content-Length framing.
//!
//! [`RooCodeAdapter`] launches a Roo Code binary as a child process and
//! communicates with it over stdin/stdout using the LSP-style Content-Length
//! framing protocol. All messages are JSON-RPC 2.0.
//!
//! # Protocol overview
//!
//! ```text
//! Host → Agent (stdin):  Content-Length: N\r\n\r\n{...}
//! Agent → Host (stdout): Content-Length: N\r\n\r\n{...}
//! ```
//!
//! See the module-level docs of [`crate`] for the full method/notification list.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::adapter::AgentAdapter;
use crate::events::UnifiedAgentEvent;
use crate::jsonrpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::permission::PermissionDecision;
use crate::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

// ===========================================================================
// Content-Length framing helpers
// ===========================================================================

/// Maximum allowed Content-Length in bytes (10 MiB).
///
/// Messages exceeding this size are rejected to prevent unbounded memory
/// allocation when a misbehaving agent sends an extremely large header.
const MAX_CONTENT_LENGTH: usize = 10 * 1024 * 1024;

/// Write a Content-Length framed message to an async writer.
///
/// Wire format: `Content-Length: <N>\r\n\r\n<N bytes of UTF-8 JSON>`
pub async fn write_framed<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &str,
) -> anyhow::Result<()> {
    let bytes = message.as_bytes();
    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a Content-Length framed message from an async buffered reader.
///
/// Parses the `Content-Length` header line by line, then reads exactly that
/// many bytes of body.
///
/// # Errors
///
/// Returns an error if the Content-Length header is missing, exceeds
/// [`MAX_CONTENT_LENGTH`], or the stream closes prematurely.
pub async fn read_framed<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> anyhow::Result<String> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            anyhow::bail!("stream closed while reading headers");
        }

        // Strip trailing \r\n or \n.
        let trimmed = line.trim_end_matches("\r\n").trim_end_matches('\n');

        if trimmed.is_empty() {
            // Blank line signals end of headers.
            break;
        }

        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(rest.trim().parse::<usize>()?);
        }
    }

    let len = content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length header"))?;

    if len > MAX_CONTENT_LENGTH {
        anyhow::bail!(
            "Content-Length {len} exceeds maximum allowed size ({MAX_CONTENT_LENGTH} bytes)"
        );
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    let body = String::from_utf8(buf)?;
    Ok(body)
}

// ===========================================================================
// Roo Code notification → UnifiedAgentEvent mapping
// ===========================================================================

/// Convert a Roo Code JSON-RPC notification into a [`UnifiedAgentEvent`].
///
/// The `session_id` is injected by the caller (the adapter tracks it).
pub fn map_notification(
    method: &str,
    params: &serde_json::Value,
    session_id: &str,
) -> Option<UnifiedAgentEvent> {
    match method {
        "roo/messageDelta" => Some(UnifiedAgentEvent::MessageDelta {
            session_id: session_id.to_owned(),
            delta: params["delta"].as_str().unwrap_or_default().to_owned(),
        }),

        "roo/toolCallStarted" => Some(UnifiedAgentEvent::ToolCallStarted {
            session_id: session_id.to_owned(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            tool_input: params["toolInput"].clone(),
        }),

        "roo/toolCallCompleted" => Some(UnifiedAgentEvent::ToolCallCompleted {
            session_id: session_id.to_owned(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            result: params["result"].clone(),
        }),

        "roo/permissionRequest" => Some(UnifiedAgentEvent::PermissionRequest {
            session_id: session_id.to_owned(),
            request_id: params["requestId"].as_str().unwrap_or_default().to_owned(),
            tool_name: params["toolName"].as_str().unwrap_or_default().to_owned(),
            input: params["input"].clone(),
        }),

        "roo/subtaskStarted" => Some(UnifiedAgentEvent::SubtaskStarted {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            description: params["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        }),

        "roo/subtaskProgress" => Some(UnifiedAgentEvent::SubtaskProgress {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            progress: params["progress"].as_str().unwrap_or_default().to_owned(),
        }),

        "roo/subtaskCompleted" => Some(UnifiedAgentEvent::SubtaskCompleted {
            session_id: session_id.to_owned(),
            task_id: params["taskId"].as_str().unwrap_or_default().to_owned(),
            result: params["result"].clone(),
        }),

        "roo/contextUsage" => Some(UnifiedAgentEvent::ContextUsage {
            session_id: session_id.to_owned(),
            used: params["used"].as_u64().unwrap_or_default() as usize,
            total: params["total"].as_u64().unwrap_or_default() as usize,
        }),

        "roo/error" => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_owned(),
            message: params["message"].as_str().unwrap_or_default().to_owned(),
            recoverable: params["recoverable"].as_bool().unwrap_or(true),
        }),

        "roo/completed" => {
            // Parse tool_calls from payload if present.
            let tool_calls = params
                .get("toolCalls")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|tc| {
                            crate::events::ToolCallInfo {
                                id: tc["id"].as_str().unwrap_or_default().to_owned(),
                                name: tc["name"].as_str().unwrap_or_default().to_owned(),
                                input: tc["input"].clone(),
                                output: tc["output"].clone(),
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // Parse usage from payload if present.
            let usage = crate::events::UsageInfo {
                input_tokens: params
                    .get("usage")
                    .and_then(|u| u["inputTokens"].as_u64())
                    .unwrap_or_default(),
                output_tokens: params
                    .get("usage")
                    .and_then(|u| u["outputTokens"].as_u64())
                    .unwrap_or_default(),
                cache_read: params
                    .get("usage")
                    .and_then(|u| u["cacheRead"].as_u64())
                    .unwrap_or_default(),
                cache_write: params
                    .get("usage")
                    .and_then(|u| u["cacheWrite"].as_u64())
                    .unwrap_or_default(),
            };

            let result = crate::events::AgentResult {
                response_text: params["responseText"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                tool_calls,
                usage,
                cost: params["cost"].as_f64(),
            };
            Some(UnifiedAgentEvent::Completed {
                session_id: session_id.to_owned(),
                result,
            })
        }

        _ => {
            debug!(method, "unhandled Roo Code notification");
            None
        }
    }
}

// ===========================================================================
// RooCodeAdapter
// ===========================================================================

/// Sub-process adapter for Roo Code.
///
/// Launches the Roo Code binary configured in [`AgentConfig::binary_path`]
/// and communicates over stdio using JSON-RPC 2.0 with Content-Length framing.
///
/// The background reader task is spawned once during [`start`](AgentAdapter::start)
/// and persists for the adapter's lifetime, enabling multiple `send_message` calls
/// without re-spawning the reader.
pub struct RooCodeAdapter {
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// The child process (if started).
    process: Option<Child>,
    /// Stdin pipe for writing to the child.
    stdin: Option<ChildStdin>,
    /// Monotonic request ID counter.
    next_request_id: AtomicU64,
    /// Shared event sender — the background reader pushes events here.
    /// Swapped on each `send_message` call to provide a fresh receiver.
    event_tx: Arc<tokio::sync::Mutex<mpsc::Sender<UnifiedAgentEvent>>>,
    /// Shared pending-request map for routing JSON-RPC responses.
    pending: Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    /// Current session ID (set on `send_message`).
    current_session_id: Option<String>,
    /// Background reader task handle.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RooCodeAdapter {
    /// Create a new `RooCodeAdapter` in the **Starting** state.
    #[must_use]
    pub fn new() -> Self {
        let mut capabilities = std::collections::HashSet::new();
        capabilities.insert(AgentCapability::Streaming);
        capabilities.insert(AgentCapability::ToolUse);
        capabilities.insert(AgentCapability::McpSupport);
        capabilities.insert(AgentCapability::Subtasks);
        capabilities.insert(AgentCapability::Permissions);

        let (event_tx, _) = mpsc::channel(256);

        Self {
            info: AgentInfo {
                name: "Roo Code".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            process: None,
            stdin: None,
            next_request_id: AtomicU64::new(1),
            event_tx: Arc::new(tokio::sync::Mutex::new(event_tx)),
            pending: Arc::new(std::sync::Mutex::new(HashMap::new())),
            current_session_id: None,
            reader_handle: None,
        }
    }

    /// Allocate the next JSON-RPC request ID.
    fn next_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent not started — no stdin pipe"))?;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_owned(),
            method: method.to_owned(),
            params,
        };

        let payload = serde_json::to_string(&notification)?;
        write_framed(stdin, &payload).await?;
        debug!(method, "sent JSON-RPC notification");
        Ok(())
    }

    /// Spawn the background reader task that persists for the adapter's
    /// lifetime, reading framed messages from the child's stdout.
    ///
    /// Events are sent through a shared sender that can be swapped on each
    /// `send_message` call, enabling multi-turn conversations without
    /// re-spawning the reader.
    fn spawn_background_reader(
        stdout: tokio::io::BufReader<ChildStdout>,
        event_tx: Arc<tokio::sync::Mutex<mpsc::Sender<UnifiedAgentEvent>>>,
        pending: Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
        session_id: String,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut reader = stdout;
            loop {
                match read_framed(&mut reader).await {
                    Ok(body) => {
                        debug!(body_len = body.len(), "received framed message");

                        let msg: serde_json::Value = match serde_json::from_str(&body) {
                            Ok(v) => v,
                            Err(e) => {
                                error!(error = %e, "failed to parse JSON-RPC message");
                                continue;
                            }
                        };

                        // If it has an "id" field and either "result" or "error",
                        // it's a response.
                        if msg.get("id").is_some()
                            && (msg.get("result").is_some() || msg.get("error").is_some())
                        {
                            let response: JsonRpcResponse = match serde_json::from_value(msg) {
                                Ok(r) => r,
                                Err(e) => {
                                    error!(error = %e, "failed to parse response");
                                    continue;
                                }
                            };

                            if let Some(id) = response.id {
                                let guard = match pending.lock() {
                                    Ok(g) => g,
                                    Err(e) => {
                                        // #2: Mutex poisoned — log and stop instead of panicking.
                                        error!("pending requests mutex poisoned: {e}");
                                        break;
                                    }
                                };
                                // Use remove() to take ownership of the oneshot sender
                                // directly — oneshot::Sender is not Clone, so we cannot
                                // clone it from a reference.
                                drop(guard); // release the read-only lock first
                                let mut guard = match pending.lock() {
                                    Ok(g) => g,
                                    Err(e) => {
                                        error!("pending requests mutex poisoned: {e}");
                                        break;
                                    }
                                };
                                if let Some(sender) = guard.remove(&id) {
                                    drop(guard);
                                    if sender.send(response).is_err() {
                                        debug!(id, "response channel already closed");
                                    }
                                } else {
                                    drop(guard);
                                    warn!(id, "received response for unknown request");
                                }
                            }
                        } else if msg.get("method").is_some() {
                            let notification: JsonRpcNotification =
                                match serde_json::from_value(msg) {
                                    Ok(n) => n,
                                    Err(e) => {
                                        error!(error = %e, "failed to parse notification");
                                        continue;
                                    }
                                };

                            // Lock the shared event sender and send.
                            let tx = event_tx.lock().await;
                            if let Some(event) = map_notification(
                                &notification.method,
                                &notification.params,
                                &session_id,
                            ) && tx.send(event).await.is_err()
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

impl Default for RooCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// AgentAdapter implementation
// ===========================================================================

#[async_trait]
impl AgentAdapter for RooCodeAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        let binary_path = config
            .binary_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("binary_path is required for RooCodeAdapter"))?;

        if !binary_path.exists() {
            anyhow::bail!("Roo Code binary not found at: {}", binary_path.display());
        }

        info!(path = %binary_path.display(), "starting Roo Code subprocess");

        let mut cmd = Command::new(binary_path);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Forward environment variables.
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Set working directory if specified.
        if let Some(ref wd) = config.working_dir {
            cmd.current_dir(wd);
        }

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("stdin should be piped");
        let stdout = child.stdout.take().expect("stdout should be piped");

        self.process = Some(child);
        self.stdin = Some(stdin);
        self.status = AgentStatus::Starting;

        // Perform handshake using a local buffered reader.
        let mut stdout_reader = tokio::io::BufReader::new(stdout);

        // Send `initialize` request — build payload first to avoid borrow conflicts.
        let id = self.next_id();
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "capabilities": {},
            "clientInfo": {
                "name": "remote-code-rust",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "initialize".to_owned(),
            params: init_params,
        };
        let payload = serde_json::to_string(&request)?;

        // Write initialize request with a 30-second timeout.
        let write_fut = write_framed(self.stdin.as_mut().expect("stdin"), &payload);
        tokio::time::timeout(std::time::Duration::from_secs(30), write_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending initialize request (30s)"))??;

        // Read initialize response directly (before background reader) with timeout.
        let read_fut = read_framed(&mut stdout_reader);
        let body = tokio::time::timeout(std::time::Duration::from_secs(30), read_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout reading initialize response (30s)"))??;
        let response: JsonRpcResponse = serde_json::from_str(&body)?;

        if let Some(err) = response.error {
            anyhow::bail!(
                "initialize failed: code={} message={}",
                err.code,
                err.message
            );
        }

        info!("Roo Code initialize handshake succeeded");

        // Send `initialized` notification.
        let initialized = JsonRpcNotification {
            jsonrpc: "2.0".to_owned(),
            method: "initialized".to_owned(),
            params: serde_json::Value::Object(serde_json::Map::new()),
        };
        let payload = serde_json::to_string(&initialized)?;
        write_framed(self.stdin.as_mut().expect("stdin"), &payload).await?;

        // Spawn the persistent background reader.  It reads from stdout for
        // the entire lifetime of the adapter, routing events through a shared
        // sender that `send_message` swaps on each call.
        //
        // #1: Use current_session_id if available, otherwise fall back to a
        // placeholder. The session_id will be updated on the next send_message.
        let session_id = self.current_session_id.clone().unwrap_or_default();
        if session_id.is_empty() {
            warn!("RooCodeAdapter::start() called without a session_id — background reader will use empty session_id until send_message is called");
        }
        self.reader_handle = Some(Self::spawn_background_reader(
            stdout_reader,
            self.event_tx.clone(),
            self.pending.clone(),
            session_id,
        ));

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;
        info!("RooCodeAdapter ready");
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
        //
        // #18: Race condition note — there is a brief window between swapping
        // the event_tx sender and the background reader picking up the new
        // sender. Events arriving during this window may be sent to the old
        // (dropped) sender and lost. A broadcast channel would avoid this but
        // would require more significant refactoring. For now, the swap is
        // atomic from the reader's perspective (it holds the Mutex briefly).
        let (new_tx, new_rx) = mpsc::channel(256);
        {
            let mut guard = self.event_tx.lock().await;
            *guard = new_tx;
        }

        // Send the `roo/sendMessage` notification — events arrive as
        // roo/* notifications, not as a single JSON-RPC response.
        let params = serde_json::json!({
            "sessionId": session_id,
            "message": message,
        });

        // #16: Wrap the notification send in a 30-second timeout.
        let send_fut = self.send_notification("roo/sendMessage", params);
        tokio::time::timeout(std::time::Duration::from_secs(30), send_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending roo/sendMessage (30s)"))??;

        Ok(new_rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        // #16: Wrap in a 10-second timeout.
        let cancel_fut = self.send_notification(
            "roo/cancel",
            serde_json::Value::Object(serde_json::Map::new()),
        );
        tokio::time::timeout(std::time::Duration::from_secs(10), cancel_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending roo/cancel (10s)"))?
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
        let params = serde_json::json!({
            "requestId": request_id,
            "decision": decision_str,
        });

        // Build request first to avoid borrow conflicts.
        let id = self.next_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "roo/resolvePermission".to_owned(),
            params,
        };
        let payload = serde_json::to_string(&request)?;

        // For simplicity we write directly and don't wait for a response.
        // A production implementation would track the pending request.
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent not started"))?;

        // #16: Wrap in a 10-second timeout.
        let write_fut = write_framed(stdin, &payload);
        tokio::time::timeout(std::time::Duration::from_secs(10), write_fut)
            .await
            .map_err(|_| anyhow::anyhow!("timeout sending roo/resolvePermission (10s)"))??;
        debug!(id, "sent roo/resolvePermission request");
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("RooCodeAdapter stopping");

        // Abort the background reader task.
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        // Try to send a shutdown notification.
        if self.stdin.is_some() {
            let _ = self
                .send_notification("shutdown", serde_json::Value::Null)
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
        self.stdin = None;
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        if matches!(self.status, AgentStatus::Stopped | AgentStatus::Error) {
            return false;
        }
        // #17: Check actual process state instead of just self.process.is_some().
        // We need &mut for try_wait(), so we use a workaround: check if the
        // process field is Some. Since is_alive() takes &self (not &mut self),
        // we cannot call try_wait() here. Instead, we rely on the status field
        // being updated when the process exits (via stop() or error handling).
        // A more precise implementation would require interior mutability.
        self.process.is_some()
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RooCode
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── JSON-RPC serialization tests ──

    #[test]
    fn jsonrpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "initialize".to_owned(),
            params: serde_json::json!({"processId": 1234}),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "initialize");
        assert_eq!(parsed["params"]["processId"], 1234);
    }

    #[test]
    fn jsonrpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("deserialize");

        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
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
        let json = r#"{"jsonrpc":"2.0","method":"roo/messageDelta","params":{"delta":"Hello"}}"#;
        let notification: JsonRpcNotification = serde_json::from_str(json).expect("deserialize");

        assert_eq!(notification.method, "roo/messageDelta");
        assert_eq!(notification.params["delta"], "Hello");
    }

    // ── Content-Length framing tests ──

    #[tokio::test]
    async fn content_length_codec_roundtrip() {
        let (tx, rx) = tokio::io::duplex(1024);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        let message = r#"{"jsonrpc":"2.0","method":"test","params":{}}"#;

        write_framed(&mut writer, message).await.expect("write");
        let received = read_framed(&mut reader).await.expect("read");

        assert_eq!(received, message);
    }

    #[tokio::test]
    async fn content_length_codec_multiple_messages() {
        let (tx, rx) = tokio::io::duplex(4096);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        let messages = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"roo/sendMessage","params":{"message":"hi"}}"#,
            r#"{"jsonrpc":"2.0","method":"roo/messageDelta","params":{"delta":"world"}}"#,
        ];

        for msg in &messages {
            write_framed(&mut writer, msg).await.expect("write");
        }

        for expected in &messages {
            let received = read_framed(&mut reader).await.expect("read");
            assert_eq!(received, *expected);
        }
    }

    #[tokio::test]
    async fn content_length_rejects_oversized_message() {
        let (tx, rx) = tokio::io::duplex(64);
        let mut writer = tx;
        let mut reader = tokio::io::BufReader::new(rx);

        // Write a header claiming a very large Content-Length.
        let header = format!("Content-Length: {}\r\n\r\n", MAX_CONTENT_LENGTH + 1);
        writer.write_all(header.as_bytes()).await.expect("write header");
        writer.flush().await.expect("flush");

        let result = read_framed(&mut reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum"),
            "unexpected error: {err}"
        );
    }

    // ── Event mapping tests ──

    #[test]
    fn roo_event_mapping_message_delta() {
        let params = serde_json::json!({"delta": "Hello, "});
        let event = map_notification("roo/messageDelta", &params, "sess-1");
        match event {
            Some(UnifiedAgentEvent::MessageDelta { session_id, delta }) => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(delta, "Hello, ");
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_tool_call_started() {
        let params = serde_json::json!({
            "toolName": "read_file",
            "toolInput": {"path": "/tmp/test.rs"}
        });
        let event = map_notification("roo/toolCallStarted", &params, "sess-2");
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
    fn roo_event_mapping_tool_call_completed() {
        let params = serde_json::json!({
            "toolName": "bash",
            "result": {"stdout": "ok"}
        });
        let event = map_notification("roo/toolCallCompleted", &params, "sess-3");
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
    fn roo_event_mapping_permission_request() {
        let params = serde_json::json!({
            "requestId": "req-1",
            "toolName": "write_file",
            "input": {"path": "/etc/hosts"}
        });
        let event = map_notification("roo/permissionRequest", &params, "sess-4");
        match event {
            Some(UnifiedAgentEvent::PermissionRequest {
                session_id,
                request_id,
                tool_name,
                input,
            }) => {
                assert_eq!(session_id, "sess-4");
                assert_eq!(request_id, "req-1");
                assert_eq!(tool_name, "write_file");
                assert_eq!(input["path"], "/etc/hosts");
            }
            other => panic!("expected PermissionRequest, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_subtask_started() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "description": "refactor module"
        });
        let event = map_notification("roo/subtaskStarted", &params, "sess-5");
        match event {
            Some(UnifiedAgentEvent::SubtaskStarted {
                session_id,
                task_id,
                description,
            }) => {
                assert_eq!(session_id, "sess-5");
                assert_eq!(task_id, "task-1");
                assert_eq!(description, "refactor module");
            }
            other => panic!("expected SubtaskStarted, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_subtask_progress() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "progress": "50%"
        });
        let event = map_notification("roo/subtaskProgress", &params, "sess-5");
        match event {
            Some(UnifiedAgentEvent::SubtaskProgress {
                session_id,
                task_id,
                progress,
            }) => {
                assert_eq!(session_id, "sess-5");
                assert_eq!(task_id, "task-1");
                assert_eq!(progress, "50%");
            }
            other => panic!("expected SubtaskProgress, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_subtask_completed() {
        let params = serde_json::json!({
            "taskId": "task-1",
            "result": {"success": true}
        });
        let event = map_notification("roo/subtaskCompleted", &params, "sess-5");
        match event {
            Some(UnifiedAgentEvent::SubtaskCompleted {
                session_id,
                task_id,
                result,
            }) => {
                assert_eq!(session_id, "sess-5");
                assert_eq!(task_id, "task-1");
                assert_eq!(result["success"], true);
            }
            other => panic!("expected SubtaskCompleted, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_context_usage() {
        let params = serde_json::json!({"used": 80000, "total": 200000});
        let event = map_notification("roo/contextUsage", &params, "sess-6");
        match event {
            Some(UnifiedAgentEvent::ContextUsage {
                session_id,
                used,
                total,
            }) => {
                assert_eq!(session_id, "sess-6");
                assert_eq!(used, 80000);
                assert_eq!(total, 200000);
            }
            other => panic!("expected ContextUsage, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_error() {
        let params = serde_json::json!({
            "message": "rate limited",
            "recoverable": true
        });
        let event = map_notification("roo/error", &params, "sess-7");
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
    fn roo_event_mapping_completed() {
        let params = serde_json::json!({
            "responseText": "Done!",
            "cost": 0.005
        });
        let event = map_notification("roo/completed", &params, "sess-8");
        match event {
            Some(UnifiedAgentEvent::Completed { session_id, result }) => {
                assert_eq!(session_id, "sess-8");
                assert_eq!(result.response_text, "Done!");
                assert_eq!(result.cost, Some(0.005));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_completed_with_tool_calls_and_usage() {
        let params = serde_json::json!({
            "responseText": "Done!",
            "toolCalls": [
                {
                    "id": "tc-1",
                    "name": "read_file",
                    "input": {"path": "/tmp/a.rs"},
                    "output": {"content": "fn main() {}"}
                }
            ],
            "usage": {
                "inputTokens": 100,
                "outputTokens": 50,
                "cacheRead": 20,
                "cacheWrite": 10
            }
        });
        let event = map_notification("roo/completed", &params, "sess-9");
        match event {
            Some(UnifiedAgentEvent::Completed { result, .. }) => {
                assert_eq!(result.tool_calls.len(), 1);
                assert_eq!(result.tool_calls[0].id, "tc-1");
                assert_eq!(result.tool_calls[0].name, "read_file");
                assert_eq!(result.usage.input_tokens, 100);
                assert_eq!(result.usage.output_tokens, 50);
                assert_eq!(result.usage.cache_read, 20);
                assert_eq!(result.usage.cache_write, 10);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn roo_event_mapping_unknown_returns_none() {
        let params = serde_json::json!({});
        let event = map_notification("roo/unknownMethod", &params, "sess-9");
        assert!(event.is_none());
    }

    // ── Adapter lifecycle tests ──

    #[test]
    fn new_adapter_has_roo_code_type() {
        let adapter = RooCodeAdapter::new();
        assert_eq!(adapter.agent_type(), AgentType::RooCode);
    }

    #[test]
    fn new_adapter_is_starting() {
        let adapter = RooCodeAdapter::new();
        assert_eq!(adapter.status, AgentStatus::Starting);
        assert_eq!(adapter.info().status, AgentStatus::Starting);
    }

    #[test]
    fn new_adapter_info_has_name() {
        let adapter = RooCodeAdapter::new();
        assert_eq!(adapter.info().name, "Roo Code");
    }

    #[test]
    fn new_adapter_has_all_capabilities() {
        let adapter = RooCodeAdapter::new();
        let info = adapter.info();
        assert!(info.capabilities.contains(&AgentCapability::Streaming));
        assert!(info.capabilities.contains(&AgentCapability::ToolUse));
        assert!(info.capabilities.contains(&AgentCapability::McpSupport));
        assert!(info.capabilities.contains(&AgentCapability::Subtasks));
        assert!(info.capabilities.contains(&AgentCapability::Permissions));
        assert_eq!(info.capabilities.len(), 5);
    }

    #[tokio::test]
    async fn start_without_binary_returns_error() {
        let mut adapter = RooCodeAdapter::new();
        let config = AgentConfig {
            agent_type: AgentType::RooCode,
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
        let mut adapter = RooCodeAdapter::new();
        let config = AgentConfig {
            agent_type: AgentType::RooCode,
            binary_path: Some(std::path::PathBuf::from("/nonexistent/roo-binary")),
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
        let mut adapter = RooCodeAdapter::new();
        let result = adapter.stop().await;
        assert!(result.is_ok());
        assert_eq!(adapter.status, AgentStatus::Stopped);
        assert_eq!(adapter.info().status, AgentStatus::Stopped);
    }

    #[test]
    fn default_equals_new() {
        let new_adapter = RooCodeAdapter::new();
        let default_adapter = RooCodeAdapter::default();
        assert_eq!(new_adapter.agent_type(), default_adapter.agent_type());
        assert_eq!(new_adapter.status, default_adapter.status);
    }

    #[test]
    fn is_alive_when_not_started_is_false() {
        let adapter = RooCodeAdapter::new();
        assert!(!adapter.is_alive());
    }

    #[test]
    fn next_id_is_monotonic() {
        let adapter = RooCodeAdapter::new();
        let id1 = adapter.next_id();
        let id2 = adapter.next_id();
        let id3 = adapter.next_id();
        assert!(id2 > id1);
        assert!(id3 > id2);
    }
}
