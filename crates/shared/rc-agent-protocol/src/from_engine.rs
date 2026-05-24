//! Direct conversion from [`EngineEvent`] to [`UnifiedAgentEvent`] — **test only**.
//!
//! # Why this exists
//!
//! The Claude adapter's production path converts events in two stages:
//!
//! ```text
//! EngineEvent → [QueryEngine] → QueryObserverEvent → [event_mapper] → UnifiedAgentEvent
//! ```
//!
//! The `QueryEngine` enriches raw engine events with higher-level semantics:
//! budget evaluation (`ContextBudgetEvaluated`), final results with usage stats
//! (`QueryFinished`), and stop-hook handling. These synthesized events do **not**
//! exist in the raw `EngineEvent` stream.
//!
//! This module provides a **simplified shortcut** that bypasses the enrichment
//! layer, converting `EngineEvent` directly to `UnifiedAgentEvent`. It is used
//! only in integration tests and benchmarks where the full pipeline is unnecessary.
//!
//! # Known gaps vs the production path
//!
//! - No `ContextUsage` events (requires `ContextBudgetEvaluated`/`StreamingUsageUpdated`)
//! - Empty `Completed` result (no final text, no usage stats)
//! - No `BudgetExceeded` error mapping
//! - Thinking deltas are suppressed (production maps them to `MessageDelta`)
//!
//! # Codex and Roo adapters
//!
//! These adapters do not use `EngineEvent` at all — they have their own native
//! event types (`AppServerEvent`, `RooTaskEvent`) with dedicated conversion
//! functions in their respective crates.

use rc_engine_events::types::{ContentBlockDelta, EngineEvent};
use tracing::warn;

use crate::events::{AgentResult, UnifiedAgentEvent, UsageInfo};

/// Convert an [`EngineEvent`] into an optional [`UnifiedAgentEvent`].
///
/// Returns `Some(...)` for events with adapter-protocol semantics
/// (streaming text, tool calls, compaction, errors, completion).
/// Returns `None` for internal engine events (stream lifecycle,
/// agent lifecycle, state/cost/usage snapshots).
#[must_use]
pub fn engine_event_to_unified(event: &EngineEvent, session_id: &str) -> Option<UnifiedAgentEvent> {
    match event {
        // ── Streaming text ──────────────────────────────────────
        EngineEvent::StreamContentBlockDelta {
            delta: ContentBlockDelta::TextDelta { text },
            ..
        } => Some(UnifiedAgentEvent::MessageDelta {
            session_id: session_id.to_owned(),
            delta: text.clone(),
        }),

        // Non-text content-block deltas (thinking, JSON, signature)
        // are engine-internal and not surfaced through the adapter protocol.
        EngineEvent::StreamContentBlockDelta {
            delta: ContentBlockDelta::ThinkingDelta { .. },
            ..
        }
        | EngineEvent::StreamContentBlockDelta {
            delta: ContentBlockDelta::InputJsonDelta { .. },
            ..
        }
        | EngineEvent::StreamContentBlockDelta {
            delta: ContentBlockDelta::SignatureDelta { .. },
            ..
        } => None,

        // ── Tool calls ──────────────────────────────────────────
        EngineEvent::ToolUseStarted {
            tool_use_id,
            tool_name,
            input,
            ..
        } => {
            let mut tool_input = (**input).clone();
            insert_tool_call_id(&mut tool_input, tool_use_id.as_ref());
            Some(UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_owned(),
                tool_name: tool_name.to_string(),
                tool_input,
            })
        }

        EngineEvent::ToolUseProgress {
            tool_use_id,
            progress,
            ..
        } => Some(UnifiedAgentEvent::ToolCallProgress {
            session_id: session_id.to_owned(),
            tool_name: String::new(),
            progress: format!(
                "[{}] {}",
                tool_use_id.as_ref(),
                progress.message.clone().unwrap_or_default()
            ),
        }),

        EngineEvent::ToolUseCompleted {
            tool_use_id,
            result,
            ..
        } => Some(UnifiedAgentEvent::ToolCallCompleted {
            session_id: session_id.to_owned(),
            tool_name: String::new(),
            result: serde_json::json!({
                "tool_call_id": tool_use_id.as_ref(),
                "content": result.content,
                "is_error": result.is_error,
            }),
        }),

        EngineEvent::ToolUseError {
            tool_use_id, error, ..
        } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_owned(),
            message: format!("Tool {} failed: {}", tool_use_id, error.message),
            recoverable: error.retryable,
        }),

        EngineEvent::ToolUseRejected { tool_use_id, .. } => {
            warn!(%tool_use_id, "Tool use rejected");
            Some(UnifiedAgentEvent::Error {
                session_id: session_id.to_owned(),
                message: format!("Tool {tool_use_id} rejected by permission policy"),
                recoverable: false,
            })
        }

        // ── Context / compaction ────────────────────────────────
        EngineEvent::CompactCompleted { result } => Some(UnifiedAgentEvent::ContextCompacted {
            session_id: session_id.to_owned(),
            entries_removed: result.before_messages.saturating_sub(result.after_messages),
            usage_ratio: 0.0,
        }),

        // ── Stream completion / errors ──────────────────────────
        EngineEvent::StreamMessageStop => Some(UnifiedAgentEvent::Completed {
            session_id: session_id.to_owned(),
            result: AgentResult {
                response_text: String::new(),
                tool_calls: Vec::new(),
                usage: UsageInfo::default(),
                cost: None,
            },
        }),

        EngineEvent::StreamError { error } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_owned(),
            message: error.clone(),
            recoverable: true,
        }),

        // ── Internal / not mapped ───────────────────────────────
        EngineEvent::QueryStarted { .. }
        | EngineEvent::QueryCompleted { .. }
        | EngineEvent::QueryAborted { .. }
        | EngineEvent::StreamStarted { .. }
        | EngineEvent::StreamMessageStart { .. }
        | EngineEvent::StreamContentBlockStart { .. }
        | EngineEvent::StreamContentBlockStop { .. }
        | EngineEvent::StreamMessageDelta { .. }
        | EngineEvent::CompactStarted { .. }
        | EngineEvent::CompactProgress { .. }
        | EngineEvent::AgentStarted { .. }
        | EngineEvent::AgentCompleted { .. }
        | EngineEvent::AgentFailed { .. }
        | EngineEvent::StateUpdated { .. }
        | EngineEvent::CostUpdated { .. }
        | EngineEvent::UsageUpdated { .. } => None,
    }
}

