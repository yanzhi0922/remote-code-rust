//! Terminal registry for managing multiple terminal instances.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::terminal::{DefaultTerminal, RooTerminal, TerminalError};
use crate::types::TerminalId;

/// Registry for managing multiple terminal instances.
///
/// `TerminalRegistry` provides CRUD operations for terminal instances,
/// allowing creation, retrieval, and removal of terminals identified by
/// their unique [`TerminalId`].
#[derive(Debug, Clone)]
pub struct TerminalRegistry {
    terminals: Arc<Mutex<HashMap<TerminalId, Arc<Mutex<DefaultTerminal>>>>>,
    next_id: Arc<Mutex<u32>>,
}

impl TerminalRegistry {
    /// Create a new empty terminal registry.
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Allocate the next unique terminal ID.
    async fn allocate_id(&self) -> TerminalId {
        let mut next = self.next_id.lock().await;
        let id = *next;
        *next += 1;
        TerminalId::new(id)
    }

    /// Create a new terminal with the given working directory.
    ///
    /// Returns the [`TerminalId`] of the newly created terminal.
    pub async fn create_terminal(&self, cwd: impl Into<PathBuf>) -> TerminalId {
        let id = self.allocate_id().await;
        let terminal = DefaultTerminal::new(id, cwd);
        self.terminals
            .lock()
            .await
            .insert(id, Arc::new(Mutex::new(terminal)));
        id
    }

    /// Create a new terminal with a specific ID and working directory.
    ///
    /// Returns an error if a terminal with the given ID already exists.
    pub async fn create_terminal_with_id(
        &self,
        id: TerminalId,
        cwd: impl Into<PathBuf>,
    ) -> Result<TerminalId, TerminalError> {
        let mut terminals = self.terminals.lock().await;
        if terminals.contains_key(&id) {
            return Err(TerminalError::SpawnFailed {
                command: format!("terminal with id {id} already exists"),
                reason: "duplicate id".into(),
            });
        }
        let terminal = DefaultTerminal::new(id, cwd);
        terminals.insert(id, Arc::new(Mutex::new(terminal)));
        Ok(id)
    }

    /// Get a reference to a terminal by its ID.
    ///
    /// Returns `None` if no terminal with the given ID exists.
    pub async fn get_terminal(
        &self,
        id: TerminalId,
    ) -> Option<Arc<Mutex<DefaultTerminal>>> {
        self.terminals.lock().await.get(&id).cloned()
    }

    /// Remove a terminal from the registry.
    ///
    /// Returns `true` if the terminal was found and removed, `false` otherwise.
    pub async fn remove_terminal(&self, id: TerminalId) -> bool {
        if let Some(terminal_arc) = self.terminals.lock().await.remove(&id) {
            // Close the terminal before dropping
            if let Ok(mut terminal) = terminal_arc.try_lock() {
                terminal.close();
            }
            true
        } else {
            false
        }
    }

    /// Get or create a terminal with the given working directory.
    ///
    /// If a terminal with the given ID exists, returns it. Otherwise, creates
    /// a new terminal with the specified ID and working directory.
    pub async fn get_or_create_terminal(
        &self,
        id: TerminalId,
        cwd: impl Into<PathBuf>,
    ) -> Arc<Mutex<DefaultTerminal>> {
        let mut terminals = self.terminals.lock().await;
        if let Some(terminal) = terminals.get(&id) {
            return Arc::clone(terminal);
        }
        let terminal = DefaultTerminal::new(id, cwd);
        let arc = Arc::new(Mutex::new(terminal));
        terminals.insert(id, Arc::clone(&arc));
        arc
    }

