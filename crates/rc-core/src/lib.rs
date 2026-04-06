use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const APP_NAME: &str = "remote-code";
pub const PRODUCT_NAME: &str = "Remote Code Rust";
pub const DEFAULT_PROFILE_DIR_NAME: &str = ".remote-code-rust";
pub const LEGACY_PROFILE_DIR_NAME: &str = ".remote-code";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
    DontAsk,
    Plan,
}

impl PermissionMode {
    pub fn as_legacy_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
            Self::DontAsk => "dontAsk",
            Self::Plan => "plan",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ProviderProtocol {
    #[default]
    OpenAi,
    Anthropic,
}

impl ProviderProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum InputFormat {
    #[default]
    Text,
    StreamJson,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Text,
    StreamJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Running,
    RequiresAction,
}

impl SessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::RequiresAction => "requires_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageSummary {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub role: ConversationRole,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub history_text: Option<String>,
    #[serde(default)]
    pub content_blocks: Vec<Value>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_error: bool,
}

impl ConversationEntry {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: ConversationRole::System,
            text: text.into(),
            history_text: None,
            content_blocks: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: ConversationRole::User,
            text: text.into(),
            history_text: None,
            content_blocks: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: ConversationRole::Assistant,
            text: text.into(),
            history_text: None,
            content_blocks: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        }
    }

    pub fn tool(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        text: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: ConversationRole::Tool,
            text: text.into(),
            history_text: None,
            content_blocks: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
            is_error,
        }
    }

    pub fn history_text(&self) -> String {
        self.history_text
            .clone()
            .unwrap_or_else(|| self.text.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub text: String,
    #[serde(default)]
    pub history_text: Option<String>,
    #[serde(default)]
    pub content_blocks: Vec<Value>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub usage: UsageSummary,
    #[serde(default = "default_stop_reason")]
    pub stop_reason: String,
}

fn default_stop_reason() -> String {
    "end_turn".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid,
    pub event_type: String,
    #[serde(default)]
    pub conversation: Option<ConversationEntry>,
    #[serde(default)]
    pub payload: Option<Value>,
}

pub fn default_system_prompt(cwd: &std::path::Path) -> String {
    format!(
        "You are Remote Code Rust, a concise coding agent running inside {}. Keep responses practical, prefer safe actions, and preserve compatibility with the Remote Code stream-json runtime where possible.",
        cwd.display()
    )
}
