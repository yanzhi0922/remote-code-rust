//! Error types for the roo-skills crate.

use thiserror::Error;

/// Errors that can occur during skill operations.
#[derive(Debug, Error)]
pub enum SkillsError {
    /// The skill name is invalid.
    #[error("Invalid skill name: {0}")]
    InvalidName(String),

    /// The skill description is too long.
    #[error("Description is too long ({length} characters, max 1000)")]
    InvalidDescription { length: usize },

    /// A skill with the same name already exists at the given path.
    #[error("Skill '{name}' already exists at {path}")]
    AlreadyExists { name: String, path: String },

    /// The requested skill was not found.
    #[error("Skill '{name}' not found (source: {skill_source}, mode: {mode_info})")]
    NotFound {
        name: String,
        skill_source: String,
        mode_info: String,
    },

    /// No workspace directory is set.
    #[error("No workspace directory configured")]
    NoWorkspace,

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IoError(String),

    /// A parse error occurred while reading a SKILL.md file.
    #[error("Failed to parse '{path}': {reason}")]
    ParseError { path: String, reason: String },
}

impl From<std::io::Error> for SkillsError {
    fn from(err: std::io::Error) -> Self {
        SkillsError::IoError(err.to_string())
    }
}
