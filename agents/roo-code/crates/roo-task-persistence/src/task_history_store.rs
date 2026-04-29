//! Task history store with per-task file persistence and index caching.
//!
//! Encapsulates all task history persistence logic. Each task's `HistoryItem` is
//! stored as an individual JSON file in its task directory. A single index file
//! (`_index.json`) is maintained as a cache for fast list reads at startup.
//!
//! Source: `src/core/task-persistence/TaskHistoryStore.ts`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::storage::TaskFileSystem;
use crate::types::HistoryItem;
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// HistoryIndex — index file format
// ---------------------------------------------------------------------------

/// Index file format for fast startup reads.
#[derive(Debug, Serialize, Deserialize)]
struct HistoryIndex {
    version: u32,
    updated_at: u64,
    entries: Vec<HistoryItem>,
}

// ---------------------------------------------------------------------------
// TaskHistoryStoreOptions
// ---------------------------------------------------------------------------

/// Options for constructing a [`TaskHistoryStore`].
///
/// Source: `src/core/task-persistence/TaskHistoryStore.ts` — `TaskHistoryStoreOptions`
pub struct TaskHistoryStoreOptions {
    /// Optional callback invoked inside the write lock after each mutation
    /// (upsert, delete, delete_many). Used for serialized write-through to
    /// globalState during the transition period.
    pub on_write: Option<Arc<dyn Fn(&[HistoryItem]) + Send + Sync>>,
}

// ---------------------------------------------------------------------------
// TaskHistoryStore
// ---------------------------------------------------------------------------

/// Task history store with per-task file persistence and index caching.
///
/// Source: `src/core/task-persistence/TaskHistoryStore.ts`
pub struct TaskHistoryStore {
    global_storage_path: PathBuf,
    on_write: Option<Arc<dyn Fn(&[HistoryItem]) + Send + Sync>>,
    cache: Mutex<HashMap<String, HistoryItem>>,
    #[allow(dead_code)]
    disposed: Mutex<bool>,
}

/// File names used in the TS source.
const HISTORY_ITEM_FILE: &str = "history_item.json";
const INDEX_FILE: &str = "_index.json";

impl TaskHistoryStore {
    /// Create a new `TaskHistoryStore`.
    pub fn new(global_storage_path: PathBuf, options: Option<TaskHistoryStoreOptions>) -> Self {
        Self {
            global_storage_path,
            on_write: options.and_then(|o| o.on_write),
            cache: Mutex::new(HashMap::new()),
            disposed: Mutex::new(false),
        }
    }

    // ────────────────────────────── Lifecycle ──────────────────────────────

    /// Load index, reconcile if needed.
    ///
    /// Source: `TaskHistoryStore.initialize()`
    pub fn initialize(&self, fs: &dyn TaskFileSystem) -> Result<(), TaskPersistenceError> {
        let tasks_dir = self.tasks_dir(fs)?;

        // Ensure tasks directory exists
        fs.create_dir_all(&tasks_dir)?;

        // 1. Load existing index into the cache
        self.load_index(fs)?;

        // 2. Reconcile cache against actual task directories on disk
        self.reconcile(fs)?;

        Ok(())
    }

    // ────────────────────────────── Reads ──────────────────────────────

    /// Get a single history item by task ID.
    ///
    /// Source: `TaskHistoryStore.get()`
    pub fn get(&self, task_id: &str) -> Option<HistoryItem> {
        self.cache.lock().get(task_id).cloned()
    }

    /// Get all history items, sorted by timestamp descending (newest first).
    ///
    /// Source: `TaskHistoryStore.getAll()`
    pub fn get_all(&self) -> Vec<HistoryItem> {
        let mut items: Vec<HistoryItem> = self.cache.lock().values().cloned().collect();
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        items
    }

    /// Get history items filtered by workspace path.
    ///
    /// Source: `TaskHistoryStore.getByWorkspace()`
    pub fn get_by_workspace(&self, workspace: &str) -> Vec<HistoryItem> {
        self.get_all()
            .into_iter()
            .filter(|item| item.workspace == workspace)
            .collect()
    }

    // ────────────────────────────── Mutations ──────────────────────────────

