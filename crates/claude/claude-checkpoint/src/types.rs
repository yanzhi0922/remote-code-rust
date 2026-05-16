//! Core types for the checkpoint system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A checkpoint captures the state of workspace files at a specific point in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub session_id: String,
    pub message_id: String,
    pub message_index: usize,
    pub created_at: DateTime<Utc>,
    pub file_changes: Vec<FileChange>,
    pub summary: String,
    pub stats: CheckpointStats,
}

/// Statistics about the changes in a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub files_added: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl CheckpointStats {
    pub fn is_empty(&self) -> bool {
        self.files_added == 0 && self.files_modified == 0 && self.files_deleted == 0
    }
}

/// A single file change within a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Path relative to workspace root.
    pub path: String,
    /// Type of change.
    pub operation: FileOperation,
    /// SHA256 hash of the file content before the change (None for created files).
    pub hash_before: Option<String>,
    /// SHA256 hash of the file content after the change (None for deleted files).
    pub hash_after: Option<String>,
    /// Number of lines added.
    pub lines_added: usize,
    /// Number of lines removed.
    pub lines_removed: usize,
}

/// The type of file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}

impl std::fmt::Display for FileOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileOperation::Created => write!(f, "created"),
            FileOperation::Modified => write!(f, "modified"),
            FileOperation::Deleted => write!(f, "deleted"),
        }
    }
}

/// A file snapshot taken at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub hash: String,
    pub size: u64,
}

/// A complete workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub files: Vec<FileSnapshot>,
}

/// Summary of a checkpoint for listing purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub id: CheckpointId,
    pub session_id: String,
    pub message_id: String,
    pub message_index: usize,
    pub created_at: DateTime<Utc>,
    pub summary: String,
    pub stats: CheckpointStats,
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub checkpoint_id: CheckpointId,
    pub files_restored: Vec<FileRestore>,
    pub success: bool,
    pub error: Option<String>,
}

/// A single file restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRestore {
    pub path: String,
    pub operation: RestoreOperation,
}

/// The type of restore operation performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreOperation {
    /// Restored file content to previous version.
    ContentRestored,
    /// Recreated a previously deleted file.
    FileRecreated,
    /// Deleted a newly created file.
    FileDeleted,
}

/// A unified diff for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub operation: FileOperation,
    pub hunks: Vec<DiffHunk>,
}

/// A single diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
}

/// The type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
}

/// Events emitted by the checkpoint system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckpointEvent {
    /// A new checkpoint was created.
    CheckpointCreated {
        session_id: String,
        checkpoint_id: CheckpointId,
        stats: CheckpointStats,
    },
    /// A restore operation completed.
    RestoreCompleted {
        session_id: String,
        checkpoint_id: CheckpointId,
        files_affected: usize,
        success: bool,
    },
}
