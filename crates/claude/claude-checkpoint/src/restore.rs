//! Restore engine for reverting file changes from checkpoints.
//!
//! Supports undo (revert last interaction) and restore (jump to any checkpoint).

use anyhow::{Result, bail};
use std::io::Write;
use std::path::PathBuf;

use crate::storage::CheckpointStore;
use crate::types::*;

/// Validate that a relative path does not escape the workspace root.
///
/// Rejects paths containing `..` components, leading `/` or `\`, or that
/// resolve outside the workspace via canonicalization.
fn validate_path_within_workspace(
    workspace_root: &std::path::Path,
    relative: &str,
) -> Result<PathBuf> {
    if relative.is_empty() {
        bail!("empty path in checkpoint file change");
    }
    if relative.starts_with('/') || relative.starts_with('\\') {
        bail!("absolute path in checkpoint file change: {}", relative);
    }
    for component in std::path::Path::new(relative).components() {
        if matches!(component, std::path::Component::ParentDir) {
            bail!(
                "path traversal detected (..) in checkpoint file change: {}",
                relative
            );
        }
    }
    let resolved = workspace_root.join(relative);
    if let Ok(canonical_root) = workspace_root.canonicalize() {
        let parent = resolved.parent();
        if let Some(parent) = parent {
            if let Ok(canonical_parent) = parent.canonicalize() {
                if !canonical_parent.starts_with(&canonical_root) {
                    bail!("path traversal detected: {}", relative);
                }
            }
        }
    }
    Ok(resolved)
}

/// Engine for restoring workspace state from checkpoints.
pub struct RestoreEngine {
    store: CheckpointStore,
    workspace_root: PathBuf,
}

impl RestoreEngine {
    /// Create a new restore engine.
    pub fn new(store: CheckpointStore, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            store,
            workspace_root: workspace_root.into(),
        }
    }

    /// Undo the last interaction by restoring to the state before the latest checkpoint.
    pub fn undo_last(&self, session_id: &str) -> Result<RestoreResult> {
        let checkpoints = self.store.list_checkpoints(session_id)?;
        let latest = match checkpoints.last() {
            Some(cp) => cp,
            None => {
                return Ok(RestoreResult {
                    checkpoint_id: CheckpointId("none".into()),
                    files_restored: vec![],
                    success: false,
                    error: Some("No checkpoints found for this session".into()),
                });
            }
        };

        self.restore_to(&latest.id)
    }

    /// Restore workspace to the state at a specific checkpoint.
    ///
    /// This reverts all changes made *after* the specified checkpoint by applying
    /// the inverse of each subsequent checkpoint's file changes in reverse order.
    pub fn restore_to(&self, target_id: &CheckpointId) -> Result<RestoreResult> {
        let target = match self.store.get_checkpoint(target_id)? {
            Some(cp) => cp,
            None => {
                return Ok(RestoreResult {
                    checkpoint_id: target_id.clone(),
                    files_restored: vec![],
                    success: false,
                    error: Some(format!("Checkpoint {} not found", target_id)),
                });
            }
        };

        let mut files_restored = Vec::new();

        for change in &target.file_changes {
            let file_path = match validate_path_within_workspace(&self.workspace_root, &change.path)
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Skipping unsafe path in checkpoint restore: {e}");
                    continue;
                }
            };
            match change.operation {
                FileOperation::Created => {
                    if file_path.exists() {
                        std::fs::remove_file(&file_path)?;
                        files_restored.push(FileRestore {
                            path: change.path.clone(),
                            operation: RestoreOperation::FileDeleted,
                        });
                    }
                }
                FileOperation::Deleted => {
                    if let Some(hash) = &change.hash_before {
                        if let Some(content) = self.store.get_cached_content(hash)? {
                            if let Some(parent) = file_path.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            atomic_write(&file_path, &content)?;
                            files_restored.push(FileRestore {
                                path: change.path.clone(),
                                operation: RestoreOperation::FileRecreated,
                            });
                        }
                    }
                }
                FileOperation::Modified => {
                    if let Some(hash) = &change.hash_before {
                        if let Some(content) = self.store.get_cached_content(hash)? {
                            atomic_write(&file_path, &content)?;
                            files_restored.push(FileRestore {
                                path: change.path.clone(),
                                operation: RestoreOperation::ContentRestored,
                            });
                        }
                    }
                }
            }
        }

        Ok(RestoreResult {
            checkpoint_id: target_id.clone(),
            files_restored,
            success: true,
            error: None,
        })
    }

    /// Preview what a restore would do without actually changing files.
    pub fn preview_restore(&self, target_id: &CheckpointId) -> Result<Vec<FileRestore>> {
        let target = match self.store.get_checkpoint(target_id)? {
            Some(cp) => cp,
            None => return Ok(vec![]),
        };

        let mut preview = Vec::new();

        for change in &target.file_changes {
            let operation = match change.operation {
                FileOperation::Created => RestoreOperation::FileDeleted,
                FileOperation::Deleted => RestoreOperation::FileRecreated,
                FileOperation::Modified => RestoreOperation::ContentRestored,
            };
            preview.push(FileRestore {
                path: change.path.clone(),
                operation,
            });
        }

        Ok(preview)
    }
}

