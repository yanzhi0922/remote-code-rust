//! Task manager for managing active tasks.
//!
//! Provides thread-safe storage and retrieval of [`TaskLifecycle`] instances,
//! with support for tracking the currently active task.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::task_lifecycle::TaskLifecycle;

// ---------------------------------------------------------------------------
// TaskManager
// ---------------------------------------------------------------------------

/// Manages active tasks in the server.
///
/// Stores [`TaskLifecycle`] instances keyed by task ID and tracks which task
/// is currently active. All operations are thread-safe via internal
/// `std::sync::Mutex` guards for the HashMap and `tokio::sync::Mutex` for
/// individual lifecycles (since lifecycle methods are async).
///
/// # Example
///
/// ```ignore
/// use roo_task::task_manager::TaskManager;
/// use roo_task::TaskLifecycle;
///
/// let tm = TaskManager::new();
/// let lifecycle = TaskLifecycle::new(engine);
/// tm.create_task("task-1".to_string(), lifecycle);
///
/// let active = tm.get_active_task(); // returns Some(...)
/// ```
pub struct TaskManager {
    tasks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<TaskLifecycle>>>>>,
    active_task_id: Arc<Mutex<Option<String>>>,
}

impl TaskManager {
    /// Create a new empty task manager.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            active_task_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a task: insert the lifecycle and set it as the active task.
    pub fn create_task(&self, task_id: String, lifecycle: TaskLifecycle) {
        let wrapped = Arc::new(tokio::sync::Mutex::new(lifecycle));
        self.tasks.lock().unwrap().insert(task_id.clone(), wrapped);
        *self.active_task_id.lock().unwrap() = Some(task_id);
    }

    /// Get a task lifecycle by its ID.
    pub fn get_task(&self, task_id: &str) -> Option<Arc<tokio::sync::Mutex<TaskLifecycle>>> {
        self.tasks.lock().unwrap().get(task_id).cloned()
    }

    /// Get the currently active task lifecycle.
    pub fn get_active_task(&self) -> Option<Arc<tokio::sync::Mutex<TaskLifecycle>>> {
        let active_id = self.active_task_id.lock().unwrap().clone();
        match active_id {
            Some(ref id) => self.tasks.lock().unwrap().get(id).cloned(),
            None => None,
        }
    }

    /// Set the currently active task by ID.
    ///
    /// No-op if the task ID is not found in the task map.
    pub fn set_active_task(&self, task_id: &str) {
        if self.tasks.lock().unwrap().contains_key(task_id) {
            *self.active_task_id.lock().unwrap() = Some(task_id.to_string());
        }
    }

    /// Remove a task by ID.
    ///
    /// If the removed task was the active task, the active task is cleared.
    /// Returns the removed lifecycle, if any.
    pub fn remove_task(&self, task_id: &str) -> Option<Arc<tokio::sync::Mutex<TaskLifecycle>>> {
        let removed = self.tasks.lock().unwrap().remove(task_id);
        if removed.is_some() {
            let mut active = self.active_task_id.lock().unwrap();
            if active.as_deref() == Some(task_id) {
                *active = None;
            }
        }
        removed
    }

    /// List all task IDs.
    pub fn list_tasks(&self) -> Vec<String> {
        self.tasks.lock().unwrap().keys().cloned().collect()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskConfig;
    use crate::engine::TaskEngine;

    fn make_lifecycle(task_id: &str) -> TaskLifecycle {
        let config = TaskConfig::new(task_id, "/tmp/test").with_start_task(false);
        let engine = TaskEngine::new(config).unwrap();
        TaskLifecycle::new(engine)
    }

    #[test]
    fn test_create_and_get_task() {
        let tm = TaskManager::new();
        let lifecycle = make_lifecycle("t1");
        tm.create_task("t1".to_string(), lifecycle);

        let got = tm.get_task("t1");
        assert!(got.is_some());
        assert!(tm.get_task("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_active_task() {
        let tm = TaskManager::new();
        assert!(tm.get_active_task().is_none());

        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        assert!(tm.get_active_task().is_some());

        tm.create_task("t2".to_string(), make_lifecycle("t2"));
        // t2 should now be active
        {
            let active = tm.get_active_task().unwrap();
            let lc = active.lock().await;
            assert_eq!(lc.task_id(), "t2");
        }
    }

    #[tokio::test]
    async fn test_set_active_task() {
        let tm = TaskManager::new();
        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        tm.create_task("t2".to_string(), make_lifecycle("t2"));

        tm.set_active_task("t1");
        let active = tm.get_active_task().unwrap();
        let lc = active.lock().await;
        assert_eq!(lc.task_id(), "t1");
    }

    #[test]
    fn test_set_active_task_nonexistent() {
        let tm = TaskManager::new();
        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        tm.set_active_task("nonexistent"); // no-op
        let active_id = tm.active_task_id.lock().unwrap().clone();
        assert_eq!(active_id, Some("t1".to_string()));
    }

    #[tokio::test]
    async fn test_remove_task() {
        let tm = TaskManager::new();
        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        tm.create_task("t2".to_string(), make_lifecycle("t2"));

        let removed = tm.remove_task("t2");
        assert!(removed.is_some());
        assert!(tm.get_task("t2").is_none());
        // Active should be cleared since t2 was active
        assert!(tm.get_active_task().is_none());
    }

    #[tokio::test]
    async fn test_remove_non_active_task() {
        let tm = TaskManager::new();
        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        tm.create_task("t2".to_string(), make_lifecycle("t2"));

        let removed = tm.remove_task("t1");
        assert!(removed.is_some());
        // t2 is still active
        let active = tm.get_active_task().unwrap();
        let lc = active.lock().await;
        assert_eq!(lc.task_id(), "t2");
    }

    #[test]
    fn test_list_tasks() {
        let tm = TaskManager::new();
        assert!(tm.list_tasks().is_empty());

        tm.create_task("t1".to_string(), make_lifecycle("t1"));
        tm.create_task("t2".to_string(), make_lifecycle("t2"));

        let mut ids = tm.list_tasks();
        ids.sort();
        assert_eq!(ids, vec!["t1", "t2"]);
    }

    #[test]
    fn test_default() {
        let tm = TaskManager::default();
        assert!(tm.list_tasks().is_empty());
    }
}
