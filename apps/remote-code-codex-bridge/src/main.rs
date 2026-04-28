//! Codex Bridge Binary.
//!
//! This binary acts as a **protocol bridge** between the host process
//! (remote-code-runner) and the actual `codex` CLI binary. It accepts
//! JSON-RPC 2.0 requests on stdin, spawns `codex` as a child process,
//! translates its output, and emits JSON-RPC notifications/responses on
//! stdout.
//!
//! # Wire protocol
//!
//! Each line on **stdin** is a JSON-RPC request. Each line on **stdout** is
//! a JSON-RPC response or notification (NDJSON).
//!
//! # Supported methods
//!
//! | Method               | Description                          |
//! |----------------------|--------------------------------------|
//! | `initialize`         | Return bridge capabilities           |
//! | `send_message`       | Run codex with the given prompt      |
//! | `cancel`             | Kill the running codex process       |
//! | `shutdown`           | Graceful shutdown                    |
//! | `resolve_permission` | (reserved, no-op for now)            |

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use rc_agent_protocol::bridge_proto;
use rc_agent_protocol::types::{AgentCapability, AgentInfo, AgentStatus};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Mutable state shared between the main loop and potential background tasks.
struct BridgeState {
    /// Currently running codex child process (if any).
    child: Option<Child>,
    /// Session ID set during `initialize`.
    session_id: String,
    /// Working directory for codex invocations.
    working_dir: Option<String>,
    /// Model override.
    model: Option<String>,
    /// API key forwarded to codex.
    api_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a single JSON-RPC message (as one NDJSON line) to stdout.
async fn write_stdout(msg: &Value) {
    let mut line = serde_json::to_string(msg).unwrap_or_default();
    line.push('\n');
    let mut stdout = tokio::io::stdout();
    let _ = stdout.write_all(line.as_bytes()).await;
    let _ = stdout.flush().await;
}

/// Build a JSON-RPC response for a given request ID.
fn make_response(id: u64, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// Build a JSON-RPC error response.
fn make_error(id: u64, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

/// Build a JSON-RPC notification.
fn make_notification(method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

/// Build the static [`AgentInfo`] for this bridge.
fn bridge_info() -> AgentInfo {
    let mut caps = HashSet::new();
    caps.insert(AgentCapability::Streaming);
    caps.insert(AgentCapability::ToolUse);
    caps.insert(AgentCapability::Subtasks);
    caps.insert(AgentCapability::Permissions);

    AgentInfo {
        name: "Codex Bridge".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        capabilities: caps,
        status: AgentStatus::Ready,
    }
}

/// Resolve the codex binary path.
fn resolve_codex_binary() -> Result<String> {
    // 1. Explicit env var
    if let Ok(p) = std::env::var("CODEX_BINARY_PATH")
        && std::path::Path::new(&p).exists()
    {
        return Ok(p);
    }
    // 2. Look for `codex` on PATH
    if let Ok(output) = std::process::Command::new("which").arg("codex").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    // 3. Common install locations
    for candidate in [
        "/usr/local/bin/codex",
        "/usr/bin/codex",
        std::env::var("HOME")
            .map(|h| format!("{h}/.local/bin/codex"))
            .unwrap_or_default()
            .as_str(),
    ] {
        if std::path::Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }

    bail!("codex binary not found. Set CODEX_BINARY_PATH or install codex on PATH.");
}

/// Classify an incoming JSON-RPC line:
/// - Has `"id"` → request (return the id)
/// - No `"id"` → notification (return None)
fn classify_message(raw: &Value) -> Option<u64> {
    raw.get("id").and_then(|v| v.as_u64())
}

// ---------------------------------------------------------------------------
// Request handlers
// ---------------------------------------------------------------------------

async fn handle_initialize(state: &Arc<Mutex<BridgeState>>, id: u64, params: Value) -> Value {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let working_dir = params
        .get("working_dir")
        .and_then(|v| v.as_str())
        .map(String::from);
    let model = params
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from);
    let api_key = params
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(String::from);

    {
        let mut s = state.lock().await;
        s.session_id = session_id;
        s.working_dir = working_dir;
        s.model = model;
        s.api_key = api_key;
    }

    // Emit started + ready notifications
    let info = bridge_info();
    write_stdout(&make_notification(
        bridge_proto::NOTIFY_STARTED,
        serde_json::to_value(&info).unwrap_or_default(),
    ))
    .await;

    write_stdout(&make_notification(bridge_proto::NOTIFY_READY, json!({}))).await;

    make_response(id, json!({ "status": "ok" }))
}

async fn handle_send_message(
    state: &Arc<Mutex<BridgeState>>,
    id: u64,
    params: Value,
    shutting_down: &AtomicBool,
) -> Value {
    let message = match params.get("message").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => return make_error(id, -32_602, "missing 'message' parameter"),
    };

    let session_id = {
        let s = state.lock().await;
        s.session_id.clone()
    };

    // Resolve codex binary
    let codex_bin = match resolve_codex_binary() {
        Ok(p) => p,
        Err(e) => {
            let err_notif = make_notification(
                bridge_proto::NOTIFY_ERROR,
                json!({
                    "session_id": session_id,
                    "message": e.to_string(),
                    "recoverable": false,
                }),
            );
            write_stdout(&err_notif).await;
            return make_error(id, -1, &e.to_string());
        }
    };

    // Build the codex command
    let mut cmd = Command::new(&codex_bin);
    cmd.arg("--quiet")
        .arg(&message)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    {
        let s = state.lock().await;
        if let Some(wd) = &s.working_dir {
            cmd.current_dir(wd);
        }
        if let Some(model) = &s.model {
            cmd.env("CODEX_MODEL", model);
        }
        if let Some(api_key) = &s.api_key {
            cmd.env("OPENAI_API_KEY", api_key);
        }
    }

    // Spawn codex
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_notif = make_notification(
                bridge_proto::NOTIFY_ERROR,
                json!({
                    "session_id": session_id,
                    "message": format!("failed to spawn codex: {e}"),
                    "recoverable": true,
                }),
            );
            write_stdout(&err_notif).await;
            return make_error(id, -1, &format!("failed to spawn codex: {e}"));
        }
    };

