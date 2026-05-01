//! Type definitions for the command system.
//!
//! Maps to TypeScript source: `src/services/command/commands.ts` (Command interface, CommandSource, CommandFileInfo)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Source of a command — determines priority ordering.
///
/// Priority: `Project` > `Global` > `BuiltIn`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandSource {
    BuiltIn,
    Global,
    Project,
}

impl std::fmt::Display for CommandSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandSource::BuiltIn => write!(f, "built-in"),
            CommandSource::Global => write!(f, "global"),
            CommandSource::Project => write!(f, "project"),
        }
    }
}

/// A slash command loaded from a `.md` file or built-in definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Command name (filename without `.md` extension).
    pub name: String,
    /// The body content of the command (after frontmatter extraction).
    pub content: String,
    /// Where the command was loaded from.
    pub source: CommandSource,
    /// Absolute path to the command file on disk.
    pub file_path: PathBuf,
    /// Optional description extracted from YAML frontmatter.
    pub description: Option<String>,
    /// Optional argument hint extracted from YAML frontmatter.
    pub argument_hint: Option<String>,
    /// Optional mode extracted from YAML frontmatter.
    pub mode: Option<String>,
}

/// Information about a resolved command file, including symlink resolution.
#[derive(Debug, Clone)]
pub struct CommandFileInfo {
    /// Original path (symlink path if symlinked, otherwise the file path).
    pub original_path: PathBuf,
    /// Resolved path (target of symlink if symlinked, otherwise the file path).
    pub resolved_path: PathBuf,
}
