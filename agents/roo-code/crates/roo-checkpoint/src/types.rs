use serde::{Deserialize, Serialize};

/// A pair of relative and absolute file paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathPair {
    pub relative: String,
    pub absolute: String,
}

/// A pair of before/after file content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPair {
    pub before: String,
    pub after: String,
}

/// Represents a diff between two checkpoint states for a single file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointDiff {
    pub paths: PathPair,
    pub content: ContentPair,
}

/// Summary statistics of a commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub changes: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Result of a checkpoint save operation.
/// Mirrors `Partial<CommitResult> & Pick<CommitResult, "commit">` from simple-git.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub commit: String,
    pub branch: Option<String>,
    pub summary: Option<CommitSummary>,
}

/// Options for creating a checkpoint service.
#[derive(Clone, Debug)]
pub struct CheckpointServiceOptions {
    pub task_id: String,
    pub workspace_dir: String,
    pub shadow_dir: String,
    pub log: Option<fn(&str)>,
}

/// Events emitted by the checkpoint service during its lifecycle.
#[derive(Clone, Debug)]
pub enum CheckpointEvent {
    /// Emitted when the shadow git repo is initialized.
    Initialize {
        workspace_dir: String,
        base_hash: String,
        created: bool,
        duration_ms: u64,
    },
    /// Emitted when a checkpoint is saved.
    Checkpoint {
        from_hash: String,
        to_hash: String,
        duration_ms: u64,
        suppress_message: bool,
    },
    /// Emitted when a checkpoint is restored.
    Restore {
        commit_hash: String,
        duration_ms: u64,
    },
    /// Emitted when an error occurs.
    Error {
        error: String,
    },
}

/// Options for the `save_checkpoint` method.
#[derive(Clone, Debug, Default)]
pub struct SaveCheckpointOptions {
    pub allow_empty: bool,
    pub suppress_message: bool,
}

/// Parameters for the `get_diff` method.
#[derive(Clone, Debug, Default)]
pub struct GetDiffParams {
    pub from: Option<String>,
    pub to: Option<String>,
}
