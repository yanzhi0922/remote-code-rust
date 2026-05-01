//! Input data types for environment details generation.
//!
//! All data is passed in via parameters — this crate has no external
//! side-effects and does not depend on VS Code.

use serde::{Deserialize, Serialize};

/// Terminal information for environment details (active terminals).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub working_directory: String,
    pub last_command: String,
    pub new_output: Option<String>,
}

/// A completed process from an inactive terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedProcess {
    pub command: String,
    pub output: String,
}

/// Inactive terminal info with completed processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactiveTerminalInfo {
    pub id: String,
    pub working_directory: String,
    pub completed_processes: Vec<CompletedProcess>,
}

/// Mode details for environment display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeDisplayInfo {
    pub slug: String,
    pub name: String,
    pub model_id: String,
}

/// Settings that control what's included in environment details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSettings {
    pub include_current_time: bool,
    pub include_current_cost: bool,
    pub max_git_status_files: usize,
    pub todo_list_enabled: bool,
    pub max_workspace_files: usize,
    pub max_open_tabs: usize,
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            include_current_time: true,
            include_current_cost: true,
            max_git_status_files: 0,
            todo_list_enabled: true,
            max_workspace_files: 200,
            max_open_tabs: 20,
        }
    }
}

/// Todo item for reminder section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItemInput {
    pub content: String,
    pub status: String, // "pending", "in_progress", "completed"
}

/// Workspace files listing info.
#[derive(Debug, Clone)]
pub struct WorkspaceFilesInfo {
    pub files: Vec<String>,
    pub did_hit_limit: bool,
}

/// Input data for generating environment details.
///
/// Every field that the TS version reads from `cline`, `state`, or VS Code
/// APIs is represented here as a plain data field so the formatting logic
/// remains a pure function.
#[derive(Debug, Clone)]
pub struct EnvironmentInput {
    /// Current working directory (absolute path).
    pub cwd: String,
    /// Relative paths of files open in the editor's visible area.
    pub visible_files: Vec<String>,
    /// Relative paths of files in open tabs.
    pub open_tabs: Vec<String>,
    /// Currently running (busy) terminals.
    pub active_terminals: Vec<TerminalInfo>,
    /// Inactive terminals that may have completed-process output.
    pub inactive_terminals: Vec<InactiveTerminalInfo>,
    /// Files modified since the last call.
    pub recently_modified_files: Vec<String>,
    /// Pre-computed git status string (already filtered / truncated).
    pub git_status: Option<String>,
    /// Running total cost in USD.
    pub total_cost: Option<f64>,
    /// Current mode / model metadata.
    pub mode_info: ModeDisplayInfo,
    /// Feature flags and limits.
    pub settings: EnvironmentSettings,
    /// Optional todo-list items.
    pub todo_list: Option<Vec<TodoItemInput>>,
    /// When `includeFileDetails` is `true`, this contains the file listing.
    pub workspace_files: Option<WorkspaceFilesInfo>,
    /// Whether the CWD is the user's Desktop.
    pub is_desktop: bool,
}
