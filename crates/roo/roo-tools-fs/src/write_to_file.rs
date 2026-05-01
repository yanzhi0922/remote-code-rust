//! write_to_file tool implementation.
//!
//! Handles content cleaning (markdown fence stripping), directory creation,
//! detecting whether a file is new or being modified, and creating backups
//! of existing files before overwriting.

use crate::helpers::*;
use crate::types::*;
use roo_ignore::RooIgnoreController;
use roo_types::tool::WriteToFileParams;

/// Validate write_to_file parameters.
pub fn validate_write_to_file_params(params: &WriteToFileParams) -> Result<(), FsToolError> {
    if params.path.trim().is_empty() {
        return Err(FsToolError::Validation("path must not be empty".to_string()));
    }

    if params.path.contains("..") {
        return Err(FsToolError::InvalidPath(
            "path must not contain '..'".to_string(),
        ));
    }

    // NOTE: TS source checks `newContent === undefined` (not provided),
    // but allows empty string content for creating empty files.
    // Since we have a String field, it's always present — no validation needed.

    Ok(())
}

/// Clean content for writing:
/// 1. Strip markdown fences if present
/// 2. Unescape HTML entities (e.g. `<` → `<`) — skipped for Claude models
/// 3. Strip line numbers if every line has them
///
/// When `model_id` is `Some` and contains `"claude"`, HTML entity unescaping
/// is skipped because Claude models don't produce HTML entities in their output.
/// This matches the TS source: `if (!task.api.getModel().id.includes("claude"))`.
pub fn clean_write_content(content: &str, model_id: Option<&str>) -> String {
    let mut cleaned = strip_markdown_fences(content);

    // Unescape HTML entities — but NOT for Claude models
    // TS: if (!task.api.getModel().id.includes("claude")) { unescapeHtmlEntities(...) }
    let is_claude = model_id.map_or(false, |id| id.contains("claude"));
    if !is_claude {
        cleaned = unescape_html_entities(&cleaned);
    }

    // Strip line numbers if every non-empty line has them
    if every_line_has_line_numbers(&cleaned) {
        cleaned = strip_line_numbers(&cleaned);
    }

    cleaned
}

/// Process a write_to_file operation.
///
/// If the file already exists, creates a `.bak` backup before overwriting.
///
/// `model_id` is used for model-dependent content cleaning (e.g. skipping
/// HTML entity unescaping for Claude models). Pass `None` to apply all
/// cleaning steps unconditionally.
pub fn process_write_to_file(
    params: &WriteToFileParams,
    cwd: &std::path::Path,
    ignore_controller: Option<&RooIgnoreController>,
    model_id: Option<&str>,
) -> Result<WriteResult, FsToolError> {
    validate_write_to_file_params(params)?;

    // Check .rooignore before any file I/O
    check_roo_ignore(&params.path, ignore_controller)?;

    let file_path = resolve_path(&params.path, cwd)?;
    let is_new_file = !file_path.exists();

    // Create parent directories if needed
    create_directories_for_file(&file_path)?;

    // Backup existing file before overwriting (L5.3)
    if !is_new_file {
        if let Err(e) = create_backup(&file_path) {
            // Log warning but don't fail the write
            eprintln!(
                "Warning: failed to create backup for {}: {}",
                file_path.display(),
                e
            );
        }
    }

    // Clean content (model-dependent HTML entity unescaping)
    let cleaned_content = clean_write_content(&params.content, model_id);

    // Count lines
    let lines_written = cleaned_content.lines().count();

    // Write the file
    std::fs::write(&file_path, &cleaned_content)?;

    Ok(WriteResult {
        path: params.path.clone(),
        is_new_file,
        lines_written,
    })
}

/// Create a `.bak` backup of an existing file.
///
/// The backup is placed alongside the original file with a `.bak` extension.
/// For example, `src/main.rs` → `src/main.rs.bak`.
///
/// Returns `Ok(())` if the backup was created, or an error if it failed.
pub fn create_backup(file_path: &std::path::Path) -> Result<(), std::io::Error> {
    if !file_path.exists() {
        return Ok(());
    }

    let backup_path = {
        let mut p = file_path.as_os_str().to_owned();
        p.push(".bak");
        std::path::PathBuf::from(p)
    };

    std::fs::copy(file_path, &backup_path)?;
    Ok(())
}

