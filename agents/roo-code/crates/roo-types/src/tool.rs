//! Tool type definitions.
//!
//! Derived from `packages/types/src/tool.ts`.
//! Defines all 24 tool names, 5 tool groups, and tool usage tracking.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ToolGroup
// ---------------------------------------------------------------------------

/// The 5 built-in tool groups. Each group controls a category of tools
/// that can be enabled/disabled per mode.
///
/// Source: `packages/types/src/tool.ts` — `toolGroups`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolGroup {
    Read,
    Edit,
    Command,
    Mcp,
    Modes,
}

/// Deprecated tool groups that may still exist in user config files.
/// Used during schema preprocessing to silently strip them.
pub const DEPRECATED_TOOL_GROUPS: &[&str] = &["browser"];

// ---------------------------------------------------------------------------
// ToolName
// ---------------------------------------------------------------------------

/// All 24 built-in tool names plus the `custom_tool` placeholder.
///
/// Source: `packages/types/src/tool.ts` — `toolNames`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    ExecuteCommand,
    ReadFile,
    ReadCommandOutput,
    WriteToFile,
    ApplyDiff,
    /// Legacy "edit" tool name — kept for backward compatibility.
    Edit,
    SearchAndReplace,
    /// Legacy alias for search_and_replace.
    SearchReplace,
    EditFile,
    ApplyPatch,
    SearchFiles,
    ListFiles,
    UseMcpTool,
    AccessMcpResource,
    AskFollowupQuestion,
    AttemptCompletion,
    SwitchMode,
    NewTask,
    CodebaseSearch,
    UpdateTodoList,
    RunSlashCommand,
    Skill,
    GenerateImage,
    CustomTool,
}

impl ToolName {
    /// Returns the snake_case string representation as used in JSON-RPC.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExecuteCommand => "execute_command",
            Self::ReadFile => "read_file",
            Self::ReadCommandOutput => "read_command_output",
            Self::WriteToFile => "write_to_file",
            Self::ApplyDiff => "apply_diff",
            Self::Edit => "edit",
            Self::SearchAndReplace => "search_and_replace",
            Self::SearchReplace => "search_replace",
            Self::EditFile => "edit_file",
            Self::ApplyPatch => "apply_patch",
            Self::SearchFiles => "search_files",
            Self::ListFiles => "list_files",
            Self::UseMcpTool => "use_mcp_tool",
            Self::AccessMcpResource => "access_mcp_resource",
            Self::AskFollowupQuestion => "ask_followup_question",
            Self::AttemptCompletion => "attempt_completion",
            Self::SwitchMode => "switch_mode",
            Self::NewTask => "new_task",
            Self::CodebaseSearch => "codebase_search",
            Self::UpdateTodoList => "update_todo_list",
            Self::RunSlashCommand => "run_slash_command",
            Self::Skill => "skill",
            Self::GenerateImage => "generate_image",
            Self::CustomTool => "custom_tool",
        }
    }

    /// All tool names as a static slice.
    pub fn all() -> &'static [ToolName] {
        &[
            Self::ExecuteCommand,
            Self::ReadFile,
            Self::ReadCommandOutput,
            Self::WriteToFile,
            Self::ApplyDiff,
            Self::Edit,
            Self::SearchAndReplace,
            Self::SearchReplace,
            Self::EditFile,
            Self::ApplyPatch,
            Self::SearchFiles,
            Self::ListFiles,
            Self::UseMcpTool,
            Self::AccessMcpResource,
            Self::AskFollowupQuestion,
            Self::AttemptCompletion,
            Self::SwitchMode,
            Self::NewTask,
            Self::CodebaseSearch,
            Self::UpdateTodoList,
            Self::RunSlashCommand,
            Self::Skill,
            Self::GenerateImage,
            Self::CustomTool,
        ]
    }

    /// Returns which tool group this tool belongs to.
    ///
    /// Source: `src/shared/tools.ts` — tool group mappings
    pub fn group(&self) -> ToolGroup {
        match self {
            | Self::ReadFile
            | Self::ListFiles
            | Self::CodebaseSearch
            | Self::SearchFiles
            | Self::ReadCommandOutput => ToolGroup::Read,

            | Self::WriteToFile
            | Self::ApplyDiff
            | Self::Edit
            | Self::SearchAndReplace
            | Self::SearchReplace
            | Self::EditFile
            | Self::ApplyPatch
            | Self::UpdateTodoList => ToolGroup::Edit,

            | Self::ExecuteCommand => ToolGroup::Command,

            | Self::UseMcpTool
            | Self::AccessMcpResource => ToolGroup::Mcp,

            | Self::SwitchMode
            | Self::NewTask
            | Self::AskFollowupQuestion
            | Self::AttemptCompletion
            | Self::RunSlashCommand
            | Self::Skill
            | Self::GenerateImage
            | Self::CustomTool => ToolGroup::Modes,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolUsage
// ---------------------------------------------------------------------------

/// Tracks the number of attempts and failures for each tool.
///
/// Source: `packages/types/src/tool.ts` — `toolUsageSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolUsageEntry {
    pub attempts: u32,
    pub failures: u32,
}

/// A map from tool name to usage statistics.
pub type ToolUsage = std::collections::HashMap<ToolName, ToolUsageEntry>;

// ---------------------------------------------------------------------------
// GroupEntry — used in ModeConfig.groups
// ---------------------------------------------------------------------------

/// A tool group entry can be either a plain group name or a tuple
/// of (group name, options) where options may include a file regex
/// and description.
///
/// Source: `packages/types/src/mode.ts` — `groupEntrySchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GroupEntry {
    /// Plain group name, e.g. `"read"`.
    Plain(ToolGroup),
    /// Group with options, e.g. `["edit", { "fileRegex": "\\.md$" }]`.
    WithOptions(ToolGroup, GroupOptions),
}

