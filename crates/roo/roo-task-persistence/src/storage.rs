//! Filesystem abstraction and task storage operations.
//!
//! Provides a [`TaskFileSystem`] trait for abstracting file operations,
//! an [`OsFileSystem`] implementation using the real filesystem, and
//! task directory management functions.

use std::path::{Path, PathBuf};

use crate::types::DirEntry;
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// TaskFileSystem trait
// ---------------------------------------------------------------------------

/// Abstraction over filesystem operations for task persistence.
///
/// This trait enables testing with mock filesystems while using the real
/// OS filesystem in production.
pub trait TaskFileSystem: Send + Sync {
    /// Read the entire contents of a file as a string.
    fn read_file(&self, path: &Path) -> Result<String, TaskPersistenceError>;

    /// Write a string to a file, creating it if it doesn't exist.
    fn write_file(&self, path: &Path, content: &str) -> Result<(), TaskPersistenceError>;

    /// Check whether a file exists at the given path.
    fn file_exists(&self, path: &Path) -> Result<bool, TaskPersistenceError>;

    /// Recursively create all directories in the path.
    fn create_dir_all(&self, path: &Path) -> Result<(), TaskPersistenceError>;

    /// Calculate the total size (in bytes) of all files in a directory tree.
    fn dir_size(&self, path: &Path) -> Result<u64, TaskPersistenceError>;

    /// Recursively remove a directory and all its contents.
    fn remove_dir_all(&self, path: &Path) -> Result<(), TaskPersistenceError>;

    /// Read the entries of a directory (non-recursive).
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, TaskPersistenceError>;
}

// ---------------------------------------------------------------------------
// OsFileSystem — real filesystem implementation
// ---------------------------------------------------------------------------

/// Production filesystem implementation using [`std::fs`].
pub struct OsFileSystem;

impl TaskFileSystem for OsFileSystem {
    fn read_file(&self, path: &Path) -> Result<String, TaskPersistenceError> {
        Ok(std::fs::read_to_string(path)?)
    }

    fn write_file(&self, path: &Path, content: &str) -> Result<(), TaskPersistenceError> {
        Ok(std::fs::write(path, content)?)
    }

    fn file_exists(&self, path: &Path) -> Result<bool, TaskPersistenceError> {
        Ok(path.exists())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), TaskPersistenceError> {
        Ok(std::fs::create_dir_all(path)?)
    }

    fn dir_size(&self, path: &Path) -> Result<u64, TaskPersistenceError> {
        if !path.exists() {
            return Ok(0);
        }
        let mut total: u64 = 0;
        compute_dir_size_recursive(path, &mut total)?;
        Ok(total)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), TaskPersistenceError> {
        if path.exists() {
            Ok(std::fs::remove_dir_all(path)?)
        } else {
            Ok(())
        }
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, TaskPersistenceError> {
        let mut entries = Vec::new();
        if !path.exists() {
            return Ok(entries);
        }
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry
                .file_name()
                .to_string_lossy()
                .to_string();
            let is_dir = entry
                .file_type()
                .map(|ft| ft.is_dir())
                .unwrap_or(false);
            entries.push(DirEntry {
                path: entry.path(),
                file_name,
                is_dir,
            });
        }
        Ok(entries)
    }
}

