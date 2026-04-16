//! Elicitation request handling for MCP servers.
//!
//! Elicitation is an MCP feature where a server can request information
//! from the user mid-interaction. This module provides types for handling
//! elicitation requests, including automatic decline and queued handlers
//! for external processing.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

// ── Elicitation params ──────────────────────────────────────────────────────

/// Parameters sent by an MCP server when requesting elicitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationParams {
    /// Human-readable message describing what information is needed.
    pub message: String,
    /// Optional JSON Schema describing the expected response format.
    #[serde(default)]
    pub requested_schema: Option<serde_json::Value>,
}

// ── Elicitation result ──────────────────────────────────────────────────────

/// Response to an elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action")]
pub enum ElicitationResult {
    /// User accepted and provided the requested information.
    #[serde(rename = "accept")]
    Accept {
        /// The content provided by the user.
        content: serde_json::Value,
    },
    /// User declined to provide information.
    #[serde(rename = "decline")]
    Decline,
    /// User cancelled the elicitation request.
    #[serde(rename = "cancel")]
    Cancel,
}

// ── Elicitation waiting state ───────────────────────────────────────────────

/// State of an elicitation request's lifecycle.
#[derive(Debug, Clone)]
pub enum ElicitationWaitingState {
    /// Waiting for user response.
    Waiting,
    /// User has responded.
    Completed(Box<ElicitationResult>),
    /// Request has expired without a response.
    Expired,
}

impl ElicitationWaitingState {
    /// Returns `true` if still waiting for a response.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        matches!(self, Self::Waiting)
    }

    /// Returns `true` if the request has completed.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed(_))
    }
}

// ── Elicitation request event ───────────────────────────────────────────────

/// Event representing an incoming elicitation request from an MCP server.
#[derive(Debug)]
pub struct ElicitationRequestEvent {
    /// Name of the MCP server making the request.
    pub server_name: String,
    /// Unique request identifier.
    pub request_id: String,
    /// Elicitation parameters.
    pub params: ElicitationParams,
    /// Current waiting state.
    pub waiting_state: ElicitationWaitingState,
}

// ── Elicitation handler trait ───────────────────────────────────────────────

/// Trait for handling elicitation requests from MCP servers.
///
/// Implementations decide how to respond when a server asks for
/// user input during a tool call or other interaction.
pub trait ElicitationHandler: Send + Sync {
    /// Handle an elicitation request and return the result.
    fn handle_elicitation(&self, event: ElicitationRequestEvent) -> ElicitationResult;
}

// ── Auto-decline handler ────────────────────────────────────────────────────

/// Default elicitation handler that automatically declines all requests.
///
/// Useful as a safe default when no user interaction is available.
#[derive(Debug, Clone, Default)]
pub struct AutoDeclineElicitationHandler;

impl AutoDeclineElicitationHandler {
    /// Create a new auto-decline handler.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ElicitationHandler for AutoDeclineElicitationHandler {
    fn handle_elicitation(&self, _event: ElicitationRequestEvent) -> ElicitationResult {
        ElicitationResult::Decline
    }
}

// ── Queued handler ──────────────────────────────────────────────────────────

/// Elicitation handler that queues requests for external processing.
///
/// Collects all incoming elicitation events in an internal queue so
/// that an external consumer (e.g., a UI) can process them at its
/// own pace.
#[derive(Debug, Default)]
pub struct QueuedElicitationHandler {
    pending: Arc<Mutex<Vec<ElicitationRequestEvent>>>,
}

impl QueuedElicitationHandler {
    /// Create a new queued handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Drain all pending elicitation events.
    ///
    /// Returns all queued events and clears the internal buffer.
    pub fn drain_pending(&self) -> Vec<ElicitationRequestEvent> {
        let mut guard = self.pending.lock().expect("elicitation lock");
        std::mem::take(&mut *guard)
    }

    /// Get the number of pending events.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().expect("elicitation lock").len()
    }
}

impl ElicitationHandler for QueuedElicitationHandler {
    fn handle_elicitation(&self, event: ElicitationRequestEvent) -> ElicitationResult {
        let mut guard = self.pending.lock().expect("elicitation lock");
        guard.push(event);
        // Return decline by default; the queued events can be processed
        // asynchronously and the response can be updated later.
        ElicitationResult::Decline
    }
}

// ── Callback handler ────────────────────────────────────────────────────────

/// Elicitation handler that delegates to a closure.
///
/// Allows inline handling of elicitation requests without defining
/// a full struct implementation.
pub struct CallbackElicitationHandler<F>
where
    F: Fn(ElicitationRequestEvent) -> ElicitationResult + Send + Sync,
{
    callback: F,
}

