//! Processing user content for @ mentions.
//!
//! Maps to TypeScript source: `src/core/mentions/processUserContentMentions.ts`

use std::path::Path;

use crate::parser::parse_mentions;
use crate::types::{ContentBlock, ProcessUserContentMentionsResult};

/// Process mentions in user content, specifically within `<user_message>` tags.
///
/// This function iterates over content blocks, finds text blocks containing
/// `<user_message>`, and applies mention parsing to them. File/folder mentions
/// are returned as separate text blocks formatted like read_file tool results.
pub async fn process_user_content_mentions(
    user_content: &[ContentBlock],
    cwd: &Path,
) -> ProcessUserContentMentionsResult {
    let mut command_mode: Option<String> = None;

    let mut result_blocks: Vec<ContentBlock> = Vec::new();

    for block in user_content {
        match block {
            ContentBlock::Text { text } => {
                if text.contains("<user_message>") {
                    let parse_result = parse_mentions(text, cwd).await;

                    // Capture the first mode found
                    if command_mode.is_none() && parse_result.mode.is_some() {
                        command_mode = parse_result.mode.clone();
                    }

                    // Add the main text block (with mentions replaced)
                    result_blocks.push(ContentBlock::text(&parse_result.text));

                    // Add file/folder content blocks
                    for content_block in &parse_result.content_blocks {
                        result_blocks.push(ContentBlock::text(&content_block.content));
                    }

                    // Add slash command help if any
                    if let Some(help) = &parse_result.slash_command_help {
                        result_blocks.push(ContentBlock::text(help));
                    }
                } else {
                    result_blocks.push(block.clone());
                }
            }
            ContentBlock::Image { .. } => {
                result_blocks.push(block.clone());
            }
        }
    }

    ProcessUserContentMentionsResult {
        content: result_blocks,
        mode: command_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_user_content_no_mentions() {
        let blocks = vec![ContentBlock::text("<user_message>hello world</user_message>")];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].as_text(), Some("<user_message>hello world</user_message>"));
        assert!(result.mode.is_none());
    }

    #[tokio::test]
    async fn test_process_user_content_with_file_mention() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        tokio::fs::write(&file_path, "fn main() {}")
            .await
            .unwrap();

        let blocks = vec![ContentBlock::text(
            "<user_message>\nlook at @/test.rs\n</user_message>",
        )];
        let result = process_user_content_mentions(&blocks, dir.path()).await;

        // Should have: main text + file content block
        assert!(result.content.len() >= 2);
        // First block should have the mention replaced
        let first_text = result.content[0].as_text().unwrap();
        assert!(first_text.contains("'test.rs'"));
        assert!(!first_text.contains("@/test.rs"));
        // Second block should contain file content
        let second_text = result.content[1].as_text().unwrap();
        assert!(second_text.contains("fn main() {}"));
    }

    #[tokio::test]
    async fn test_process_user_content_no_user_message_tag() {
        let blocks = vec![ContentBlock::text("plain text without tags")];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].as_text(), Some("plain text without tags"));
    }

    #[tokio::test]
    async fn test_process_user_content_multiple_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("hello.rs");
        tokio::fs::write(&file_path, "fn hello() {}")
            .await
            .unwrap();

        let blocks = vec![
            ContentBlock::text("<user_message>\ncheck @/hello.rs\n</user_message>"),
            ContentBlock::text("other text"),
        ];
        let result = process_user_content_mentions(&blocks, dir.path()).await;

        // First block should be expanded
        assert!(result.content.len() >= 2);
        // Last block should be unchanged
        assert_eq!(
            result.content.last().unwrap().as_text(),
            Some("other text")
        );
    }

    #[tokio::test]
    async fn test_process_user_content_image_passthrough() {
        let image_block = ContentBlock::Image {
            source: serde_json::json!({"url": "test.png"}),
        };
        let blocks = vec![
            ContentBlock::text("<user_message>hello</user_message>"),
            image_block.clone(),
        ];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;

        // Should have text block + image block
        assert!(result.content.len() >= 2);
        // Check that image block is preserved
        let has_image = result.content.iter().any(|b| matches!(b, ContentBlock::Image { .. }));
        assert!(has_image);
    }

    #[tokio::test]
    async fn test_process_user_content_with_problems_mention() {
        let blocks = vec![ContentBlock::text(
            "<user_message>\ncheck @problems\n</user_message>",
        )];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;

        let text = result.content[0].as_text().unwrap();
        assert!(text.contains("Workspace Problems"));
        assert!(text.contains("<workspace_diagnostics>"));
    }

    #[tokio::test]
    async fn test_process_user_content_with_terminal_mention() {
        let blocks = vec![ContentBlock::text(
            "<user_message>\ncheck @terminal\n</user_message>",
        )];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;

        let text = result.content[0].as_text().unwrap();
        assert!(text.contains("Terminal Output"));
    }

    #[tokio::test]
    async fn test_process_user_content_empty_blocks() {
        let blocks: Vec<ContentBlock> = vec![];
        let result = process_user_content_mentions(&blocks, Path::new("/tmp")).await;
        assert!(result.content.is_empty());
        assert!(result.mode.is_none());
    }
}
