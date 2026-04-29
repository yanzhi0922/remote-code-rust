//! MCP Server Manager — singleton management.
//!
//! Corresponds to TS: `McpServerManager`.
//! Ensures only one McpHub instance runs across all consumers.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing;

use crate::error::McpResult;
use crate::hub::McpHub;

/// Callback type for provider notifications.
pub type NotifyCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Singleton manager for MCP server instances.
///
/// Ensures only one set of MCP servers runs across all consumers.
/// Thread-safe implementation using async locks.
pub struct McpServerManager {
    /// The singleton McpHub instance.
    hub: Arc<RwLock<Option<Arc<McpHub>>>>,
    /// Registered notification callbacks.
    callbacks: Arc<RwLock<Vec<NotifyCallback>>>,
}

impl McpServerManager {
    /// Create a new manager.
    pub fn new() -> Self {
        Self {
            hub: Arc::new(RwLock::new(None)),
            callbacks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the singleton McpHub instance.
    ///
    /// Creates a new instance if one doesn't exist.
    pub async fn get_instance(&self) -> Arc<McpHub> {
        let mut hub = self.hub.write().await;
        if hub.is_none() {
            let new_hub = Arc::new(McpHub::new());
            *hub = Some(new_hub.clone());
            tracing::info!("McpServerManager: Created new McpHub instance");
        }
        hub.as_ref().unwrap().clone()
    }

    /// Get the existing instance if it exists.
    pub async fn try_get_instance(&self) -> Option<Arc<McpHub>> {
        let hub = self.hub.read().await;
        hub.clone()
    }

    /// Register a notification callback.
    pub async fn register_callback(&self, callback: NotifyCallback) {
        let mut callbacks = self.callbacks.write().await;
        callbacks.push(callback);
    }

    /// Unregister all callbacks (called when a consumer is disposed).
    pub async fn unregister(&self) {
        let hub = self.hub.read().await;
        if let Some(h) = hub.as_ref() {
            let _ = h.unregister_client().await;
        }
    }

    /// Notify all registered callbacks.
    pub async fn notify_providers(&self, message: &str) {
        let callbacks = self.callbacks.read().await;
        for callback in callbacks.iter() {
            callback(message);
        }
    }

    /// Clean up the singleton instance and all its resources.
    pub async fn cleanup(&self) -> McpResult<()> {
        let mut hub = self.hub.write().await;
        if let Some(h) = hub.take() {
            h.dispose().await?;
            tracing::info!("McpServerManager: Cleaned up McpHub instance");
        }
        let mut callbacks = self.callbacks.write().await;
        callbacks.clear();
        Ok(())
    }
}

impl Default for McpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_singleton() {
        let manager = McpServerManager::new();

        let hub1 = manager.get_instance().await;
        let hub2 = manager.get_instance().await;

        // Both should point to the same hub (Arc ref count)
        assert!(Arc::ptr_eq(&hub1, &hub2));
    }

    #[tokio::test]
    async fn test_manager_try_get_none() {
        let manager = McpServerManager::new();
        let result = manager.try_get_instance().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_manager_try_get_some() {
        let manager = McpServerManager::new();
        let _ = manager.get_instance().await;
        let result = manager.try_get_instance().await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_manager_cleanup() {
        let manager = McpServerManager::new();
        let _ = manager.get_instance().await;

        manager.cleanup().await.unwrap();

        let result = manager.try_get_instance().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_manager_notify() {
        let manager = McpServerManager::new();

        let received = Arc::new(RwLock::new(String::new()));
        let received_clone = received.clone();

        manager
            .register_callback(Box::new(move |msg| {
                let received = received_clone.clone();
                let msg = msg.to_string();
                tokio::spawn(async move {
                    let mut r = received.write().await;
                    *r = msg;
                });
            }))
            .await;

        manager.notify_providers("test message").await;

        // Give the spawned task time to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let msg = received.read().await;
        assert_eq!(*msg, "test message");
    }
}