/// Resolve a relative path against the current working directory.
fn resolve_path(path: &str, cwd: &std::path::Path) -> Result<std::path::PathBuf, FsToolError> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        Ok(cwd.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_path() {
        let params = WriteToFileParams {
            path: "".to_string(),
            content: "hello".to_string(),
        };
        assert!(validate_write_to_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let params = WriteToFileParams {
            path: "../etc/passwd".to_string(),
            content: "hello".to_string(),
        };
        assert!(validate_write_to_file_params(&params).is_err());
    }

    #[test]
    fn test_validate_empty_content_allowed() {
        // TS allows empty content for creating empty files
        let params = WriteToFileParams {
            path: "test.txt".to_string(),
            content: "".to_string(),
        };
        assert!(validate_write_to_file_params(&params).is_ok());
    }

    #[test]
    fn test_validate_valid() {
        let params = WriteToFileParams {
            path: "test.txt".to_string(),
            content: "hello world".to_string(),
        };
        assert!(validate_write_to_file_params(&params).is_ok());
    }

    #[test]
    fn test_clean_content_no_fence() {
        let content = "fn main() { println!(\"hello\"); }";
        assert_eq!(clean_write_content(content, None), content);
    }

    #[test]
    fn test_clean_content_with_fence() {
        let content = "```rust\nfn main() {}\n```";
        assert_eq!(clean_write_content(content, None), "fn main() {}\n");
    }

    #[test]
    fn test_process_write_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "hello\nworld".to_string(),
        };
        let result = process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();
        assert!(result.is_new_file);
        assert_eq!(result.lines_written, 2);

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "hello\nworld");
    }

    #[test]
    fn test_process_write_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "old content").unwrap();

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "new content".to_string(),
        };
        let result = process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();
        assert!(!result.is_new_file);

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "new content");
    }

    #[test]
    fn test_process_write_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a").join("b").join("c").join("deep.txt");

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "deep content".to_string(),
        };
        let result = process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();
        assert!(result.is_new_file);
        assert!(file_path.exists());
    }

    #[test]
    fn test_process_write_with_fence() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("fenced.txt");

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "```javascript\nconsole.log(\"hello\");\n```".to_string(),
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "console.log(\"hello\");\n");
    }

    // --- L5.3: Backup tests ---

    #[test]
    fn test_backup_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "original content").unwrap();

        create_backup(&file_path).unwrap();

        let backup_path = dir.path().join("test.txt.bak");
        assert!(backup_path.exists());
        assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), "original content");
    }

    #[test]
    fn test_backup_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.txt");

        // Should succeed without error (no-op)
        create_backup(&file_path).unwrap();
    }

    #[test]
    fn test_write_creates_backup_for_existing() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "old content").unwrap();

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "new content".to_string(),
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        // Original should have new content
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "new content");

        // Backup should have old content
        let backup_path = dir.path().join("existing.txt.bak");
        assert!(backup_path.exists());
        assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), "old content");
    }

    #[test]
    fn test_write_no_backup_for_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "fresh content".to_string(),
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        // No backup should exist
        let backup_path = dir.path().join("new_file.txt.bak");
        assert!(!backup_path.exists());
    }

    // --- HTML entity unescaping tests ---

    #[test]
    fn test_clean_content_unescapes_html_entities() {
        // Build HTML entity string at runtime to avoid tool unescaping
        let content = format!("if (x {}lt; 10 {}amp;{}amp; y {}gt; 5)", '&', '&', '&', '&');
        assert_eq!(
            clean_write_content(&content, None),
            "if (x < 10 && y > 5)"
        );
    }

    #[test]
    fn test_clean_content_unescapes_quot() {
        let content = format!("msg = {}quot;hello{}quot;", '&', '&');
        let expected = "msg = \"hello\"";
        assert_eq!(clean_write_content(&content, None), expected);
    }

    #[test]
    fn test_clean_content_no_entities() {
        let content = "plain text without entities";
        assert_eq!(clean_write_content(content, None), content);
    }

    #[test]
    fn test_write_unescapes_html_entities() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Build HTML entity string at runtime to avoid tool unescaping
        let content = format!("if (x {}lt; 10 {}amp;{}amp; y {}gt; 5)", '&', '&', '&', '&');
        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content,
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "if (x < 10 && y > 5)");
    }

    // --- Line number stripping tests ---

    #[test]
    fn test_clean_content_strips_line_numbers() {
        let content = "  1 | fn main() {\n  2 |     println!(\"hello\");\n  3 | }\n";
        let expected = "fn main() {\n    println!(\"hello\");\n}\n";
        assert_eq!(clean_write_content(content, None), expected);
    }

    #[test]
    fn test_clean_content_no_line_numbers() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        assert_eq!(clean_write_content(content, None), content);
    }

    #[test]
    fn test_write_strips_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");

        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "  1 | fn main() {\n  2 |     println!(\"hello\");\n  3 | }\n".to_string(),
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "fn main() {\n    println!(\"hello\");\n}\n");
    }

    #[test]
    fn test_write_combined_fence_entities_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Content with markdown fence, HTML entities, AND line numbers
        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: "```\n  1 | x < 10\n  2 | y > 5\n```".to_string(),
        };
        process_write_to_file(&params, std::path::Path::new("."), None, None).unwrap();

        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "x < 10\ny > 5\n");
    }

    // --- Model-dependent HTML entity unescaping tests ---

    #[test]
    fn test_claude_model_skips_html_unescaping() {
        // TS: if (!task.api.getModel().id.includes("claude")) { unescapeHtmlEntities(...) }
        let content = format!("if (x {}lt; 10)", '&');
        // With Claude model ID → should NOT unescape
        assert_eq!(
            clean_write_content(&content, Some("claude-3.5-sonnet")),
            content
        );
        // With non-Claude model ID → should unescape
        assert_eq!(
            clean_write_content(&content, Some("gpt-4o")),
            "if (x < 10)"
        );
        // With no model ID → should unescape (default behavior)
        assert_eq!(
            clean_write_content(&content, None),
            "if (x < 10)"
        );
    }

    #[test]
    fn test_write_claude_model_preserves_entities() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        let entity_content = format!("if (x {}lt; 10)", '&');
        let params = WriteToFileParams {
            path: file_path.to_str().unwrap().to_string(),
            content: entity_content.clone(),
        };
        // With Claude model → entities preserved
        process_write_to_file(&params, std::path::Path::new("."), None, Some("claude-3.5-sonnet")).unwrap();
        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, entity_content);
    }
}
