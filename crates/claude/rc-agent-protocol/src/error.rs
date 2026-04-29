//! Error types for the Agent protocol layer.
//!
//! # Design decision: `AgentProtocolError` vs `anyhow::Result`
//!
//! This module defines a structured [`AgentProtocolError`] enum, but most
//! public APIs in this crate return `anyhow::Result`. This is intentional:
//!
//! - `AgentProtocolError` provides structured, typed errors for **callers**
//!   that need to pattern-match on specific failure modes (e.g. timeout,
//!   config error).
//! - `anyhow::Result` is used internally for ergonomic error propagation
//!   without requiring exhaustive error mapping at every layer boundary.
//!
//! Future work may convert more methods to return `Result<T, AgentProtocolError>`
//! once the error taxonomy stabilizes.

use thiserror::Error;

/// Errors that can occur during Agent protocol operations.
#[derive(Debug, Error)]
pub enum AgentProtocolError {
    /// The Agent has not been started yet.
    #[error("agent not started")]
    AgentNotStarted,

    /// The Agent has stopped unexpectedly.
    #[error("agent stopped: {reason}")]
    AgentStopped {
        /// Why the Agent stopped.
        reason: String,
    },

    /// Communication with the Agent failed.
    #[error("communication error: {details}")]
    CommunicationError {
        /// Underlying cause of the communication failure.
        details: String,
    },

    /// A protocol-level error (malformed message, unexpected response, etc.).
    #[error("protocol error: {message}")]
    ProtocolError {
        /// Description of the protocol violation.
        message: String,
    },

    /// An operation timed out.
    #[error("timeout after {duration_ms}ms")]
    Timeout {
        /// How long we waited before giving up, in milliseconds.
        duration_ms: u64,
    },

    /// The Agent configuration is invalid.
    #[error("config error: {message}")]
    ConfigError {
        /// What is wrong with the configuration.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        assert_eq!(
            AgentProtocolError::AgentNotStarted.to_string(),
            "agent not started"
        );
        assert_eq!(
            AgentProtocolError::AgentStopped {
                reason: "crashed".into()
            }
            .to_string(),
            "agent stopped: crashed"
        );
        assert_eq!(
            AgentProtocolError::CommunicationError {
                details: "io".into()
            }
            .to_string(),
            "communication error: io"
        );
        assert_eq!(
            AgentProtocolError::ProtocolError {
                message: "bad frame".into()
            }
            .to_string(),
            "protocol error: bad frame"
        );
        assert_eq!(
            AgentProtocolError::Timeout { duration_ms: 5000 }.to_string(),
            "timeout after 5000ms"
        );
        assert_eq!(
            AgentProtocolError::ConfigError {
                message: "missing binary".into()
            }
            .to_string(),
            "config error: missing binary"
        );
    }
}
