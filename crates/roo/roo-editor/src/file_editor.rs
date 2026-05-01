//! File system operations for the editor.
//!
//! [`FileEditor`] provides async CRUD operations on files, plus backup/restore
//! and line-diff computation via the `similar` crate.

use std::path::{Path, PathBuf};

use similar::TextDiff;

use crate::types::EditOperation;

/// Errors that can occur during file editing operations.
#[derive(Debug, thiserror::Error)]
pub enum FileEditorError {
    /// The file was not found.
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The file path is invalid (e.g., empty or contains invalid characters).
    #[error("Invalid file path: {0}")]
    InvalidPath(String),
}

/// A single line-level diff entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDiff {
    /// The tag indicating the type of change.
    pub tag: DiffTag,
    /// The line number in the old content (1-based, None for inserts).
    pub old_line: Option<usize>,
    /// The line number in the new content (1-based, None for deletes).
    pub new_line: Option<usize>,
    /// The content of the line (without newline).
    pub content: String,
}

/// The type of change in a line diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTag {
    /// The line is present only in the old content (removed).
    Delete,
    /// The line is present only in the new content (added).
    Insert,
    /// The line is present in both (unchanged).
    Equal,
}

/// Performs file system operations for the editor layer.
///
/// All async methods accept an absolute or relative `Path` and use
/// [`tokio::fs`] for non-blocking I/O.
pub struct FileEditor;