/// Optional configuration for a tool group within a mode.
///
/// Source: `packages/types/src/mode.ts` — `groupOptionsSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupOptions {
    /// A regex pattern that restricts which files the tool can operate on.
    pub file_regex: Option<String>,
    /// Human-readable description of the restriction.
    pub description: Option<String>,
}

/// Configuration for a tool within a group, used in the tool group config
/// array format.
///
/// Source: `src/shared/tools.ts` — `ToolGroupConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolGroupConfig {
    pub tool: ToolName,
    pub file_regex: Option<String>,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// DiffResult — used by diff tools
// ---------------------------------------------------------------------------

/// Result of applying a diff operation.
///
/// Source: `src/shared/tools.ts` — `DiffResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiffResult {
    #[serde(rename = "success")]
    Success {
        content: String,
        details: Option<Vec<DiffItem>>,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
    #[serde(rename = "partial")]
    Partial {
        content: String,
        details: Option<Vec<DiffItem>>,
        errors: Vec<String>,
    },
}

/// A single diff item showing what changed.
///
/// Source: `src/shared/tools.ts` — `DiffItem`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItem {
    pub original: String,
    pub search: String,
    pub replace: String,
    pub line: Option<usize>,
}

// ---------------------------------------------------------------------------
// DiffStrategy trait placeholder
// ---------------------------------------------------------------------------

/// Strategy for applying diffs. Implementations include MultiSearchReplace.
///
/// Source: `src/shared/tools.ts` — `DiffStrategy`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStrategyType {
    MultiSearchReplace,
    SearchAndReplace,
    ApplyPatch,
}

// ---------------------------------------------------------------------------
// AskApproval type
// ---------------------------------------------------------------------------

/// Callback type for asking user approval before tool execution.
///
/// Source: `src/shared/tools.ts` — `AskApproval`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AskApproval {
    #[serde(rename = "execute_command")]
    ExecuteCommand { command: String },
    #[serde(rename = "write_to_file")]
    WriteToFile { path: String },
    #[serde(rename = "apply_diff")]
    ApplyDiff { path: String },
    #[serde(rename = "edit_file")]
    EditFile { path: String },
    #[serde(rename = "use_mcp_tool")]
    UseMcpTool { server_name: String, tool_name: String },
    #[serde(rename = "access_mcp_resource")]
    AccessMcpResource { server_name: String, uri: String },
    #[serde(rename = "switch_mode")]
    SwitchMode { mode_slug: String },
    #[serde(rename = "new_task")]
    NewTask { mode_slug: String },
    #[serde(rename = "generate_image")]
    GenerateImage,
}