    /// Get or create a terminal by working directory path.
    ///
    /// Corresponds to TS: `getOrCreateTerminal(cwd, taskId, provider)`.
    /// First looks for an existing non-busy terminal with matching cwd,
    /// then creates a new one if none is found.
    pub async fn get_or_create_terminal_by_cwd(
        &self,
        cwd: impl Into<PathBuf>,
        task_id: Option<String>,
    ) -> Arc<Mutex<DefaultTerminal>> {
        let cwd = cwd.into();
        let mut terminals = self.terminals.lock().await;

        // First priority: Find a terminal with matching task_id and cwd
        if let Some(ref tid) = task_id {
            for (_, terminal_arc) in terminals.iter() {
                let terminal = terminal_arc.lock().await;
                if !terminal.is_busy() && !terminal.is_closed() && terminal.get_cwd() == cwd {
                    if let Some(ref terminal_task_id) = terminal.task_id() {
                        if terminal_task_id == tid {
                            drop(terminal);
                            return Arc::clone(terminal_arc);
                        }
                    }
                }
            }
        }

        // Second priority: Find any available terminal with matching cwd
        for (_, terminal_arc) in terminals.iter() {
            let terminal = terminal_arc.lock().await;
            if !terminal.is_busy() && !terminal.is_closed() && terminal.get_cwd() == cwd {
                drop(terminal);
                return Arc::clone(terminal_arc);
            }
        }

        // No suitable terminal found, create a new one
        let id = self.allocate_id().await;
        let mut terminal = DefaultTerminal::new(id, cwd);
        if let Some(tid) = task_id {
            terminal.set_task_id(tid);
        }
        let arc = Arc::new(Mutex::new(terminal));
        terminals.insert(id, Arc::clone(&arc));
        arc
    }

