//! Workspace file snapshot scanner.
//!
//! Scans the workspace directory and records file hashes for change detection.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::types::{FileSnapshot, WorkspaceSnapshot};

/// Scans workspace files and creates snapshots for change detection.
pub struct SnapshotScanner {
    workspace_root: PathBuf,
    /// Patterns to exclude from scanning (e.g., node_modules, .git, target).
    exclude_patterns: Vec<String>,
}

impl SnapshotScanner {
    /// Create a new scanner for the given workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            exclude_patterns: Self::default_exclude_patterns(),
        }
    }

    /// Set custom exclude patterns.
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Scan the entire workspace and create a snapshot.
    pub fn scan(&self) -> Result<WorkspaceSnapshot> {
        let mut files = Vec::new();

        for entry in WalkDir::new(&self.workspace_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.should_exclude(e.path()))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let relative = path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if let Some(snapshot) = self.snapshot_file(path, &relative)? {
                files.push(snapshot);
            }
        }

        Ok(WorkspaceSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: chrono::Utc::now(),
            files,
        })
    }

    /// Snapshot a single file.
    fn snapshot_file(&self, path: &Path, relative_path: &str) -> Result<Option<FileSnapshot>> {
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        // Skip files larger than 10 MB to avoid excessive memory usage.
        if metadata.len() > 10 * 1024 * 1024 {
            return Ok(None);
        }

        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let hash = Self::hash_content(&content);

        Ok(Some(FileSnapshot {
            path: relative_path.to_string(),
            hash,
            size: metadata.len(),
        }))
    }

    /// Compute SHA256 hash of file content.
    pub fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }

    /// Compare two snapshots and return the set of changed files.
    pub fn diff_snapshots(
        &self,
        before: &WorkspaceSnapshot,
        after: &WorkspaceSnapshot,
    ) -> HashMap<String, FileChangeEntry> {
        let before_map: HashMap<&str, &FileSnapshot> =
            before.files.iter().map(|f| (f.path.as_str(), f)).collect();
        let after_map: HashMap<&str, &FileSnapshot> =
            after.files.iter().map(|f| (f.path.as_str(), f)).collect();

        let mut changes = HashMap::new();

        // Files in after but not in before → Created
        // Files in both but different hash → Modified
        for (path, after_snap) in &after_map {
            match before_map.get(path) {
                None => {
                    changes.insert(
                        (*path).to_string(),
                        FileChangeEntry::Created((*after_snap).clone()),
                    );
                }
                Some(before_snap) => {
                    if before_snap.hash != after_snap.hash {
                        changes.insert(
                            (*path).to_string(),
                            FileChangeEntry::Modified {
                                before: (*before_snap).clone(),
                                after: (*after_snap).clone(),
                            },
                        );
                    }
                }
            }
        }

        // Files in before but not in after → Deleted
        for (path, before_snap) in &before_map {
            if !after_map.contains_key(path) {
                changes.insert(
                    (*path).to_string(),
                    FileChangeEntry::Deleted((*before_snap).clone()),
                );
            }
        }

        changes
    }

    /// Check if a path should be excluded from scanning.
    fn should_exclude(&self, path: &Path) -> bool {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return false,
        };

        self.exclude_patterns
            .iter()
            .any(|pattern| file_name == *pattern)
    }

    /// Default exclude patterns for common directories.
    fn default_exclude_patterns() -> Vec<String> {
        vec![
            ".git".into(),
            "node_modules".into(),
            "target".into(),
            "__pycache__".into(),
            ".next".into(),
            ".nuxt".into(),
            "dist".into(),
            "build".into(),
            ".cache".into(),
            ".DS_Store".into(),
            "venv".into(),
            ".venv".into(),
        ]
    }
}

/// An entry representing a file change between two snapshots.
#[derive(Debug, Clone)]
pub enum FileChangeEntry {
    Created(FileSnapshot),
    Modified {
        before: FileSnapshot,
        after: FileSnapshot,
    },
    Deleted(FileSnapshot),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content_deterministic() {
        let content = b"hello world";
        let hash1 = SnapshotScanner::hash_content(content);
        let hash2 = SnapshotScanner::hash_content(content);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex length
    }

    #[test]
    fn test_hash_content_different() {
        let hash1 = SnapshotScanner::hash_content(b"hello");
        let hash2 = SnapshotScanner::hash_content(b"world");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_should_exclude() {
        let scanner = SnapshotScanner::new("/tmp/test");
        assert!(scanner.should_exclude(Path::new("/tmp/test/.git")));
        assert!(scanner.should_exclude(Path::new("/tmp/test/node_modules")));
        assert!(scanner.should_exclude(Path::new("/tmp/test/target")));
        assert!(!scanner.should_exclude(Path::new("/tmp/test/src")));
    }

    #[test]
    fn test_diff_snapshots_detects_creation() {
        let scanner = SnapshotScanner::new("/tmp/test");
        let before = WorkspaceSnapshot {
            id: "1".into(),
            created_at: chrono::Utc::now(),
            files: vec![],
        };
        let after = WorkspaceSnapshot {
            id: "2".into(),
            created_at: chrono::Utc::now(),
            files: vec![FileSnapshot {
                path: "new_file.rs".into(),
                hash: "abc123".into(),
                size: 100,
            }],
        };
        let changes = scanner.diff_snapshots(&before, &after);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes.get("new_file.rs"),
            Some(FileChangeEntry::Created(_))
        ));
    }

    #[test]
    fn test_diff_snapshots_detects_modification() {
        let scanner = SnapshotScanner::new("/tmp/test");
        let before = WorkspaceSnapshot {
            id: "1".into(),
            created_at: chrono::Utc::now(),
            files: vec![FileSnapshot {
                path: "file.rs".into(),
                hash: "old_hash".into(),
                size: 100,
            }],
        };
        let after = WorkspaceSnapshot {
            id: "2".into(),
            created_at: chrono::Utc::now(),
            files: vec![FileSnapshot {
                path: "file.rs".into(),
                hash: "new_hash".into(),
                size: 120,
            }],
        };
        let changes = scanner.diff_snapshots(&before, &after);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes.get("file.rs"),
            Some(FileChangeEntry::Modified { .. })
        ));
    }

    #[test]
    fn test_diff_snapshots_detects_deletion() {
        let scanner = SnapshotScanner::new("/tmp/test");
        let before = WorkspaceSnapshot {
            id: "1".into(),
            created_at: chrono::Utc::now(),
            files: vec![FileSnapshot {
                path: "old_file.rs".into(),
                hash: "abc".into(),
                size: 50,
            }],
        };
        let after = WorkspaceSnapshot {
            id: "2".into(),
            created_at: chrono::Utc::now(),
            files: vec![],
        };
        let changes = scanner.diff_snapshots(&before, &after);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes.get("old_file.rs"),
            Some(FileChangeEntry::Deleted(_))
        ));
    }

    #[test]
    fn test_diff_snapshots_no_changes() {
        let scanner = SnapshotScanner::new("/tmp/test");
        let snap = FileSnapshot {
            path: "file.rs".into(),
            hash: "same_hash".into(),
            size: 100,
        };
        let before = WorkspaceSnapshot {
            id: "1".into(),
            created_at: chrono::Utc::now(),
            files: vec![snap.clone()],
        };
        let after = WorkspaceSnapshot {
            id: "2".into(),
            created_at: chrono::Utc::now(),
            files: vec![snap],
        };
        let changes = scanner.diff_snapshots(&before, &after);
        assert!(changes.is_empty());
    }
}
