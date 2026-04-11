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
    #[must_use]
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
    /// AWS Bedrock (placeholder — uses SigV4 auth).
    Bedrock,
    /// Google Vertex AI (placeholder — uses GCP auth).
    Vertex,
}

impl ProviderProtocol {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Bedrock => "bedrock",
            Self::Vertex => "vertex",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum HookEvent {
    SessionStart,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
}

impl HookEvent {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum HookShell {
    Bash,
    PowerShell,
}

impl HookShell {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::PowerShell => "powershell",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandHook {
    pub command: String,
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
    #[serde(default)]
    pub shell: Option<HookShell>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub once: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookCommand {
    Command(CommandHook),
}

impl HookCommand {
    #[must_use]
    pub fn as_command(&self) -> &CommandHook {
        match self {
            Self::Command(command) => command,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookMatcher {
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub hooks: Vec<HookCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Running,
    RequiresAction,
}

impl SessionState {
    #[must_use]
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

    #[must_use]
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

#[must_use]
pub fn default_system_prompt(cwd: &std::path::Path) -> String {
    format!(
        "You are Remote Code Rust, a concise coding agent running inside {}. Keep responses practical, prefer safe actions, and preserve compatibility with the Remote Code stream-json runtime where possible.",
        cwd.display()
    )
}

#[cfg(test)]
mod tests {
    use super::{HookCommand, HookEvent, HookShell};

    #[test]
    fn hook_event_round_trips_as_upstream_name() {
        let encoded =
            serde_json::to_string(&HookEvent::PreToolUse).expect("hook event encode should work");
        assert_eq!(encoded, "\"PreToolUse\"");

        let decoded: HookEvent =
            serde_json::from_str(&encoded).expect("hook event decode should work");
        assert_eq!(decoded, HookEvent::PreToolUse);
    }

    #[test]
    fn command_hook_deserializes_upstream_shape() {
        let hook: HookCommand = serde_json::from_str(
            r#"{
                "type": "command",
                "command": "echo ready",
                "if": "Bash(git status *)",
                "shell": "powershell",
                "timeout": 5,
                "once": true
            }"#,
        )
        .expect("command hook decode should work");

        let command = hook.as_command();
        assert_eq!(command.command, "echo ready");
        assert_eq!(command.condition.as_deref(), Some("Bash(git status *)"));
        assert_eq!(command.shell, Some(HookShell::PowerShell));
        assert_eq!(command.timeout, Some(5));
        assert!(command.once);
    }
}
