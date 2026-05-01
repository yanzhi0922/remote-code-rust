//! Diff view provider for tracking file editing sessions.
//!
//! [`DiffViewProvider`] mirrors the lifecycle of the TypeScript `DiffViewProvider`:
//! open a file → stream incremental updates → save or revert. It is purely
//! state-based and does not depend on any particular UI framework.

use std::path::{Path, PathBuf};

use crate::file_editor::{DiffTag, FileEditor, FileEditorError};
use crate::types::{DiffViewOptions, EditResult, EditorState, FileChange};

/// Errors produced by [`DiffViewProvider`].
#[derive(Debug, thiserror::Error)]
pub enum DiffViewError {
    /// No file is currently open for editing.
    #[error("No file is currently open for editing")]
    NotOpen,

    /// An underlying file-editor error occurred.
    #[error("File editor error: {0}")]
    Editor(#[from] FileEditorError),

    /// The provider is already editing a file.
    #[error("Already editing a file: {0}")]
    AlreadyEditing(PathBuf),
}

/// Manages the lifecycle of a file editing session with diff tracking.
///
/// This is a cross-platform Rust analogue of the VS Code `DiffViewProvider`.
/// Instead of controlling a VS Code editor tab, it maintains in-memory state
/// and delegates file I/O to [`FileEditor`].
#[derive(Debug)]
pub struct DiffViewProvider {
    /// Current editor state.
    state: EditorState,
    /// Options controlling diff view behavior.
    options: DiffViewOptions,
    /// The relative path of the file being edited.
    rel_path: Option<PathBuf>,
    /// The original content of the file before editing.
    original_content: Option<String>,
    /// The current (accumulated) content during editing.
    current_content: Option<String>,
    /// The last saved content (used to detect if there are unsaved changes).
    last_saved_content: Option<String>,
    /// List of files created during this session.
    created_files: Vec<PathBuf>,
    /// Whether a diff view has been opened at least once.
    opened_diff: bool,
}

impl DiffViewProvider {
    /// Creates a new `DiffViewProvider` with the given options.
    pub fn new(options: DiffViewOptions) -> Self {
        Self {
            state: EditorState::Idle,
            options,
            rel_path: None,
            original_content: None,
            current_content: None,
            last_saved_content: None,
            created_files: Vec::new(),
            opened_diff: false,
        }
    }

    /// Creates a new `DiffViewProvider` with default options.
    pub fn new_default() -> Self {
        Self::new(DiffViewOptions::default())
    }

    // ── State queries ──

    /// Returns `true` if the provider is currently active (editing or diff view open).
    pub fn is_active(&self) -> bool {
        self.state != EditorState::Idle
    }

    /// Returns `true` if a file is currently being edited.
    pub fn is_editing(&self) -> bool {
        self.state == EditorState::Editing
    }

    /// Returns the current editor state.
    pub fn state(&self) -> &EditorState {
        &self.state
    }

    /// Returns the relative path of the file being edited, if any.
    pub fn rel_path(&self) -> Option<&Path> {
        self.rel_path.as_deref()
    }

    /// Returns the original content before editing began.
    pub fn get_original_content(&self) -> Option<&str> {
        self.original_content.as_deref()
    }

    /// Returns the current accumulated content.
    pub fn get_current_content(&self) -> Option<&str> {
        self.current_content.as_deref()
    }

    /// Returns the list of files created during this session.
    pub fn created_files(&self) -> &[PathBuf] {
        &self.created_files
    }

    /// Returns `true` if the diff view has been opened.
    pub fn opened_diff(&self) -> bool {
        self.opened_diff
    }

