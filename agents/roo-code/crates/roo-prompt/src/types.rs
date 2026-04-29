//! Prompt type definitions.
//!
//! Derived from `src/core/prompts/types.ts`.

/// Settings passed to system prompt generation functions.
///
/// Source: `src/core/prompts/types.ts` — `SystemPromptSettings`
#[derive(Debug, Clone, Default)]
pub struct SystemPromptSettings {
    /// Whether the todo list feature is enabled.
    pub todo_list_enabled: bool,
    /// Whether to use agent rules (AGENTS.md).
    pub use_agent_rules: bool,
    /// When true, recursively discover and load .roo/rules from subdirectories.
    pub enable_subfolder_rules: bool,
    /// Whether new tasks require todos.
    pub new_task_require_todos: bool,
    /// When true, model should hide vendor/company identity in responses.
    pub is_stealth_model: bool,
}

/// Skill information for the skills section.
///
/// Source: `src/core/prompts/sections/skills.ts` — skill entries from SkillsManager
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: String,
}

/// File entry for formatting file lists.
///
/// Used by `format_files_list` in responses.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub relative_path: String,
    pub is_ignored: bool,
    pub is_protected: bool,
}

/// Parameters for system prompt generation.
///
/// Source: `src/core/prompts/system.ts` — `generatePrompt` parameters
pub struct SystemPromptParams {
    /// Current working directory (project root).
    pub cwd: String,
    /// Current mode slug (e.g., "code", "architect").
    pub mode: String,
    /// Role definition for the current mode.
    pub role_definition: String,
    /// Base instructions for the current mode.
    pub base_instructions: Option<String>,
    /// Global custom instructions.
    pub global_custom_instructions: Option<String>,
    /// Whether MCP servers are available.
    pub has_mcp: bool,
    /// Language code (e.g., "en", "zh-CN").
    pub language: Option<String>,
    /// .rooignore instructions.
    pub roo_ignore_instructions: Option<String>,
    /// System prompt settings.
    pub settings: Option<SystemPromptSettings>,
    /// All available modes for the modes section.
    pub modes: Vec<roo_types::mode::ModeConfig>,
    /// Skills for the current mode.
    pub skills: Vec<SkillInfo>,
    /// OS information string (e.g., "Windows 11", "macOS 14.0").
    pub os_info: String,
    /// Default shell (e.g., "cmd.exe", "/bin/bash").
    pub shell: String,
    /// Home directory path.
    pub home_dir: String,
    /// Custom instructions loaded from rule files.
    pub custom_rules_content: String,
}
