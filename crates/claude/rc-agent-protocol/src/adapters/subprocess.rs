//! Subprocess-based Agent adapter.
//!
//! [`SubprocessAdapter`] communicates with an Agent binary over JSON-RPC via
//! stdio. The subprocess is started with `--serve-stdio` and communicates
//! using newline-delimited JSON-RPC 2.0 messages:
//!
//! - Each line written to the subprocess's **stdin** is a JSON-RPC request
//!   or notification.
//! - Each line read from the subprocess's **stdout** is a JSON-RPC response
//!   or notification.
//!
//! A background tokio task continuously reads from stdout, parses messages,
//! and forwards them as [`UnifiedAgentEvent`] values to the current sender.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use crate::adapter::AgentAdapter;
use crate::bridge_proto;
use crate::error::AgentProtocolError;
use crate::events::{AgentResult, UnifiedAgentEvent};
use crate::jsonrpc::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::permission::PermissionDecision;
use crate::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

// ---------------------------------------------------------------------------
// Shared reader state
// ---------------------------------------------------------------------------

/// State shared between [`SubprocessAdapter`] and the background stdout
/// reader task.
struct ReaderState {
    /// The sender that should receive events from the reader task.
    /// Set to `Some(tx)` when a request is active, `None` otherwise.
    current_tx: Option<mpsc::Sender<UnifiedAgentEvent>>,
}

// ---------------------------------------------------------------------------
// SubprocessAdapter
// ---------------------------------------------------------------------------

/// Subprocess-based Agent adapter that communicates via JSON-RPC over stdio.
///
/// The subprocess is started with `--serve-stdio` and communicates using
/// newline-delimited JSON-RPC 2.0 messages. Each line on stdout is a
/// JSON-RPC response or notification; each line written to stdin is a
/// JSON-RPC request or notification.
///
/// # Lifecycle
///
/// 1. [`new()`](SubprocessAdapter::new) — create in `Starting` state.
/// 2. [`start()`](AgentAdapter::start) — spawn the subprocess.
/// 3. [`send_message()`](AgentAdapter::send_message) — send a user message,
///    receive events via the returned channel.
/// 4. [`stop()`](AgentAdapter::stop) — kill the subprocess.
pub struct SubprocessAdapter {
    /// The agent type (`RemoteCodex` or `RemoteRoo`).
    agent_type: AgentType,
    /// Path to the agent binary.
    binary_path: PathBuf,
    /// The child process handle (for killing on stop).
    child: Option<Child>,
    /// Stdin of the child process (for writing requests).
    stdin: Option<ChildStdin>,
    /// Runtime status.
    status: AgentStatus,
    /// Static agent metadata.
    info: AgentInfo,
    /// Next JSON-RPC request ID (monotonically increasing).
    next_request_id: u64,
    /// Shared state with the background reader task.
    reader_state: Arc<Mutex<ReaderState>>,
    /// Whether the background reader task is still alive.
    reader_alive: Arc<AtomicBool>,
}

impl SubprocessAdapter {
    /// Create a new `SubprocessAdapter` for the given agent type.
    ///
    /// The adapter is created in the `Starting` state. Call
    /// [`start`](AgentAdapter::start) to spawn the subprocess.
    pub fn new(agent_type: AgentType, binary_path: PathBuf) -> Self {
        let (name, caps) = match agent_type {
            AgentType::RemoteCodex => {
                let mut c = HashSet::new();
                c.insert(AgentCapability::Streaming);
                c.insert(AgentCapability::ToolUse);
                c.insert(AgentCapability::Subtasks);
                c.insert(AgentCapability::Permissions);
                ("Remote Codex (subprocess)", c)
            }
            AgentType::RemoteRoo => {
                let mut c = HashSet::new();
                c.insert(AgentCapability::Streaming);
                c.insert(AgentCapability::ToolUse);
                c.insert(AgentCapability::McpSupport);
                c.insert(AgentCapability::Subtasks);
                c.insert(AgentCapability::Permissions);
                ("Remote Roo (subprocess)", c)
            }
            AgentType::RemoteClaude => {
                let mut c = HashSet::new();
                c.insert(AgentCapability::Streaming);
                c.insert(AgentCapability::ToolUse);
                c.insert(AgentCapability::McpSupport);
                c.insert(AgentCapability::Subtasks);
                c.insert(AgentCapability::Permissions);
                ("Remote Claude (subprocess)", c)
            }
        };

        Self {
            agent_type,
            binary_path,
            child: None,
            stdin: None,
            status: AgentStatus::Starting,
            info: AgentInfo {
                name: name.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            next_request_id: 1,
            reader_state: Arc::new(Mutex::new(ReaderState { current_tx: None })),
            reader_alive: Arc::new(AtomicBool::new(false)),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Allocate the next JSON-RPC request ID.
    fn alloc_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    /// Send a JSON-RPC request to the subprocess via stdin.
    async fn write_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<u64> {
        // Allocate ID before borrowing stdin to satisfy the borrow checker.
        let id = self.alloc_request_id();

        let stdin = self
            .stdin
            .as_mut()
            .ok_or(AgentProtocolError::AgentNotStarted)?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        };

        let mut line = serde_json::to_string(&request)?;
        line.push('\n');

        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;

        Ok(id)
    }

    /// Send a JSON-RPC notification (no ID, no response expected).
    async fn write_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(AgentProtocolError::AgentNotStarted)?;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        };

        let mut line = serde_json::to_string(&notification)?;
        line.push('\n');

        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;

        Ok(())
    }

