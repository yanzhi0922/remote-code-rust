//! Core @ mention parsing logic.
//!
//! Maps to TypeScript source: `src/core/mentions/index.ts` (parseMentions)

use std::collections::{HashMap, HashSet};
use std::path::Path;

use roo_command::{get_command, Command};

use crate::file_content::get_file_or_folder_content;
use crate::regex::{command_regex, is_git_hash, mention_regex};
use crate::types::{MentionContentBlock, ParseMentionsResult};

/// Parse @ mentions and slash commands in the given text.
///
/// This function:
/// 1. First pass: checks which slash commands exist and caches results
/// 2. Second pass: replaces @ mentions with clean references
/// 3. Fetches content for each mention type (files, git, etc.)
///
/// # Arguments
/// * `text` - The input text containing @ mentions and/or slash commands
/// * `cwd` - The current working directory for resolving file paths
///
/// # Returns
/// A `ParseMentionsResult` with the processed text, content blocks, and optional mode.
pub async fn parse_mentions(text: &str, cwd: &Path) -> ParseMentionsResult {
    let mut mentions: HashSet<String> = HashSet::new();
    let mut content_blocks: Vec<MentionContentBlock> = Vec::new();
    let mut command_mode: Option<String> = None;

    // First pass: check which command mentions exist and cache the results
    let command_matches: Vec<_> = command_regex().captures_iter(text).collect();
    let unique_command_names: HashSet<String> = command_matches
        .iter()
        .map(|cap| cap[1].to_string())
        .collect();

    let mut valid_commands: HashMap<String, Command> = HashMap::new();

    for command_name in &unique_command_names {
        if let Some(command) = get_command(cwd, command_name).await {
            // Capture the mode from the first command that has one
            if command_mode.is_none() && command.mode.is_some() {
                command_mode = command.mode.clone();
            }
            valid_commands.insert(command_name.clone(), command);
        }
    }

    // Replace text for commands that actually exist
    let mut parsed_text = text.to_string();
    for cap in &command_matches {
        let full_match = &cap[0];
        let command_name = &cap[1];
        if valid_commands.contains_key(command_name) {
            parsed_text = parsed_text.replace(
                full_match,
                &format!("Command '{}' (see below for command content)", command_name),
            );
        }
    }

    // Second pass: handle regular mentions - replace with clean references
    parsed_text = mention_regex()
        .replace_all(&parsed_text, |caps: &regex::Captures| {
            let mention = &caps[1];
            mentions.insert(mention.to_string());

            if mention.starts_with("http") {
                format!("'{}'", mention)
            } else if mention.starts_with('/') {
                let mention_path = &mention[1..];
                format!("'{}'", mention_path)
            } else if mention == "problems" {
                "Workspace Problems (see below for diagnostics)".to_string()
            } else if mention == "git-changes" {
                "Working directory changes (see below for details)".to_string()
            } else if is_git_hash(mention) {
                format!("Git commit '{}' (see below for commit info)", mention)
            } else if mention == "terminal" {
                "Terminal Output (see below for output)".to_string()
            } else {
                caps[0].to_string()
            }
        })
        .to_string();

    // Process each mention and gather content
    for mention in &mentions {
        if mention.starts_with('/') {
            let mention_path = &mention[1..];
            match get_file_or_folder_content(mention_path, cwd).await {
                Ok(block) => {
                    content_blocks.push(block);
                }
                Err(error_msg) => {
                    let block_type = if mention.ends_with('/') {
                        crate::types::MentionBlockType::Folder
                    } else {
                        crate::types::MentionBlockType::File
                    };
                    content_blocks.push(MentionContentBlock {
                        block_type,
                        path: Some(mention_path.to_string()),
                        content: format!(
                            "[read_file for '{}']\nError: {}",
                            mention_path, error_msg
                        ),
                        metadata: None,
                    });
                }
            }
        } else if mention == "problems" {
            let content = crate::providers::get_problems_content(None);
            parsed_text.push_str(&format!(
                "\n\n<workspace_diagnostics>\n{}\n</workspace_diagnostics>",
                content
            ));
            content_blocks.push(MentionContentBlock::diagnostics(&content));
        } else if mention == "git-changes" {
            let content = crate::providers::get_git_changes_content(cwd).await;
            parsed_text.push_str(&format!(
                "\n\n<git_working_state>\n{}\n</git_working_state>",
                content
            ));
            content_blocks.push(MentionContentBlock::git_changes(&content));
        } else if is_git_hash(mention) {
            // Get git commit info via command line
            let content = get_git_commit_info(cwd, mention).await;
            parsed_text.push_str(&format!(
                "\n\n<git_commit hash=\"{}\">\n{}\n</git_commit>",
                mention, content
            ));
            content_blocks.push(MentionContentBlock::git_commit(mention, &content));
        } else if mention == "terminal" {
            let content = crate::providers::get_terminal_output(None);
            parsed_text.push_str(&format!(
                "\n\n<terminal_output>\n{}\n</terminal_output>",
                content
            ));
            content_blocks.push(MentionContentBlock::terminal(&content));
        } else if mention.starts_with("http") {
            // URL mention — fetch content
            let content = crate::providers::get_url_content(mention).await;
            parsed_text.push_str(&format!(
                "\n\n<url_content url=\"{}\">\n{}\n</url_content>",
                mention, content
            ));
            content_blocks.push(MentionContentBlock::url(&content));
        }
    }

    // Process valid command mentions using cached results
    let mut slash_command_help = String::new();
    for (command_name, command) in &valid_commands {
        let mut command_output = String::new();
        if let Some(desc) = &command.description {
            command_output.push_str(&format!("Description: {}\n\n", desc));
        }
        command_output.push_str(&command.content);
        slash_command_help.push_str(&format!(
            "\n\n<command name=\"{}\">\n{}\n</command>",
            command_name, command_output
        ));
    }

    ParseMentionsResult {
        text: parsed_text,
        content_blocks,
        slash_command_help: if slash_command_help.trim().is_empty() {
            None
        } else {
            Some(slash_command_help.trim().to_string())
        },
        mode: command_mode,
    }
}

