//! Core type definitions for the remote-code-rust workspace.
//!
//! This crate defines the shared domain types used across all other crates:
//! permission modes, provider protocols, conversation entries, tool calls,
//! usage summaries, hook definitions, and session events.

use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Application binary name.
pub const APP_NAME: &str = "remote-code";
/// Human-readable product name.
pub const PRODUCT_NAME: &str = "Remote Code Rust";
/// Default directory name for the application profile.
pub const DEFAULT_PROFILE_DIR_NAME: &str = ".remote-code-rust";
/// Legacy directory name used by the upstream Node.js runtime.
pub const LEGACY_PROFILE_DIR_NAME: &str = ".remote-code";

/// Permission mode controlling how tool executions are authorised.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Ask for every non-read operation.
    #[default]
    Default,
    /// Auto-accept file edits, ask for everything else.
    AcceptEdits,
    /// Skip all permission prompts (dangerous).
    BypassPermissions,
    /// Auto-allow based on tool class; never prompt.
    DontAsk,
    /// Plan-only mode — no tool execution at all.
    Plan,
}

impl PermissionMode {
    /// Return the legacy string representation used by the upstream runtime.
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

/// LLM provider wire protocol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ProviderProtocol {
    /// OpenAI-compatible chat completions API.
    #[default]
    OpenAi,
    /// Anthropic Messages API.
    Anthropic,
    /// AWS Bedrock (placeholder — uses SigV4 auth).
    Bedrock,
    /// Google Vertex AI (placeholder — uses GCP auth).
    Vertex,
}

impl ProviderProtocol {
    /// Return the kebab-case protocol identifier.
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

/// Input format for the CLI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum InputFormat {
    /// Human-readable text input.
    #[default]
    Text,
    /// Line-delimited JSON streaming input.
    StreamJson,
}

/// Output format for the CLI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Human-readable text output.
    #[default]
    Text,
    /// Line-delimited JSON streaming output.
    StreamJson,
}

/// Hook lifecycle events that can trigger command hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum HookEvent {
    /// Fired when a new session starts.
    SessionStart,
    /// Fired before a tool is executed.
    PreToolUse,
    /// Fired after a tool succeeds.
    PostToolUse,
    /// Fired after a tool fails.
    PostToolUseFailure,
}

impl HookEvent {
    /// Return the PascalCase event name used by the upstream runtime.
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

/// Shell interpreter used to execute hook commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum HookShell {
    /// POSIX bash shell.
    Bash,
    /// Windows PowerShell.
    PowerShell,
}

impl HookShell {
    /// Return the lowercase shell name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::PowerShell => "powershell",
        }
    }
}

/// A single command hook definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandHook {
    /// The shell command to execute.
    pub command: String,
    /// Optional condition expression (e.g. `Bash(git status *)`).
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
    /// Shell interpreter override.
    #[serde(default)]
    pub shell: Option<HookShell>,
    /// Timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Status message shown while the hook runs.
    #[serde(default)]
    pub status_message: Option<String>,
    /// Whether the hook should only fire once per session.
    #[serde(default)]
    pub once: bool,
}

/// Tagged union of hook command types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookCommand {
    /// A shell command hook.
    Command(CommandHook),
}

impl HookCommand {
    /// Borrow the inner [`CommandHook`].
    #[must_use]
    pub fn as_command(&self) -> &CommandHook {
        match self {
            Self::Command(command) => command,
        }
    }
}

/// A hook matcher that groups a pattern with its associated hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookMatcher {
    /// Optional tool-name pattern (e.g. `Bash` or `Bash(git *)`).
    #[serde(default)]
    pub matcher: Option<String>,
    /// Hooks to run when the matcher fires.
    #[serde(default)]
    pub hooks: Vec<HookCommand>,
}

/// Current state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// No active prompt is being processed.
    Idle,
    /// A prompt is currently being processed.
    Running,
    /// Waiting for user approval of a tool call.
    RequiresAction,
}

impl SessionState {
    /// Return the snake_case state name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::RequiresAction => "requires_action",
        }
    }
}

/// Role of a conversation entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    /// System prompt.
    System,
    /// User input.
    User,
    /// Assistant response.
    Assistant,
    /// Tool result.
    Tool,
}

/// A tool call requested by the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Tool name (e.g. `"read_file"`).
    pub name: String,
    /// JSON object of tool arguments.
    #[serde(default)]
    pub input: Value,
}

/// Token usage statistics returned by the provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageSummary {
    /// Number of input (prompt) tokens.
    #[serde(default)]
    pub input_tokens: u64,
    /// Number of output (completion) tokens.
    #[serde(default)]
    pub output_tokens: u64,
}

/// A single entry in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    /// Who produced this entry.
    pub role: ConversationRole,
    /// Primary text content.
    #[serde(default)]
    pub text: String,
    /// Optional abbreviated text for context-window compaction.
    #[serde(default)]
    pub history_text: Option<String>,
    /// Anthropic-style content blocks.
    #[serde(default)]
    pub content_blocks: Vec<Value>,
    /// Tool calls embedded in an assistant message.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Tool-call ID this entry responds to (for tool-role entries).
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Tool name for tool-role entries.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether this entry represents an error.
    #[serde(default)]
    pub is_error: bool,
}

impl ConversationEntry {
    /// Create a system-role entry.
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

    /// Create a user-role entry.
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

    /// Create an assistant-role entry.
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

    /// Create a tool-role entry responding to a tool call.
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

    /// Return the history text, falling back to the full text if none is set.
    #[must_use]
    pub fn history_text(&self) -> String {
        self.history_text
            .clone()
            .unwrap_or_else(|| self.text.clone())
    }
}

/// Parsed response from the LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// Primary text content of the response.
    pub text: String,
    /// Optional abbreviated text for context compaction.
    #[serde(default)]
    pub history_text: Option<String>,
    /// Anthropic-style content blocks.
    #[serde(default)]
    pub content_blocks: Vec<Value>,
    /// Tool calls requested by the assistant.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Token usage statistics.
    #[serde(default)]
    pub usage: UsageSummary,
    /// Provider stop reason (e.g. `"end_turn"`, `"tool_use"`).
    #[serde(default = "default_stop_reason")]
    pub stop_reason: String,
}

fn default_stop_reason() -> String {
    "end_turn".to_owned()
}

/// Result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The tool output content.
    pub content: String,
    /// Whether the tool execution resulted in an error.
    pub is_error: bool,
}

/// Trait for providing LLM completion capability to sub-agents.
///
/// This trait breaks the circular dependency between `rc-tools` and
/// `rc-provider`: `rc-tools` defines the agent tool that needs LLM access,
/// but cannot depend on `rc-provider` directly. Instead, the completion
/// capability is injected via this trait at the TUI/application layer.
#[async_trait::async_trait]
pub trait SubAgentCompletion: Send + Sync {
    /// Send a conversation to the LLM and return the response.
    ///
    /// The implementation is responsible for provider selection, retry logic,
    /// and message formatting.
    async fn complete(
        &self,
        conversation: &[ConversationEntry],
    ) -> anyhow::Result<ProviderResponse>;
}

/// A persisted event in the session transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Which session this event belongs to.
    pub session_id: Uuid,
    /// Event type discriminator (e.g. `"prompt"`, `"tool_result"`).
    pub event_type: String,
    /// Optional conversation entry associated with this event.
    #[serde(default)]
    pub conversation: Option<ConversationEntry>,
    /// Optional JSON payload with event-specific data.
    #[serde(default)]
    pub payload: Option<Value>,
}

/// Generate the default system prompt for the given working directory.
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
