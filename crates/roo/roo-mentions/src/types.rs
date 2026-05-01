//! Type definitions for the mentions system.
//!
//! Maps to TypeScript source: `src/core/mentions/index.ts` (MentionContentBlock, ParseMentionsResult)
//! and `src/core/mentions/processUserContentMentions.ts` (ProcessUserContentMentionsResult)

use serde::{Deserialize, Serialize};

/// The type of content block generated from an @ mention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionBlockType {
    File,
    Folder,
    Url,
    Diagnostics,
    GitChanges,
    GitCommit,
    Terminal,
    Command,
}

impl std::fmt::Display for MentionBlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MentionBlockType::File => write!(f, "file"),
            MentionBlockType::Folder => write!(f, "folder"),
            MentionBlockType::Url => write!(f, "url"),
            MentionBlockType::Diagnostics => write!(f, "diagnostics"),
            MentionBlockType::GitChanges => write!(f, "git_changes"),
            MentionBlockType::GitCommit => write!(f, "git_commit"),
            MentionBlockType::Terminal => write!(f, "terminal"),
            MentionBlockType::Command => write!(f, "command"),
        }
    }
}

/// Metadata about a file content block, including truncation info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionMetadata {
    /// Total number of lines in the file.
    pub total_lines: usize,
    /// Number of lines returned.
    pub returned_lines: usize,
    /// Whether the content was truncated.
    pub was_truncated: bool,
    /// The range of lines shown `[start, end]` (1-based, inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_shown: Option<(usize, usize)>,
}

/// Represents a content block generated from an @ mention.
///
/// These are returned separately from the user's text to enable
/// proper formatting as distinct message blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionContentBlock {
    /// The type of this content block.
    #[serde(rename = "type")]
    pub block_type: MentionBlockType,
    /// Path for file/folder mentions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The content to display.
    pub content: String,
    /// Metadata about truncation (for files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MentionMetadata>,
}

