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
    /// If the file was an image, this contains the base64 data URL.
    /// When present, `content` holds a human-readable notice and `is_binary`
    /// is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_data_url: Option<String>,
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

    #[error("Write protected: {0}")]
    WriteProtected(String),
}

/// Maximum default line length for read operations.
pub const DEFAULT_MAX_LINE_LENGTH: usize = 2000;

/// Maximum file size in bytes (50 MB).
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// Default line limit for read operations.
pub const DEFAULT_READ_LIMIT: usize = 2000;

/// Maximum number of diagnostic messages to include in post-write results.
pub const MAX_DIAGNOSTIC_MESSAGES: usize = 50;

/// Result of running post-write diagnostics on a file.
#[derive(Debug, Clone)]
pub struct DiagnosticsResult {
    /// The file path that was checked.
    pub path: String,
    /// Diagnostic messages (errors only), formatted for display.
    pub messages: Vec<String>,
    /// Whether the diagnostics check was actually performed.
    /// `false` if the file type is not supported for diagnostics.
    pub ran: bool,
}

impl DiagnosticsResult {
    /// Create an empty result (no diagnostics run).
    pub fn skipped(path: &str) -> Self {
        Self {
            path: path.to_string(),
            messages: Vec::new(),
            ran: false,
        }
    }

    /// Create a result with diagnostic messages.
    pub fn with_messages(path: &str, messages: Vec<String>) -> Self {
        let ran = true;
        Self {
            path: path.to_string(),
            messages,
            ran,
        }
    }

    /// Returns true if there are any diagnostic messages.
    pub fn has_problems(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Format the diagnostics as a human-readable string.
    pub fn to_problems_string(&self) -> String {
        if !self.ran || self.messages.is_empty() {
            return String::new();
        }
        format!(
            "\n\nNew problems detected after saving the file:\n{}",
            self.messages.join("\n")
        )
    }
}
