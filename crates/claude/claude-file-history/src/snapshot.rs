//! File history snapshot types.
//!
//! Corresponds to `src/utils/fileHistory.ts` (FileHistorySnapshot type, lines 39–43).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::backup::BackupRecord;

/// Maximum number of snapshots to retain before evicting the oldest.
pub const MAX_SNAPSHOTS: usize = 100;

/// A point-in-time snapshot of all tracked file backups.
///
/// Each snapshot is associated with a specific message ID and captures
/// the backup state of every tracked file at that moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistorySnapshot {
    /// The message ID that triggered this snapshot.
    pub message_id: String,
    /// Map of tracking paths to their backup records at this point in time.
    pub tracked_file_backups: HashMap<String, BackupRecord>,
    /// When this snapshot was created.
    pub timestamp: DateTime<Utc>,
}

impl FileHistorySnapshot {
    /// Create a new empty snapshot associated with the given message ID.
    #[must_use]
    pub fn new(message_id: String) -> Self {
        Self {
            message_id,
            tracked_file_backups: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Create a snapshot with pre-populated backups.
    #[must_use]
    pub fn with_backups(message_id: String, backups: HashMap<String, BackupRecord>) -> Self {
        Self {
            message_id,
            tracked_file_backups: backups,
            timestamp: Utc::now(),
        }
    }

    /// Get the backup record for a specific file path, if tracked.
    #[must_use]
    pub fn get_backup(&self, file_path: &str) -> Option<&BackupRecord> {
        self.tracked_file_backups.get(file_path)
    }

    /// Insert or update a backup record for a file path.
    pub fn set_backup(&mut self, file_path: String, backup: BackupRecord) {
        self.tracked_file_backups.insert(file_path, backup);
    }

    /// Check if a file path is tracked in this snapshot.
    #[must_use]
    pub fn has_file(&self, file_path: &str) -> bool {
        self.tracked_file_backups.contains_key(file_path)
    }

    /// Get the number of files tracked in this snapshot.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.tracked_file_backups.len()
    }

    /// Get all tracked file paths in this snapshot.
    pub fn tracked_paths(&self) -> impl Iterator<Item = &String> {
        self.tracked_file_backups.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BackupRecord;

    fn make_backup(version: u32) -> BackupRecord {
        BackupRecord {
            file_path: "test.txt".to_string(),
            version,
            backup_time: Utc::now(),
            content_hash: Some(format!("hash-v{version}")),
        }
    }

    #[test]
    fn new_snapshot_is_empty() {
        let snap = FileHistorySnapshot::new("msg-1".to_string());
        assert_eq!(snap.message_id, "msg-1");
        assert!(snap.tracked_file_backups.is_empty());
        assert_eq!(snap.file_count(), 0);
    }

    #[test]
    fn with_backups_populates() {
        let mut backups = HashMap::new();
        backups.insert("a.txt".to_string(), make_backup(1));
        backups.insert("b.txt".to_string(), make_backup(2));

        let snap = FileHistorySnapshot::with_backups("msg-2".to_string(), backups);
        assert_eq!(snap.file_count(), 2);
        assert!(snap.has_file("a.txt"));
        assert!(snap.has_file("b.txt"));
    }

    #[test]
    fn get_set_backup() {
        let mut snap = FileHistorySnapshot::new("msg-1".to_string());
        assert!(!snap.has_file("test.txt"));

        snap.set_backup("test.txt".to_string(), make_backup(1));
        assert!(snap.has_file("test.txt"));

        let backup = snap
            .get_backup("test.txt")
            .expect("backup should be present");
        assert_eq!(backup.version, 1);
    }

    #[test]
    fn tracked_paths_iterator() {
        let mut backups = HashMap::new();
        backups.insert("x.rs".to_string(), make_backup(1));
        backups.insert("y.rs".to_string(), make_backup(2));

        let snap = FileHistorySnapshot::with_backups("msg-1".to_string(), backups);
        let paths: Vec<&String> = snap.tracked_paths().collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn max_snapshots_constant() {
        assert_eq!(MAX_SNAPSHOTS, 100);
    }

    #[test]
    fn snapshot_serializes_to_json() {
        let mut snap = FileHistorySnapshot::new("msg-1".to_string());
        snap.set_backup("main.rs".to_string(), make_backup(1));

        let json = serde_json::to_string(&snap).expect("snapshot should serialize");
        assert!(json.contains("msg-1"));
        assert!(json.contains("main.rs"));

        let deserialized: FileHistorySnapshot =
            serde_json::from_str(&json).expect("snapshot should deserialize");
        assert_eq!(deserialized.message_id, "msg-1");
        assert_eq!(deserialized.file_count(), 1);
    }
}
