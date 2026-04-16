//! MCP lifecycle events and hooks.
//!
//! Defines events emitted during the MCP server connection lifecycle and a
//! trait for observing these events. Lifecycle hooks allow external code to
//! react to state changes without coupling to the connection manager internals.

/// MCP lifecycle event.
#[derive(Debug, Clone)]
pub enum McpLifecycleEvent {
    /// A connection attempt is starting.
    Connecting {
        /// Server name.
        name: String,
    },
    /// The server has been successfully connected.
    Connected {
        /// Server name.
        name: String,
    },
    /// The server has been disconnected.
    Disconnected {
        /// Server name.
        name: String,
        /// Reason for disconnection.
        reason: DisconnectReason,
    },
    /// A reconnection attempt is in progress.
    Reconnecting {
        /// Server name.
        name: String,
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum number of attempts.
        max_attempts: u32,
    },
    /// The server has been successfully reconnected.
    Reconnected {
        /// Server name.
        name: String,
    },
    /// The connection has permanently failed.
    Failed {
        /// Server name.
        name: String,
        /// Error message.
        error: String,
    },
    /// The server requires authentication.
    NeedsAuth {
        /// Server name.
        name: String,
    },
    /// The server has been disabled.
    Disabled {
        /// Server name.
        name: String,
    },
    /// The server has been enabled.
    Enabled {
        /// Server name.
        name: String,
    },
    /// Tools have been discovered for a server.
    ToolsDiscovered {
        /// Server name.
        name: String,
        /// Number of tools discovered.
        count: usize,
    },
    /// Resources have been discovered for a server.
    ResourcesDiscovered {
        /// Server name.
        name: String,
        /// Number of resources discovered.
        count: usize,
    },
}

impl McpLifecycleEvent {
    /// Return the server name associated with this event.
    #[must_use]
    pub fn server_name(&self) -> &str {
        match self {
            Self::Connecting { name }
            | Self::Connected { name }
            | Self::Disconnected { name, .. }
            | Self::Reconnecting { name, .. }
            | Self::Reconnected { name }
            | Self::Failed { name, .. }
            | Self::NeedsAuth { name }
            | Self::Disabled { name }
            | Self::Enabled { name }
            | Self::ToolsDiscovered { name, .. }
            | Self::ResourcesDiscovered { name, .. } => name,
        }
    }
}

/// Reason for a server disconnection.
#[derive(Debug, Clone)]
pub enum DisconnectReason {
    /// The connection was closed normally.
    Closed,
    /// An error caused the disconnection.
    Error(String),
    /// The session expired (e.g. token timeout).
    SessionExpired,
    /// The disconnection was initiated manually.
    Manual,
}

/// Lifecycle hook trait for observing MCP connection events.
///
/// Implementations can be registered with [`crate::manager::McpConnectionManager`]
/// to receive notifications about connection state changes.
pub trait McpLifecycleHook: Send + Sync {
    /// Called when a lifecycle event occurs.
    fn on_event(&self, event: &McpLifecycleEvent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_server_name_connecting() {
        let event = McpLifecycleEvent::Connecting {
            name: "test-server".to_owned(),
        };
        assert_eq!(event.server_name(), "test-server");
    }

    #[test]
    fn event_server_name_connected() {
        let event = McpLifecycleEvent::Connected {
            name: "my-server".to_owned(),
        };
        assert_eq!(event.server_name(), "my-server");
    }

    #[test]
    fn event_server_name_disconnected() {
        let event = McpLifecycleEvent::Disconnected {
            name: "remote".to_owned(),
            reason: DisconnectReason::Closed,
        };
        assert_eq!(event.server_name(), "remote");
    }

    #[test]
    fn event_server_name_reconnecting() {
        let event = McpLifecycleEvent::Reconnecting {
            name: "retry-srv".to_owned(),
            attempt: 2,
            max_attempts: 5,
        };
        assert_eq!(event.server_name(), "retry-srv");
    }

    #[test]
    fn event_server_name_failed() {
        let event = McpLifecycleEvent::Failed {
            name: "bad".to_owned(),
            error: "timeout".to_owned(),
        };
        assert_eq!(event.server_name(), "bad");
    }

    #[test]
    fn event_server_name_tools_discovered() {
        let event = McpLifecycleEvent::ToolsDiscovered {
            name: "tools-srv".to_owned(),
            count: 10,
        };
        assert_eq!(event.server_name(), "tools-srv");
    }

    #[test]
    fn disconnect_reason_variants() {
        let reasons = vec![
            DisconnectReason::Closed,
            DisconnectReason::Error("connection reset".to_owned()),
            DisconnectReason::SessionExpired,
            DisconnectReason::Manual,
        ];
        assert_eq!(reasons.len(), 4);
    }

    /// A no-op lifecycle hook for testing the trait object.
    struct NullHook;
    impl McpLifecycleHook for NullHook {
        fn on_event(&self, _event: &McpLifecycleEvent) {}
    }

    #[test]
    fn lifecycle_hook_trait_object() {
        let hook: Box<dyn McpLifecycleHook> = Box::new(NullHook);
        let event = McpLifecycleEvent::Connected {
            name: "test".to_owned(),
        };
        hook.on_event(&event);
    }
}