// ---------------------------------------------------------------------------
// TextContent / ImageContent — content block types
// ---------------------------------------------------------------------------

/// Text content block.
///
/// Source: `src/shared/tools.ts` — `TextContent`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// ---------------------------------------------------------------------------
// ToolUse / McpToolUse — parsed tool call structures
// ---------------------------------------------------------------------------

/// A parsed tool use block from the assistant's response.
///
/// Source: `src/shared/tools.ts` — `ToolUse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUse {
    pub r#type: String, // always "tool_use"
    pub name: ToolName,
    pub params: serde_json::Value,
    pub partial: bool,
    pub id: Option<String>,
}

/// A parsed MCP tool use block.
///
/// Source: `src/shared/tools.ts` — `McpToolUse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolUse {
    pub r#type: String, // always "mcp_tool_use"
    pub tool_name: String,
    pub server_name: String,
    pub params: serde_json::Value,
    pub partial: bool,
    pub id: Option<String>,
}

// ---------------------------------------------------------------------------
// Tool-specific parameter types
// ---------------------------------------------------------------------------

/// Parameters for execute_command tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteCommandParams {
    pub command: String,
}

/// Parameters for read_file tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileParams {
    pub path: String,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub limit: Option<u64>,
}

/// Parameters for write_to_file tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteToFileParams {
    pub path: String,
    pub content: String,
}

/// Parameters for apply_diff tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDiffParams {
    pub path: String,
    pub diff: String,
}

/// Parameters for search_files tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesParams {
    pub path: String,
    pub regex: String,
    #[serde(default)]
    pub file_pattern: Option<String>,
}

/// Parameters for list_files tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesParams {
    pub path: String,
    pub recursive: bool,
}

/// Parameters for ask_followup_question tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskFollowupQuestionParams {
    pub question: String,
    pub follow_up: Vec<FollowUpOption>,
}

/// A suggested follow-up answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpOption {
    pub text: String,
    #[serde(default)]
    pub mode: Option<String>,
}

/// Parameters for attempt_completion tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptCompletionParams {
    pub result: String,
    #[serde(default)]
    pub command: Option<String>,
}

/// Parameters for switch_mode tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchModeParams {
    pub mode_slug: String,
    pub reason: Option<String>,
}

/// Parameters for new_task tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTaskParams {
    pub mode: String,
    pub message: String,
    #[serde(default)]
    pub todos: Option<String>,
}

/// Parameters for codebase_search tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodebaseSearchParams {
    pub query: String,
    #[serde(default)]
    pub directory_prefix: Option<String>,
}

/// Parameters for update_todo_list tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTodoListParams {
    pub todos: String,
}

/// Parameters for use_mcp_tool tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UseMcpToolParams {
    pub server_name: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

/// Parameters for access_mcp_resource tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessMcpResourceParams {
    pub server_name: String,
    pub uri: String,
}

/// Parameters for read_command_output tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadCommandOutputParams {
    pub artifact_id: String,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub search: Option<String>,
}

/// Parameters for run_slash_command tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSlashCommandParams {
    pub command: String,
    #[serde(default)]
    pub args: Option<String>,
}

/// Parameters for skill tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillParams {
    pub skill: String,
    #[serde(default)]
    pub args: Option<String>,
}

/// Parameters for generate_image tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateImageParams {
    pub prompt: String,
}

/// Parameters for edit_file tool.
///
/// Source: `src/core/tools/EditFileTool.ts` — `EditFileParams`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditFileParams {
    /// The path to the file to modify or create.
    pub file_path: String,
    /// The exact literal text to replace. Use empty string to create a new file.
    pub old_string: String,
    /// The exact literal text to replace old_string with.
    pub new_string: String,
    /// Number of replacements expected. Defaults to 1.
    #[serde(default)]
    pub expected_replacements: Option<u32>,
}

/// Parameters for apply_patch tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPatchParams {
    pub path: String,
    pub patch: String,
}
