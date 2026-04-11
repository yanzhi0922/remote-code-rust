//! Abstract UI bridge for multi-frontend support (TUI, GUI, Remote-Control).
//!
//! This crate defines the **trait boundaries** that every frontend must implement
//! to integrate with the remote-code-rust core. By programming against these
//! traits, the core engine remains completely decoupled from any specific UI
//! framework (ratatui, egui, iced, web, etc.).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │                Core Engine                  │
//! │  (rc-provider, rc-tools, rc-session, etc.)  │
//! └──────────────┬──────────────────────────────┘
//!                │  calls UiFrontend trait
//! ┌──────────────┴──────────────────────────────┐
//! │            rc-ui-bridge                     │
//! │  UiFrontend trait + UiEvent enum            │
//! └──────┬──────────┬──────────┬────────────────┘
//!        │          │          │
//!   ┌────┴───┐ ┌───┴───┐ ┌───┴──────────┐
//!   │  TUI   │ │  GUI  │ │Remote-Control│
//!   │(ratatui│ │(egui/ │ │  (HTTP/WS)   │
//!   │/crosst)│ │ iced) │ │              │
//!   └────────┘ └───────┘ └──────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use rc_ui_bridge::{UiFrontend, UiEvent};
//!
//! struct MyGuiFrontend;
//!
//! #[async_trait]
//! impl UiFrontend for MyGuiFrontend {
//!     async fn render_event(&self, event: &UiEvent) -> anyhow::Result<()> {
//!         match event {
//!             UiEvent::AssistantText { text } => println!("{text}"),
//!             _ => {}
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// UI Events — the universal language between core and frontends
// ---------------------------------------------------------------------------

/// Events emitted by the core engine for frontends to render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiEvent {
    // ── Session lifecycle ────────────────────────────────────────────
    /// Session initialized with metadata.
    SessionInit {
        /// Session unique identifier.
        session_id: Uuid,
        /// Model being used.
        model: String,
        /// Provider name.
        provider: String,
        /// Current working directory.
        cwd: String,
        /// Permission mode.
        permission_mode: String,
    },
    /// Session is shutting down.
    SessionEnd {
        /// Session unique identifier.
        session_id: Uuid,
        /// Final cost summary.
        cost_summary: Option<String>,
    },

    // ── Conversation ─────────────────────────────────────────────────
    /// User submitted a message.
    UserMessage {
        /// The user's input text.
        text: String,
    },
    /// Assistant is generating text (streaming delta).
    AssistantText {
        /// Incremental text chunk.
        text: String,
    },
    /// Assistant finished generating.
    AssistantComplete {
        /// Full response text.
        text: String,
        /// Stop reason from the provider.
        stop_reason: String,
        /// Token usage for this turn.
        usage: UiUsage,
    },

    // ── Tool execution ───────────────────────────────────────────────
    /// A tool call has started.
    ToolStart {
        /// Tool call ID from the provider.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Tool input parameters (JSON).
        input: serde_json::Value,
    },
    /// A tool call produced intermediate output.
    ToolProgress {
        /// Tool call ID.
        tool_call_id: String,
        /// Progress message.
        message: String,
        /// Optional percentage (0-100).
        percent: Option<u8>,
    },
    /// A tool call completed.
    ToolResult {
        /// Tool call ID.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Whether the tool execution was an error.
        is_error: bool,
        /// Result content (may be truncated).
        output: String,
    },

    // ── Permission ───────────────────────────────────────────────────
    /// Permission request pending user approval.
    PermissionRequest {
        /// Request unique identifier.
        request_id: String,
        /// Tool name requesting permission.
        tool_name: String,
        /// Short description of the action.
        description: String,
    },
    /// Permission decision rendered.
    PermissionDecision {
        /// Request unique identifier.
        request_id: String,
        /// Whether the action was allowed.
        allowed: bool,
        /// Optional explanation.
        reason: Option<String>,
    },

    // ── Context management ───────────────────────────────────────────
    /// Context window compaction occurred.
    ContextCompacted {
        /// Number of entries removed.
        entries_removed: usize,
        /// Remaining context usage ratio (0.0-1.0).
        usage_ratio: f64,
    },
    /// Context usage update.
    ContextUsage {
        /// Current usage ratio (0.0-1.0).
        ratio: f64,
        /// Estimated token count.
        estimated_tokens: u64,
        /// Maximum context tokens.
        max_tokens: u64,
    },

    // ── Cost tracking ────────────────────────────────────────────────
    /// Cost updated after a provider call.
    CostUpdate {
        /// Turn cost in USD.
        turn_cost_usd: f64,
        /// Total session cost in USD.
        total_cost_usd: f64,
        /// Input tokens for this turn.
        input_tokens: u64,
        /// Output tokens for this turn.
        output_tokens: u64,
    },

    // ── Error ────────────────────────────────────────────────────────
    /// An error occurred.
    Error {
        /// Error category.
        category: ErrorCategory,
        /// Human-readable error message.
        message: String,
        /// Suggested recovery action.
        suggestion: Option<String>,
    },

    // ── Status / info ────────────────────────────────────────────────
    /// Status message (spinner text, info line, etc.).
    Status {
        /// Status message text.
        message: String,
    },
    /// Provider is thinking / processing.
    Thinking {
        /// Optional thinking content (for models that expose it).
        content: Option<String>,
    },

    // ── Multi-agent ──────────────────────────────────────────────────
    /// A sub-agent was dispatched.
    AgentDispatched {
        /// Agent identifier.
        agent_id: String,
        /// Agent task description.
        task: String,
    },
    /// A sub-agent completed.
    AgentComplete {
        /// Agent identifier.
        agent_id: String,
        /// Whether the agent succeeded.
        success: bool,
        /// Agent result summary.
        summary: String,
    },

    // ── Streaming ────────────────────────────────────────────────────
    /// Streaming started.
    StreamStart {
        /// Provider protocol.
        protocol: String,
    },
    /// Streaming ended.
    StreamEnd {
        /// Total chunks received.
        chunks: u64,
        /// Total duration in milliseconds.
        duration_ms: u64,
    },
}