impl MentionContentBlock {
    /// Create a new file content block.
    pub fn file(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::File,
            path: Some(path.into()),
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new file content block with metadata.
    pub fn file_with_metadata(
        path: impl Into<String>,
        content: impl Into<String>,
        metadata: MentionMetadata,
    ) -> Self {
        Self {
            block_type: MentionBlockType::File,
            path: Some(path.into()),
            content: content.into(),
            metadata: Some(metadata),
        }
    }

    /// Create a new folder content block.
    pub fn folder(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::Folder,
            path: Some(path.into()),
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new URL content block.
    pub fn url(content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::Url,
            path: None,
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new diagnostics content block.
    pub fn diagnostics(content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::Diagnostics,
            path: None,
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new git changes content block.
    pub fn git_changes(content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::GitChanges,
            path: None,
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new git commit content block.
    pub fn git_commit(hash: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::GitCommit,
            path: Some(hash.into()),
            content: content.into(),
            metadata: None,
        }
    }

    /// Create a new terminal content block.
    pub fn terminal(content: impl Into<String>) -> Self {
        Self {
            block_type: MentionBlockType::Terminal,
            path: None,
            content: content.into(),
            metadata: None,
        }
    }
}

/// Result of parsing @ mentions in a text string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseMentionsResult {
    /// User's text with @ mentions replaced by clean path references.
    pub text: String,
    /// Separate content blocks for each mention (file content, URLs, etc.).
    pub content_blocks: Vec<MentionContentBlock>,
    /// Slash command help text (if any valid slash commands were found).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slash_command_help: Option<String>,
    /// Mode from the first slash command that has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl ParseMentionsResult {
    /// Create an empty result with just the original text.
    pub fn empty(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            content_blocks: Vec::new(),
            slash_command_help: None,
            mode: None,
        }
    }
}

/// Result of processing user content mentions.
///
/// Maps to TypeScript source: `processUserContentMentions.ts` (ProcessUserContentMentionsResult)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessUserContentMentionsResult {
    /// Processed content blocks.
    pub content: Vec<ContentBlock>,
    /// Mode from the first slash command that has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// A simplified content block used in processing results.
/// Represents a text block in the processed output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// A text content block.
    Text { text: String },
    /// An image content block (passed through unchanged).
    Image {
        #[serde(rename = "source")]
        source: serde_json::Value,
    },
}

impl ContentBlock {
    /// Create a new text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
        }
    }

    /// Get the text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mention_block_type_display() {
        assert_eq!(MentionBlockType::File.to_string(), "file");
        assert_eq!(MentionBlockType::Folder.to_string(), "folder");
        assert_eq!(MentionBlockType::Url.to_string(), "url");
        assert_eq!(MentionBlockType::Diagnostics.to_string(), "diagnostics");
        assert_eq!(MentionBlockType::GitChanges.to_string(), "git_changes");
        assert_eq!(MentionBlockType::GitCommit.to_string(), "git_commit");
        assert_eq!(MentionBlockType::Terminal.to_string(), "terminal");
        assert_eq!(MentionBlockType::Command.to_string(), "command");
    }

    #[test]
    fn test_mention_content_block_file() {
        let block = MentionContentBlock::file("src/main.rs", "fn main() {}");
        assert_eq!(block.block_type, MentionBlockType::File);
        assert_eq!(block.path.as_deref(), Some("src/main.rs"));
        assert_eq!(block.content, "fn main() {}");
        assert!(block.metadata.is_none());
    }

    #[test]
    fn test_mention_content_block_file_with_metadata() {
        let metadata = MentionMetadata {
            total_lines: 100,
            returned_lines: 50,
            was_truncated: true,
            lines_shown: Some((1, 50)),
        };
        let block = MentionContentBlock::file_with_metadata(
            "src/main.rs",
            "fn main() {}",
            metadata.clone(),
        );
        assert_eq!(block.block_type, MentionBlockType::File);
        assert_eq!(block.metadata.as_ref(), Some(&metadata));
    }

    #[test]
    fn test_mention_content_block_folder() {
        let block = MentionContentBlock::folder("src/", "listing...");
        assert_eq!(block.block_type, MentionBlockType::Folder);
        assert_eq!(block.path.as_deref(), Some("src/"));
    }

    #[test]
    fn test_mention_content_block_url() {
        let block = MentionContentBlock::url("https://example.com");
        assert_eq!(block.block_type, MentionBlockType::Url);
        assert!(block.path.is_none());
    }

    #[test]
    fn test_mention_content_block_diagnostics() {
        let block = MentionContentBlock::diagnostics("No errors");
        assert_eq!(block.block_type, MentionBlockType::Diagnostics);
    }

    #[test]
    fn test_mention_content_block_git_changes() {
        let block = MentionContentBlock::git_changes("M file.rs");
        assert_eq!(block.block_type, MentionBlockType::GitChanges);
    }

    #[test]
    fn test_mention_content_block_git_commit() {
        let block = MentionContentBlock::git_commit("abc1234", "commit msg");
        assert_eq!(block.block_type, MentionBlockType::GitCommit);
        assert_eq!(block.path.as_deref(), Some("abc1234"));
    }

    #[test]
    fn test_mention_content_block_terminal() {
        let block = MentionContentBlock::terminal("output");
        assert_eq!(block.block_type, MentionBlockType::Terminal);
    }

    #[test]
    fn test_parse_mentions_result_empty() {
        let result = ParseMentionsResult::empty("hello world");
        assert_eq!(result.text, "hello world");
        assert!(result.content_blocks.is_empty());
        assert!(result.slash_command_help.is_none());
        assert!(result.mode.is_none());
    }

    #[test]
    fn test_content_block_text() {
        let block = ContentBlock::text("hello");
        assert_eq!(block.as_text(), Some("hello"));
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::text("hello");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn test_mention_content_block_serialization() {
        let block = MentionContentBlock::file("src/main.rs", "content");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"file\""));
        assert!(json.contains("\"path\":\"src/main.rs\""));
        assert!(json.contains("\"content\":\"content\""));
    }

    #[test]
    fn test_mention_metadata_serialization() {
        let metadata = MentionMetadata {
            total_lines: 200,
            returned_lines: 50,
            was_truncated: true,
            lines_shown: Some((1, 50)),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"total_lines\":200"));
        assert!(json.contains("\"was_truncated\":true"));
        assert!(json.contains("\"lines_shown\":[1,50]"));
    }
}
