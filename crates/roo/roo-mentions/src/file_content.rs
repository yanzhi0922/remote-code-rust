//! File and folder content reading for @ mentions.
//!
//! Maps to TypeScript source: `src/core/mentions/index.ts`
//! (getFileOrFolderContentWithMetadata, extractTextFromFileWithMetadata)

use std::path::Path;

use tokio::fs;

use crate::format::{format_file_read_result, ExtractTextResult, DEFAULT_LINE_LIMIT};
use crate::regex::unescape_spaces;
use crate::types::{MentionBlockType, MentionContentBlock, MentionMetadata};

/// Binary file extensions that should not be read as text.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg", "avif",
    "mp3", "mp4", "wav", "avi", "mov", "mkv", "flv", "wmv", "webm",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    "exe", "dll", "so", "dylib", "bin", "dat",
    "woff", "woff2", "ttf", "eot", "otf",
    "sqlite", "db", "iso", "dmg", "jar", "class",
    "pyc", "o", "obj", "pdb", "wasm",
];

/// Check if a file extension suggests a binary file.
pub fn is_binary_extension(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    BINARY_EXTENSIONS
        .iter()
        .any(|ext| path_lower.ends_with(&format!(".{}", ext)))
}

/// Extract text from a file with metadata, truncating to the default line limit.
///
/// This reads the file content, splits it into lines, and truncates if necessary.
pub async fn extract_text_from_file_with_metadata(
    abs_path: &Path,
) -> std::io::Result<ExtractTextResult> {
    let raw = fs::read_to_string(abs_path).await?;
    let lines: Vec<&str> = raw.lines().collect();
    let total_lines = lines.len();

    if total_lines <= DEFAULT_LINE_LIMIT {
        Ok(ExtractTextResult {
            content: raw,
            total_lines,
            returned_lines: total_lines,
            was_truncated: false,
            lines_shown: None,
        })
    } else {
        let truncated_lines: Vec<&str> = lines[..DEFAULT_LINE_LIMIT].to_vec();
        let content = truncated_lines.join("\n");
        Ok(ExtractTextResult {
            content,
            total_lines,
            returned_lines: DEFAULT_LINE_LIMIT,
            was_truncated: true,
            lines_shown: Some((1, DEFAULT_LINE_LIMIT)),
        })
    }
}