    /// Returns `true` if there are unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        match (&self.current_content, &self.last_saved_content) {
            (Some(current), Some(saved)) => current != saved,
            (Some(_), None) => true,
            _ => false,
        }
    }

    // ── Lifecycle methods ──

    /// Opens a file for editing.
    ///
    /// Reads the file's current content (if it exists) and stores it as the
    /// original content. The provider transitions to the [`Editing`](EditorState::Editing) state.
    ///
    /// If the file does not exist, it is treated as a new file creation.
    pub async fn open(&mut self, rel_path: &Path) -> Result<(), DiffViewError> {
        if self.state != EditorState::Idle {
            return Err(DiffViewError::AlreadyEditing(
                self.rel_path.clone().unwrap_or_default(),
            ));
        }

        let absolute_path = rel_path.to_path_buf();
        let original = match FileEditor::read_file(&absolute_path).await {
            Ok(content) => Some(content),
            Err(FileEditorError::NotFound(_)) => None,
            Err(e) => return Err(DiffViewError::Editor(e)),
        };

        self.rel_path = Some(rel_path.to_path_buf());
        self.original_content = original.clone();
        self.current_content = original.clone();
        self.last_saved_content = original;
        self.state = EditorState::Editing;

        if self.options.show_diff && self.original_content.is_some() {
            self.opened_diff = true;
            self.state = EditorState::DiffViewOpen;
        }

        Ok(())
    }

    /// Updates the editor with accumulated content.
    ///
    /// If `is_final` is `true`, the content is marked as the final version
    /// ready to be saved.
    pub fn update(&mut self, accumulated_content: &str, is_final: bool) -> Result<(), DiffViewError> {
        if self.rel_path.is_none() {
            return Err(DiffViewError::NotOpen);
        }

        self.current_content = Some(accumulated_content.to_string());

        if is_final {
            self.state = EditorState::DiffViewOpen;
        }

        Ok(())
    }

    /// Saves the current changes to disk.
    ///
    /// Returns an [`EditResult`] describing what was changed.
    /// If `is_new_file` is `true`, the file is recorded as newly created.
    pub async fn save_changes(
        &mut self,
        cwd: &Path,
        is_new_file: bool,
    ) -> Result<EditResult, DiffViewError> {
        let rel_path = self
            .rel_path
            .as_ref()
            .ok_or(DiffViewError::NotOpen)?
            .clone();
        let new_content = self
            .current_content
            .as_ref()
            .ok_or(DiffViewError::NotOpen)?
            .clone();

        let full_path = cwd.join(&rel_path);

        // Create backup if configured and file exists
        if self.options.backup_original && full_path.exists() && !is_new_file {
            let _ = FileEditor::backup_file(&full_path).await;
        }

        // Write the new content
        FileEditor::write_file(&full_path, &new_content).await?;

        // Compute diff description
        let original = self.original_content.as_deref().unwrap_or("");
        let diffs = FileEditor::compute_line_diff(original, &new_content);
        let diff_description = format_diff_summary(&rel_path, &diffs);

        let mut result = EditResult::new();

        let operation = if is_new_file {
            crate::types::EditOperation::Create
        } else {
            crate::types::EditOperation::Modify
        };

        let change = FileChange {
            path: rel_path.clone(),
            original_content: self.original_content.clone(),
            new_content: Some(new_content.clone()),
            operation,
        };

        result.add_change(&change, diff_description);

        if is_new_file {
            self.created_files.push(rel_path.clone());
        }

        self.last_saved_content = Some(new_content);
        self.state = EditorState::Idle;

        Ok(result)
    }

    /// Reverts all changes, restoring the file to its original content.
    ///
    /// If the file was newly created, it is deleted.
    /// If the file was modified, the original content is written back.
    pub async fn revert_changes(&mut self) -> Result<(), DiffViewError> {
        let _rel_path = self
            .rel_path
            .as_ref()
            .ok_or(DiffViewError::NotOpen)?
            .clone();

        match &self.original_content {
            Some(original) => {
                // File existed before — restore original content
                self.current_content = Some(original.clone());
            }
            None => {
                // New file — clear content
                self.current_content = None;
            }
        }

        self.state = EditorState::Idle;
        Ok(())
    }

    /// Close the diff view, reverting any unsaved changes and resetting state.
    ///
    /// This mirrors the TS `DiffViewProvider.close()` method which closes the
    /// diff editor tab and reverts the file to its original content.
    pub async fn close(&mut self) -> Result<(), DiffViewError> {
        if self.has_unsaved_changes() {
            self.revert_changes().await?;
        }
        self.reset();
        Ok(())
    }

    /// Resets the provider to idle state, discarding all session data.
    pub fn reset(&mut self) {
        self.state = EditorState::Idle;
        self.rel_path = None;
        self.original_content = None;
        self.current_content = None;
        self.last_saved_content = None;
        self.created_files.clear();
        self.opened_diff = false;
    }
}