/// Get git commit info by running `git show --stat <hash>`.
async fn get_git_commit_info(cwd: &Path, hash: &str) -> String {
    let output = tokio::process::Command::new("git")
        .args(["show", "--stat", "--format=fuller", hash])
        .current_dir(cwd)
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("Git commit info not available: {}", stderr.trim())
        }
        Err(e) => format!("Failed to get git commit info: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_mentions_no_mentions() {
        let result = parse_mentions("hello world", Path::new("/tmp")).await;
        assert_eq!(result.text, "hello world");
        assert!(result.content_blocks.is_empty());
        assert!(result.slash_command_help.is_none());
        assert!(result.mode.is_none());
    }

    #[tokio::test]
    async fn test_parse_mentions_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        tokio::fs::write(&file_path, "fn main() {}")
            .await
            .unwrap();

        let result = parse_mentions(
            &format!("look at @/test.rs in the project",),
            dir.path(),
        )
        .await;

        assert!(result.text.contains("'test.rs'"));
        assert!(!result.text.contains("@/test.rs"));
        assert_eq!(result.content_blocks.len(), 1);
        assert_eq!(result.content_blocks[0].block_type, crate::types::MentionBlockType::File);
        assert!(result.content_blocks[0].content.contains("fn main() {}"));
    }

    #[tokio::test]
    async fn test_parse_mentions_folder_path() {
        let dir = tempfile::tempdir().unwrap();
        let sub_dir = dir.path().join("src");
        tokio::fs::create_dir(&sub_dir).await.unwrap();
        let file = sub_dir.join("main.rs");
        tokio::fs::write(&file, "fn main() {}").await.unwrap();

        let result = parse_mentions("look at @/src/ folder", dir.path()).await;

        assert!(result.text.contains("'src/'"));
        assert_eq!(result.content_blocks.len(), 1);
        assert_eq!(result.content_blocks[0].block_type, crate::types::MentionBlockType::Folder);
    }

    #[tokio::test]
    async fn test_parse_mentions_problems() {
        let result = parse_mentions("check @problems", Path::new("/tmp")).await;
        assert!(result.text.contains("Workspace Problems (see below for diagnostics)"));
        assert!(result.text.contains("<workspace_diagnostics>"));
    }

    #[tokio::test]
    async fn test_parse_mentions_git_changes() {
        let result = parse_mentions("see @git-changes", Path::new("/tmp")).await;
        assert!(result.text.contains("Working directory changes (see below for details)"));
        assert!(result.text.contains("<git_working_state>"));
    }

    #[tokio::test]
    async fn test_parse_mentions_git_commit_hash() {
        let result = parse_mentions("commit @abc1234", Path::new("/tmp")).await;
        assert!(result.text.contains("Git commit 'abc1234'"));
        assert!(result.text.contains("<git_commit hash=\"abc1234\">"));
    }

    #[tokio::test]
    async fn test_parse_mentions_terminal() {
        let result = parse_mentions("check @terminal", Path::new("/tmp")).await;
        assert!(result.text.contains("Terminal Output (see below for output)"));
        assert!(result.text.contains("<terminal_output>"));
    }

    #[tokio::test]
    async fn test_parse_mentions_url() {
        let result = parse_mentions("see @https://example.com for info", Path::new("/tmp")).await;
        assert!(result.text.contains("'https://example.com'"));
        assert!(!result.text.contains("@https://example.com"));
    }

    #[tokio::test]
    async fn test_parse_mentions_http_url() {
        let result = parse_mentions("see @http://example.com", Path::new("/tmp")).await;
        assert!(result.text.contains("'http://example.com'"));
    }

    #[tokio::test]
    async fn test_parse_mentions_multiple_mentions() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        tokio::fs::write(&file_path, "fn main() {}")
            .await
            .unwrap();

        let result = parse_mentions(
            "@/test.rs and @problems and @terminal",
            dir.path(),
        )
        .await;

        assert!(result.text.contains("'test.rs'"));
        assert!(result.text.contains("Workspace Problems"));
        assert!(result.text.contains("Terminal Output"));
        // Should have 3 content blocks (file, problems, terminal)
        assert_eq!(result.content_blocks.len(), 3);
    }

    #[tokio::test]
    async fn test_parse_mentions_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = parse_mentions("@/nonexistent.rs", dir.path()).await;
        // Should still replace the mention but with an error content block
        assert!(result.text.contains("'nonexistent.rs'"));
        assert_eq!(result.content_blocks.len(), 1);
        assert!(result.content_blocks[0].content.contains("Error"));
    }

    #[tokio::test]
    async fn test_parse_mentions_empty_text() {
        let result = parse_mentions("", Path::new("/tmp")).await;
        assert_eq!(result.text, "");
        assert!(result.content_blocks.is_empty());
    }

    #[tokio::test]
    async fn test_parse_mentions_text_only() {
        let result = parse_mentions("just some text without mentions", Path::new("/tmp")).await;
        assert_eq!(result.text, "just some text without mentions");
        assert!(result.content_blocks.is_empty());
    }

    #[tokio::test]
    async fn test_parse_mentions_git_hash_40_chars() {
        let hash = "a".repeat(40);
        let result = parse_mentions(&format!("@{}", hash), Path::new("/tmp")).await;
        assert!(result.text.contains(&format!("Git commit '{}'", hash)));
    }

    #[tokio::test]
    async fn test_parse_mentions_mixed_text_and_mentions() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("hello.rs");
        tokio::fs::write(&file_path, "println!(\"hello\")")
            .await
            .unwrap();

        let result = parse_mentions(
            "Please look at @/hello.rs and also check @terminal",
            dir.path(),
        )
        .await;

        assert!(result.text.contains("'hello.rs'"));
        assert!(result.text.contains("Terminal Output"));
        assert!(result.text.contains("Please look at"));
        assert_eq!(result.content_blocks.len(), 2);
    }
}
