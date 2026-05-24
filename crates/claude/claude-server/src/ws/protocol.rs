use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- Client -> Server ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    UserMessage {
        content: String,
        #[serde(default)]
        attachments: Vec<Attachment>,
    },
    PermissionResponse {
        request_id: String,
        allowed: bool,
        rule: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
    },
    StopGeneration,
    SetRuntimeConfig {
        provider_id: Option<String>,
        model_id: Option<String>,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

// ---- Server -> Client ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Connected {
        session_id: Uuid,
    },
    ContentStart {
        block_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
    ContentDelta {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_input: Option<String>,
    },
    ToolUseComplete {
        tool_name: String,
        tool_use_id: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    PermissionRequest {
        request_id: String,
        tool_name: String,
        input: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    MessageComplete {
        usage: UsagePayload,
    },
    Thinking {
        text: String,
    },
    Status {
        state: AgentStatus,
    },
    Error {
        message: String,
        code: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePayload {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Thinking,
    ToolExecuting,
    Streaming,
    PermissionPending,
}
