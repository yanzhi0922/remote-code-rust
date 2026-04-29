use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use thiserror::Error;

use crate::types::TaskMetadata;

/// Errors that can occur during metadata store operations.
#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Trait for persisting and loading task metadata.
///
/// Abstracts all IO operations so the core tracker logic remains testable
/// without touching the filesystem.
pub trait MetadataStore: Send + Sync {
    /// Load task metadata for the given task ID.
    /// Returns a default (empty) `TaskMetadata` if none exists.
    fn load(&self, task_id: &str) -> Result<TaskMetadata>;

    /// Save task metadata for the given task ID.
    fn save(&self, task_id: &str, metadata: &TaskMetadata) -> Result<()>;
}

/// Filesystem-backed metadata store.
///
/// Stores each task's metadata as a JSON file at
/// `<base_dir>/<task_id>/task_metadata.json`.
pub struct FileMetadataStore {
    base_dir: PathBuf,
}

impl FileMetadataStore {
    /// Create a new `FileMetadataStore` rooted at `base_dir`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Returns the path to the metadata file for a given task.
    fn file_path(&self, task_id: &str) -> PathBuf {
        self.base_dir.join(task_id).join("task_metadata.json")
    }
}

impl MetadataStore for FileMetadataStore {
    fn load(&self, task_id: &str) -> Result<TaskMetadata> {
        let path = self.file_path(task_id);
        if !path.exists() {
            return Ok(TaskMetadata::default());
        }
        let data = fs::read_to_string(&path)?;
        let metadata: TaskMetadata = serde_json::from_str(&data)?;
        Ok(metadata)
    }

    fn save(&self, task_id: &str, metadata: &TaskMetadata) -> Result<()> {
        let dir = self.base_dir.join(task_id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("task_metadata.json");
        let json = serde_json::to_string_pretty(metadata)?;
        fs::write(path, json)?;
        Ok(())
    }
}

/// In-memory metadata store for testing.
///
/// Uses a `RwLock`-protected `HashMap` internally so it can be shared
/// across threads when needed.
pub struct InMemoryMetadataStore {
    data: RwLock<HashMap<String, TaskMetadata>>,
}

impl InMemoryMetadataStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryMetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataStore for InMemoryMetadataStore {
    fn load(&self, task_id: &str) -> Result<TaskMetadata> {
        let data = self.data.read().unwrap();
        Ok(data.get(task_id).cloned().unwrap_or_default())
    }

    fn save(&self, task_id: &str, metadata: &TaskMetadata) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.insert(task_id.to_string(), metadata.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileMetadataEntry, RecordSource, RecordState};

    // --- InMemoryMetadataStore tests ---

    #[test]
    fn test_in_memory_store_load_empty() {
        let store = InMemoryMetadataStore::new();
        let metadata = store.load("task-1").unwrap();
        assert!(metadata.files_in_context.is_empty());
    }

    #[test]
    fn test_in_memory_store_save_and_load() {
        let store = InMemoryMetadataStore::new();
        let metadata = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "main.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(1000),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        store.save("task-1", &metadata).unwrap();
        let loaded = store.load("task-1").unwrap();
        assert_eq!(loaded, metadata);
    }

    #[test]
    fn test_in_memory_store_overwrite() {
        let store = InMemoryMetadataStore::new();
        let metadata_v1 = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "a.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(100),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        store.save("task-1", &metadata_v1).unwrap();

        let metadata_v2 = TaskMetadata {
            files_in_context: vec![],
        };
        store.save("task-1", &metadata_v2).unwrap();

        let loaded = store.load("task-1").unwrap();
        assert_eq!(loaded, metadata_v2);
    }

    #[test]
    fn test_in_memory_store_different_task_ids() {
        let store = InMemoryMetadataStore::new();
        let metadata_a = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "a.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(100),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        let metadata_b = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "b.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::RooEdited,
                roo_read_date: Some(200),
                roo_edit_date: Some(200),
                user_edit_date: None,
            }],
        };
        store.save("task-a", &metadata_a).unwrap();
        store.save("task-b", &metadata_b).unwrap();

        assert_eq!(store.load("task-a").unwrap(), metadata_a);
        assert_eq!(store.load("task-b").unwrap(), metadata_b);
    }

    #[test]
    fn test_in_memory_store_default() {
        let store = InMemoryMetadataStore::default();
        let metadata = store.load("nonexistent").unwrap();
        assert!(metadata.files_in_context.is_empty());
    }

    // --- FileMetadataStore tests ---

    #[test]
    fn test_file_store_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileMetadataStore::new(tmp.path().to_path_buf());

        let metadata = TaskMetadata {
            files_in_context: vec![
                FileMetadataEntry {
                    path: "src/main.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(1234567890),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "lib.rs".to_string(),
                    record_state: RecordState::Stale,
                    record_source: RecordSource::UserEdited,
                    roo_read_date: Some(999),
                    roo_edit_date: Some(888),
                    user_edit_date: Some(777),
                },
            ],
        };

        store.save("task-42", &metadata).unwrap();
        let loaded = store.load("task-42").unwrap();
        assert_eq!(loaded, metadata);
    }

    #[test]
    fn test_file_store_load_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileMetadataStore::new(tmp.path().to_path_buf());

        let loaded = store.load("nonexistent-task").unwrap();
        assert!(loaded.files_in_context.is_empty());
    }

    #[test]
    fn test_file_store_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileMetadataStore::new(tmp.path().to_path_buf());

        let metadata = TaskMetadata::default();
        store.save("deep/nested/task", &metadata).unwrap();

        let loaded = store.load("deep/nested/task").unwrap();
        assert_eq!(loaded, metadata);

        let file_path = tmp.path().join("deep/nested/task/task_metadata.json");
        assert!(file_path.exists());
    }

    #[test]
    fn test_file_store_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileMetadataStore::new(tmp.path().to_path_buf());

        let v1 = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "old.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(100),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        store.save("task-1", &v1).unwrap();

        let v2 = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "new.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::RooEdited,
                roo_read_date: Some(200),
                roo_edit_date: Some(200),
                user_edit_date: None,
            }],
        };
        store.save("task-1", &v2).unwrap();

        let loaded = store.load("task-1").unwrap();
        assert_eq!(loaded, v2);
    }

    #[test]
    fn test_file_store_pretty_json() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileMetadataStore::new(tmp.path().to_path_buf());

        let metadata = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "test.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(42),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        store.save("task-1", &metadata).unwrap();

        let file_path = tmp.path().join("task-1/task_metadata.json");
        let content = fs::read_to_string(file_path).unwrap();
        assert!(content.contains('\n'));
    }
}
