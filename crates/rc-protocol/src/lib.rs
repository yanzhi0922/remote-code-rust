//! Stream-JSON protocol for headless / machine-readable mode.
//!
//! Defines the line-delimited JSON protocol used between the CLI and external
//! consumers. [`ProtocolEmitter`] writes events; [`parse_input_line`] reads them.

use std::io::Write;

use anyhow::Result;
use rc_core::SessionState;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// An input message parsed from the external consumer.
#[derive(Debug, Clone)]
pub enum ProtocolInput {
    /// User text input.
    User {
        /// The user's text content.
        content: String,
    },
    /// Response to a permission request.
    ControlResponse {
        /// ID of the original permission request.
        request_id: String,
        /// Whether the action is allowed.
        allow: bool,
        /// Optional message.
        message: Option<String>,
    },
    /// Interrupt signal.
    Interrupt,
}

/// Writes line-delimited JSON protocol events to an underlying writer.
pub struct ProtocolEmitter<W: Write> {
    writer: W,
    session_id: Uuid,
}

impl<W: Write> ProtocolEmitter<W> {
    /// Create a new emitter writing to `writer` for the given session.
    pub fn new(writer: W, session_id: Uuid) -> Self {
        Self { writer, session_id }
    }

    /// Emit an `init` system event with session metadata.
    pub fn emit_init(&mut self, payload: InitPayload) -> Result<()> {
        self.emit(json!({
            "type": "system",
            "subtype": "init",
            "apiKeySource": payload.api_key_source,
            "remote_code_version": payload.version,
            "cwd": payload.cwd,
            "tools": payload.tools,
            "mcp_servers": payload.mcp_servers,
            "model": payload.model,
            "permissionMode": payload.permission_mode,
            "slash_commands": payload.slash_commands,
            "output_style": payload.output_style,
            "skills": payload.skills,
            "plugins": payload.plugins,
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    /// Emit a session state change event.
    pub fn emit_state(&mut self, state: SessionState) -> Result<()> {
        self.emit(json!({
            "type": "system",
            "subtype": "session_state_changed",
            "state": state.as_str(),
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    /// Emit a status message event.
    pub fn emit_status(&mut self, status: impl AsRef<str>) -> Result<()> {
        self.emit(json!({
            "type": "system",
            "subtype": "status",
            "status": status.as_ref(),
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    /// Emit an assistant text message.
    pub fn emit_assistant(&mut self, text: impl AsRef<str>) -> Result<()> {
        self.emit(json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": text.as_ref()}],
            },
            "parent_tool_use_id": Value::Null,
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    /// Emit a result event summarising the completed turn.
    pub fn emit_result(&mut self, payload: ResultPayload) -> Result<()> {
        let mut event = json!({
            "type": "result",
            "subtype": if payload.is_error { "error_during_execution" } else { "success" },
            "duration_ms": payload.duration_ms,
            "duration_api_ms": payload.duration_api_ms,
            "is_error": payload.is_error,
            "num_turns": payload.num_turns,
            "result": payload.result,
            "stop_reason": payload.stop_reason,
            "total_cost_usd": payload.total_cost_usd,
            "usage": {
                "input_tokens": payload.usage.input_tokens,
                "output_tokens": payload.usage.output_tokens,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0,
                "service_tier": "standard",
            },
            "modelUsage": payload.model_usage,
            "permission_denials": payload.permission_denials,
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        });
        if !payload.errors.is_empty() {
            event["errors"] = json!(payload.errors);
        }
        self.emit(event)
    }

    /// Emit a permission request event for the external consumer.
    pub fn emit_permission_request(&mut self, payload: PermissionRequestPayload) -> Result<()> {
        self.emit(json!({
            "type": "control_request",
            "request_id": payload.request_id,
            "request": {
                "subtype": "can_use_tool",
                "tool_name": payload.tool_name,
                "input": payload.input,
                "tool_use_id": payload.tool_use_id,
                "title": payload.title,
                "description": payload.description,
                "blocked_path": payload.blocked_path,
                "permission_suggestions": payload.permission_suggestions,
            },
        }))
    }

    /// Emit a cancellation event for a previously sent permission request.
    pub fn emit_permission_cancelled(&mut self, request_id: &str) -> Result<()> {
        self.emit(json!({
            "type": "control_cancel_request",
            "request_id": request_id,
        }))
    }

    /// Emit a tool progress heartbeat event.
    pub fn emit_tool_progress(&mut self, tool_name: &str, elapsed_time_seconds: u64) -> Result<()> {
        self.emit(json!({
            "type": "tool_progress",
            "tool_name": tool_name,
            "elapsed_time_seconds": elapsed_time_seconds,
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    fn emit<T: Serialize>(&mut self, event: T) -> Result<()> {
        serde_json::to_writer(&mut self.writer, &event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

/// Payload for the `init` event emitted at session start.
#[derive(Debug, Clone)]
pub struct InitPayload {
    /// API key source description.
    pub api_key_source: String,
    /// Application version string.
    pub version: String,
    /// Current working directory.
    pub cwd: String,
    /// List of available tool names.
    pub tools: Vec<String>,
    /// List of configured MCP server names.
    pub mcp_servers: Vec<String>,
    /// Model identifier.
    pub model: Option<String>,
    /// Active permission mode.
    pub permission_mode: String,
    /// Available slash commands.
    pub slash_commands: Vec<String>,
    /// Output style setting.
    pub output_style: String,
    /// Available skill names.
    pub skills: Vec<String>,
    /// Available plugin names.
    pub plugins: Vec<String>,
}

/// Payload for the `result` event emitted at turn completion.
#[derive(Debug, Clone)]
pub struct ResultPayload {
    /// Whether the turn ended in error.
    pub is_error: bool,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// API duration in milliseconds.
    pub duration_api_ms: u64,
    /// Number of conversation turns.
    pub num_turns: u32,
    /// Final result text.
    pub result: String,
    /// Provider stop reason.
    pub stop_reason: String,
    /// Total estimated cost in USD.
    pub total_cost_usd: f64,
    /// Token usage breakdown.
    pub usage: UsagePayload,
    /// Per-model usage data.
    pub model_usage: Value,
    /// Permission denial records.
    pub permission_denials: Vec<Value>,
    /// Error messages encountered during the turn.
    pub errors: Vec<String>,
}

/// Token usage payload included in result events.
#[derive(Debug, Clone, Default)]
pub struct UsagePayload {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
}

/// Payload for the `control_request` permission event.
#[derive(Debug, Clone)]
pub struct PermissionRequestPayload {
    /// Unique request identifier.
    pub request_id: String,
    /// Tool name requesting permission.
    pub tool_name: String,
    /// Tool use identifier.
    pub tool_use_id: String,
    /// Short human-readable title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Tool input JSON.
    pub input: Value,
    /// Affected path, if any.
    pub blocked_path: Option<String>,
    /// Suggested permission rules.
    pub permission_suggestions: Vec<Value>,
}

/// Parse a single line of JSON input from the external consumer.
///
/// Returns `None` if the line cannot be parsed or is not a recognised event type.
pub fn parse_input_line(line: &str) -> Option<ProtocolInput> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let kind = value.get("type")?.as_str()?;
    match kind {
        "user" => {
            let content = value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|content| !content.is_empty())?;
            Some(ProtocolInput::User {
                content: content.to_owned(),
            })
        }
        "control_response" => {
            let response = value.get("response")?;
            let request_id = response.get("request_id")?.as_str()?.to_owned();
            let behavior = response
                .get("response")
                .and_then(|value| value.get("behavior"))
                .and_then(Value::as_str)
                .unwrap_or("deny");
            let message = response
                .get("response")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            Some(ProtocolInput::ControlResponse {
                request_id,
                allow: behavior.eq_ignore_ascii_case("allow"),
                message,
            })
        }
        "control_request" => {
            let subtype = value
                .get("request")
                .and_then(|request| request.get("subtype"))
                .and_then(Value::as_str)?;
            if subtype == "interrupt" {
                Some(ProtocolInput::Interrupt)
            } else {
                None
            }
        }
        _ => None,
    }
}
