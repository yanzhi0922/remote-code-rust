use std::io::Write;

use anyhow::Result;
use rc_core::SessionState;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ProtocolInput {
    User {
        content: String,
    },
    ControlResponse {
        request_id: String,
        allow: bool,
        message: Option<String>,
    },
    Interrupt,
}

pub struct ProtocolEmitter<W: Write> {
    writer: W,
    session_id: Uuid,
}

impl<W: Write> ProtocolEmitter<W> {
    pub fn new(writer: W, session_id: Uuid) -> Self {
        Self { writer, session_id }
    }

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

    pub fn emit_state(&mut self, state: SessionState) -> Result<()> {
        self.emit(json!({
            "type": "system",
            "subtype": "session_state_changed",
            "state": state.as_str(),
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

    pub fn emit_status(&mut self, status: impl AsRef<str>) -> Result<()> {
        self.emit(json!({
            "type": "system",
            "subtype": "status",
            "status": status.as_ref(),
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

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

    pub fn emit_result(&mut self, payload: ResultPayload) -> Result<()> {
        self.emit(json!({
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
            "errors": if payload.errors.is_empty() { Value::Null } else { json!(payload.errors) },
            "uuid": Uuid::new_v4(),
            "session_id": self.session_id,
        }))
    }

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

    pub fn emit_permission_cancelled(&mut self, request_id: &str) -> Result<()> {
        self.emit(json!({
            "type": "control_cancel_request",
            "request_id": request_id,
        }))
    }

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

#[derive(Debug, Clone)]
pub struct InitPayload {
    pub api_key_source: String,
    pub version: String,
    pub cwd: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub model: Option<String>,
    pub permission_mode: String,
    pub slash_commands: Vec<String>,
    pub output_style: String,
    pub skills: Vec<String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResultPayload {
    pub is_error: bool,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u32,
    pub result: String,
    pub stop_reason: String,
    pub total_cost_usd: f64,
    pub usage: UsagePayload,
    pub model_usage: Value,
    pub permission_denials: Vec<Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UsagePayload {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct PermissionRequestPayload {
    pub request_id: String,
    pub tool_name: String,
    pub tool_use_id: String,
    pub title: String,
    pub description: String,
    pub input: Value,
    pub blocked_path: Option<String>,
    pub permission_suggestions: Vec<Value>,
}

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
