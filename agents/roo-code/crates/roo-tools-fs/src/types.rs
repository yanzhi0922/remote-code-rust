//! Types for file system tool results and errors.

use serde::{Deserialize, Serialize};

/// Result of a read_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResult {
    /// The file content, possibly with line numbers prepended.
    pub content: String,
    /// The source path that was read.
    pub path: String,
    /// Total lines in the file.
    pub total_lines: usize,
    /// Whether the content was truncated.
    pub truncated: bool,
    /// Whether the file was detected as binary.
    pub is_binary: bool,
    /// The first line number shown (1-based). 0 for binary files.
    pub start_line: usize,
    /// The last line number shown (1-based). 0 for binary files.
    pub end_line: usize,
}

/// Result of a write_to_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResult {
    /// The path that was written to.
    pub path: String,
    /// Whether a new file was created (vs modifying an existing one).
    pub is_new_file: bool,
    /// Number of lines written.
    pub lines_written: usize,
}

/// Result of an apply_diff operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffApplyResult {
    /// The path that was modified.
    pub path: String,
    /// Number of diff blocks successfully applied.
    pub blocks_applied: usize,
    /// Any warnings encountered during application.
    pub warnings: Vec<String>,
}

/// Result of an edit_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileResult {
    /// The path that was modified.
    pub path: String,
    /// Whether the edit was applied successfully.
    pub success: bool,
    /// Optional message about the result.
    pub message: Option<String>,
}

/// Type of edit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditType {
    /// Create a new file.
    Create,
    /// Modify an existing file.
    Modify,
    /// Delete content.
    Delete,
}

/// Error type for file system tool operations.
#[derive(Debug, thiserror::Error)]
pub enum FsToolError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Binary file detected: {0}")]
    BinaryFile(String),

    #[error("Invalid diff format: {0}")]
    InvalidDiff(String),

    #[error("Content too large: {0} bytes, max {1}")]
    ContentTooLarge(usize, usize),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Diff apply error: {0}")]
    DiffApply(String),

    #[error("Access denied by .rooignore: {0}")]
    AccessDenied(String),
}

/// Maximum default line length for read operations.
pub const DEFAULT_MAX_LINE_LENGTH: usize = 2000;

/// Maximum file size in bytes (50 MB).
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// Default line limit for read operations.
pub const DEFAULT_READ_LIMIT: usize = 2000;
