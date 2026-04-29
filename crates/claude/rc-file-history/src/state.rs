//! File history state management.
//!
//! Corresponds to `src/utils/fileHistory.ts` (FileHistoryState, FileHistorySnapshot).

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::backup::BackupRecord;
use crate::snapshot::FileHistorySnapshot;

/// Maximum number of snapshots to retain.
pub const MAX_SNAPSHOTS: usize = 100;

/// The state of the file history system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryState {
    /// All snapshots taken so far.
    pub snapshots: Vec<FileHistorySnapshot>,
    /// Set of file paths currently being tracked.
    pub tracked_files: HashSet<String>,
    /// Monotonically-increasing counter incremented on every snapshot.
    pub snapshot_sequence: u64,
}

impl Default for FileHistoryState {
    fn default() -> Self {
        Self::new()
    }
}

impl FileHistoryState {
    /// Create a new empty file history state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            tracked_files: HashSet::new(),
            snapshot_sequence: 0,
        }
    }

    /// Track a file for backup.
    pub fn track_file(&mut self, path: String) {
        self.tracked_files.insert(path);
    }

    /// Untrack a file.
    pub fn untrack_file(&mut self, path: &str) {
        self.tracked_files.remove(path);
    }

    /// Check if a file is being tracked.
    #[must_use]
    pub fn is_tracked(&self, path: &str) -> bool {
        self.tracked_files.contains(path)
    }

    /// Get the number of tracked files.
    #[must_use]
    pub fn tracked_file_count(&self) -> usize {
        self.tracked_files.len()
    }

    /// Create a new snapshot of all tracked files.
    pub fn create_snapshot(&mut self, message_id: String) {
        let mut tracked_backups = HashMap::new();

        for file_path in &self.tracked_files {
            let backup = BackupRecord {
                file_path: file_path.clone(),
                version: self.next_version_for_file(file_path),
                backup_time: Utc::now(),
                content_hash: None, // Will be filled when backup is actually created
            };
            tracked_backups.insert(file_path.clone(), backup);
        }

        let snapshot = FileHistorySnapshot {
            message_id,
            tracked_file_backups: tracked_backups,
            timestamp: Utc::now(),
        };

        self.snapshots.push(snapshot);
        self.snapshot_sequence += 1;

        // Evict oldest snapshots if over limit
        while self.snapshots.len() > MAX_SNAPSHOTS {
            self.snapshots.remove(0);
        }
    }

    /// Get the next version number for a file.
    fn next_version_for_file(&self, file_path: &str) -> u32 {
        let mut max_version = 0u32;
        for snapshot in &self.snapshots {
            if let Some(backup) = snapshot.tracked_file_backups.get(file_path) {
                max_version = max_version.max(backup.version);
            }
        }
        max_version + 1
    }

    /// Get a snapshot by index.
    #[must_use]
    pub fn get_snapshot(&self, index: usize) -> Option<&FileHistorySnapshot> {
        self.snapshots.get(index)
    }

    /// Get the most recent snapshot.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<&FileHistorySnapshot> {
        self.snapshots.last()
    }

    /// Get the number of snapshots.
    #[must_use]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Check if we can restore to a given snapshot index.
    #[must_use]
    pub fn can_restore(&self, snapshot_index: usize) -> bool {
        snapshot_index < self.snapshots.len()
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.tracked_files.clear();
        self.snapshot_sequence = 0;
    }
}

/// Check if file history can restore to a given snapshot.
#[must_use]
pub fn file_history_can_restore(state: &FileHistoryState, snapshot_index: usize) -> bool {
    state.can_restore(snapshot_index)
}

/// Rewind to a given snapshot, returning the files that need to be restored.
/// Does not actually perform file I/O — that's left to the caller.
#[must_use]
pub fn rewind_to_snapshot(
    state: &FileHistoryState,
    snapshot_index: usize,
) -> Option<Vec<BackupRecord>> {
    let snapshot = state.get_snapshot(snapshot_index)?;
    Some(snapshot.tracked_file_backups.values().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_empty() {
        let state = FileHistoryState::new();
        assert!(state.snapshots.is_empty());
        assert!(state.tracked_files.is_empty());
        assert_eq!(state.snapshot_sequence, 0);
    }

    #[test]
    fn track_and_check_files() {
        let mut state = FileHistoryState::new();
        state.track_file("src/main.rs".to_string());
        state.track_file("src/lib.rs".to_string());

        assert!(state.is_tracked("src/main.rs"));
        assert!(state.is_tracked("src/lib.rs"));
        assert!(!state.is_tracked("Cargo.toml"));
        assert_eq!(state.tracked_file_count(), 2);
    }

    #[test]
    fn untrack_file() {
        let mut state = FileHistoryState::new();
        state.track_file("src/main.rs".to_string());
        state.untrack_file("src/main.rs");
        assert!(!state.is_tracked("src/main.rs"));
    }

    #[test]
    fn create_snapshot() {
        let mut state = FileHistoryState::new();
        state.track_file("src/main.rs".to_string());
        state.create_snapshot("msg-1".to_string());

        assert_eq!(state.snapshot_count(), 1);
        assert_eq!(state.snapshot_sequence, 1);

        let snap = state.latest_snapshot().expect("snapshot should be created");
        assert_eq!(snap.message_id, "msg-1");
        assert!(snap.tracked_file_backups.contains_key("src/main.rs"));
    }

    #[test]
    fn version_increments() {
        let mut state = FileHistoryState::new();
        state.track_file("src/main.rs".to_string());

        state.create_snapshot("msg-1".to_string());
        state.create_snapshot("msg-2".to_string());

        let snap1 = state.get_snapshot(0).expect("first snapshot should exist");
        let snap2 = state.get_snapshot(1).expect("second snapshot should exist");

        let v1 = snap1
            .tracked_file_backups
            .get("src/main.rs")
            .expect("tracked backup should exist")
            .version;
        let v2 = snap2
            .tracked_file_backups
            .get("src/main.rs")
            .expect("tracked backup should exist")
            .version;

        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
    }

    #[test]
    fn snapshot_eviction() {
        let mut state = FileHistoryState::new();
        state.track_file("test.txt".to_string());

        for i in 0..150 {
            state.create_snapshot(format!("msg-{}", i));
        }

        assert!(state.snapshot_count() <= MAX_SNAPSHOTS);
        assert_eq!(state.snapshot_sequence, 150);
    }

    #[test]
    fn can_restore_checks() {
        let mut state = FileHistoryState::new();
        state.track_file("test.txt".to_string());
        state.create_snapshot("msg-1".to_string());

        assert!(state.can_restore(0));
        assert!(!state.can_restore(1));
    }

    #[test]
    fn rewind_returns_backups() {
        let mut state = FileHistoryState::new();
        state.track_file("a.txt".to_string());
        state.track_file("b.txt".to_string());
        state.create_snapshot("msg-1".to_string());

        let backups = rewind_to_snapshot(&state, 0).expect("snapshot rewind should succeed");
        assert_eq!(backups.len(), 2);
    }

    #[test]
    fn clear_resets_everything() {
        let mut state = FileHistoryState::new();
        state.track_file("test.txt".to_string());
        state.create_snapshot("msg-1".to_string());
        state.clear();

        assert!(state.snapshots.is_empty());
        assert!(state.tracked_files.is_empty());
        assert_eq!(state.snapshot_sequence, 0);
    }
}
