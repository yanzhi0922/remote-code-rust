//! Core types for the roo-editor crate.
//!
//! Defines the data structures used across the editor integration layer,
//! including edit operations, file changes, diff view options, and editor state.

use std::path::PathBuf;

/// The kind of edit operation being performed on a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOperation {
    /// Create a new file.
    Create,
    /// Modify an existing file.
    Modify,
    /// Delete an existing file.
    Delete,
}

/// Represents a single file change with before/after content.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// The file path (relative to workspace root).
    pub path: PathBuf,
    /// The original content before the edit (None for new files).
    pub original_content: Option<String>,
    /// The new content after the edit (None for deleted files).
    pub new_content: Option<String>,
    /// The type of edit operation.
    pub operation: EditOperation,
}

impl FileChange {
    /// Creates a new `FileChange` for file creation.
    pub fn new_create(path: PathBuf, content: String) -> Self {
        Self {
            path,
            original_content: None,
            new_content: Some(content),
            operation: EditOperation::Create,
        }
    }

    /// Creates a new `FileChange` for file modification.
    pub fn new_modify(path: PathBuf, original: String, new: String) -> Self {
        Self {
            path,
            original_content: Some(original),
            new_content: Some(new),
            operation: EditOperation::Modify,
        }
    }

    /// Creates a new `FileChange` for file deletion.
    pub fn new_delete(path: PathBuf, original: String) -> Self {
        Self {
            path,
            original_content: Some(original),
            new_content: None,
            operation: EditOperation::Delete,
        }
    }

    /// Returns true if this change represents a new file creation.
    pub fn is_new_file(&self) -> bool {
        self.operation == EditOperation::Create
    }

    /// Returns a human-readable description of the change.
    pub fn describe(&self) -> String {
        match self.operation {
            EditOperation::Create => format!("Created {}", self.path.display()),
            EditOperation::Modify => {
                let original_lines = self
                    .original_content
                    .as_ref()
                    .map(|c| c.lines().count())
                    .unwrap_or(0);
                let new_lines = self
                    .new_content
                    .as_ref()
                    .map(|c| c.lines().count())
                    .unwrap_or(0);
                format!(
                    "Modified {} ({} -> {} lines)",
                    self.path.display(),
                    original_lines,
                    new_lines
                )
            }
            EditOperation::Delete => format!("Deleted {}", self.path.display()),
        }
    }
}

/// Options controlling diff view behavior.
#[derive(Debug, Clone)]
pub struct DiffViewOptions {
    /// Whether to show the diff view when editing.
    pub show_diff: bool,
    /// Whether to automatically save changes.
    pub auto_save: bool,
    /// Whether to create a backup of the original file before editing.
    pub backup_original: bool,
}

impl Default for DiffViewOptions {
    fn default() -> Self {
        Self {
            show_diff: true,
            auto_save: false,
            backup_original: true,
        }
    }
}

/// The result of an editing session.
#[derive(Debug, Clone, Default)]
pub struct EditResult {
    /// List of files that were changed.
    pub changed_files: Vec<PathBuf>,
    /// Diff descriptions for each changed file.
    pub diffs: Vec<String>,
    /// List of newly created files.
    pub new_files: Vec<PathBuf>,
}

impl EditResult {
    /// Creates an empty `EditResult`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if no changes were made.
    pub fn is_empty(&self) -> bool {
        self.changed_files.is_empty()
    }

    /// Adds a file change to the result.
    pub fn add_change(&mut self, change: &FileChange, diff: String) {
        if change.is_new_file() {
            self.new_files.push(change.path.clone());
        }
        self.changed_files.push(change.path.clone());
        self.diffs.push(diff);
    }
}

/// The current state of the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorState {
    /// The editor is idle, no file is being edited.
    Idle,
    /// A file is currently being edited.
    Editing,
    /// A diff view is open for review.
    DiffViewOpen,
}

impl Default for EditorState {
    fn default() -> Self {
        Self::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_operation_equality() {
        assert_eq!(EditOperation::Create, EditOperation::Create);
        assert_eq!(EditOperation::Modify, EditOperation::Modify);
        assert_eq!(EditOperation::Delete, EditOperation::Delete);
        assert_ne!(EditOperation::Create, EditOperation::Modify);
    }

    #[test]
    fn test_file_change_new_create() {
        let change = FileChange::new_create(PathBuf::from("foo.rs"), "hello".to_string());
        assert!(change.is_new_file());
        assert_eq!(change.operation, EditOperation::Create);
        assert!(change.original_content.is_none());
        assert_eq!(change.new_content.as_deref(), Some("hello"));
    }

    #[test]
    fn test_file_change_new_modify() {
        let change = FileChange::new_modify(
            PathBuf::from("bar.rs"),
            "old".to_string(),
            "new".to_string(),
        );
        assert!(!change.is_new_file());
        assert_eq!(change.operation, EditOperation::Modify);
        assert_eq!(change.original_content.as_deref(), Some("old"));
        assert_eq!(change.new_content.as_deref(), Some("new"));
    }

    #[test]
    fn test_file_change_new_delete() {
        let change = FileChange::new_delete(PathBuf::from("baz.rs"), "content".to_string());
        assert!(!change.is_new_file());
        assert_eq!(change.operation, EditOperation::Delete);
        assert_eq!(change.original_content.as_deref(), Some("content"));
        assert!(change.new_content.is_none());
    }

    #[test]
    fn test_file_change_describe() {
        let create = FileChange::new_create(PathBuf::from("new.rs"), "content".to_string());
        assert!(create.describe().contains("Created"));

        let modify = FileChange::new_modify(
            PathBuf::from("mod.rs"),
            "line1\nline2".to_string(),
            "line1\nline2\nline3".to_string(),
        );
        assert!(modify.describe().contains("Modified"));
        assert!(modify.describe().contains("2 -> 3 lines"));

        let delete = FileChange::new_delete(PathBuf::from("old.rs"), "content".to_string());
        assert!(delete.describe().contains("Deleted"));
    }

    #[test]
    fn test_diff_view_options_default() {
        let opts = DiffViewOptions::default();
        assert!(opts.show_diff);
        assert!(!opts.auto_save);
        assert!(opts.backup_original);
    }

    #[test]
    fn test_edit_result_empty() {
        let result = EditResult::new();
        assert!(result.is_empty());
        assert!(result.changed_files.is_empty());
        assert!(result.diffs.is_empty());
        assert!(result.new_files.is_empty());
    }

    #[test]
    fn test_edit_result_add_change() {
        let mut result = EditResult::new();
        let change = FileChange::new_create(PathBuf::from("new.rs"), "hello".to_string());
        result.add_change(&change, "diff output".to_string());
        assert!(!result.is_empty());
        assert_eq!(result.changed_files.len(), 1);
        assert_eq!(result.new_files.len(), 1);
        assert_eq!(result.diffs.len(), 1);
    }

    #[test]
    fn test_editor_state_default() {
        assert_eq!(EditorState::default(), EditorState::Idle);
    }
}
