//! Team memory synchronization service.

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;

use crate::types::{MemoryEntry, MemoryType, SyncStatus};

/// Service responsible for synchronizing memory entries across a team.
///
/// Maintains local and remote stores and provides conflict resolution
/// strategies for merging divergent memory states.
pub struct TeamMemoryService {
    local: HashMap<String, MemoryEntry>,
    remote: HashMap<String, MemoryEntry>,
    status: SyncStatus,
}

impl TeamMemoryService {
    /// Create a new team memory service with empty stores.
    pub fn new() -> Self {
        Self {
            local: HashMap::new(),
            remote: HashMap::new(),
            status: SyncStatus::default(),
        }
    }

    /// Add an entry to the local store.
    pub fn add_local(&mut self, entry: MemoryEntry) {
        self.local.insert(entry.id.clone(), entry);
        self.status.pending_changes += 1;
    }

    /// Push local entries to the remote team store.
    ///
    /// Returns the updated sync status.
    pub fn sync_to_team(&mut self, entries: &[MemoryEntry]) -> Result<SyncStatus> {
        for entry in entries {
            self.remote.insert(entry.id.clone(), entry.clone());
            self.local.insert(entry.id.clone(), entry.clone());
        }
        self.status.last_sync = Some(Utc::now());
        self.status.pending_changes = 0;
        Ok(self.status.clone())
    }

    /// Pull remote entries into the local store.
    ///
    /// Returns the list of entries fetched from the remote store.
    pub fn sync_from_team(&mut self) -> Result<Vec<MemoryEntry>> {
        let entries: Vec<MemoryEntry> = self.remote.values().cloned().collect();
        for entry in &entries {
            self.local.insert(entry.id.clone(), entry.clone());
        }
        self.status.last_sync = Some(Utc::now());
        Ok(entries)
    }

    /// Resolve conflicts between local and remote entries using last-write-wins.
    ///
    /// For each pair of (local, remote) entries sharing the same ID, the entry
    /// with the later `updated_at` timestamp wins.
    pub fn resolve_conflicts(local: &[MemoryEntry], remote: &[MemoryEntry]) -> Vec<MemoryEntry> {
        let mut merged: HashMap<String, MemoryEntry> = HashMap::new();

        for entry in local {
            merged.insert(entry.id.clone(), entry.clone());
        }

        for entry in remote {
            if let Some(existing) = merged.get(&entry.id) {
                if entry.updated_at > existing.updated_at {
                    merged.insert(entry.id.clone(), entry.clone());
                }
            } else {
                merged.insert(entry.id.clone(), entry.clone());
            }
        }

        merged.into_values().collect()
    }

    /// Merge memories from two sources with deduplication.
    ///
    /// Entries with the same content and type are deduplicated, keeping the
    /// one with the most recent `updated_at`.
    pub fn merge_memories(local: &[MemoryEntry], remote: &[MemoryEntry]) -> Vec<MemoryEntry> {
        // Key by (content, memory_type) for dedup
        let mut dedup: HashMap<(String, MemoryType), MemoryEntry> = HashMap::new();

        for entry in local.iter().chain(remote.iter()) {
            let key = (entry.content.clone(), entry.memory_type);
            match dedup.get(&key) {
                Some(existing) if existing.updated_at >= entry.updated_at => {}
                _ => {
                    dedup.insert(key, entry.clone());
                }
            }
        }

        dedup.into_values().collect()
    }

    /// Get the current sync status.
    pub fn status(&self) -> &SyncStatus {
        &self.status
    }

    /// Get all local entries.
    pub fn local_entries(&self) -> Vec<MemoryEntry> {
        self.local.values().cloned().collect()
    }

    /// Get all remote entries.
    pub fn remote_entries(&self) -> Vec<MemoryEntry> {
        self.remote.values().cloned().collect()
    }
}

impl Default for TeamMemoryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, content: &str, mt: MemoryType) -> MemoryEntry {
        MemoryEntry::new(id, content, mt)
    }

    #[test]
    fn new_service_is_empty() {
        let svc = TeamMemoryService::new();
        assert!(svc.local_entries().is_empty());
        assert!(svc.remote_entries().is_empty());
        assert_eq!(svc.status().pending_changes, 0);
    }

    #[test]
    fn add_local_increments_pending() {
        let mut svc = TeamMemoryService::new();
        svc.add_local(make_entry("1", "hello", MemoryType::Team));
        assert_eq!(svc.status().pending_changes, 1);
        assert_eq!(svc.local_entries().len(), 1);
    }

    #[test]
    fn sync_to_team_clears_pending() {
        let mut svc = TeamMemoryService::new();
        svc.add_local(make_entry("a", "data", MemoryType::Team));
        let status = svc.sync_to_team(&svc.local_entries()).expect("sync");
        assert_eq!(status.pending_changes, 0);
        assert!(status.last_sync.is_some());
    }

    #[test]
    fn sync_from_team_pulls_entries() {
        let mut svc = TeamMemoryService::new();
        let entry = make_entry("r1", "remote data", MemoryType::Team);
        svc.remote.insert("r1".to_owned(), entry);

        let entries = svc.sync_from_team().expect("sync");
        assert_eq!(entries.len(), 1);
        assert_eq!(svc.local_entries().len(), 1);
    }

    #[test]
    fn resolve_conflicts_last_write_wins() {
        let mut local_entry = make_entry("c1", "local version", MemoryType::Team);
        local_entry.updated_at = Utc::now();

        let mut remote_entry = make_entry("c1", "remote version", MemoryType::Team);
        // Make remote newer
        remote_entry.updated_at = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        remote_entry.touch();

        let resolved =
            TeamMemoryService::resolve_conflicts(&[local_entry.clone()], &[remote_entry.clone()]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].content, "remote version");
    }

    #[test]
    fn resolve_conflicts_keeps_both_when_ids_differ() {
        let local = make_entry("l1", "local", MemoryType::Team);
        let remote = make_entry("r1", "remote", MemoryType::Team);
        let resolved = TeamMemoryService::resolve_conflicts(&[local], &[remote]);
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn merge_memories_deduplicates() {
        let e1 = make_entry("1", "same content", MemoryType::Team);
        let e2 = make_entry("2", "same content", MemoryType::Team);
        let merged = TeamMemoryService::merge_memories(&[e1], &[e2]);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_memories_keeps_different_content() {
        let e1 = make_entry("1", "content A", MemoryType::Team);
        let e2 = make_entry("2", "content B", MemoryType::Team);
        let merged = TeamMemoryService::merge_memories(&[e1], &[e2]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_memories_different_types_not_deduped() {
        let e1 = make_entry("1", "content", MemoryType::Team);
        let e2 = make_entry("2", "content", MemoryType::Project);
        let merged = TeamMemoryService::merge_memories(&[e1], &[e2]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn default_trait() {
        let svc = TeamMemoryService::default();
        assert!(svc.local_entries().is_empty());
    }
}
