//! Offline command queue — persists commands when disconnected, replays on reconnect.

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::transport::TransportCommand;

/// Maximum number of queued commands.
const MAX_QUEUE_SIZE: usize = 100;
/// Commands older than this are considered stale and dropped on replay.
const STALE_THRESHOLD_SECS: i64 = 300; // 5 minutes
/// Maximum retry attempts per command before discarding.
const MAX_RETRY_COUNT: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub id: String,
    pub session_id: String,
    pub command: TransportCommand,
    pub queued_at: i64, // Unix timestamp
    pub retry_count: u32,
}

/// In-memory offline queue (optionally persisted to disk on mobile).
#[derive(Debug, Clone)]
pub struct OfflineQueue {
    inner: Arc<RwLock<VecDeque<QueuedCommand>>>,
    stale_threshold_secs: i64,
}

impl OfflineQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::new())),
            stale_threshold_secs: STALE_THRESHOLD_SECS,
        }
    }

    /// Create an offline queue with a custom stale threshold in seconds.
    pub fn with_stale_threshold(stale_threshold_secs: i64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::new())),
            stale_threshold_secs: stale_threshold_secs.max(60),
        }
    }

    /// Enqueue a command for later delivery.
    pub async fn enqueue(&self, session_id: String, command: TransportCommand) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let item = QueuedCommand {
            id: id.clone(),
            session_id,
            command,
            queued_at: chrono::Utc::now().timestamp(),
            retry_count: 0,
        };
        let mut queue = self.inner.write().await;
        if queue.len() >= MAX_QUEUE_SIZE {
            let dropped = queue.pop_front(); // Drop oldest — O(1) for VecDeque
            if let Some(dropped) = dropped {
                tracing::warn!(
                    command_type = ?dropped.command,
                    "offline queue full; dropped oldest command"
                );
            }
        }
        queue.push_back(item);
        id
    }

    /// Drain all non-stale commands, ordered oldest first.
    /// Increments retry_count and drops commands that have been retried too many times.
    pub async fn drain(&self) -> Vec<QueuedCommand> {
        let now = chrono::Utc::now().timestamp();
        let mut queue = self.inner.write().await;
        let (mut valid, stale): (Vec<_>, Vec<_>) = queue.drain(..).partition(|item| {
            now - item.queued_at < self.stale_threshold_secs && item.retry_count < MAX_RETRY_COUNT
        });
        if !stale.is_empty() {
            tracing::info!("dropped {} stale/exhausted offline commands", stale.len(),);
        }
        // Increment retry count for all drained commands.
        for cmd in &mut valid {
            cmd.retry_count += 1;
        }
        valid
    }

    /// Number of pending commands.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Whether the queue is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }

    /// Whether the queue is at capacity.
    pub async fn is_full(&self) -> bool {
        self.inner.read().await.len() >= MAX_QUEUE_SIZE
    }
}

impl Default for OfflineQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enqueue_and_drain() {
        let queue = OfflineQueue::new();
        queue
            .enqueue(
                "s1".into(),
                TransportCommand::SendPrompt {
                    content: "hello".into(),
                },
            )
            .await;
        queue
            .enqueue(
                "s1".into(),
                TransportCommand::SendPrompt {
                    content: "world".into(),
                },
            )
            .await;
        assert_eq!(queue.len().await, 2);
        let items = queue.drain().await;
        assert_eq!(items.len(), 2);
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn drops_stale_commands() {
        let queue = OfflineQueue::new();
        let mut q = queue.inner.write().await;
        q.push_back(QueuedCommand {
            id: "old".into(),
            session_id: "s1".into(),
            command: TransportCommand::Interrupt,
            queued_at: chrono::Utc::now().timestamp() - 600, // 10 min ago
            retry_count: 0,
        });
        q.push_back(QueuedCommand {
            id: "fresh".into(),
            session_id: "s1".into(),
            command: TransportCommand::Interrupt,
            queued_at: chrono::Utc::now().timestamp(),
            retry_count: 0,
        });
        drop(q);
        let items = queue.drain().await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "fresh");
    }
}
