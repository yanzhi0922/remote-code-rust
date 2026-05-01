//! Types for mode tool results and errors.

use serde::{Deserialize, Serialize};

/// Result of a switch_mode operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeSwitchResult {
    pub mode_slug: String,
    pub reason: Option<String>,
    pub is_same_mode: bool,
}

/// Result of a new_task operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTaskResult {
    pub mode: String,
    pub message: String,
    pub todos: Option<String>,
}

/// Validation result for mode operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeValidation {
    pub mode_slug: String,
    pub is_valid: bool,
    pub error: Option<String>,
}

/// Error type for mode tool operations.
#[derive(Debug, thiserror::Error)]
pub enum ModeToolError {
    #[error("Invalid mode: {0}")]
    InvalidMode(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Same mode: {0}")]
    SameMode(String),
}

/// Valid mode slugs.
pub const VALID_MODE_SLUGS: &[&str] = &[
    "code",
    "architect",
    "ask",
    "debug",
    "orchestrator",
];
