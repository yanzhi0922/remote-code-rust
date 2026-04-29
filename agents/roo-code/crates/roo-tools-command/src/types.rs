//! Types for command tool results and errors.

use serde::{Deserialize, Serialize};

/// Result of an execute_command operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// The command that was executed.
    pub command: String,
    /// Standard output from the command.
    pub stdout: String,
    /// Standard error from the command.
    pub stderr: String,
    /// Exit code (None if still running).
    pub exit_code: Option<i32>,
    /// Whether the output was truncated.
    pub truncated: bool,
    /// Artifact ID for reading full output later.
    pub artifact_id: Option<String>,
}

/// Persisted command output for later retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedOutput {
    /// Unique artifact identifier.
    pub artifact_id: String,
    /// Full stdout content.
    pub stdout: String,
    /// Full stderr content.
    pub stderr: String,
    /// Whether the command has finished.
    pub finished: bool,
    /// Exit code if finished.
    pub exit_code: Option<i32>,
}

/// Result of reading command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadOutputResult {
    /// The artifact ID that was read.
    pub artifact_id: String,
    /// The output content (possibly filtered/paginated).
    pub content: String,
    /// Total bytes in the full output.
    pub total_bytes: usize,
    /// Whether there's more output available.
    pub has_more: bool,
    /// Number of lines matched by search filter.
    pub matched_lines: Option<usize>,
}

/// Execution status of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Timeout,
}

/// Error type for command tool operations.
#[derive(Debug, thiserror::Error)]
pub enum CommandToolError {
    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Invalid artifact ID: {0}")]
    InvalidArtifactId(String),

    #[error("Artifact not found: {0}")]
    ArtifactNotFound(String),

    #[error("Invalid timeout: {0}")]
    InvalidTimeout(String),

    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    #[error("Output too large: {0} bytes")]
    OutputTooLarge(usize),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Execution error: {0}")]
    Execution(String),
}

/// Maximum output size before truncation (1 MB).
pub const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

/// Default command timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Maximum artifact ID length.
pub const MAX_ARTIFACT_ID_LENGTH: usize = 256;

/// Default page size for reading output.
pub const DEFAULT_PAGE_SIZE: usize = 40960; // 40KB