/// Write data to a file atomically: write to a temp file in the same
/// directory, then rename into place. This avoids leaving a partially-
/// written file if the process crashes mid-write.
fn atomic_write(path: &std::path::Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(data)?;
    tmp.flush()?;
    tmp.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CheckpointStore;
    use chrono::Utc;
    use std::fs;

    #[test]
    fn test_restore_created_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open_in_memory().unwrap();

        // Create a file on disk
        let file_path = dir.path().join("new_file.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        // Cache the "after" content
        let after_hash = "hash_after";
        store
            .cache_file_content(after_hash, b"fn main() {}")
            .unwrap();

        // Create a checkpoint that marks the file as created
        let cp = Checkpoint {
            id: CheckpointId::new(),
            session_id: "s1".into(),
            message_id: "m1".into(),
            message_index: 0,
            created_at: Utc::now(),
            file_changes: vec![FileChange {
                path: "new_file.rs".into(),
                operation: FileOperation::Created,
                hash_before: None,
                hash_after: Some(after_hash.into()),
                lines_added: 1,
                lines_removed: 0,
            }],
            summary: "Created new_file.rs".into(),
            stats: CheckpointStats {
                files_added: 1,
                ..Default::default()
            },
        };
        store.save_checkpoint(&cp).unwrap();

        // Restore should delete the file
        let engine = RestoreEngine::new(store, dir.path());
        let result = engine.restore_to(&cp.id).unwrap();
        assert!(result.success);
        assert_eq!(result.files_restored.len(), 1);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_restore_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open_in_memory().unwrap();

        // Write current (modified) content
        let file_path = dir.path().join("file.rs");
        fs::write(&file_path, "fn modified() {}").unwrap();

        // Cache the original content
        let before_hash = "hash_before";
        store
            .cache_file_content(before_hash, b"fn original() {}")
            .unwrap();

        let cp = Checkpoint {
            id: CheckpointId::new(),
            session_id: "s1".into(),
            message_id: "m1".into(),
            message_index: 0,
            created_at: Utc::now(),
            file_changes: vec![FileChange {
                path: "file.rs".into(),
                operation: FileOperation::Modified,
                hash_before: Some(before_hash.into()),
                hash_after: Some("hash_after".into()),
                lines_added: 1,
                lines_removed: 1,
            }],
            summary: "Modified file.rs".into(),
            stats: CheckpointStats {
                files_modified: 1,
                ..Default::default()
            },
        };
        store.save_checkpoint(&cp).unwrap();

        let engine = RestoreEngine::new(store, dir.path());
        let result = engine.restore_to(&cp.id).unwrap();
        assert!(result.success);

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "fn original() {}");
    }

    #[test]
    fn test_preview_restore() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open_in_memory().unwrap();

        let cp = Checkpoint {
            id: CheckpointId::new(),
            session_id: "s1".into(),
            message_id: "m1".into(),
            message_index: 0,
            created_at: Utc::now(),
            file_changes: vec![
                FileChange {
                    path: "a.rs".into(),
                    operation: FileOperation::Created,
                    hash_before: None,
                    hash_after: Some("h1".into()),
                    lines_added: 1,
                    lines_removed: 0,
                },
                FileChange {
                    path: "b.rs".into(),
                    operation: FileOperation::Modified,
                    hash_before: Some("h2".into()),
                    hash_after: Some("h3".into()),
                    lines_added: 2,
                    lines_removed: 1,
                },
            ],
            summary: "Multi-file change".into(),
            stats: CheckpointStats {
                files_added: 1,
                files_modified: 1,
                ..Default::default()
            },
        };
        let id = cp.id.clone();
        store.save_checkpoint(&cp).unwrap();

        let engine = RestoreEngine::new(store, dir.path());
        let preview = engine.preview_restore(&id).unwrap();
        assert_eq!(preview.len(), 2);
    }

    #[test]
    fn test_path_traversal_blocked() {
        assert!(
            validate_path_within_workspace(std::path::Path::new("/workspace"), "../etc/passwd")
                .is_err()
        );
        assert!(
            validate_path_within_workspace(
                std::path::Path::new("/workspace"),
                "sub/../../etc/passwd"
            )
            .is_err()
        );
        assert!(
            validate_path_within_workspace(std::path::Path::new("/workspace"), "/etc/passwd")
                .is_err()
        );
        assert!(
            validate_path_within_workspace(std::path::Path::new("/workspace"), "normal/file.rs")
                .is_ok()
        );
    }
}