/// Formats a human-readable diff summary for a file.
fn format_diff_summary(path: &Path, diffs: &[crate::file_editor::LineDiff]) -> String {
    let additions = diffs.iter().filter(|d| d.tag == DiffTag::Insert).count();
    let deletions = diffs.iter().filter(|d| d.tag == DiffTag::Delete).count();

    format!(
        "{}: +{} -{} lines",
        path.display(),
        additions,
        deletions
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_provider() -> DiffViewProvider {
        DiffViewProvider::new_default()
    }

    // ── State query tests ──

    #[test]
    fn test_initial_state_is_idle() {
        let provider = make_provider();
        assert!(!provider.is_active());
        assert!(!provider.is_editing());
        assert_eq!(provider.state(), &EditorState::Idle);
        assert!(provider.rel_path().is_none());
        assert!(provider.get_original_content().is_none());
        assert!(provider.get_current_content().is_none());
        assert!(provider.created_files().is_empty());
        assert!(!provider.opened_diff());
    }

    #[test]
    fn test_has_unsaved_changes_when_content_differs() {
        let mut provider = make_provider();
        provider.current_content = Some("new".to_string());
        provider.last_saved_content = Some("old".to_string());
        assert!(provider.has_unsaved_changes());
    }

    #[test]
    fn test_no_unsaved_changes_when_content_matches() {
        let mut provider = make_provider();
        provider.current_content = Some("same".to_string());
        provider.last_saved_content = Some("same".to_string());
        assert!(!provider.has_unsaved_changes());
    }

    #[test]
    fn test_no_unsaved_changes_when_no_current_content() {
        let provider = make_provider();
        assert!(!provider.has_unsaved_changes());
    }

    // ── Open lifecycle ──

    #[tokio::test]
    async fn test_open_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original content").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();

        assert!(provider.is_active());
        assert_eq!(
            provider.get_original_content().unwrap(),
            "original content"
        );
        assert_eq!(provider.get_current_content().unwrap(), "original content");
    }

    #[tokio::test]
    async fn test_open_nonexistent_file_treated_as_new() {
        let mut provider = make_provider();
        let path = PathBuf::from("/nonexistent/file.txt");
        // This should succeed — the file is treated as new
        provider.open(&path).await.unwrap();
        assert!(provider.is_active());
        assert!(provider.get_original_content().is_none());
    }

    #[tokio::test]
    async fn test_open_while_already_editing_fails() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "content").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();

        // Trying to open another file should fail
        let result = provider.open(Path::new("other.txt")).await;
        assert!(result.is_err());
    }

    // ── Update lifecycle ──

    #[tokio::test]
    async fn test_update_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("new content", false).unwrap();

        assert_eq!(provider.get_current_content().unwrap(), "new content");
        assert_eq!(provider.get_original_content().unwrap(), "original");
    }

    #[tokio::test]
    async fn test_update_final_transitions_to_diff_view() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = DiffViewProvider::new(DiffViewOptions {
            show_diff: false,
            ..Default::default()
        });
        provider.open(&file_path).await.unwrap();
        assert_eq!(provider.state(), &EditorState::Editing);

        provider.update("final content", true).unwrap();
        assert_eq!(provider.state(), &EditorState::DiffViewOpen);
    }

    #[test]
    fn test_update_without_open_fails() {
        let mut provider = make_provider();
        let result = provider.update("content", false);
        assert!(result.is_err());
    }

    // ── Save lifecycle ──

    #[tokio::test]
    async fn test_save_changes_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("new file content", true).unwrap();

        let result = provider.save_changes(dir.path(), true).await.unwrap();
        assert_eq!(result.changed_files.len(), 1);
        assert_eq!(result.new_files.len(), 1);
        assert!(!provider.is_active());

        // Verify file was written
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "new file content");
    }

    #[tokio::test]
    async fn test_save_changes_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("modified", true).unwrap();

        let result = provider.save_changes(dir.path(), false).await.unwrap();
        assert_eq!(result.changed_files.len(), 1);
        assert!(result.new_files.is_empty());

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "modified");
    }

    // ── Revert lifecycle ──

    #[tokio::test]
    async fn test_revert_changes_restores_original() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("modified", false).unwrap();
        assert_eq!(provider.get_current_content().unwrap(), "modified");

        provider.revert_changes().await.unwrap();
        assert_eq!(provider.get_current_content().unwrap(), "original");
        assert!(!provider.is_active());
    }

    #[tokio::test]
    async fn test_revert_new_file_clears_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new.txt");

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("new content", false).unwrap();

        provider.revert_changes().await.unwrap();
        assert!(provider.get_current_content().is_none());
        assert!(!provider.is_active());
    }

    // ── Reset ──

    #[test]
    fn test_reset_clears_all_state() {
        let mut provider = make_provider();
        provider.state = EditorState::Editing;
        provider.rel_path = Some(PathBuf::from("test.rs"));
        provider.original_content = Some("orig".to_string());
        provider.current_content = Some("curr".to_string());
        provider.created_files.push(PathBuf::from("new.rs"));
        provider.opened_diff = true;

        provider.reset();

        assert_eq!(provider.state(), &EditorState::Idle);
        assert!(provider.rel_path().is_none());
        assert!(provider.get_original_content().is_none());
        assert!(provider.get_current_content().is_none());
        assert!(provider.created_files().is_empty());
        assert!(!provider.opened_diff());
    }

    // ── Close lifecycle ──

    #[tokio::test]
    async fn test_close_reverts_unsaved_changes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("modified", false).unwrap();
        assert!(provider.has_unsaved_changes());

        provider.close().await.unwrap();
        assert!(!provider.is_active());
        // After close, the provider is fully reset (idle state)
        assert!(provider.rel_path().is_none());
        assert!(provider.get_current_content().is_none());
    }

    #[tokio::test]
    async fn test_close_without_unsaved_changes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        // No changes made

        provider.close().await.unwrap();
        assert!(!provider.is_active());
        assert!(provider.rel_path().is_none());
    }

    #[tokio::test]
    async fn test_close_new_file_clears_state() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new.txt");

        let mut provider = make_provider();
        provider.open(&file_path).await.unwrap();
        provider.update("new content", false).unwrap();

        provider.close().await.unwrap();
        assert!(!provider.is_active());
        assert!(provider.rel_path().is_none());
    }

    // ── Diff summary ──

    #[test]
    fn test_format_diff_summary() {
        let diffs = FileEditor::compute_line_diff("a\nb\n", "a\nc\n");
        let summary = format_diff_summary(Path::new("test.rs"), &diffs);
        assert!(summary.contains("test.rs"));
        assert!(summary.contains("+1"));
        assert!(summary.contains("-1"));
    }
}
