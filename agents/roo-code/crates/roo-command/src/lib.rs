//! # roo-command — Slash Command Loading and Management
//!
//! This crate provides the command loading infrastructure for Roo Code's slash
//! commands. Commands can come from three sources with different priorities:
//!
//! | Priority | Source   | Location                              |
//! |----------|----------|---------------------------------------|
//! | High     | Project  | `<project>/.roo/commands/*.md`        |
//! | Medium   | Global   | `~/.roo/commands/*.md`                |
//! | Low      | Built-in | Hard-coded (placeholder for now)      |
//!
//! ## Module Structure
//!
//! - [`types`]       — `Command`, `CommandSource`, `CommandFileInfo`
//! - [`frontmatter`] — YAML frontmatter parser for `description`, `argument-hint`, `mode`
//! - [`scanner`]     — Directory scanning and symlink resolution
//! - [`loader`]      — High-level API: `get_commands`, `get_command`, `get_command_names`
//! - [`utils`]       — Filename helpers: `get_command_name_from_file`, `is_markdown_file`
//!
//! ## Maps to TypeScript Source
//!
//! `src/services/command/commands.ts`

pub mod frontmatter;
pub mod loader;
pub mod scanner;
pub mod types;
pub mod utils;

// Re-export primary types and functions for convenience
pub use types::{Command, CommandFileInfo, CommandSource};

pub use loader::{get_built_in_command, get_built_in_commands, get_command, get_command_names, get_commands, try_load_command};
pub use utils::{get_command_name_from_file, is_markdown_file};
