//! Event mapper: [`QueryObserverEvent`] → [`UnifiedAgentEvent`].
//!
//! Converts internal query-engine observer events into the unified agent event
//! model used by the adapter protocol.

use claude_agent_protocol::events::{AgentResult, UnifiedAgentEvent, UsageInfo};
use claude_query_engine::QueryObserverEvent;

/// Map a [`QueryObserverEvent`] to an optional [`UnifiedAgentEvent`].
///
/// Returns `None` for internal events that should not be surfaced to consumers
/// (e.g. `AssistantMessageCommitted`, `MessagesAppended`, `CheckpointCreated`).
pub fn map_observer_event(
    event: QueryObserverEvent,
    session_id: &str,
) -> Option<UnifiedAgentEvent> {
    match event {
        // ── Lifecycle ──────────────────────────────────────────────
        QueryObserverEvent::QueryStarted { .. } => {
            // The Started event is emitted by the adapter itself, not mapped.
            None
        }

        // ── Streaming ──────────────────────────────────────────────
        QueryObserverEvent::StreamingTextDelta { delta, .. } => {
            Some(UnifiedAgentEvent::MessageDelta {
                session_id: session_id.to_owned(),
                delta,
            })
        }

        QueryObserverEvent::StreamingToolCallStarted {
            tool_call_id: _,
            tool_name,
            ..
        } => Some(UnifiedAgentEvent::ToolCallStarted {
            session_id: session_id.to_owned(),
            tool_name,
            tool_input: serde_json::Value::Null,
        }),

        QueryObserverEvent::StreamingToolCallDelta {
            tool_call_id,
            delta,
            ..
        } => Some(UnifiedAgentEvent::ToolCallProgress {
            session_id: session_id.to_owned(),
            tool_name: String::new(),
            progress: format!("[{tool_call_id}] {delta}"),
        }),

        QueryObserverEvent::StreamingUsageUpdated { usage, .. } => {
            Some(UnifiedAgentEvent::ContextUsage {
                session_id: session_id.to_owned(),
                used: usage.total_tokens as usize,
                total: 0, // total context window size unknown at this layer
            })
        }

        // ── Tool execution ─────────────────────────────────────────
        QueryObserverEvent::ToolCallStarted { tool_call, .. } => {
            Some(UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_owned(),
                tool_name: tool_call.name.clone(),
                tool_input: tool_call.input.clone(),
            })
        }

        QueryObserverEvent::ToolResultCommitted {
            tool_call, result, ..
        } => Some(UnifiedAgentEvent::ToolCallCompleted {
            session_id: session_id.to_owned(),
            tool_name: tool_call.name.clone(),
            result: serde_json::json!({
                "tool_call_id": tool_call.id,
                "content": result.content,
                "is_error": result.is_error,
            }),
        }),

        // ── Context management ─────────────────────────────────────
        QueryObserverEvent::ContextCompactionApplied {
            before_messages,
            after_messages,
            usage_ratio_after,
            ..
        } => {
            let removed = before_messages.saturating_sub(after_messages);
            Some(UnifiedAgentEvent::ContextCompacted {
                session_id: session_id.to_owned(),
                entries_removed: removed,
                usage_ratio: usage_ratio_after,
            })
        }

        QueryObserverEvent::ContextBudgetEvaluated { context, .. } => {
            Some(UnifiedAgentEvent::ContextUsage {
                session_id: session_id.to_owned(),
                used: context.estimated_tokens as usize,
                total: context.max_input_tokens as usize,
            })
        }

        // ── Terminal states ────────────────────────────────────────
        QueryObserverEvent::QueryFinished {
            stop_reason: _,
            final_text,
            usage,
            ..
        } => Some(UnifiedAgentEvent::Completed {
            session_id: session_id.to_owned(),
            result: AgentResult {
                response_text: final_text.unwrap_or_default(),
                tool_calls: Vec::new(),
                usage: UsageInfo {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read: usage.cache_read_input_tokens,
                    cache_write: usage.cache_creation_input_tokens,
                },
                cost: None,
            },
        }),

        QueryObserverEvent::QueryFailed { error, usage: _, .. } => {
            Some(UnifiedAgentEvent::Error {
                session_id: session_id.to_owned(),
                message: error,
                recoverable: true,
            })
        }

        QueryObserverEvent::BudgetExceeded { reason, .. } => Some(UnifiedAgentEvent::Completed {
            session_id: session_id.to_owned(),
            result: AgentResult {
                response_text: format!("Budget exceeded: {reason}"),
                tool_calls: Vec::new(),
                usage: UsageInfo::default(),
                cost: None,
            },
        }),

        // ── Internal / not mapped ──────────────────────────────────
        QueryObserverEvent::AssistantMessageCommitted { .. }
        | QueryObserverEvent::MessagesAppended { .. }
        | QueryObserverEvent::BudgetEvaluated { .. }
        | QueryObserverEvent::CheckpointCreated { .. }
        | QueryObserverEvent::CheckpointCleared { .. } => None,
    }
}