impl FileEditor {
    /// Reads the entire content of a file as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns [`FileEditorError::NotFound`] if the file does not exist,
    /// or [`FileEditorError::Io`] for other I/O failures.
    pub async fn read_file(path: &Path) -> Result<String, FileEditorError> {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(FileEditorError::NotFound(path.to_path_buf()))
            }
            Err(e) => Err(FileEditorError::Io(e)),
        }
    }

    /// Writes content to a file, creating it if it doesn't exist.
    /// Parent directories are created automatically.
    pub async fn write_file(path: &Path, content: &str) -> Result<(), FileEditorError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Creates a new file with the given content.
    /// Returns an error if the file already exists.
    pub async fn create_file(path: &Path, content: &str) -> Result<(), FileEditorError> {
        if path.exists() {
            return Err(FileEditorError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("File already exists: {}", path.display()),
            )));
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Deletes a file from the filesystem.
    pub async fn delete_file(path: &Path) -> Result<(), FileEditorError> {
        tokio::fs::remove_file(path)
            .await
            .map_err(FileEditorError::Io)
    }

    /// Creates a backup of the file at `{path}.bak`.
    /// Returns the path to the backup file.
    pub async fn backup_file(path: &Path) -> Result<PathBuf, FileEditorError> {
        let backup_path = Self::backup_path(path);
        let content = Self::read_file(path).await?;
        Self::write_file(&backup_path, &content).await?;
        Ok(backup_path)
    }

    /// Restores a file from its backup at `{path}.bak`.
    /// Returns an error if no backup exists.
    pub async fn restore_backup(path: &Path) -> Result<(), FileEditorError> {
        let backup_path = Self::backup_path(path);
        let content = Self::read_file(&backup_path).await?;
        Self::write_file(path, &content).await?;
        // Clean up the backup file
        let _ = tokio::fs::remove_file(&backup_path).await;
        Ok(())
    }

    /// Computes a line-by-line unified diff between two strings.
    ///
    /// Returns a vector of [`LineDiff`] entries describing the changes.
    /// The output follows the convention of unified diff tools:
    /// `-` lines are deletions, `+` lines are insertions, and context
    /// lines are shown as equal.
    pub fn compute_line_diff(original: &str, new: &str) -> Vec<LineDiff> {
        let diff = TextDiff::from_lines(original, new);
        let mut result = Vec::new();
        let mut old_line = 1usize;
        let mut new_line = 1usize;

        for change in diff.iter_all_changes() {
            let tag = match change.tag() {
                similar::ChangeTag::Delete => DiffTag::Delete,
                similar::ChangeTag::Insert => DiffTag::Insert,
                similar::ChangeTag::Equal => DiffTag::Equal,
            };

            let content = change.to_string();
            // Trim trailing newline that similar includes
            let content = content.trim_end_matches('\n').trim_end_matches('\r');

            // Split on newlines in case a single change spans multiple lines
            for line in content.split('\n') {
                let (ol, nl) = match tag {
                    DiffTag::Delete => (Some(old_line), None),
                    DiffTag::Insert => (None, Some(new_line)),
                    DiffTag::Equal => (Some(old_line), Some(new_line)),
                };

                result.push(LineDiff {
                    tag,
                    old_line: ol,
                    new_line: nl,
                    content: line.to_string(),
                });

                match tag {
                    DiffTag::Delete => old_line += 1,
                    DiffTag::Insert => new_line += 1,
                    DiffTag::Equal => {
                        old_line += 1;
                        new_line += 1;
                    }
                }
            }
        }

        result
    }

    /// Formats a line diff as a unified diff string.
    pub fn format_line_diff(diffs: &[LineDiff]) -> String {
        let mut output = String::new();
        for diff in diffs {
            let prefix = match diff.tag {
                DiffTag::Delete => '-',
                DiffTag::Insert => '+',
                DiffTag::Equal => ' ',
            };
            output.push_str(&format!("{}{}\n", prefix, diff.content));
        }
        output
    }

    /// Determines the edit operation type based on file existence and content changes.
    pub fn determine_operation(
        file_exists: bool,
        original_content: Option<&str>,
        new_content: Option<&str>,
    ) -> EditOperation {
        match (file_exists, original_content, new_content) {
            (false, _, Some(_)) => EditOperation::Create,
            (true, _, None) => EditOperation::Delete,
            _ => EditOperation::Modify,
        }
    }

    /// Returns the backup path for a given file path.
    fn backup_path(path: &Path) -> PathBuf {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".bak");
        PathBuf::from(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── compute_line_diff tests ──

    #[test]
    fn test_diff_identical_content() {
        let diffs = FileEditor::compute_line_diff("hello\nworld", "hello\nworld");
        assert!(diffs.iter().all(|d| d.tag == DiffTag::Equal));
        assert_eq!(diffs.len(), 2);
    }

    #[test]
    fn test_diff_add_lines() {
        let diffs = FileEditor::compute_line_diff("line1\n", "line1\nline2\nline3\n");
        let inserts: Vec<_> = diffs.iter().filter(|d| d.tag == DiffTag::Insert).collect();
        assert_eq!(inserts.len(), 2);
    }

    #[test]
    fn test_diff_remove_lines() {
        let diffs = FileEditor::compute_line_diff("a\nb\nc\n", "a\n");
        let deletes: Vec<_> = diffs.iter().filter(|d| d.tag == DiffTag::Delete).collect();
        assert_eq!(deletes.len(), 2);
    }

    #[test]
    fn test_diff_empty_to_content() {
        let diffs = FileEditor::compute_line_diff("", "new line\n");
        assert!(diffs.iter().all(|d| d.tag == DiffTag::Insert));
    }

    #[test]
    fn test_diff_content_to_empty() {
        let diffs = FileEditor::compute_line_diff("old line\n", "");
        assert!(diffs.iter().all(|d| d.tag == DiffTag::Delete));
    }

    #[test]
    fn test_diff_both_empty() {
        let diffs = FileEditor::compute_line_diff("", "");
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_format_line_diff() {
        let diffs = FileEditor::compute_line_diff("a\n", "b\n");
        let formatted = FileEditor::format_line_diff(&diffs);
        assert!(formatted.contains("-a"));
        assert!(formatted.contains("+b"));
    }

    #[test]
    fn test_determine_operation_create() {
        let op = FileEditor::determine_operation(false, None, Some("content"));
        assert_eq!(op, EditOperation::Create);
    }

    #[test]
    fn test_determine_operation_delete() {
        let op = FileEditor::determine_operation(true, Some("content"), None);
        assert_eq!(op, EditOperation::Delete);
    }

    #[test]
    fn test_determine_operation_modify() {
        let op = FileEditor::determine_operation(true, Some("old"), Some("new"));
        assert_eq!(op, EditOperation::Modify);
    }

    #[test]
    fn test_backup_path() {
        let path = Path::new("src/main.rs");
        let backup = FileEditor::backup_path(path);
        assert_eq!(backup, PathBuf::from("src/main.rs.bak"));
    }

    // ── async file operation tests ──

    #[tokio::test]
    async fn test_write_and_read_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        FileEditor::write_file(&file_path, "hello world").await.unwrap();
        let content = FileEditor::read_file(&file_path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let result = FileEditor::read_file(Path::new("/nonexistent/path/file.txt")).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            FileEditorError::NotFound(_) => {}
            e => panic!("Expected NotFound, got: {}", e),
        }
    }

    #[tokio::test]
    async fn test_create_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");
        FileEditor::create_file(&file_path, "new content").await.unwrap();
        let content = FileEditor::read_file(&file_path).await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_create_existing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        FileEditor::write_file(&file_path, "existing").await.unwrap();
        let result = FileEditor::create_file(&file_path, "new").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("to_delete.txt");
        FileEditor::write_file(&file_path, "content").await.unwrap();
        FileEditor::delete_file(&file_path).await.unwrap();
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_backup_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("backup_test.txt");
        FileEditor::write_file(&file_path, "original content").await.unwrap();

        // Backup
        let backup_path = FileEditor::backup_file(&file_path).await.unwrap();
        assert!(backup_path.exists());

        // Modify the file
        FileEditor::write_file(&file_path, "modified content").await.unwrap();
        let modified = FileEditor::read_file(&file_path).await.unwrap();
        assert_eq!(modified, "modified content");

        // Restore from backup
        FileEditor::restore_backup(&file_path).await.unwrap();
        let restored = FileEditor::read_file(&file_path).await.unwrap();
        assert_eq!(restored, "original content");

        // Backup file should be cleaned up
        assert!(!backup_path.exists());
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("nested/deep/dir/file.txt");
        FileEditor::write_file(&file_path, "deep content").await.unwrap();
        let content = FileEditor::read_file(&file_path).await.unwrap();
        assert_eq!(content, "deep content");
    }
}
