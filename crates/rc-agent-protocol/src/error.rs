//! Error types for the Agent protocol layer.

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
