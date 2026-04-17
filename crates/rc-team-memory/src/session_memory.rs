//! Session memory store — in-memory storage with optional persistence.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;
use regex::Regex;

use crate::types::{MemoryEntry, MemoryType};

/// In-memory session memory store.
///
/// Stores conversation-derived memories keyed by session ID. Thread-safe via
/// an internal `Mutex`.
pub struct SessionMemoryStore {
    entries: Mutex<HashMap<String, MemoryEntry>>,
}

impl SessionMemoryStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Store content associated with a session ID.
    ///
    /// If a memory already exists for the given session it is updated;
    /// otherwise a new entry is created.
    pub fn store(&self, session_id: &str, content: &str) -> Result<()> {
        let mut entries = self.entries.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        if let Some(existing) = entries.get_mut(session_id) {
            existing.content = content.to_owned();
            existing.touch();
        } else {
            let entry = MemoryEntry::new(session_id, content, MemoryType::Session);
            entries.insert(session_id.to_owned(), entry);
        }
        Ok(())
    }

    /// Retrieve stored content for a session.
    pub fn retrieve(&self, session_id: &str) -> Result<Option<String>> {
        let entries = self.entries.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        Ok(entries.get(session_id).map(|e| e.content.clone()))
    }

    /// List all stored session memory entries.
    pub fn list_sessions(&self) -> Result<Vec<MemoryEntry>> {
        let entries = self.entries.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        Ok(entries.values().cloned().collect())
    }

    /// Remove a session entry.
    pub fn remove(&self, session_id: &str) -> Result<bool> {
        let mut entries = self.entries.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        Ok(entries.remove(session_id).is_some())
    }

    /// Clear all session entries.
    pub fn clear(&self) -> Result<()> {
        let mut entries = self.entries.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        entries.clear();
        Ok(())
    }

    /// Number of stored sessions.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SessionMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract key memories from a conversation string.
///
/// Uses simple heuristics to identify important statements:
/// - Lines containing "remember" or "important"
/// - Lines that look like facts (start with a capital, end with period)
/// - Action items (contain "TODO", "FIXME", "ACTION")
pub fn extract_memories(conversation: &str) -> Vec<String> {
    let mut memories = Vec::new();

    // Patterns for memory-worthy content
    let remember_re = Regex::new(r"(?i)\b(remember|note|keep in mind|important)\b")
        .expect("valid regex");
    let action_re = Regex::new(r"(?i)\b(TODO|FIXME|ACTION|HACK|XXX)\b").expect("valid regex");
    let fact_re = Regex::new(r"^[A-Z][^.]*\.$").expect("valid regex");

    for line in conversation.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let is_memory = remember_re.is_match(trimmed)
            || action_re.is_match(trimmed)
            || (fact_re.is_match(trimmed) && trimmed.len() > 10);

        if is_memory {
            memories.push(trimmed.to_owned());
        }
    }

    memories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_retrieve() {
        let store = SessionMemoryStore::new();
        assert!(store.retrieve("s1").expect("retrieve").is_none());

        store.store("s1", "hello world").expect("store");
        let content = store.retrieve("s1").expect("retrieve").expect("some");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn store_updates_existing() {
        let store = SessionMemoryStore::new();
        store.store("s1", "v1").expect("store");
        store.store("s1", "v2").expect("store");
        assert_eq!(store.retrieve("s1").expect("r").expect("some"), "v2");
    }

    #[test]
    fn list_sessions_returns_all() {
        let store = SessionMemoryStore::new();
        store.store("a", "alpha").expect("store");
        store.store("b", "beta").expect("store");
        let list = store.list_sessions().expect("list");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn remove_session() {
        let store = SessionMemoryStore::new();
        store.store("x", "data").expect("store");
        assert!(store.remove("x").expect("remove"));
        assert!(!store.remove("x").expect("remove again"));
        assert!(store.retrieve("x").expect("retrieve").is_none());
    }

    #[test]
    fn clear_empties_store() {
        let store = SessionMemoryStore::new();
        store.store("a", "a").expect("store");
        store.store("b", "b").expect("store");
        store.clear().expect("clear");
        assert!(store.is_empty());
    }

    #[test]
    fn len_and_is_empty() {
        let store = SessionMemoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        store.store("1", "x").expect("store");
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn extract_memories_remember() {
        let conv = "Please remember to use tabs.\njust a normal line here\nImportant: always commit.";
        let mems = extract_memories(conv);
        assert_eq!(mems.len(), 2);
        assert!(mems[0].contains("remember"));
        assert!(mems[1].contains("Important"));
    }

    #[test]
    fn extract_memories_action_items() {
        let conv = "TODO: fix the bug\nFIXME: refactor this\nNormal line here";
        let mems = extract_memories(conv);
        assert_eq!(mems.len(), 2);
    }

    #[test]
    fn extract_memories_facts() {
        let conv = "The project uses Rust for the backend.\nshort.\nAnother normal line";
        let mems = extract_memories(conv);
        assert_eq!(mems.len(), 1);
        assert!(mems[0].contains("Rust"));
    }

    #[test]
    fn extract_memories_empty() {
        let mems = extract_memories("");
        assert!(mems.is_empty());
    }

    #[test]
    fn default_trait() {
        let store = SessionMemoryStore::default();
        assert!(store.is_empty());
    }
}
