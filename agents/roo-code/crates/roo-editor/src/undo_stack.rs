//! Undo/redo stack for editor operations.
//!
//! Provides an [`UndoStack`] that tracks edit operations and supports
//! undoing and redoing them in LIFO order.

use crate::types::{EditOperation, FileChange};

/// A snapshot captured before an edit operation, used for undo.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// The file change that was applied.
    pub change: FileChange,
}

/// Stack-based undo/redo manager for file edit operations.
#[derive(Debug, Clone)]
pub struct UndoStack {
    /// Operations that can be undone (most recent at the top).
    undo_stack: Vec<UndoEntry>,
    /// Operations that can be redone (most recent at the top).
    redo_stack: Vec<UndoEntry>,
}

impl UndoStack {
    /// Creates a new, empty `UndoStack`.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Pushes a new edit operation onto the undo stack.
    /// This clears the redo stack, as is standard in undo/redo semantics.
    pub fn push(&mut self, change: FileChange) {
        self.redo_stack.clear();
        self.undo_stack.push(UndoEntry { change });
    }

    /// Undoes the most recent operation and returns it.
    /// The undone operation is moved to the redo stack.
    /// Returns `None` if there is nothing to undo.
    pub fn undo(&mut self) -> Option<FileChange> {
        let entry = self.undo_stack.pop()?;
        let change = entry.change;
        self.redo_stack.push(UndoEntry {
            change: reverse_change(&change),
        });
        Some(change)
    }

    /// Redoes the most recently undone operation and returns it.
    /// The redone operation is moved back to the undo stack.
    /// Returns `None` if there is nothing to redo.
    pub fn redo(&mut self) -> Option<FileChange> {
        let entry = self.redo_stack.pop()?;
        let change = entry.change;
        self.undo_stack.push(UndoEntry {
            change: reverse_change(&change),
        });
        Some(change)
    }

    /// Returns `true` if there are operations that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are operations that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the number of undo-able operations.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of redo-able operations.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clears both the undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Returns `true` if both stacks are empty.
    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty() && self.redo_stack.is_empty()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Reverses a file change so it can be used as the inverse operation.
///
/// - Create → Delete (the file that was created should be deleted)
/// - Delete → Create (the file that was deleted should be recreated)
/// - Modify → Modify (swap original and new content)
fn reverse_change(change: &FileChange) -> FileChange {
    match change.operation {
        EditOperation::Create => FileChange {
            path: change.path.clone(),
            original_content: change.new_content.clone(),
            new_content: None,
            operation: EditOperation::Delete,
        },
        EditOperation::Delete => FileChange {
            path: change.path.clone(),
            original_content: None,
            new_content: change.original_content.clone(),
            operation: EditOperation::Create,
        },
        EditOperation::Modify => FileChange {
            path: change.path.clone(),
            original_content: change.new_content.clone(),
            new_content: change.original_content.clone(),
            operation: EditOperation::Modify,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_create(path: &str, content: &str) -> FileChange {
        FileChange::new_create(PathBuf::from(path), content.to_string())
    }

    fn make_modify(path: &str, old: &str, new: &str) -> FileChange {
        FileChange::new_modify(PathBuf::from(path), old.to_string(), new.to_string())
    }

    fn make_delete(path: &str, content: &str) -> FileChange {
        FileChange::new_delete(PathBuf::from(path), content.to_string())
    }

    #[test]
    fn test_new_stack_is_empty() {
        let stack = UndoStack::new();
        assert!(stack.is_empty());
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_len(), 0);
        assert_eq!(stack.redo_len(), 0);
    }

    #[test]
    fn test_default_is_empty() {
        let stack = UndoStack::default();
        assert!(stack.is_empty());
    }

    #[test]
    fn test_push_enables_undo() {
        let mut stack = UndoStack::new();
        stack.push(make_create("a.rs", "hello"));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_len(), 1);
    }

    #[test]
    fn test_push_clears_redo() {
        let mut stack = UndoStack::new();
        stack.push(make_create("a.rs", "hello"));
        let _ = stack.undo();
        assert!(stack.can_redo());
        // Pushing a new operation clears redo
        stack.push(make_create("b.rs", "world"));
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_len(), 1);
    }

    #[test]
    fn test_undo_returns_original_change() {
        let mut stack = UndoStack::new();
        let change = make_create("a.rs", "hello");
        stack.push(change.clone());
        let undone = stack.undo().unwrap();
        assert_eq!(undone.path, change.path);
        assert_eq!(undone.operation, EditOperation::Create);
    }

    #[test]
    fn test_undo_none_when_empty() {
        let mut stack = UndoStack::new();
        assert!(stack.undo().is_none());
    }

    #[test]
    fn test_redo_none_when_empty() {
        let mut stack = UndoStack::new();
        assert!(stack.redo().is_none());
    }

    #[test]
    fn test_undo_then_redo_cycle() {
        let mut stack = UndoStack::new();
        stack.push(make_create("a.rs", "hello"));
        // Undo
        let undone = stack.undo().unwrap();
        assert_eq!(undone.operation, EditOperation::Create);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
        // Redo
        let redone = stack.redo().unwrap();
        assert_eq!(redone.path, PathBuf::from("a.rs"));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_multiple_pushes_undo_in_lifo_order() {
        let mut stack = UndoStack::new();
        stack.push(make_create("a.rs", "a"));
        stack.push(make_create("b.rs", "b"));
        stack.push(make_create("c.rs", "c"));
        assert_eq!(stack.undo_len(), 3);
        // Undo returns most recent first
        let u1 = stack.undo().unwrap();
        assert_eq!(u1.path, PathBuf::from("c.rs"));
        let u2 = stack.undo().unwrap();
        assert_eq!(u2.path, PathBuf::from("b.rs"));
        let u3 = stack.undo().unwrap();
        assert_eq!(u3.path, PathBuf::from("a.rs"));
        assert!(!stack.can_undo());
    }

    #[test]
    fn test_clear_empties_both_stacks() {
        let mut stack = UndoStack::new();
        stack.push(make_create("a.rs", "a"));
        stack.push(make_create("b.rs", "b"));
        let _ = stack.undo();
        assert!(stack.can_undo());
        assert!(stack.can_redo());
        stack.clear();
        assert!(stack.is_empty());
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_reverse_create_becomes_delete() {
        let change = make_create("a.rs", "content");
        let reversed = reverse_change(&change);
        assert_eq!(reversed.operation, EditOperation::Delete);
        assert_eq!(reversed.original_content.as_deref(), Some("content"));
        assert!(reversed.new_content.is_none());
    }

    #[test]
    fn test_reverse_delete_becomes_create() {
        let change = make_delete("a.rs", "content");
        let reversed = reverse_change(&change);
        assert_eq!(reversed.operation, EditOperation::Create);
        assert!(reversed.original_content.is_none());
        assert_eq!(reversed.new_content.as_deref(), Some("content"));
    }

    #[test]
    fn test_reverse_modify_swaps_content() {
        let change = make_modify("a.rs", "old", "new");
        let reversed = reverse_change(&change);
        assert_eq!(reversed.operation, EditOperation::Modify);
        assert_eq!(reversed.original_content.as_deref(), Some("new"));
        assert_eq!(reversed.new_content.as_deref(), Some("old"));
    }
}