    /// Insert or update a history item.
    ///
    /// Writes the per-task file immediately (source of truth),
    /// updates the in-memory Map, and writes the index.
    ///
    /// Source: `TaskHistoryStore.upsert()`
    pub fn upsert(
        &self,
        fs: &dyn TaskFileSystem,
        item: HistoryItem,
    ) -> Result<Vec<HistoryItem>, TaskPersistenceError> {
        let existing = self.cache.lock().get(&item.id).cloned();

        // Merge: preserve existing metadata unless explicitly overwritten
        let merged = if let Some(existing) = existing {
            let mut merged = existing;
            merged.timestamp = item.timestamp;
            merged.task = item.task;
            merged.tokens_in = item.tokens_in;
            merged.tokens_out = item.tokens_out;
            merged.cache_writes = item.cache_writes;
            merged.cache_reads = item.cache_reads;
            merged.total_cost = item.total_cost;
            merged.size = item.size;
            if item.workspace != merged.workspace && !item.workspace.is_empty() {
                merged.workspace = item.workspace;
            }
            if item.mode.is_some() {
                merged.mode = item.mode;
            }
            if item.api_config_name.is_some() {
                merged.api_config_name = item.api_config_name;
            }
            if item.root_task_id.is_some() {
                merged.root_task_id = item.root_task_id;
            }
            if item.parent_task_id.is_some() {
                merged.parent_task_id = item.parent_task_id;
            }
            merged
        } else {
            item
        };

        // Write per-task file (source of truth)
        self.write_task_file(fs, &merged)?;

        // Update in-memory cache
        self.cache.lock().insert(merged.id.clone(), merged);

        // Write index
        self.write_index(fs)?;

        let all = self.get_all();

        // Call on_write callback
        if let Some(on_write) = &self.on_write {
            on_write(&all);
        }

        Ok(all)
    }

    /// Delete a single task's history item.
    ///
    /// Source: `TaskHistoryStore.delete()`
    pub fn delete(
        &self,
        fs: &dyn TaskFileSystem,
        task_id: &str,
    ) -> Result<(), TaskPersistenceError> {
        self.cache.lock().remove(task_id);

        // Remove per-task file (best-effort)
        if let Ok(file_path) = self.task_file_path(fs, task_id) {
            let _ = fs.write_file(&file_path, ""); // best-effort delete
        }

        self.write_index(fs)?;

        // Call on_write callback
        if let Some(on_write) = &self.on_write {
            on_write(&self.get_all());
        }

        Ok(())
    }

    /// Delete multiple tasks' history items in a batch.
    ///
    /// Source: `TaskHistoryStore.deleteMany()`
    pub fn delete_many(
        &self,
        fs: &dyn TaskFileSystem,
        task_ids: &[String],
    ) -> Result<(), TaskPersistenceError> {
        {
            let mut cache = self.cache.lock();
            for task_id in task_ids {
                cache.remove(task_id);
            }
        }

        self.write_index(fs)?;

        // Call on_write callback
        if let Some(on_write) = &self.on_write {
            on_write(&self.get_all());
        }

        Ok(())
    }

    // ────────────────────────────── Reconciliation ──────────────────────────────

    /// Scan task directories vs index and fix any drift.
    ///
    /// - Tasks on disk but missing from cache: read and add
    /// - Tasks in cache but missing from disk: remove
    ///
    /// Source: `TaskHistoryStore.reconcile()`
    pub fn reconcile(&self, fs: &dyn TaskFileSystem) -> Result<(), TaskPersistenceError> {
        let tasks_dir = self.tasks_dir(fs)?;

        let entries = fs.read_dir(&tasks_dir)?;
        let task_dir_names: Vec<String> = entries
            .into_iter()
            .filter(|e| e.is_dir && !e.file_name.starts_with('_') && !e.file_name.starts_with('.'))
            .map(|e| e.file_name)
            .collect();

        let on_disk_ids: std::collections::HashSet<String> = task_dir_names.into_iter().collect();
        let cache_ids: std::collections::HashSet<String> =
            self.cache.lock().keys().cloned().collect();
        let mut changed = false;

        // Tasks on disk but not in cache: read their history_item.json
        for task_id in &on_disk_ids {
            if !cache_ids.contains(task_id) {
                if let Ok(Some(item)) = self.read_task_file(fs, task_id) {
                    self.cache.lock().insert(task_id.clone(), item);
                    changed = true;
                }
            }
        }

        // Tasks in cache but not on disk: remove from cache
        for task_id in &cache_ids {
            if !on_disk_ids.contains(task_id) {
                self.cache.lock().remove(task_id);
                changed = true;
            }
        }

        if changed {
            self.write_index(fs)?;
        }

        Ok(())
    }