fn insert_tool_call_id(value: &mut serde_json::Value, tool_call_id: &str) {
    match value {
        serde_json::Value::Object(map) => {
            map.entry("tool_call_id")
                .or_insert_with(|| serde_json::Value::String(tool_call_id.to_owned()));
        }
        other => {
            *other = serde_json::json!({
                "tool_call_id": tool_call_id,
                "input": other.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_engine_events::types::{CompactionResult, ToolError};
    use std::sync::Arc;

    const SID: &str = "test-session";

    #[test]
    fn engine_stream_content_block_delta_text_maps_to_message_delta() {
        let event = EngineEvent::StreamContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::TextDelta {
                text: "Hello".into(),
            },
        };
        let result = engine_event_to_unified(&event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::MessageDelta { session_id, delta } => {
                assert_eq!(session_id, SID);
                assert_eq!(delta, "Hello");
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn engine_stream_content_block_delta_thinking_is_not_mapped() {
        let event = EngineEvent::StreamContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::ThinkingDelta {
                thinking: "reasoning".into(),
            },
        };
        assert!(engine_event_to_unified(&event, SID).is_none());
    }

    #[test]
    fn engine_tool_use_started_maps_to_tool_call_started() {
        let event = EngineEvent::ToolUseStarted {
            tool_use_id: Arc::from("tu-1"),
            tool_name: Arc::from("read_file"),
            input: Arc::new(serde_json::json!({"path": "/tmp/test.txt"})),
        };
        let result = engine_event_to_unified(&event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::ToolCallStarted {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(tool_input["path"], "/tmp/test.txt");
            }
            other => panic!("expected ToolCallStarted, got {other:?}"),
        }
    }

    #[test]
    fn engine_tool_use_error_is_recoverable() {
        let event = EngineEvent::ToolUseError {
            tool_use_id: Arc::from("tu-1"),
            error: ToolError {
                message: "timeout".into(),
                retryable: true,
            },
        };
        let result = engine_event_to_unified(&event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::Error {
                message,
                recoverable,
                ..
            } => {
                assert!(message.contains("timeout"));
                assert!(recoverable);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn engine_stream_message_stop_maps_to_completed() {
        let event = EngineEvent::StreamMessageStop;
        let result = engine_event_to_unified(&event, SID).expect("should map");
        assert!(matches!(result, UnifiedAgentEvent::Completed { .. }));
    }

    #[test]
    fn engine_query_started_not_mapped() {
        let event = EngineEvent::QueryStarted {
            session_id: uuid::Uuid::new_v4(),
        };
        assert!(engine_event_to_unified(&event, SID).is_none());
    }

    #[test]
    fn engine_state_updated_not_mapped() {
        let event = EngineEvent::StateUpdated {
            state_snapshot: rc_engine_events::types::EngineStateSnapshot::default(),
        };
        assert!(engine_event_to_unified(&event, SID).is_none());
    }

    #[test]
    fn engine_cost_updated_not_mapped() {
        let event = EngineEvent::CostUpdated {
            total_cost_usd: 0.05,
        };
        assert!(engine_event_to_unified(&event, SID).is_none());
    }

    #[test]
    fn engine_compact_completed_maps_to_context_compacted() {
        let event = EngineEvent::CompactCompleted {
            result: CompactionResult {
                strategy: "summary".into(),
                before_messages: 50,
                after_messages: 20,
                summary: Some("compacted".into()),
            },
        };
        let result = engine_event_to_unified(&event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::ContextCompacted {
                entries_removed, ..
            } => {
                assert_eq!(entries_removed, 30);
            }
            other => panic!("expected ContextCompacted, got {other:?}"),
        }
    }
}