/// Token usage information for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiUsage {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
}

/// Error category for structured error display.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCategory {
    /// API / provider error.
    Provider,
    /// Tool execution error.
    Tool,
    /// Permission denied.
    Permission,
    /// Network connectivity.
    Network,
    /// File system error.
    FileSystem,
    /// Configuration error.
    Config,
    /// Context window overflow.
    ContextOverflow,
    /// Internal error.
    Internal,
}

// ---------------------------------------------------------------------------
// UiFrontend trait — the contract every frontend must implement
// ---------------------------------------------------------------------------

/// The core trait that any UI frontend must implement.
///
/// This trait defines the bidirectional interface between the core engine
/// and the user interface. The core calls `render_event` to push information
/// to the frontend, and the frontend can call methods on the core through
/// the `UiAction` channel.
#[async_trait]
pub trait UiFrontend: Send + Sync {
    /// Render a UI event. Called by the core engine whenever something
    /// happens that the user should see.
    ///
    /// # Errors
    /// Returns an error if the frontend fails to render the event.
    async fn render_event(&self, event: &UiEvent) -> Result<()>;

    /// Request user input. Called when the core needs text input from the user.
    ///
    /// # Errors
    /// Returns an error if the input request fails or is cancelled.
    async fn request_input(&self, prompt: &str) -> Result<String>;

    /// Request a permission decision from the user.
    ///
    /// # Errors
    /// Returns an error if the permission request fails.
    async fn request_permission(
        &self,
        tool_name: &str,
        description: &str,
    ) -> Result<bool>;

    /// Check if the frontend supports a specific feature.
    fn supports_feature(&self, feature: UiFeature) -> bool;

    /// Get the frontend name for diagnostics.
    fn frontend_name(&self) -> &str;
}

/// Features that a frontend may or may not support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiFeature {
    /// Rich text / Markdown rendering.
    RichText,
    /// Syntax-highlighted code blocks.
    SyntaxHighlighting,
    /// Inline images.
    Images,
    /// Interactive approval dialogs.
    InteractiveApproval,
    /// Multi-panel layout.
    MultiPanel,
    /// Progress spinners.
    Spinner,
    /// Auto-completion.
    AutoCompletion,
    /// Mouse interaction.
    Mouse,
    /// Resize handling.
    Resize,
    /// Streaming text display.
    Streaming,
    /// Color / themes.
    Color,
}

// ---------------------------------------------------------------------------
// UiAction — requests from frontend to core
// ---------------------------------------------------------------------------

/// Actions that the frontend can request the core to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiAction {
    /// Submit user text input.
    SubmitInput {
        /// The user's text.
        text: String,
    },
    /// Respond to a permission request.
    PermissionResponse {
        /// The request ID.
        request_id: String,
        /// Whether to allow.
        allow: bool,
    },
    /// Interrupt the current operation.
    Interrupt,
    /// Request context compaction.
    CompactContext,
    /// Change the active model.
    ChangeModel {
        /// New model identifier.
        model: String,
    },
    /// Quit the session.
    Quit,
}

// ---------------------------------------------------------------------------
// NullFrontend — a no-op frontend for headless / testing
// ---------------------------------------------------------------------------

/// A no-op frontend that discards all events. Useful for headless mode
/// and testing.
pub struct NullFrontend;