    // ────────────────────────────── Cache invalidation ──────────────────────────────

    /// Invalidate a single task's cache entry (re-read from disk).
    ///
    /// Source: `TaskHistoryStore.invalidate()`
    pub fn invalidate(
        &self,
        fs: &dyn TaskFileSystem,
        task_id: &str,
    ) -> Result<(), TaskPersistenceError> {
        match self.read_task_file(fs, task_id) {
            Ok(Some(item)) => {
                self.cache.lock().insert(task_id.to_string(), item);
            }
            _ => {
                self.cache.lock().remove(task_id);
            }
        }
        Ok(())
    }

    /// Clear all in-memory cache.
    ///
    /// Source: `TaskHistoryStore.invalidateAll()`
    pub fn invalidate_all(&self) {
        self.cache.lock().clear();
    }

    // ────────────────────────────── Migration ──────────────────────────────

    /// Migrate from a globalState task history array to per-task files.
    ///
    /// For each entry in the array, writes a `history_item.json` file if one
    /// doesn't already exist. This is idempotent and safe to re-run.
    ///
    /// Source: `TaskHistoryStore.migrateFromGlobalState()`
    pub fn migrate_from_global_state(
        &self,
        fs: &dyn TaskFileSystem,
        entries: &[HistoryItem],
    ) -> Result<(), TaskPersistenceError> {
        if entries.is_empty() {
            return Ok(());
        }

        for item in entries {
            if item.id.is_empty() {
                continue;
            }

            // Check if task directory exists on disk
            let tasks_dir = self.tasks_dir(fs)?;
            let task_dir = tasks_dir.join(&item.id);

            if !fs.file_exists(&task_dir)? {
                // Task directory doesn't exist; skip this entry as it's orphaned
                continue;
            }

            // Write history_item.json if it doesn't exist yet
            let file_path = task_dir.join(HISTORY_ITEM_FILE);
            if !fs.file_exists(&file_path)? {
                let content = serde_json::to_string_pretty(item)?;
                fs.write_file(&file_path, &content)?;
                self.cache.lock().insert(item.id.clone(), item.clone());
            }
        }

        // Write the index
        self.write_index(fs)?;
        Ok(())
    }

    // ────────────────────────────── Private: Index management ──────────────────────────────

    /// Load the `_index.json` file into the in-memory cache.
    fn load_index(&self, fs: &dyn TaskFileSystem) -> Result<(), TaskPersistenceError> {
        let index_path = self.index_path(fs)?;

        if !fs.file_exists(&index_path)? {
            return Ok(());
        }

        let content = fs.read_file(&index_path)?;
        match serde_json::from_str::<HistoryIndex>(&content) {
            Ok(index) => {
                if index.version == 1 {
                    let mut cache = self.cache.lock();
                    for entry in index.entries {
                        if !entry.id.is_empty() {
                            cache.insert(entry.id.clone(), entry);
                        }
                    }
                }
            }
            Err(_) => {
                // Index doesn't exist or is corrupted; cache stays empty.
                // Reconciliation will rebuild it from per-task files.
            }
        }

        Ok(())
    }

    /// Write the full index to disk.
    fn write_index(&self, fs: &dyn TaskFileSystem) -> Result<(), TaskPersistenceError> {
        let index_path = self.index_path(fs)?;
        let index = HistoryIndex {
            version: 1,
            updated_at: chrono::Utc::now().timestamp_millis() as u64,
            entries: self.get_all(),
        };

        let content = serde_json::to_string_pretty(&index)?;
        fs.write_file(&index_path, &content)?;
        Ok(())
    }

    // ────────────────────────────── Private: Per-task file I/O ──────────────────────────────