    /// Get terminals filtered by busy state and optionally by task ID.
    ///
    /// Corresponds to TS: `getTerminals(busy, taskId?)`.
    pub async fn get_terminals(
        &self,
        busy: bool,
        task_id: Option<&str>,
    ) -> Vec<Arc<Mutex<DefaultTerminal>>> {
        let terminals = self.terminals.lock().await;
        let mut result = Vec::new();
        for (_, terminal_arc) in terminals.iter() {
            let terminal = terminal_arc.lock().await;
            if terminal.is_busy() != busy || terminal.is_closed() {
                continue;
            }
            if let Some(tid) = task_id {
                if let Some(ref terminal_tid) = terminal.task_id() {
                    if *terminal_tid != tid {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            drop(terminal);
            result.push(Arc::clone(terminal_arc));
        }
        result
    }

    /// Get background terminals (no task_id) that have unretrieved output
    /// or are still running.
    ///
    /// Corresponds to TS: `getBackgroundTerminals(busy?)`.
    pub async fn get_background_terminals(
        &self,
        busy: Option<bool>,
    ) -> Vec<Arc<Mutex<DefaultTerminal>>> {
        let terminals = self.terminals.lock().await;
        let mut result = Vec::new();
        for (_, terminal_arc) in terminals.iter() {
            let terminal = terminal_arc.lock().await;
            // Only background terminals (no task_id)
            if terminal.task_id().is_some() || terminal.is_closed() {
                continue;
            }
            if let Some(b) = busy {
                if terminal.is_busy() != b {
                    continue;
                }
            }
            drop(terminal);
            result.push(Arc::clone(terminal_arc));
        }
        result
    }

    /// Release all terminals associated with a task.
    ///
    /// Corresponds to TS: `releaseTerminalsForTask(taskId)`.
    pub async fn release_terminals_for_task(&self, task_id: &str) {
        let terminals = self.terminals.lock().await;
        for (_, terminal_arc) in terminals.iter() {
            let mut terminal = terminal_arc.lock().await;
            if let Some(ref tid) = terminal.task_id() {
                if *tid == task_id {
                    terminal.set_task_id(String::new());
                }
            }
        }
    }

    /// Get the number of registered terminals.
    pub async fn len(&self) -> usize {
        self.terminals.lock().await.len()
    }

    /// Check if the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.terminals.lock().await.is_empty()
    }

    /// Check if a terminal with the given ID exists.
    pub async fn contains(&self, id: TerminalId) -> bool {
        self.terminals.lock().await.contains_key(&id)
    }

    /// Close and remove all terminals from the registry.
    pub async fn clear(&self) {
        let mut terminals = self.terminals.lock().await;
        for (_, terminal_arc) in terminals.drain() {
            if let Ok(mut terminal) = terminal_arc.try_lock() {
                terminal.close();
            }
        }
    }

    /// Get all terminal IDs currently in the registry.
    pub async fn ids(&self) -> Vec<TerminalId> {
        self.terminals.lock().await.keys().copied().collect()
    }
}

impl Default for TerminalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_registry_new() {
        let registry = TerminalRegistry::new();
        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn test_registry_create_terminal() {
        let registry = TerminalRegistry::new();
        let id = registry.create_terminal("/tmp").await;
        assert_eq!(id, TerminalId::new(1));
        assert!(!registry.is_empty().await);
        assert_eq!(registry.len().await, 1);
        assert!(registry.contains(id).await);
    }

    #[tokio::test]
    async fn test_registry_create_multiple_terminals() {
        let registry = TerminalRegistry::new();
        let id1 = registry.create_terminal("/tmp").await;
        let id2 = registry.create_terminal("/var").await;
        assert_ne!(id1, id2);
        assert_eq!(registry.len().await, 2);
    }

    #[tokio::test]
    async fn test_registry_get_terminal() {
        let registry = TerminalRegistry::new();
        let id = registry.create_terminal("/tmp").await;

        let terminal = registry.get_terminal(id).await;
        assert!(terminal.is_some());

        let terminal = registry.get_terminal(TerminalId::new(999)).await;
        assert!(terminal.is_none());
    }

    #[tokio::test]
    async fn test_registry_remove_terminal() {
        let registry = TerminalRegistry::new();
        let id = registry.create_terminal("/tmp").await;

        assert!(registry.remove_terminal(id).await);
        assert!(!registry.contains(id).await);
        assert!(registry.is_empty().await);

        // Removing again should return false
        assert!(!registry.remove_terminal(id).await);
    }

    #[tokio::test]
    async fn test_registry_get_or_create_existing() {
        let registry = TerminalRegistry::new();
        let id = registry.create_terminal("/tmp").await;

        let terminal = registry.get_or_create_terminal(id, "/var").await;
        let guard = terminal.lock().await;
        // Should return the existing terminal with original cwd
        assert_eq!(guard.get_cwd(), Path::new("/tmp"));
    }

    #[tokio::test]
    async fn test_registry_get_or_create_new() {
        let registry = TerminalRegistry::new();
        let id = TerminalId::new(42);

        let terminal = registry.get_or_create_terminal(id, "/custom").await;
        let guard = terminal.lock().await;
        assert_eq!(guard.get_id(), id);
        assert_eq!(guard.get_cwd(), Path::new("/custom"));
    }

    #[tokio::test]
    async fn test_registry_clear() {
        let registry = TerminalRegistry::new();
        registry.create_terminal("/tmp").await;
        registry.create_terminal("/var").await;
        assert_eq!(registry.len().await, 2);

        registry.clear().await;
        assert!(registry.is_empty().await);
    }

    #[tokio::test]
    async fn test_registry_ids() {
        let registry = TerminalRegistry::new();
        let id1 = registry.create_terminal("/tmp").await;
        let id2 = registry.create_terminal("/var").await;

        let mut ids = registry.ids().await;
        ids.sort();
        assert_eq!(ids, vec![id1, id2]);
    }

    #[tokio::test]
    async fn test_registry_create_with_id() {
        let registry = TerminalRegistry::new();
        let id = TerminalId::new(100);
        let result = registry.create_terminal_with_id(id, "/tmp").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);
        assert!(registry.contains(id).await);
    }

    #[tokio::test]
    async fn test_registry_create_with_duplicate_id() {
        let registry = TerminalRegistry::new();
        let id = TerminalId::new(100);
        registry
            .create_terminal_with_id(id, "/tmp")
            .await
            .expect("first create should succeed");

        let result = registry.create_terminal_with_id(id, "/var").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_default() {
        let registry = TerminalRegistry::default();
        assert!(registry.is_empty().await);
    }
}
