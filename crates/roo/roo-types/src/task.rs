//! Task type definitions.
//!
//! Derived from `packages/types/src/task.ts`.
//! Defines task lifecycle types, task status, and task events.

use serde::{Deserialize, Serialize};

use crate::message::TokenUsage;
use crate::tool::ToolUsage;

// ---------------------------------------------------------------------------
// TaskStatus
// ---------------------------------------------------------------------------

/// The current status of a task.
///
/// Source: `packages/types/src/task.ts` — `TaskStatus`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Aborted,
}

// ---------------------------------------------------------------------------
// CreateTaskOptions
// ---------------------------------------------------------------------------

/// Options for creating a new task.
///
/// Source: `packages/types/src/task.ts` — `CreateTaskOptions`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskOptions {
    /// The text of the user's request.
    pub text: Option<String>,
    /// Base64-encoded images.
    pub images: Option<Vec<String>>,
    /// Files to include.
    pub files: Option<Vec<String>>,
    /// Whether to resume an existing task.
    pub resume: Option<bool>,
    /// Whether this is a subtask.
    pub is_subtask: Option<bool>,
    /// The parent task ID (if this is a subtask).
    pub parent_task_id: Option<String>,
    /// Task ID to resume.
    pub task_id: Option<String>,
    /// Initial mode for the task.
    pub mode: Option<String>,
    /// API configuration name.
    pub api_config_name: Option<String>,
    /// Number of retries.
    pub num_retries: Option<u32>,
    /// Whether to include file details.
    pub include_file_details: Option<bool>,
}

// ---------------------------------------------------------------------------
// TaskEvents
// ---------------------------------------------------------------------------

/// Events emitted by a task.
///
/// Source: `packages/types/src/task.ts` — `TaskEvents`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskEvent {
    #[serde(rename = "started")]
    Started { task_id: String },
    #[serde(rename = "completed")]
    Completed {
        task_id: String,
        token_usage: TokenUsage,
        tool_usage: ToolUsage,
        is_subtask: bool,
    },
    #[serde(rename = "aborted")]
    Aborted { task_id: String },
    #[serde(rename = "paused")]
    Paused { task_id: String },
    #[serde(rename = "unpaused")]
    Unpaused { task_id: String },
    #[serde(rename = "message")]
    Message {
        task_id: String,
        action: String,
        message: Option<serde_json::Value>,
    },
    #[serde(rename = "mode_switched")]
    ModeSwitched { task_id: String, mode: String },
    #[serde(rename = "token_usage_updated")]
    TokenUsageUpdated {
        task_id: String,
        token_usage: TokenUsage,
    },
    #[serde(rename = "tool_failed")]
    ToolFailed {
        task_id: String,
        tool: String,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// TaskProviderEvents
// ---------------------------------------------------------------------------

/// Events emitted by the task provider.
///
/// Source: `packages/types/src/task.ts` — `TaskProviderEvents`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskProviderEvent {
    #[serde(rename = "task_created")]
    TaskCreated { task_id: String },
    #[serde(rename = "task_active")]
    TaskActive { task_id: String },
    #[serde(rename = "task_interactive")]
    TaskInteractive { task_id: String },
    #[serde(rename = "task_resumable")]
    TaskResumable { task_id: String },
    #[serde(rename = "task_idle")]
    TaskIdle { task_id: String },
}

// ---------------------------------------------------------------------------
// TaskLike — trait-like interface for task objects
// ---------------------------------------------------------------------------

/// Minimal interface that a task object must implement.
///
/// Source: `packages/types/src/task.ts` — `TaskLike`
pub trait TaskLike: Send + Sync {
    fn task_id(&self) -> &str;
    fn status(&self) -> TaskStatus;
    fn mode(&self) -> &str;
    fn token_usage(&self) -> &TokenUsage;
}

// ---------------------------------------------------------------------------
// TaskProviderLike — trait-like interface for task providers
// ---------------------------------------------------------------------------

/// Minimal interface that a task provider must implement.
///
/// Source: `packages/types/src/task.ts` — `TaskProviderLike`
#[allow(async_fn_in_trait)]
pub trait TaskProviderLike: Send + Sync {
    async fn create_task(&self, options: CreateTaskOptions) -> Result<String, String>;
    async fn abort_task(&self, task_id: &str) -> Result<(), String>;
}