    // Take stdout before storing child
    let stdout = child.stdout.take();

    // Store child for potential cancel
    {
        let mut s = state.lock().await;
        s.child = Some(child);
    }

    // Read stdout line-by-line
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut full_output = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            if shutting_down.load(Ordering::Relaxed) {
                break;
            }
            if line.is_empty() {
                continue;
            }

            full_output.push_str(&line);
            full_output.push('\n');

            // Forward each line as a message_delta notification
            let delta_notif = make_notification(
                bridge_proto::NOTIFY_MESSAGE_DELTA,
                json!({
                    "session_id": session_id,
                    "delta": line,
                }),
            );
            write_stdout(&delta_notif).await;
        }

        // Wait for the child to finish
        let exit_status = {
            let mut s = state.lock().await;
            if let Some(ref mut c) = s.child {
                c.wait().await.ok()
            } else {
                None
            }
        };

        let success = exit_status.as_ref().is_some_and(|s| s.success());

        if !success && !shutting_down.load(Ordering::Relaxed) {
            let err_notif = make_notification(
                bridge_proto::NOTIFY_ERROR,
                json!({
                    "session_id": session_id,
                    "message": "codex exited with error",
                    "recoverable": true,
                }),
            );
            write_stdout(&err_notif).await;
        }

        // Emit done notification
        let done_notif = make_notification(
            bridge_proto::NOTIFY_DONE,
            json!({
                "session_id": session_id,
                "result": {
                    "response_text": full_output.trim(),
                    "tool_calls": [],
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                    },
                },
            }),
        );
        write_stdout(&done_notif).await;
    }

    // Clean up child reference
    {
        let mut s = state.lock().await;
        s.child = None;
    }

    make_response(id, json!({ "status": "ok" }))
}

async fn handle_cancel(state: &Arc<Mutex<BridgeState>>, id: u64) -> Value {
    let killed = {
        let mut s = state.lock().await;
        if let Some(ref mut child) = s.child {
            child.kill().await.is_ok()
        } else {
            false
        }
    };

    if killed {
        let mut s = state.lock().await;
        s.child = None;
    }

    make_response(id, json!({ "cancelled": killed }))
}

async fn handle_shutdown(state: &Arc<Mutex<BridgeState>>, id: u64) -> Value {
    {
        let mut s = state.lock().await;
        if let Some(ref mut child) = s.child {
            let _ = child.kill().await;
        }
        s.child = None;
    }

    make_response(id, json!({ "status": "shutting_down" }))
}

async fn handle_resolve_permission(id: u64) -> Value {
    // Codex bridge does not support interactive permissions yet.
    make_response(id, json!({ "status": "accepted" }))
}

// ---------------------------------------------------------------------------
// Notification handler (no ID)
// ---------------------------------------------------------------------------

async fn handle_notification(method: &str, _params: Value, state: &Arc<Mutex<BridgeState>>) {
    match method {
        bridge_proto::METHOD_CANCEL => {
            let mut s = state.lock().await;
            if let Some(ref mut child) = s.child {
                let _ = child.kill().await;
            }
            s.child = None;
        }
        _ => {
            tracing::warn!(method = %method, "unknown notification from host");
        }
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(BridgeState {
        child: None,
        session_id: String::new(),
        working_dir: None,
        model: None,
        api_key: None,
    }));

    let shutting_down = Arc::new(AtomicBool::new(false));

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.is_empty() {
            continue;
        }

        // Parse as generic JSON value first
        let raw: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse JSON-RPC message");
                continue;
            }
        };

        let method = raw
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let params = raw.get("params").cloned().unwrap_or(json!({}));

        // Classify: request (has "id") vs notification (no "id")
        if let Some(id) = classify_message(&raw) {
            let response = match method.as_str() {
                bridge_proto::METHOD_INITIALIZE => handle_initialize(&state, id, params).await,
                bridge_proto::METHOD_SEND_MESSAGE => {
                    handle_send_message(&state, id, params, &shutting_down).await
                }
                bridge_proto::METHOD_CANCEL => handle_cancel(&state, id).await,
                bridge_proto::METHOD_SHUTDOWN => {
                    let resp = handle_shutdown(&state, id).await;
                    write_stdout(&resp).await;
                    break;
                }
                bridge_proto::METHOD_RESOLVE_PERMISSION => handle_resolve_permission(id).await,
                _ => make_error(id, -32_601, &format!("unknown method: {method}")),
            };

            write_stdout(&response).await;
        } else {
            // Notification (no ID)
            handle_notification(&method, params, &state).await;
        }
    }

    // Ensure child is cleaned up on exit
    {
        let mut s = state.lock().await;
        if let Some(ref mut child) = s.child {
            let _ = child.kill().await;
        }
    }

    Ok(())
}