/// Get file or folder content as a `MentionContentBlock`.
///
/// For files: reads the content, detects binary files, and formats the result.
/// For folders: lists the directory contents and reads non-binary files.
pub async fn get_file_or_folder_content(
    mention_path: &str,
    cwd: &Path,
) -> Result<MentionContentBlock, String> {
    let unescaped = unescape_spaces(mention_path);
    let abs_path = cwd.join(&unescaped);
    let is_folder = mention_path.ends_with('/');

    let metadata = fs::metadata(&abs_path)
        .await
        .map_err(|e| format!("Failed to access path \"{}\": {}", mention_path, e))?;

    if metadata.is_file() {
        // Check for binary file
        if is_binary_extension(&unescaped) {
            return Ok(MentionContentBlock {
                block_type: MentionBlockType::File,
                path: Some(mention_path.to_string()),
                content: format!(
                    "[read_file for '{}']\nNote: Binary file omitted from context.",
                    mention_path
                ),
                metadata: None,
            });
        }

        match extract_text_from_file_with_metadata(&abs_path).await {
            Ok(result) => {
                let formatted = format_file_read_result(mention_path, &result);
                let meta = MentionMetadata {
                    total_lines: result.total_lines,
                    returned_lines: result.returned_lines,
                    was_truncated: result.was_truncated,
                    lines_shown: result.lines_shown,
                };
                Ok(MentionContentBlock::file_with_metadata(
                    mention_path,
                    formatted,
                    meta,
                ))
            }
            Err(e) => Ok(MentionContentBlock {
                block_type: MentionBlockType::File,
                path: Some(mention_path.to_string()),
                content: format!(
                    "[read_file for '{}']\nError: {}",
                    mention_path, e
                ),
                metadata: None,
            }),
        }
    } else if metadata.is_dir() {
        let mut folder_listing = String::new();
        let mut file_read_results: Vec<String> = Vec::new();

        let mut entries = fs::read_dir(&abs_path)
            .await
            .map_err(|e| format!("Failed to read directory \"{}\": {}", mention_path, e))?;

        let mut entry_list = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {}", e))?
        {
            entry_list.push(entry);
        }

        let entry_count = entry_list.len();
        for (index, entry) in entry_list.into_iter().enumerate() {
            let is_last = index == entry_count - 1;
            let line_prefix = if is_last { "└── " } else { "├── " };
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            let file_metadata = entry
                .metadata()
                .await
                .map_err(|e| format!("Failed to read metadata: {}", e))?;

            if file_metadata.is_file() {
                folder_listing.push_str(&format!("{}{}\n", line_prefix, name));
                let file_name_str = name.to_string();
                if !is_binary_extension(&file_name_str) {
                    let file_path = format!("{}/{}", mention_path.trim_end_matches('/'), name);
                    let abs_file_path = entry.path();
                    if let Ok(result) = extract_text_from_file_with_metadata(&abs_file_path).await {
                        file_read_results.push(format_file_read_result(&file_path, &result));
                    }
                }
            } else if file_metadata.is_dir() {
                folder_listing.push_str(&format!("{}{}/\n", line_prefix, name));
            } else {
                folder_listing.push_str(&format!("{}{}\n", line_prefix, name));
            }
        }

        let mut content = format!(
            "[read_file for folder '{}']\nFolder listing:\n{}",
            mention_path, folder_listing
        );
        if !file_read_results.is_empty() {
            content.push_str(&format!(
                "\n\n--- File Contents ---\n\n{}",
                file_read_results.join("\n\n")
            ));
        }

        Ok(MentionContentBlock::folder(mention_path, content))
    } else {
        Ok(MentionContentBlock {
            block_type: if is_folder {
                MentionBlockType::Folder
            } else {
                MentionBlockType::File
            },
            path: Some(mention_path.to_string()),
            content: format!(
                "[read_file for '{}']\nError: Unable to read (not a file or directory)",
                mention_path
            ),
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_extension_image() {
        assert!(is_binary_extension("image.png"));
        assert!(is_binary_extension("image.jpg"));
        assert!(is_binary_extension("image.jpeg"));
        assert!(is_binary_extension("image.gif"));
        assert!(is_binary_extension("image.bmp"));
        assert!(is_binary_extension("image.webp"));
        assert!(is_binary_extension("image.ico"));
    }

    #[test]
    fn test_is_binary_extension_video() {
        assert!(is_binary_extension("video.mp4"));
        assert!(is_binary_extension("video.avi"));
        assert!(is_binary_extension("video.mov"));
    }

    #[test]
    fn test_is_binary_extension_archive() {
        assert!(is_binary_extension("archive.zip"));
        assert!(is_binary_extension("archive.tar"));
        assert!(is_binary_extension("archive.gz"));
        assert!(is_binary_extension("archive.7z"));
    }

    #[test]
    fn test_is_binary_extension_font() {
        assert!(is_binary_extension("font.woff"));
        assert!(is_binary_extension("font.ttf"));
        assert!(is_binary_extension("font.eot"));
    }

    #[test]
    fn test_is_binary_extension_text_files() {
        assert!(!is_binary_extension("main.rs"));
        assert!(!is_binary_extension("index.ts"));
        assert!(!is_binary_extension("style.css"));
        assert!(!is_binary_extension("data.json"));
        assert!(!is_binary_extension("config.yaml"));
        assert!(!is_binary_extension("readme.md"));
        assert!(!is_binary_extension("Makefile"));
    }

    #[test]
    fn test_is_binary_extension_case_insensitive() {
        assert!(is_binary_extension("image.PNG"));
        assert!(is_binary_extension("image.Jpg"));
        assert!(is_binary_extension("image.JPEG"));
    }

    #[tokio::test]
    async fn test_extract_text_from_file_small() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line 1\nline 2\nline 3")
            .await
            .unwrap();

        let result = extract_text_from_file_with_metadata(&file_path)
            .await
            .unwrap();
        assert_eq!(result.total_lines, 3);
        assert_eq!(result.returned_lines, 3);
        assert!(!result.was_truncated);
        assert!(result.lines_shown.is_none());
        assert_eq!(result.content, "line 1\nline 2\nline 3");
    }

    #[tokio::test]
    async fn test_extract_text_from_file_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("big.txt");
        let content: Vec<String> = (0..600).map(|i| format!("line {}", i)).collect();
        fs::write(&file_path, content.join("\n"))
            .await
            .unwrap();

        let result = extract_text_from_file_with_metadata(&file_path)
            .await
            .unwrap();
        assert_eq!(result.total_lines, 600);
        assert_eq!(result.returned_lines, DEFAULT_LINE_LIMIT);
        assert!(result.was_truncated);
        assert_eq!(result.lines_shown, Some((1, DEFAULT_LINE_LIMIT)));
    }

    #[tokio::test]
    async fn test_extract_text_from_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, "").await.unwrap();

        let result = extract_text_from_file_with_metadata(&file_path)
            .await
            .unwrap();
        assert_eq!(result.total_lines, 0);
        assert_eq!(result.returned_lines, 0);
        assert!(!result.was_truncated);
    }

    #[tokio::test]
    async fn test_extract_text_from_file_not_found() {
        let result = extract_text_from_file_with_metadata(Path::new("/nonexistent/file.txt")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_file_or_folder_content_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").await.unwrap();

        let result = get_file_or_folder_content("test.rs", dir.path())
            .await
            .unwrap();
        assert_eq!(result.block_type, MentionBlockType::File);
        assert_eq!(result.path.as_deref(), Some("test.rs"));
        assert!(result.content.contains("fn main() {}"));
        assert!(result.content.contains("[read_file for 'test.rs']"));
    }

    #[tokio::test]
    async fn test_get_file_or_folder_content_binary() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("image.png");
        fs::write(&file_path, b"\x89PNG\r\n").await.unwrap();

        let result = get_file_or_folder_content("image.png", dir.path())
            .await
            .unwrap();
        assert_eq!(result.block_type, MentionBlockType::File);
        assert!(result.content.contains("Binary file omitted"));
    }

    #[tokio::test]
    async fn test_get_file_or_folder_content_folder() {
        let dir = tempfile::tempdir().unwrap();
        // Create a subdirectory named "myfolder" inside the temp dir
        let myfolder = dir.path().join("myfolder");
        fs::create_dir(&myfolder).await.unwrap();
        let sub_file = myfolder.join("hello.rs");
        fs::write(&sub_file, "fn hello() {}").await.unwrap();
        let sub_dir = myfolder.join("subdir");
        fs::create_dir(&sub_dir).await.unwrap();

        let result = get_file_or_folder_content("myfolder/", dir.path())
            .await
            .unwrap();
        assert_eq!(result.block_type, MentionBlockType::Folder);
        assert!(result.content.contains("[read_file for folder 'myfolder/']"));
        assert!(result.content.contains("hello.rs"));
        assert!(result.content.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_get_file_or_folder_content_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = get_file_or_folder_content("nonexistent.rs", dir.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_file_or_folder_content_with_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("my file.rs");
        fs::write(&file_path, "fn main() {}").await.unwrap();

        let result = get_file_or_folder_content("my\\ file.rs", dir.path())
            .await
            .unwrap();
        assert_eq!(result.block_type, MentionBlockType::File);
        assert!(result.content.contains("fn main() {}"));
    }
}