    /// Write a HistoryItem to its per-task `history_item.json` file.
    fn write_task_file(
        &self,
        fs: &dyn TaskFileSystem,
        item: &HistoryItem,
    ) -> Result<(), TaskPersistenceError> {
        let file_path = self.task_file_path(fs, &item.id)?;
        if let Some(parent) = file_path.parent() {
            fs.create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(item)?;
        fs.write_file(&file_path, &content)?;
        Ok(())
    }

    /// Read a HistoryItem from its per-task `history_item.json` file.
    fn read_task_file(
        &self,
        fs: &dyn TaskFileSystem,
        task_id: &str,
    ) -> Result<Option<HistoryItem>, TaskPersistenceError> {
        let file_path = self.task_file_path(fs, task_id)?;

        if !fs.file_exists(&file_path)? {
            return Ok(None);
        }

        let content = fs.read_file(&file_path)?;
        match serde_json::from_str::<HistoryItem>(&content) {
            Ok(item) if !item.id.is_empty() => Ok(Some(item)),
            _ => Ok(None),
        }
    }

    // ────────────────────────────── Private: Path helpers ──────────────────────────────

    /// Get the tasks base directory path.
    fn tasks_dir(&self, _fs: &dyn TaskFileSystem) -> Result<PathBuf, TaskPersistenceError> {
        Ok(self.global_storage_path.join("tasks"))
    }

    /// Get the path to a task's `history_item.json` file.
    fn task_file_path(
        &self,
        fs: &dyn TaskFileSystem,
        task_id: &str,
    ) -> Result<PathBuf, TaskPersistenceError> {
        Ok(self.tasks_dir(fs)?.join(task_id).join(HISTORY_ITEM_FILE))
    }

    /// Get the path to the `_index.json` file.
    fn index_path(&self, fs: &dyn TaskFileSystem) -> Result<PathBuf, TaskPersistenceError> {
        Ok(self.tasks_dir(fs)?.join(INDEX_FILE))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::OsFileSystem;
    use crate::types::PersistenceTaskStatus;

    fn make_item(id: &str, task: &str, ts: u64) -> HistoryItem {
        HistoryItem {
            id: id.to_string(),
            number: 1,
            timestamp: ts,
            task: task.to_string(),
            tokens_in: 0,
            tokens_out: 0,
            cache_writes: 0,
            cache_reads: 0,
            total_cost: 0.0,
            size: 0,
            workspace: "/tmp/ws".to_string(),
            mode: None,
            api_config_name: None,
            status: PersistenceTaskStatus::Active,
            root_task_id: None,
            parent_task_id: None,
        }
    }

    #[test]
    fn test_store_initialize_empty() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);

        store.initialize(&fs).unwrap();
        let all = store.get_all();
        assert!(all.is_empty());
    }

    #[test]
    fn test_store_upsert_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        let item = make_item("task-1", "Build feature", 1000);
        store.upsert(&fs, item).unwrap();