#[async_trait]
impl UiFrontend for NullFrontend {
    async fn render_event(&self, _event: &UiEvent) -> Result<()> {
        Ok(())
    }

    async fn request_input(&self, _prompt: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn request_permission(
        &self,
        _tool_name: &str,
        _description: &str,
    ) -> Result<bool> {
        Ok(true)
    }

    fn supports_feature(&self, _feature: UiFeature) -> bool {
        false
    }

    fn frontend_name(&self) -> &str {
        "null"
    }
}

// ---------------------------------------------------------------------------
// CollectingFrontend — collects events for testing
// ---------------------------------------------------------------------------

/// A frontend that collects all events into a Vec for assertion in tests.
pub struct CollectingFrontend {
    events: std::sync::Mutex<Vec<UiEvent>>,
}

impl CollectingFrontend {
    /// Create a new collecting frontend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all collected events.
    #[must_use]
    pub fn events(&self) -> Vec<UiEvent> {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Check if any collected event matches a predicate.
    pub fn has_event(&self, predicate: impl Fn(&UiEvent) -> bool) -> bool {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(predicate)
    }
}

impl Default for CollectingFrontend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UiFrontend for CollectingFrontend {
    async fn render_event(&self, event: &UiEvent) -> Result<()> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event.clone());
        Ok(())
    }

    async fn request_input(&self, _prompt: &str) -> Result<String> {
        Ok("test input".to_owned())
    }

    async fn request_permission(
        &self,
        _tool_name: &str,
        _description: &str,
    ) -> Result<bool> {
        Ok(true)
    }

    fn supports_feature(&self, feature: UiFeature) -> bool {
        matches!(feature, UiFeature::Streaming)
    }

    fn frontend_name(&self) -> &str {
        "collecting"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_event_serializes_to_json() {
        let event = UiEvent::AssistantText {
            text: "Hello, world!".to_owned(),
        };
        let json = serde_json::to_string(&event).expect("serialize should not fail");
        assert!(json.contains("AssistantText"));
        assert!(json.contains("Hello, world!"));
    }

    #[test]
    fn ui_event_deserializes_from_json() {
        let event = UiEvent::CostUpdate {
            turn_cost_usd: 0.003,
            total_cost_usd: 0.015,
            input_tokens: 500,
            output_tokens: 200,
        };
        let json = serde_json::to_string(&event).expect("serialize should not fail");
        let parsed: UiEvent = serde_json::from_str(&json).expect("deserialize should not fail");
        if let UiEvent::CostUpdate { turn_cost_usd, .. } = parsed {
            assert!((turn_cost_usd - 0.003).abs() < 0.0001);
        } else {
            panic!("Expected CostUpdate variant");
        }
    }

    #[test]
    fn error_category_serializes() {
        let cat = ErrorCategory::Provider;
        let json = serde_json::to_string(&cat).expect("serialize should not fail");
        assert!(json.contains("Provider"));
    }

    #[tokio::test]
    async fn null_frontend_discards_events() {
        let fe = NullFrontend;
        let result = fe.render_event(&UiEvent::Status {
            message: "test".to_owned(),
        }).await;
        assert!(result.is_ok());
        assert_eq!(fe.frontend_name(), "null");
        assert!(!fe.supports_feature(UiFeature::RichText));
    }

    #[tokio::test]
    async fn collecting_frontend_captures_events() {
        let fe = CollectingFrontend::new();
        fe.render_event(&UiEvent::UserMessage {
            text: "hello".to_owned(),
        }).await.expect("render should not fail");
        fe.render_event(&UiEvent::AssistantText {
            text: "world".to_owned(),
        }).await.expect("render should not fail");

        let events = fe.events();
        assert_eq!(events.len(), 2);
        assert!(fe.has_event(|e| matches!(e, UiEvent::UserMessage { .. })));
        assert!(fe.has_event(|e| matches!(e, UiEvent::AssistantText { .. })));
    }

    #[test]
    fn ui_action_serializes() {
        let action = UiAction::SubmitInput {
            text: "do something".to_owned(),
        };
        let json = serde_json::to_string(&action).expect("serialize should not fail");
        assert!(json.contains("SubmitInput"));
    }

    #[test]
    fn ui_feature_coverage() {
        let features = [
            UiFeature::RichText,
            UiFeature::SyntaxHighlighting,
            UiFeature::Images,
            UiFeature::InteractiveApproval,
            UiFeature::MultiPanel,
            UiFeature::Spinner,
            UiFeature::AutoCompletion,
            UiFeature::Mouse,
            UiFeature::Resize,
            UiFeature::Streaming,
            UiFeature::Color,
        ];
        assert_eq!(features.len(), 11);
    }
}