/// Recursively compute directory size.
fn compute_dir_size_recursive(path: &Path, total: &mut u64) -> Result<(), TaskPersistenceError> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            *total += entry.metadata()?.len();
        } else if file_type.is_dir() {
            compute_dir_size_recursive(&entry.path(), total)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Task directory management
// ---------------------------------------------------------------------------

/// Get the task directory path for a given task ID under a base storage path.
pub fn task_dir(base_storage_path: &Path, task_id: &str) -> PathBuf {
    base_storage_path.join("tasks").join(task_id)
}

/// Get the messages file path for a given task ID.
pub fn messages_path(base_storage_path: &Path, task_id: &str) -> PathBuf {
    task_dir(base_storage_path, task_id).join("messages.json")
}

/// Get the API conversation history file path for a given task ID.
///
/// Source: `src/core/task-persistence/apiMessages.ts` — `GlobalFileNames.apiConversationHistory`
pub fn api_messages_path(base_storage_path: &Path, task_id: &str) -> PathBuf {
    task_dir(base_storage_path, task_id).join("api_conversation_history.json")
}

/// Get the metadata file path for a given task ID.
pub fn metadata_path(base_storage_path: &Path, task_id: &str) -> PathBuf {
    task_dir(base_storage_path, task_id).join("meta.json")
}

/// Ensure the task directory exists, creating it if necessary.
pub fn ensure_task_dir(
    fs: &dyn TaskFileSystem,
    base_storage_path: &Path,
    task_id: &str,
) -> Result<PathBuf, TaskPersistenceError> {
    let dir = task_dir(base_storage_path, task_id);
    fs.create_dir_all(&dir)?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_dir_path() {
        let base = Path::new("/data/storage");
        let dir = task_dir(base, "abc-123");
        assert_eq!(dir, PathBuf::from("/data/storage/tasks/abc-123"));
    }

    #[test]
    fn test_messages_path() {
        let base = Path::new("/data/storage");
        let path = messages_path(base, "abc-123");
        assert_eq!(path, PathBuf::from("/data/storage/tasks/abc-123/messages.json"));
    }

    #[test]
    fn test_api_messages_path() {
        let base = Path::new("/data/storage");
        let path = api_messages_path(base, "abc-123");
        assert_eq!(path, PathBuf::from("/data/storage/tasks/abc-123/api_conversation_history.json"));
    }

    #[test]
    fn test_metadata_path() {
        let base = Path::new("/data/storage");
        let path = metadata_path(base, "abc-123");
        assert_eq!(path, PathBuf::from("/data/storage/tasks/abc-123/meta.json"));
    }

    #[test]
    fn test_ensure_task_dir_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let fs = OsFileSystem;

        let result = ensure_task_dir(&fs, base, "test-task-1").unwrap();
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn test_ensure_task_dir_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let fs = OsFileSystem;

        ensure_task_dir(&fs, base, "test-task-2").unwrap();
        let result = ensure_task_dir(&fs, base, "test-task-2").unwrap();
        assert!(result.exists());
    }

    #[test]
    fn test_os_filesystem_read_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let fs = OsFileSystem;

        fs.write_file(&path, "hello world").unwrap();
        let content = fs.read_file(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_os_filesystem_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exists.txt");
        let fs = OsFileSystem;

        assert!(!fs.file_exists(&path).unwrap());
        fs.write_file(&path, "data").unwrap();
        assert!(fs.file_exists(&path).unwrap());
    }

    #[test]
    fn test_os_filesystem_dir_size() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        // Create some files
        fs.write_file(&dir.path().join("a.txt"), "12345").unwrap(); // 5 bytes
        fs.write_file(&dir.path().join("b.txt"), "1234567890").unwrap(); // 10 bytes

        let size = fs.dir_size(dir.path()).unwrap();
        assert_eq!(size, 15);
    }

    #[test]
    fn test_os_filesystem_dir_size_nonexistent() {
        let fs = OsFileSystem;
        let size = fs.dir_size(Path::new("/nonexistent/path")).unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn test_os_filesystem_remove_dir_all() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("to_remove");
        let fs = OsFileSystem;

        fs.create_dir_all(&subdir).unwrap();
        fs.write_file(&subdir.join("file.txt"), "data").unwrap();

        fs.remove_dir_all(&subdir).unwrap();
        assert!(!subdir.exists());
    }

    #[test]
    fn test_os_filesystem_remove_dir_all_nonexistent() {
        let fs = OsFileSystem;
        // Should not error on nonexistent path
        fs.remove_dir_all(Path::new("/nonexistent/path")).unwrap();
    }

    #[test]
    fn test_os_filesystem_read_dir() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        fs.write_file(&dir.path().join("file1.txt"), "a").unwrap();
        fs.write_file(&dir.path().join("file2.txt"), "b").unwrap();

        let entries = fs.read_dir(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_os_filesystem_read_dir_nonexistent() {
        let fs = OsFileSystem;
        let entries = fs.read_dir(Path::new("/nonexistent/path")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_os_filesystem_create_dir_all() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        let fs = OsFileSystem;

        fs.create_dir_all(&nested).unwrap();
        assert!(nested.exists());
    }
}