    /// Spawn the background reader task that reads from stdout.
    fn spawn_reader_task(
        stdout: ChildStdout,
        state: Arc<Mutex<ReaderState>>,
        alive: Arc<AtomicBool>,
    ) {
        alive.store(true, Ordering::Relaxed);

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.is_empty() {
                    continue;
                }

                // Parse as JSON-RPC message
                let msg: JsonRpcMessage = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(
                            error = %e,
                            line = %line,
                            "failed to parse JSON-RPC message from subprocess"
                        );
                        continue;
                    }
                };

                // Convert to UnifiedAgentEvent
                let event = Self::convert_message(&msg);

                if let Some(event) = event {
                    let is_terminal = matches!(
                        &event,
                        UnifiedAgentEvent::Completed { .. }
                            | UnifiedAgentEvent::Error { .. }
                            | UnifiedAgentEvent::Stopped
                    );

                    let mut guard = state.lock().await;
                    if let Some(tx) = guard.current_tx.as_ref()
                        && tx.send(event).await.is_err()
                    {
                        // Receiver was dropped
                        guard.current_tx = None;
                    }

                    if is_terminal {
                        guard.current_tx = None;
                    }
                }
            }

            // Reader task ended — subprocess stdout closed
            alive.store(false, Ordering::Relaxed);

            let mut guard = state.lock().await;
            if let Some(tx) = guard.current_tx.take() {
                let _ = tx.send(UnifiedAgentEvent::Stopped).await;
            }
        });
    }

    // -----------------------------------------------------------------------
    // JSON-RPC → UnifiedAgentEvent conversion
    // -----------------------------------------------------------------------

    /// Convert a JSON-RPC message from the subprocess into a
    /// [`UnifiedAgentEvent`].
    fn convert_message(msg: &JsonRpcMessage) -> Option<UnifiedAgentEvent> {
        match msg {
            JsonRpcMessage::Response(resp) => Self::convert_response(resp),
            JsonRpcMessage::Notification(notif) => {
                Self::convert_notification(&notif.method, &notif.params)
            }
        }
    }

    /// Convert a JSON-RPC response into a [`UnifiedAgentEvent`].
    fn convert_response(resp: &JsonRpcResponse) -> Option<UnifiedAgentEvent> {
        if let Some(error) = &resp.error {
            return Some(UnifiedAgentEvent::Error {
                session_id: String::new(),
                message: error.message.clone(),
                recoverable: error.code >= 0,
            });
        }

        let result = resp.result.as_ref()?;

        // Try to deserialize the result directly as a UnifiedAgentEvent
        serde_json::from_value(result.clone()).ok()
    }

    /// Convert a JSON-RPC notification into a [`UnifiedAgentEvent`].
    fn convert_notification(method: &str, params: &serde_json::Value) -> Option<UnifiedAgentEvent> {
        match method {
            // ── Lifecycle ──
            bridge_proto::NOTIFY_STARTED => {
                let info: AgentInfo = serde_json::from_value(params.clone()).ok()?;
                Some(UnifiedAgentEvent::Started(info))
            }
            bridge_proto::NOTIFY_READY => Some(UnifiedAgentEvent::Ready),

            // ── Message streaming ──
            bridge_proto::NOTIFY_MESSAGE_DELTA => Some(UnifiedAgentEvent::MessageDelta {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                delta: params.get("delta")?.as_str()?.to_string(),
            }),
            bridge_proto::NOTIFY_TOOL_CALL_STARTED => Some(UnifiedAgentEvent::ToolCallStarted {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                tool_name: params.get("tool_name")?.as_str()?.to_string(),
                tool_input: params
                    .get("tool_input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }),
            bridge_proto::NOTIFY_TOOL_CALL_PROGRESS => Some(UnifiedAgentEvent::ToolCallProgress {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                tool_name: params.get("tool_name")?.as_str()?.to_string(),
                progress: params.get("progress")?.as_str()?.to_string(),
            }),
            bridge_proto::NOTIFY_TOOL_CALL_COMPLETED => {
                Some(UnifiedAgentEvent::ToolCallCompleted {
                    session_id: params.get("session_id")?.as_str()?.to_string(),
                    tool_name: params.get("tool_name")?.as_str()?.to_string(),
                    result: params
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                })
            }

            // ── Permissions ──
            bridge_proto::NOTIFY_PERMISSION_REQUEST => Some(UnifiedAgentEvent::PermissionRequest {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                request_id: params.get("request_id")?.as_str()?.to_string(),
                tool_name: params.get("tool_name")?.as_str()?.to_string(),
                input: params
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }),

            // ── Subtasks ──
            bridge_proto::NOTIFY_SUBTASK_STARTED => Some(UnifiedAgentEvent::SubtaskStarted {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                task_id: params.get("task_id")?.as_str()?.to_string(),
                description: params.get("description")?.as_str()?.to_string(),
            }),
            bridge_proto::NOTIFY_SUBTASK_PROGRESS => Some(UnifiedAgentEvent::SubtaskProgress {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                task_id: params.get("task_id")?.as_str()?.to_string(),
                progress: params.get("progress")?.as_str()?.to_string(),
            }),
            bridge_proto::NOTIFY_SUBTASK_COMPLETED => Some(UnifiedAgentEvent::SubtaskCompleted {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                task_id: params.get("task_id")?.as_str()?.to_string(),
                result: params
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }),

            // ── Context management ──
            bridge_proto::NOTIFY_CONTEXT_USAGE => Some(UnifiedAgentEvent::ContextUsage {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                used: params.get("used")?.as_u64()? as usize,
                total: params.get("total")?.as_u64()? as usize,
            }),
            bridge_proto::NOTIFY_CONTEXT_OVERFLOW => Some(UnifiedAgentEvent::ContextOverflow {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                used: params.get("used").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                total: params.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            }),
            bridge_proto::NOTIFY_CONTEXT_COMPACTED => Some(UnifiedAgentEvent::ContextCompacted {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                entries_removed: params
                    .get("entries_removed")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                usage_ratio: params
                    .get("usage_ratio")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            }),

            // ── Terminal states ──
            bridge_proto::NOTIFY_ERROR => Some(UnifiedAgentEvent::Error {
                session_id: params.get("session_id")?.as_str()?.to_string(),
                message: params.get("message")?.as_str()?.to_string(),
                recoverable: params
                    .get("recoverable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }),
            bridge_proto::NOTIFY_DONE => {
                let session_id = params.get("session_id")?.as_str()?.to_string();
                let result_value = params
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let result: AgentResult = serde_json::from_value(result_value).ok()?;
                Some(UnifiedAgentEvent::Completed { session_id, result })
            }
            bridge_proto::NOTIFY_STOPPED => Some(UnifiedAgentEvent::Stopped),

            _ => {
                warn!(method = %method, "unknown notification method from subprocess");
                None
            }
        }
    }
}

// ===========================================================================
// AgentAdapter implementation
// ===========================================================================

#[async_trait]
impl AgentAdapter for SubprocessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!(
            agent = %self.agent_type,
            binary = %self.binary_path.display(),
            "starting subprocess adapter"
        );

        // Resolve binary path
        let binary_path = if self.binary_path.is_absolute() {
            self.binary_path.clone()
        } else {
            std::env::current_dir()?.join(&self.binary_path)
        };

        if !binary_path.exists() {
            return Err(AgentProtocolError::ConfigError {
                message: format!("agent binary not found: {}", binary_path.display()),
            }
            .into());
        }

        // Build the command
        let mut cmd = Command::new(&binary_path);
        cmd.arg("--serve-stdio")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set working directory
        if let Some(working_dir) = &config.working_dir {
            cmd.current_dir(working_dir);
        }

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Forward API key if present
        if let Some(api_key) = &config.api_key {
            cmd.env("API_KEY", api_key);
        }

        // Spawn the subprocess
        let mut child = cmd
            .spawn()
            .map_err(|e| AgentProtocolError::CommunicationError {
                details: format!("failed to spawn agent subprocess: {e}"),
            })?;

        // Take stdin and stdout handles
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentProtocolError::CommunicationError {
                details: "failed to get stdin of agent subprocess".into(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentProtocolError::CommunicationError {
                details: "failed to get stdout of agent subprocess".into(),
            })?;

        // Spawn the background reader task
        Self::spawn_reader_task(stdout, self.reader_state.clone(), self.reader_alive.clone());

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!(agent = %self.agent_type, "subprocess adapter started");
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        if !matches!(self.status, AgentStatus::Ready | AgentStatus::Idle) {
            return Err(AgentProtocolError::AgentNotStarted.into());
        }

        // Create the caller channel
        let (tx, rx) = mpsc::channel(256);

        // Register the sender BEFORE writing the request so the reader task
        // can immediately forward events.
        {
            let mut state = self.reader_state.lock().await;
            state.current_tx = Some(tx);
        }

        // Send the message as a JSON-RPC request
        let params = serde_json::json!({
            "session_id": session_id,
            "message": message,
        });

        self.write_request(bridge_proto::METHOD_SEND_MESSAGE, params)
            .await?;

        self.status = AgentStatus::Busy;
        self.info.status = AgentStatus::Busy;

        Ok(rx)
    }

    async fn cancel(&mut self, session_id: &str) -> anyhow::Result<()> {
        let params = serde_json::json!({
            "session_id": session_id,
        });

        self.write_notification(bridge_proto::METHOD_CANCEL, params)
            .await
    }

    async fn resolve_permission(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let params = serde_json::json!({
            "session_id": session_id,
            "request_id": request_id,
            "decision": decision,
        });

        self.write_notification(bridge_proto::METHOD_RESOLVE_PERMISSION, params)
            .await
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!(agent = %self.agent_type, "stopping subprocess adapter");

        // Clear the reader state
        {
            let mut state = self.reader_state.lock().await;
            state.current_tx = None;
        }

        // Kill the child process
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        self.stdin = None;
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;

        Ok(())
    }

    fn is_alive(&self) -> bool {
        if matches!(self.status, AgentStatus::Stopped | AgentStatus::Error) {
            return false;
        }
        self.reader_alive.load(Ordering::Relaxed)
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        self.agent_type
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_codex_adapter_has_correct_type() {
        let adapter =
            SubprocessAdapter::new(AgentType::RemoteCodex, PathBuf::from("/usr/bin/codex"));
        assert_eq!(adapter.agent_type(), AgentType::RemoteCodex);
        assert_eq!(adapter.info().name, "Remote Codex (subprocess)");
        assert_eq!(adapter.status, AgentStatus::Starting);
        assert!(!adapter.is_alive());
    }

    #[test]
    fn new_roo_adapter_has_correct_type() {
        let adapter = SubprocessAdapter::new(AgentType::RemoteRoo, PathBuf::from("/usr/bin/roo"));
        assert_eq!(adapter.agent_type(), AgentType::RemoteRoo);
        assert_eq!(adapter.info().name, "Remote Roo (subprocess)");
        assert!(
            adapter
                .info()
                .capabilities
                .contains(&AgentCapability::McpSupport)
        );
    }

    #[test]
    fn alloc_request_id_increments() {
        let mut adapter =
            SubprocessAdapter::new(AgentType::RemoteCodex, PathBuf::from("/usr/bin/codex"));
        assert_eq!(adapter.alloc_request_id(), 1);
        assert_eq!(adapter.alloc_request_id(), 2);
        assert_eq!(adapter.alloc_request_id(), 3);
    }

    #[test]
    fn convert_notification_started() {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        let info = AgentInfo {
            name: "Test".into(),
            version: "0.1.0".into(),
            capabilities: caps,
            status: AgentStatus::Ready,
        };
        let params = serde_json::to_value(&info).unwrap();
        let event = SubprocessAdapter::convert_notification(bridge_proto::NOTIFY_STARTED, &params);
        assert!(event.is_some());
    }

    #[test]
    fn convert_notification_ready() {
        let event = SubprocessAdapter::convert_notification(
            bridge_proto::NOTIFY_READY,
            &serde_json::json!({}),
        );
        assert!(matches!(event, Some(UnifiedAgentEvent::Ready)));
    }

    #[test]
    fn convert_notification_message_delta() {
        let params = serde_json::json!({
            "session_id": "sess-1",
            "delta": "Hello"
        });
        let event =
            SubprocessAdapter::convert_notification(bridge_proto::NOTIFY_MESSAGE_DELTA, &params);
        match event {
            Some(UnifiedAgentEvent::MessageDelta { session_id, delta }) => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(delta, "Hello");
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn convert_notification_unknown_returns_none() {
        let event =
            SubprocessAdapter::convert_notification("unknown/method", &serde_json::json!({}));
        assert!(event.is_none());
    }

    #[test]
    fn convert_response_error() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(1),
            result: None,
            error: Some(crate::jsonrpc::JsonRpcError {
                code: -1,
                message: "something went wrong".into(),
                data: None,
            }),
        };
        let event = SubprocessAdapter::convert_response(&resp);
        match event {
            Some(UnifiedAgentEvent::Error {
                message,
                recoverable,
                ..
            }) => {
                assert_eq!(message, "something went wrong");
                assert!(!recoverable);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn convert_response_success_with_event() {
        let event = UnifiedAgentEvent::Ready;
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(1),
            result: Some(serde_json::to_value(&event).unwrap()),
            error: None,
        };
        let converted = SubprocessAdapter::convert_response(&resp);
        assert!(matches!(converted, Some(UnifiedAgentEvent::Ready)));
    }
}
