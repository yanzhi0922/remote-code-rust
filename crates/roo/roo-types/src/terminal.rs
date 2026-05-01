//! Terminal type definitions.
//!
//! Derived from `packages/types/src/terminal.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CommandExecutionStatus
// ---------------------------------------------------------------------------

/// Status of a command execution, streamed via IPC / extension host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum CommandExecutionStatus {
    Started {
        execution_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        command: String,
    },
    Output {
        execution_id: String,
        output: String,
    },
    Exited {
        execution_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },
    Fallback {
        execution_id: String,
    },
    Timeout {
        execution_id: String,
    },
}

// ---------------------------------------------------------------------------
// PersistedCommandOutput
// ---------------------------------------------------------------------------

/// Represents the result of a terminal command execution that may have been
/// truncated and persisted to disk.
///
/// When command output exceeds the configured preview threshold, the full
/// output is saved to a disk artifact file. The LLM receives this structure
/// which contains:
/// - A preview of the output (for immediate display in context)
/// - Metadata about the full output (size, truncation status)
/// - A path to the artifact file for later retrieval via `read_command_output`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedCommandOutput {
    /// Preview of the command output, truncated to the preview threshold.
    /// Always contains the beginning of the output, even if truncated.
    pub preview: String,

    /// Total size of the command output in bytes.
    pub total_bytes: u64,

    /// Absolute path to the artifact file containing full output.
    /// `None` if output wasn't truncated (no artifact was created).
    pub artifact_path: Option<String>,

    /// Whether the output was truncated (exceeded preview threshold).
    /// When `true`, use `read_command_output` to retrieve full content.
    pub truncated: bool,
}
