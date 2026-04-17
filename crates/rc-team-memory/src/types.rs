//! Core types for the team memory system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The kind of memory scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Ephemeral per-session memory.
    Session,
    /// Shared across a team.
    Team,
    /// Project-scoped memory.
    Project,
    /// User-specific memory.
    User,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Session => write!(f, "session"),
            Self::Team => write!(f, "team"),
            Self::Project => write!(f, "project"),
            Self::User => write!(f, "user"),
        }
    }
}

/// A single memory entry stored in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier for this memory entry.
    pub id: String,
    /// The textual content of the memory.
    pub content: String,
    /// The scope / type of this memory.
    pub memory_type: MemoryType,
    /// When this entry was first created.
    pub created_at: DateTime<Utc>,
    /// When this entry was last updated.
    pub updated_at: DateTime<Utc>,
    /// Provenance — where this memory originated.
    pub source: String,
}

impl MemoryEntry {
    /// Create a new memory entry with the current timestamp.
    pub fn new(id: impl Into<String>, content: impl Into<String>, memory_type: MemoryType) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            content: content.into(),
            memory_type,
            created_at: now,
            updated_at: now,
            source: String::new(),
        }
    }

    /// Builder-style setter for `source`.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Touch the `updated_at` timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

/// Versioning metadata for a memory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersion {
    /// Monotonically increasing version number.
    pub version: u64,
    /// When this version was created.
    pub timestamp: DateTime<Utc>,
    /// Checksum of the memory payload (SHA-256 hex).
    pub checksum: String,
}

/// Current synchronization status between local and remote memory stores.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Timestamp of the last successful sync.
    pub last_sync: Option<DateTime<Utc>>,
    /// Number of locally pending changes not yet pushed.
    pub pending_changes: usize,
    /// Number of unresolved conflicts.
    pub conflict_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_type_display() {
        assert_eq!(MemoryType::Session.to_string(), "session");
        assert_eq!(MemoryType::Team.to_string(), "team");
        assert_eq!(MemoryType::Project.to_string(), "project");
        assert_eq!(MemoryType::User.to_string(), "user");
    }

    #[test]
    fn memory_entry_new_sets_timestamps() {
        let entry = MemoryEntry::new("id-1", "hello", MemoryType::Session);
        assert_eq!(entry.id, "id-1");
        assert_eq!(entry.content, "hello");
        assert_eq!(entry.memory_type, MemoryType::Session);
        assert!(entry.source.is_empty());
        assert!(entry.created_at <= entry.updated_at);
    }

    #[test]
    fn memory_entry_with_source() {
        let entry = MemoryEntry::new("id-2", "content", MemoryType::Team).with_source("test");
        assert_eq!(entry.source, "test");
    }

    #[test]
    fn memory_entry_touch_updates_updated_at() {
        let mut entry = MemoryEntry::new("id-3", "data", MemoryType::Project);
        let before = entry.updated_at;
        entry.touch();
        assert!(entry.updated_at >= before);
    }

    #[test]
    fn sync_status_default() {
        let status = SyncStatus::default();
        assert!(status.last_sync.is_none());
        assert_eq!(status.pending_changes, 0);
        assert_eq!(status.conflict_count, 0);
    }

    #[test]
    fn serde_roundtrip_memory_entry() {
        let entry = MemoryEntry::new("s-1", "serialized", MemoryType::User).with_source("test");
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: MemoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, entry.id);
        assert_eq!(back.content, entry.content);
        assert_eq!(back.memory_type, entry.memory_type);
    }

    #[test]
    fn serde_roundtrip_memory_type() {
        for mt in [
            MemoryType::Session,
            MemoryType::Team,
            MemoryType::Project,
            MemoryType::User,
        ] {
            let json = serde_json::to_string(&mt).expect("serialize");
            let back: MemoryType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(mt, back);
        }
    }
}