        let retrieved = store.get("task-1").unwrap();
        assert_eq!(retrieved.task, "Build feature");
        assert_eq!(retrieved.timestamp, 1000);
    }

    #[test]
    fn test_store_upsert_merge() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        let item1 = make_item("task-1", "Original", 1000);
        store.upsert(&fs, item1).unwrap();

        let mut item2 = make_item("task-1", "Updated", 2000);
        item2.tokens_in = 100;
        store.upsert(&fs, item2).unwrap();

        let retrieved = store.get("task-1").unwrap();
        assert_eq!(retrieved.task, "Updated");
        assert_eq!(retrieved.timestamp, 2000);
        assert_eq!(retrieved.tokens_in, 100);
    }

    #[test]
    fn test_store_get_all_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "Older", 1000)).unwrap();
        store.upsert(&fs, make_item("task-2", "Newer", 2000)).unwrap();
        store.upsert(&fs, make_item("task-3", "Middle", 1500)).unwrap();

        let all = store.get_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].task, "Newer");
        assert_eq!(all[1].task, "Middle");
        assert_eq!(all[2].task, "Older");
    }

    #[test]
    fn test_store_get_by_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        let mut item1 = make_item("task-1", "WS1", 1000);
        item1.workspace = "/ws1".to_string();
        store.upsert(&fs, item1).unwrap();

        let mut item2 = make_item("task-2", "WS2", 2000);
        item2.workspace = "/ws2".to_string();
        store.upsert(&fs, item2).unwrap();

        let ws1_items = store.get_by_workspace("/ws1");
        assert_eq!(ws1_items.len(), 1);
        assert_eq!(ws1_items[0].task, "WS1");
    }

    #[test]
    fn test_store_delete() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "Delete me", 1000)).unwrap();
        assert!(store.get("task-1").is_some());

        store.delete(&fs, "task-1").unwrap();
        assert!(store.get("task-1").is_none());
    }

    #[test]
    fn test_store_delete_many() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "One", 1000)).unwrap();
        store.upsert(&fs, make_item("task-2", "Two", 2000)).unwrap();
        store.upsert(&fs, make_item("task-3", "Three", 3000)).unwrap();

        store
            .delete_many(&fs, &["task-1".to_string(), "task-3".to_string()])
            .unwrap();

        let all = store.get_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].task, "Two");
    }

    #[test]
    fn test_store_reconcile_adds_missing() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        // Manually create a task directory with a history_item.json
        let tasks_dir = dir.path().join("tasks");
        fs.create_dir_all(&tasks_dir.join("manual-task")).unwrap();
        let item = make_item("manual-task", "Manual entry", 1000);
        let content = serde_json::to_string_pretty(&item).unwrap();
        fs.write_file(&tasks_dir.join("manual-task").join("history_item.json"), &content)
            .unwrap();

        // Reconcile should pick it up
        store.reconcile(&fs).unwrap();

        let retrieved = store.get("manual-task").unwrap();
        assert_eq!(retrieved.task, "Manual entry");
    }

    #[test]
    fn test_store_reconcile_removes_orphaned() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        // Insert an item that has no directory
        store
            .upsert(&fs, make_item("orphan-task", "Orphan", 1000))
            .unwrap();

        // Manually remove the directory
        let task_dir = dir.path().join("tasks").join("orphan-task");
        let _ = std::fs::remove_dir_all(&task_dir);

        // Reconcile should remove it from cache
        store.reconcile(&fs).unwrap();
        assert!(store.get("orphan-task").is_none());
    }

    #[test]
    fn test_store_invalidate() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "Original", 1000)).unwrap();

        // Manually update the file on disk
        let file_path = dir
            .path()
            .join("tasks")
            .join("task-1")
            .join("history_item.json");
        let updated = make_item("task-1", "Updated on disk", 2000);
        fs.write_file(&file_path, &serde_json::to_string_pretty(&updated).unwrap())
            .unwrap();

        // Cache still has old value
        assert_eq!(store.get("task-1").unwrap().task, "Original");

        // Invalidate should re-read from disk
        store.invalidate(&fs, "task-1").unwrap();
        assert_eq!(store.get("task-1").unwrap().task, "Updated on disk");
    }

    #[test]
    fn test_store_invalidate_all() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "One", 1000)).unwrap();
        store.upsert(&fs, make_item("task-2", "Two", 2000)).unwrap();

        store.invalidate_all();
        assert!(store.get_all().is_empty());
    }

    #[test]
    fn test_store_on_write_callback() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        let written_items: Arc<Mutex<Vec<Vec<HistoryItem>>>> = Arc::new(Mutex::new(Vec::new()));
        let written_clone = written_items.clone();

        let store = TaskHistoryStore::new(
            dir.path().to_path_buf(),
            Some(TaskHistoryStoreOptions {
                on_write: Some(Arc::new(move |items: &[HistoryItem]| {
                    written_clone
                        .lock()
                        .push(items.to_vec());
                })),
            }),
        );
        store.initialize(&fs).unwrap();

        store.upsert(&fs, make_item("task-1", "Test", 1000)).unwrap();

        let calls = written_items.lock();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), 1);
        assert_eq!(calls[0][0].task, "Test");
    }

    #[test]
    fn test_store_migrate_from_global_state() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;
        let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
        store.initialize(&fs).unwrap();

        // Create a task directory (simulating existing task)
        let tasks_dir = dir.path().join("tasks");
        fs.create_dir_all(&tasks_dir.join("existing-task")).unwrap();

        let entries = vec![
            make_item("existing-task", "Existing", 1000),
            make_item("orphan-task", "Orphan", 2000), // no directory
        ];

        store.migrate_from_global_state(&fs, &entries).unwrap();

        // existing-task should be in cache
        assert!(store.get("existing-task").is_some());
        // orphan-task should not be in cache (no directory)
        assert!(store.get("orphan-task").is_none());
    }

    #[test]
    fn test_store_persistence_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        // First instance: write data
        {
            let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
            store.initialize(&fs).unwrap();
            store
                .upsert(&fs, make_item("task-1", "Persistent", 1000))
                .unwrap();
        }

        // Second instance: read data
        {
            let store = TaskHistoryStore::new(dir.path().to_path_buf(), None);
            store.initialize(&fs).unwrap();
            let item = store.get("task-1").unwrap();
            assert_eq!(item.task, "Persistent");
        }
    }
}
