//! Types for miscellaneous tool results and errors.

use serde::{Deserialize, Serialize};

/// Result of an attempt_completion operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResult {
    /// The completion result text.
    pub result: String,
    /// Whether a command was attached.
    pub has_command: bool,
    /// The detailed completion result data (text + images), matching TS `attempt_completion_result`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_completion_result: Option<CompletionResultData>,
    /// Warning about incomplete todos, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub todo_warning: Option<String>,
}

/// Detailed completion result data, matching TS `attempt_completion_result` field.
///
/// In the TS source, `attempt_completion_result` contains the text and images
/// returned from the user interaction after completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResultData {
    /// The completion result text.
    pub text: String,
    /// Associated image URLs (base64 data URIs).
    #[serde(default)]
    pub images: Vec<String>,
}

/// Result of an ask_followup_question operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowupResult {
    pub question: String,
    pub suggestions: Vec<String>,
}

/// Result of a skill operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub skill_name: String,
    pub args: Option<String>,
    pub is_valid: bool,
    /// The loaded skill content (instructions), if available.
    pub content: Option<String>,
}

/// A parsed todo item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub status: TodoStatus,
    pub text: String,
}

/// Status of a todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    /// Parse from markdown checkbox syntax.
    pub fn from_checkbox(checked: bool) -> Self {
        if checked {
            TodoStatus::Completed
        } else {
            TodoStatus::Pending
        }
    }

    /// Convert to markdown checkbox string.
    pub fn to_checkbox(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "[ ]",
            TodoStatus::InProgress => "[-]",
            TodoStatus::Completed => "[x]",
        }
    }
}

/// Error type for miscellaneous tool operations.
#[derive(Debug, thiserror::Error)]
pub enum MiscToolError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid skill: {0}")]
    InvalidSkill(String),

    #[error("Parse error: {0}")]
    Parse(String),
}
