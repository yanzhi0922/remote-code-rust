//! Batched state update queue.
//!
//! Collects multiple server connection updates within a time window and
//! flushes them together, reducing the overhead of frequent individual
//! state updates (inspired by React's batched state updates).

use std::time::Duration;

use crate::connection::McpServerConnection;
use crate::resources::ServerResource;
use crate::types::McpToolDescriptor;

/// Default flush interval (16 ms ≈ one frame at 60 fps).
const DEFAULT_FLUSH_INTERVAL_MS: u64 = 16;

/// A single batched update for a server.
#[derive(Debug, Clone)]
pub struct BatchUpdate {
    /// Server name.
    pub server_name: String,
    /// Updated connection state.
    pub connection: McpServerConnection,
    /// Updated tools (if discovered).
    pub tools: Option<Vec<McpToolDescriptor>>,
    /// Updated resources (if discovered).
    pub resources: Option<Vec<ServerResource>>,
}

/// Batched update queue — merges multiple server updates within a time window.
#[derive(Debug)]
pub struct BatchedUpdateQueue {
    pending: Vec<BatchUpdate>,
    flush_interval: Duration,
}

impl BatchedUpdateQueue {
    /// Create a new queue with the default flush interval (16 ms).
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            flush_interval: Duration::from_millis(DEFAULT_FLUSH_INTERVAL_MS),
        }
    }

    /// Create a new queue with a custom flush interval.
    #[must_use]
    pub fn with_flush_interval(flush_interval: Duration) -> Self {
        Self {
            pending: Vec::new(),
            flush_interval,
        }
    }

    /// Enqueue an update. If an update for the same server already exists in
    /// the pending queue, it is replaced (last-write-wins).
    pub fn enqueue(&mut self, update: BatchUpdate) {
        // Replace existing entry for the same server if present.
        if let Some(existing) = self
            .pending
            .iter_mut()
            .find(|u| u.server_name == update.server_name)
        {
            *existing = update;
        } else {
            self.pending.push(update);
        }
    }

    /// Flush and return all pending updates, clearing the queue.
    pub fn flush(&mut self) -> Vec<BatchUpdate> {
        std::mem::take(&mut self.pending)
    }

    /// Return `true` if there are pending updates.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Return the number of pending updates.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Return the configured flush interval.
    #[must_use]
    pub fn flush_interval(&self) -> Duration {
        self.flush_interval
    }
}

impl Default for BatchedUpdateQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{DisabledServer, PendingServer};
    use crate::scope::{ConfigScope, ScopedMcpServerConfig};
    use crate::config::{McpCapabilityMatrix, McpServerConfig};
    use crate::transport::McpTransportConfig;
    use std::collections::BTreeMap;

    fn test_scoped_config(name: &str) -> ScopedMcpServerConfig {
        ScopedMcpServerConfig::new(
            McpServerConfig {
                name: name.to_owned(),
                enabled: true,
                transport: McpTransportConfig::Stdio {
                    command: "echo".to_owned(),
                    args: vec![],
                    cwd: None,
                    env: BTreeMap::new(),
                },
                capabilities: McpCapabilityMatrix::default(),
                startup_timeout_secs: None,
                request_timeout_secs: None,
                metadata: BTreeMap::new(),
            },
            ConfigScope::Local,
        )
    }

    fn pending_connection(name: &str) -> McpServerConnection {
        McpServerConnection::Pending(PendingServer {
            name: name.to_owned(),
            config: test_scoped_config(name),
            reconnect_attempt: None,
            max_reconnect_attempts: None,
        })
    }

    fn disabled_connection(name: &str) -> McpServerConnection {
        McpServerConnection::Disabled(DisabledServer {
            name: name.to_owned(),
            config: test_scoped_config(name),
        })
    }

    #[test]
    fn new_queue_is_empty() {
        let queue = BatchedUpdateQueue::new();
        assert!(!queue.has_pending());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn enqueue_adds_update() {
        let mut queue = BatchedUpdateQueue::new();
        queue.enqueue(BatchUpdate {
            server_name: "test".to_owned(),
            connection: pending_connection("test"),
            tools: None,
            resources: None,
        });
        assert!(queue.has_pending());
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn enqueue_replaces_existing_server() {
        let mut queue = BatchedUpdateQueue::new();
        queue.enqueue(BatchUpdate {
            server_name: "test".to_owned(),
            connection: pending_connection("test"),
            tools: None,
            resources: None,
        });
        queue.enqueue(BatchUpdate {
            server_name: "test".to_owned(),
            connection: disabled_connection("test"),
            tools: None,
            resources: None,
        });
        assert_eq!(queue.pending_count(), 1);
        let updates = queue.flush();
        assert!(matches!(updates[0].connection, McpServerConnection::Disabled(_)));
    }

    #[test]
    fn flush_clears_queue() {
        let mut queue = BatchedUpdateQueue::new();
        queue.enqueue(BatchUpdate {
            server_name: "a".to_owned(),
            connection: pending_connection("a"),
            tools: None,
            resources: None,
        });
        queue.enqueue(BatchUpdate {
            server_name: "b".to_owned(),
            connection: pending_connection("b"),
            tools: None,
            resources: None,
        });
        let updates = queue.flush();
        assert_eq!(updates.len(), 2);
        assert!(!queue.has_pending());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn flush_on_empty_returns_empty() {
        let mut queue = BatchedUpdateQueue::new();
        let updates = queue.flush();
        assert!(updates.is_empty());
    }

    #[test]
    fn custom_flush_interval() {
        let queue = BatchedUpdateQueue::with_flush_interval(Duration::from_millis(50));
        assert_eq!(queue.flush_interval(), Duration::from_millis(50));
    }

    #[test]
    fn default_impl() {
        let queue = BatchedUpdateQueue::default();
        assert_eq!(queue.flush_interval(), Duration::from_millis(16));
        assert!(!queue.has_pending());
    }
}
