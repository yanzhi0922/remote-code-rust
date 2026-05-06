//! Event mapper: [`QueryObserverEvent`] → [`UnifiedAgentEvent`].
//!
//! Converts internal query-engine observer events into the unified agent event
//! model used by the adapter protocol.

use rc_agent_protocol::events::{AgentResult, UnifiedAgentEvent, UsageInfo};
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

        // Skip the streaming variant — the non-streaming ToolCallStarted below
        // provides the full tool_input. Emitting both would cause duplicate
        // ToolCallStarted events at the consumer.
        QueryObserverEvent::StreamingToolCallStarted { .. } => None,

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

        QueryObserverEvent::BudgetExceeded { reason, .. } => Some(UnifiedAgentEvent::Error {
            session_id: session_id.to_owned(),
            message: format!("Budget exceeded: {reason}"),
            recoverable: false,
        }),

        QueryObserverEvent::StreamingThinkingDelta { delta, .. } => {
            Some(UnifiedAgentEvent::MessageDelta {
                session_id: session_id.to_owned(),
                delta: format!("[thinking] {delta}"),
            })
        }

        // ── Internal / not mapped ──────────────────────────────────
        QueryObserverEvent::AssistantMessageCommitted { .. }
        | QueryObserverEvent::MessagesAppended { .. }
        | QueryObserverEvent::BudgetEvaluated { .. }
        | QueryObserverEvent::CheckpointCreated { .. }
        | QueryObserverEvent::CheckpointCleared { .. }
        | QueryObserverEvent::QueryResult { .. }
        | QueryObserverEvent::TokenBudgetContinuation { .. }
        | QueryObserverEvent::ReactiveCompactApplied { .. }
        | QueryObserverEvent::ToolUseSummary { .. }
        | QueryObserverEvent::Progress { .. }
        | QueryObserverEvent::Attachment { .. }
        | QueryObserverEvent::ApiRetry { .. }
        | QueryObserverEvent::StopHookBlocking { .. }
        | QueryObserverEvent::StopHookPrevented { .. }
        | QueryObserverEvent::MaxTokensEscalate { .. }
        | QueryObserverEvent::MaxTokensRecovery { .. }
        | QueryObserverEvent::ModelFallbackTriggered { .. }
        | QueryObserverEvent::CollapseDrainRetry { .. }
        | QueryObserverEvent::ReactiveCompactRetry { .. }
        | QueryObserverEvent::MaxTokensRecoveryExhausted { .. }
        | QueryObserverEvent::ImageErrorRecovery { .. }
        | QueryObserverEvent::MediaSizeErrorRecovery { .. }
        | QueryObserverEvent::ContextCollapseRecovery { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_engine_events::Usage;
    use claude_core::ToolCall;
    use claude_query_engine::{
        QueryCheckpoint, QueryCheckpointKind, QueryContextBudgetState,
    };

    const SID: &str = "test-session-001";

    fn make_usage(input: u64, output: u64, total: u64) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            total_tokens: total,
            ..Default::default()
        }
    }

    fn make_tool_call(id: &str, name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall { id: id.to_string(), name: name.to_string(), input }
    }

    // ── Streaming events ──────────────────────────────────────────

    #[test]
    fn streaming_text_delta_maps_to_message_delta() {
        let event = QueryObserverEvent::StreamingTextDelta {
            turn: 1, delta: "Hello".into(), accumulated_text: "Hello".into(),
        };
        let result = map_observer_event(event, SID);
        match result {
            Some(UnifiedAgentEvent::MessageDelta { session_id, delta }) => {
                assert_eq!(session_id, SID);
                assert_eq!(delta, "Hello");
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn streaming_thinking_delta_has_prefix() {
        let event = QueryObserverEvent::StreamingThinkingDelta {
            turn: 1, delta: "reasoning".into(), accumulated_thinking: "reasoning".into(),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::MessageDelta { delta, .. } = &result {
            assert!(delta.starts_with("[thinking] "), "got: {delta}");
        } else {
            panic!("expected MessageDelta");
        }
    }

    #[test]
    fn streaming_tool_call_started_is_suppressed() {
        let event = QueryObserverEvent::StreamingToolCallStarted {
            turn: 1, tool_call_id: "tc-1".into(), tool_name: "read_file".into(),
        };
        assert!(map_observer_event(event, SID).is_none());
    }

    #[test]
    fn streaming_tool_call_delta_maps_to_progress() {
        let event = QueryObserverEvent::StreamingToolCallDelta {
            turn: 1, tool_call_id: "tc-1".into(), delta: "partial".into(),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ToolCallProgress { progress, .. } = &result {
            assert!(progress.contains("tc-1"));
        } else {
            panic!("expected ToolCallProgress");
        }
    }

    #[test]
    fn streaming_usage_updated_maps_to_context_usage() {
        let event = QueryObserverEvent::StreamingUsageUpdated {
            turn: 1, usage: make_usage(100, 50, 150),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ContextUsage { used, total, .. } = &result {
            assert_eq!(*used, 150);
            assert_eq!(*total, 0);
        } else {
            panic!("expected ContextUsage");
        }
    }

    // ── Tool execution ─────────────────────────────────────────────

    #[test]
    fn tool_call_started_maps_name_and_input() {
        let input = serde_json::json!({"path": "/tmp/test.txt"});
        let event = QueryObserverEvent::ToolCallStarted {
            tool_call: make_tool_call("tc-1", "read_file", input.clone()),
            turn: 1, batch_size: 1, batch_index: 0,
        };
        let result = map_observer_event(event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::ToolCallStarted { tool_name, tool_input, .. } => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(tool_input, input);
            }
            other => panic!("expected ToolCallStarted, got {other:?}"),
        }
    }

    #[test]
    fn tool_result_committed_maps_fields() {
        let tool_call = make_tool_call("tc-1", "read_file", serde_json::json!({}));
        let event = QueryObserverEvent::ToolResultCommitted {
            tool_call,
            result: claude_core::ToolResult {
                content: "contents".into(), is_error: false,
                content_blocks: vec![], follow_up_user_blocks: vec![],
            },
            turn: 1, total_messages: 5,
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ToolCallCompleted { result, tool_name, .. } = &result {
            assert_eq!(tool_name, "read_file");
            assert_eq!(result["tool_call_id"], "tc-1");
            assert_eq!(result["is_error"], false);
        } else {
            panic!("expected ToolCallCompleted");
        }
    }

    #[test]
    fn tool_result_error_sets_is_error() {
        let tool_call = make_tool_call("tc-2", "bash", serde_json::json!({}));
        let event = QueryObserverEvent::ToolResultCommitted {
            tool_call,
            result: claude_core::ToolResult {
                content: "command not found".into(), is_error: true,
                content_blocks: vec![], follow_up_user_blocks: vec![],
            },
            turn: 1, total_messages: 5,
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ToolCallCompleted { result, .. } = &result {
            assert_eq!(result["is_error"], true);
        } else {
            panic!("expected ToolCallCompleted");
        }
    }

    // ── Context management ─────────────────────────────────────────

    #[test]
    fn context_compaction_calculates_removed() {
        let event = QueryObserverEvent::ContextCompactionApplied {
            turn: 2, before_messages: 50, after_messages: 20,
            compacted_conversation: vec![], max_input_tokens: 200_000,
            threshold_tokens: 160_000, usage_ratio_before: 0.85,
            usage_ratio_after: 0.35, estimated_tokens_before: 170_000,
            estimated_tokens_after: 70_000,
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ContextCompacted { entries_removed, usage_ratio, .. } = &result {
            assert_eq!(*entries_removed, 30);
            assert!((usage_ratio - 0.35).abs() < f64::EPSILON);
        } else {
            panic!("expected ContextCompacted");
        }
    }

    #[test]
    fn context_budget_evaluated_maps_tokens() {
        let event = QueryObserverEvent::ContextBudgetEvaluated {
            turn: 1,
            context: QueryContextBudgetState {
                estimated_tokens: 80_000, max_input_tokens: 200_000,
                threshold_tokens: 160_000, usage_ratio: 0.4, needs_compaction: false,
            },
            message_count: 30,
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::ContextUsage { used, total, .. } = &result {
            assert_eq!(*used, 80_000);
            assert_eq!(*total, 200_000);
        } else {
            panic!("expected ContextUsage");
        }
    }

    // ── Terminal states ─────────────────────────────────────────────

    #[test]
    fn query_finished_maps_to_completed() {
        let event = QueryObserverEvent::QueryFinished {
            stop_reason: "end_turn".into(), turns: 3,
            final_text: Some("Done!".into()), usage: make_usage(500, 200, 700),
        };
        let result = map_observer_event(event, SID).expect("should map");
        match result {
            UnifiedAgentEvent::Completed { result, .. } => {
                assert_eq!(result.response_text, "Done!");
                assert_eq!(result.usage.input_tokens, 500);
                assert_eq!(result.usage.output_tokens, 200);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn query_finished_no_text_maps_to_empty() {
        let event = QueryObserverEvent::QueryFinished {
            stop_reason: "tool_use".into(), turns: 1,
            final_text: None, usage: make_usage(100, 50, 150),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::Completed { result, .. } = &result {
            assert!(result.response_text.is_empty());
        } else {
            panic!("expected Completed");
        }
    }

    #[test]
    fn query_failed_is_recoverable() {
        let event = QueryObserverEvent::QueryFailed {
            error: "rate limit".into(), turns: 2,
            consecutive_failures: 1, usage: make_usage(100, 0, 100),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::Error { message, recoverable, .. } = &result {
            assert_eq!(message, "rate limit");
            assert!(*recoverable);
        } else {
            panic!("expected Error");
        }
    }

    #[test]
    fn budget_exceeded_is_not_recoverable() {
        let event = QueryObserverEvent::BudgetExceeded {
            budget: claude_query_engine::QueryBudgetState {
                turn: 5, total_tokens: 100_000, max_turns: 5,
                max_total_tokens: Some(100_000),
            },
            reason: "max turns".into(),
        };
        let result = map_observer_event(event, SID).expect("should map");
        if let UnifiedAgentEvent::Error { message, recoverable, .. } = &result {
            assert!(message.contains("Budget exceeded"));
            assert!(!recoverable);
        } else {
            panic!("expected Error");
        }
    }

    // ── Suppressed events ──────────────────────────────────────────

    #[test]
    fn query_started_returns_none() {
        let event = QueryObserverEvent::QueryStarted {
            session_id: uuid::Uuid::new_v4().into(), existing_messages: 0, new_messages: 0,
        };
        assert!(map_observer_event(event, SID).is_none());
    }

    #[test]
    fn checkpoint_created_returns_none() {
        let event = QueryObserverEvent::CheckpointCreated {
            checkpoint: QueryCheckpoint::new(
                QueryCheckpointKind::ResumeBoundary, uuid::Uuid::new_v4().into(),
                1, None, vec![], 5,
            ),
        };
        assert!(map_observer_event(event, SID).is_none());
    }

    #[test]
    fn max_tokens_escalate_returns_none() {
        let event = QueryObserverEvent::MaxTokensEscalate {
            turn: 2, from_max_tokens: 4096, to_max_tokens: 16384,
        };
        assert!(map_observer_event(event, SID).is_none());
    }
}