impl<F> CallbackElicitationHandler<F>
where
    F: Fn(ElicitationRequestEvent) -> ElicitationResult + Send + Sync,
{
    /// Create a new callback handler.
    #[must_use]
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> ElicitationHandler for CallbackElicitationHandler<F>
where
    F: Fn(ElicitationRequestEvent) -> ElicitationResult + Send + Sync,
{
    fn handle_elicitation(&self, event: ElicitationRequestEvent) -> ElicitationResult {
        (self.callback)(event)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(message: &str) -> ElicitationRequestEvent {
        ElicitationRequestEvent {
            server_name: "test-server".to_owned(),
            request_id: "req-1".to_owned(),
            params: ElicitationParams {
                message: message.to_owned(),
                requested_schema: Some(json!({"type": "string"})),
            },
            waiting_state: ElicitationWaitingState::Waiting,
        }
    }

    #[test]
    fn auto_decline_handler_returns_decline() {
        let handler = AutoDeclineElicitationHandler::new();
        let event = make_event("Enter your API key");
        let result = handler.handle_elicitation(event);
        assert_eq!(result, ElicitationResult::Decline);
    }

    #[test]
    fn queued_handler_collects_events() {
        let handler = QueuedElicitationHandler::new();
        let event1 = make_event("Question 1");
        let event2 = ElicitationRequestEvent {
            server_name: "other-server".to_owned(),
            request_id: "req-2".to_owned(),
            params: ElicitationParams {
                message: "Question 2".to_owned(),
                requested_schema: None,
            },
            waiting_state: ElicitationWaitingState::Waiting,
        };

        let _ = handler.handle_elicitation(event1);
        let _ = handler.handle_elicitation(event2);

        assert_eq!(handler.pending_count(), 2);
        let drained = handler.drain_pending();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].server_name, "test-server");
        assert_eq!(drained[1].server_name, "other-server");
        assert!(handler.pending_count() == 0);
    }

    #[test]
    fn callback_handler_delegates() {
        let handler = CallbackElicitationHandler::new(|_event| {
            ElicitationResult::Accept {
                content: json!("user-response"),
            }
        });
        let event = make_event("Enter value");
        let result = handler.handle_elicitation(event);
        assert_eq!(
            result,
            ElicitationResult::Accept {
                content: json!("user-response")
            }
        );
    }

    #[test]
    fn elicitation_result_serde_roundtrip() {
        let accept = ElicitationResult::Accept {
            content: json!({"key": "value"}),
        };
        let json = serde_json::to_string(&accept).expect("serialize");
        let back: ElicitationResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, accept);

        let decline = ElicitationResult::Decline;
        let json = serde_json::to_string(&decline).expect("serialize");
        let back: ElicitationResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, decline);

        let cancel = ElicitationResult::Cancel;
        let json = serde_json::to_string(&cancel).expect("serialize");
        let back: ElicitationResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, cancel);
    }

    #[test]
    fn elicitation_params_serde_roundtrip() {
        let params = ElicitationParams {
            message: "Please enter your name".to_owned(),
            requested_schema: Some(json!({"type": "string", "maxLength": 100})),
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let back: ElicitationParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.message, "Please enter your name");
        assert!(back.requested_schema.is_some());
    }

    #[test]
    fn elicitation_params_deserializes_without_schema() {
        let json = r#"{"message":"hello"}"#;
        let params: ElicitationParams = serde_json::from_str(json).expect("deserialize");
        assert_eq!(params.message, "hello");
        assert!(params.requested_schema.is_none());
    }

    #[test]
    fn waiting_state_queries() {
        let waiting = ElicitationWaitingState::Waiting;
        assert!(waiting.is_waiting());
        assert!(!waiting.is_completed());

        let completed =
            ElicitationWaitingState::Completed(Box::new(ElicitationResult::Decline));
        assert!(!completed.is_waiting());
        assert!(completed.is_completed());

        let expired = ElicitationWaitingState::Expired;
        assert!(!expired.is_waiting());
        assert!(!expired.is_completed());
    }

    #[test]
    fn elicitation_result_action_tag() {
        let accept = ElicitationResult::Accept {
            content: json!("yes"),
        };
        let json = serde_json::to_value(&accept).expect("serialize");
        assert_eq!(json["action"], "accept");

        let decline = ElicitationResult::Decline;
        let json = serde_json::to_value(&decline).expect("serialize");
        assert_eq!(json["action"], "decline");

        let cancel = ElicitationResult::Cancel;
        let json = serde_json::to_value(&cancel).expect("serialize");
        assert_eq!(json["action"], "cancel");
    }
}
