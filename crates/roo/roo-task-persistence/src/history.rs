//! Task history management.
//!
//! Provides listing, searching, and deletion of task history entries.

use std::path::Path;

use crate::metadata;
use crate::storage::{self, TaskFileSystem};
use crate::types::{HistoryItem, PersistenceTaskStatus, TaskMetadataOptions};
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// list_history
// ---------------------------------------------------------------------------

/// List all task history items from the global storage path.
///
/// Scans the `tasks/` subdirectory and computes metadata for each task.
/// Tasks that cannot be read are silently skipped.
pub fn list_history(
    fs: &dyn TaskFileSystem,
    global_storage_path: &Path,
) -> Result<Vec<HistoryItem>, TaskPersistenceError> {
    let tasks_dir = global_storage_path.join("tasks");
    let entries = fs.read_dir(&tasks_dir)?;

    let mut items = Vec::new();
    for entry in entries {
        if !entry.is_dir {
            continue;
        }

        let task_id = entry.file_name.clone();
        let opts = TaskMetadataOptions {
            task_id: task_id.clone(),
            root_task_id: None,
            parent_task_id: None,
            task_number: 0,
            messages: Vec::new(),
            global_storage_path: global_storage_path.to_path_buf(),
            workspace: String::new(),
            mode: None,
            api_config_name: None,
            initial_status: PersistenceTaskStatus::Active,
        };

        if let Ok(item) = metadata::compute_history_item(fs, &opts) {
            items.push(item);
        }
    }

    // Sort by timestamp descending (newest first)
    items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(items)
}

// ---------------------------------------------------------------------------
// search_history
// ---------------------------------------------------------------------------

/// Search task history by a query string.
///
/// Matches against task description (case-insensitive substring match).
pub fn search_history(
    fs: &dyn TaskFileSystem,
    global_storage_path: &Path,
    query: &str,
) -> Result<Vec<HistoryItem>, TaskPersistenceError> {
    let all = list_history(fs, global_storage_path)?;
    let query_lower = query.to_lowercase();

    let results: Vec<HistoryItem> = all
        .into_iter()
        .filter(|item| item.task.to_lowercase().contains(&query_lower))
        .collect();

    Ok(results)
}

// ---------------------------------------------------------------------------
// delete_task
// ---------------------------------------------------------------------------

/// Delete a task and all its associated data.
///
/// Removes the entire task directory from disk.
pub fn delete_task(
    fs: &dyn TaskFileSystem,
    global_storage_path: &Path,
    task_id: &str,
) -> Result<(), TaskPersistenceError> {
    let task_dir = storage::task_dir(global_storage_path, task_id);
    fs.remove_dir_all(&task_dir)
}

// ---------------------------------------------------------------------------
// get_history_item
// ---------------------------------------------------------------------------

/// Get a single history item by task ID.
///
/// Returns `None` if the task directory does not exist or cannot be read.
pub fn get_history_item(
    fs: &dyn TaskFileSystem,
    global_storage_path: &Path,
    task_id: &str,
) -> Result<Option<HistoryItem>, TaskPersistenceError> {
    let task_dir = storage::task_dir(global_storage_path, task_id);
    if !fs.file_exists(&task_dir)? {
        return Ok(None);
    }

    let opts = TaskMetadataOptions {
        task_id: task_id.to_string(),
        root_task_id: None,
        parent_task_id: None,
        task_number: 0,
        messages: Vec::new(),
        global_storage_path: global_storage_path.to_path_buf(),
        workspace: String::new(),
        mode: None,
        api_config_name: None,
        initial_status: PersistenceTaskStatus::Active,
    };

    match metadata::compute_history_item(fs, &opts) {
        Ok(item) => Ok(Some(item)),
        Err(TaskPersistenceError::Io(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages;
    use crate::storage::OsFileSystem;

    use roo_types::message::{ClineMessage, MessageType};

    /// Helper to create a task with messages on disk.
    fn create_test_task(
        fs: &OsFileSystem,
        base: &Path,
        task_id: &str,
        description: &str,
    ) {
        let task_dir = storage::task_dir(base, task_id);
        fs.create_dir_all(&task_dir).unwrap();

        let msg = ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some(description.to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        let msg_path = storage::messages_path(base, task_id);
        messages::save_task_messages(fs, &msg_path, &[msg]).unwrap();
    }

    #[test]
    fn test_list_history_empty() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        // No tasks directory yet
        let result = list_history(&fs, dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_history_with_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-1", "First task");
        create_test_task(&fs, dir.path(), "task-2", "Second task");

        let result = list_history(&fs, dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_list_history_sorted_by_timestamp_desc() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        // Create tasks with different timestamps
        let task_dir1 = storage::task_dir(dir.path(), "older-task");
        fs.create_dir_all(&task_dir1).unwrap();
        let msg1 = ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some("Older".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        messages::save_task_messages(&fs, &storage::messages_path(dir.path(), "older-task"), &[msg1]).unwrap();

        let task_dir2 = storage::task_dir(dir.path(), "newer-task");
        fs.create_dir_all(&task_dir2).unwrap();
        let msg2 = ClineMessage {
            ts: 1800000000.0,
            r#type: MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some("Newer".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        messages::save_task_messages(&fs, &storage::messages_path(dir.path(), "newer-task"), &[msg2]).unwrap();

        let result = list_history(&fs, dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].task, "Newer");
        assert_eq!(result[1].task, "Older");
    }

    #[test]
    fn test_search_history_found() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-1", "Build a REST API");
        create_test_task(&fs, dir.path(), "task-2", "Fix CSS styling");

        let result = search_history(&fs, dir.path(), "rest").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].task, "Build a REST API");
    }

    #[test]
    fn test_search_history_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-1", "Build a REST API");

        let result = search_history(&fs, dir.path(), "rest").unwrap();
        assert_eq!(result.len(), 1);

        let result = search_history(&fs, dir.path(), "REST").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_search_history_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-1", "Build a REST API");

        let result = search_history(&fs, dir.path(), "nonexistent").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_delete_task() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-to-delete", "Delete me");

        let task_dir = storage::task_dir(dir.path(), "task-to-delete");
        assert!(task_dir.exists());

        delete_task(&fs, dir.path(), "task-to-delete").unwrap();
        assert!(!task_dir.exists());
    }

    #[test]
    fn test_delete_task_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        // Should not error
        delete_task(&fs, dir.path(), "nonexistent").unwrap();
    }

    #[test]
    fn test_get_history_item_found() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        create_test_task(&fs, dir.path(), "task-1", "Find me");

        let result = get_history_item(&fs, dir.path(), "task-1").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().task, "Find me");
    }

    #[test]
    fn test_get_history_item_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        let result = get_history_item(&fs, dir.path(), "nonexistent").unwrap();
        assert!(result.is_none());
    }
}
