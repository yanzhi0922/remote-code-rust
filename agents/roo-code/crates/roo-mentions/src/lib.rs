//! # roo-mentions — @ Mention Parsing and Processing
//!
//! This crate provides the @ mention parsing and processing infrastructure for
//! Roo Code. It handles parsing of various mention types in user text and
//! resolves them into content blocks.
//!
//! ## Supported Mention Types
//!
//! | Mention | Type | Description |
//! |---------|------|-------------|
//! | `@/path/to/file` | File | Reads file content |
//! | `@/path/to/folder/` | Folder | Lists directory contents |
//! | `@problems` | Diagnostics | Workspace diagnostics |
//! | `@git-changes` | Git | Git working tree changes |
//! | `@[7-40 hex chars]` | Git Commit | Git commit information |
//! | `@terminal` | Terminal | Terminal output |
//! | `@https://...` | URL | URL reference |
//!
//! ## Module Structure
//!
//! - [`types`] — `MentionContentBlock`, `ParseMentionsResult`, `ContentBlock`
//! - [`regex`] — `mention_regex()`, `command_regex()`, `unescape_spaces()`, `is_git_hash()`
//! - [`parser`] — `parse_mentions()` core parsing logic
//! - [`processor`] — `process_user_content_mentions()` for user content processing
//! - [`file_content`] — `get_file_or_folder_content()` for reading files/folders
//! - [`format`] — `format_file_read_result()` for formatting file results
//!
//! ## Maps to TypeScript Source
//!
//! - `src/core/mentions/index.ts` (parseMentions, MentionContentBlock)
//! - `src/core/mentions/processUserContentMentions.ts` (processUserContentMentions)
//! - `src/shared/context-mentions.ts` (mentionRegexGlobal, commandRegexGlobal)

pub mod file_content;
pub mod format;
pub mod parser;
pub mod processor;
pub mod providers;
pub mod regex;
pub mod types;

// Re-export primary types and functions for convenience
pub use types::{
    ContentBlock, MentionBlockType, MentionContentBlock, MentionMetadata,
    ParseMentionsResult, ProcessUserContentMentionsResult,
};

pub use parser::parse_mentions;
pub use processor::process_user_content_mentions;

pub use regex::{command_regex, is_git_hash, mention_regex, unescape_spaces};

pub use format::{format_file_read_result, ExtractTextResult, DEFAULT_LINE_LIMIT};

pub use file_content::{extract_text_from_file_with_metadata, get_file_or_folder_content, is_binary_extension};
