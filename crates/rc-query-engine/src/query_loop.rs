use anyhow::{Result, anyhow};
use rc_core::{ConversationEntry, Message, ToolResult};
use rc_engine_events::{
    EngineEvent, EngineStateSnapshot, ToolError, ToolResult as EventToolResult, Usage,
};
use serde_json::json;

use crate::config::{ProcessUserInputContext, QueryEngineConfig};
use crate::engine::{
    EngineError, EngineState, QueryResult, assistant_message_from_response, budget_stop_message,
    tool_result_message,
};
use crate::token_budget::TokenBudgetDecision;

/// Execute the Phase 2 compat query loop in-memory.
pub async fn run_query_loop(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    user_input: Vec<Message>,
    context: &ProcessUserInputContext,
) -> Result<QueryResult, EngineError> {
    state.messages.extend(user_input);
    let max_turns = context
        .task_budget
        .as_ref()
        .and_then(|budget| budget.max_turns)
        .unwrap_or(config.max_turns);
    state.budget_tracker.max_turns = max_turns;
    state.budget_tracker.max_total_tokens = context
        .task_budget
        .as_ref()
        .and_then(|budget| budget.max_total_tokens);

    loop {
        match state
            .budget_tracker
            .evaluate(state.turn, state.usage.total_tokens())
        {
            TokenBudgetDecision::Continue => {}
            TokenBudgetDecision::Stop { reason } => {
                state.stop_reason = Some("budget_exceeded".to_owned());
                state.messages.push(budget_stop_message(reason.clone()));
                return Err(EngineError::Stopped(reason));
            }
        }

        let mut legacy_conversation = state.legacy_conversation();
        maybe_compact_conversation(config, state, &mut legacy_conversation);

        let response = config
            .backend
            .complete(&legacy_conversation)
            .await
            .map_err(|error| {
                state.consecutive_failures += 1;
                EngineError::Other(error)
            })?;
        state.consecutive_failures = 0;
        state.turn += 1;
        state.usage.record_summary(&response.usage);
        state.stop_reason = Some(response.stop_reason.clone());
        config.event_stream.emit(EngineEvent::UsageUpdated {
            usage: usage_from_accumulator(&state.usage),
        });
        state
            .messages
            .push(assistant_message_from_response(&response));
        config.event_stream.emit(EngineEvent::StateUpdated {
            state_snapshot: state_snapshot(state, response.tool_calls.len()),
        });

        if response.tool_calls.is_empty() {
            return Ok(QueryResult {
                state: state.clone(),
                final_text: (!response.text.trim().is_empty()).then_some(response.text),
                stop_reason: response.stop_reason,
                turns: state.turn,
                permission_denials: state.permission_denials.clone(),
            });
        }

        for tool_call in &response.tool_calls {
            config.event_stream.emit(EngineEvent::ToolUseStarted {
                tool_use_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                input: tool_call.input.clone(),
            });
            let tool_result = match config.tool_runner.run_tool(tool_call, context).await {
                Ok(result) => result,
                Err(error) => {
                    config.event_stream.emit(EngineEvent::ToolUseError {
                        tool_use_id: tool_call.id.clone(),
                        error: ToolError {
                            message: format!("{error:#}"),
                            retryable: false,
                        },
                    });
                    ToolResult {
                        content: format!("Tool execution error: {error:#}"),
                        is_error: true,
                    }
                }
            };
            record_permission_denial(state, tool_call, &tool_result);
            config.event_stream.emit(EngineEvent::ToolUseCompleted {
                tool_use_id: tool_call.id.clone(),
                result: EventToolResult {
                    content: tool_result.content.clone(),
                    is_error: tool_result.is_error,
                    mime_type: None,
                },
            });
            state
                .messages
                .push(tool_result_message(tool_call, &tool_result));
            config.event_stream.emit(EngineEvent::StateUpdated {
                state_snapshot: state_snapshot(state, 1),
            });
        }
    }
}

fn maybe_compact_conversation(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    legacy_conversation: &mut Vec<ConversationEntry>,
) {
    if !config.context_manager.needs_compaction(legacy_conversation) {
        return;
    }
    let compacted = config.context_manager.compact(legacy_conversation);
    if compacted.len() == legacy_conversation.len() {
        return;
    }
    *legacy_conversation = compacted;
    state.replace_from_legacy(legacy_conversation);
}

fn record_permission_denial(
    state: &mut EngineState,
    tool_call: &rc_core::ToolCall,
    tool_result: &ToolResult,
) {
    if tool_result.is_error
        && tool_result
            .content
            .to_ascii_lowercase()
            .contains("permission denied")
    {
        state.permission_denials.push(json!({
            "tool_name": tool_call.name,
            "tool_use_id": tool_call.id,
            "message": tool_result.content,
        }));
    }
}

fn usage_from_accumulator(accumulator: &rc_core::UsageAccumulator) -> Usage {
    Usage {
        input_tokens: accumulator.input_tokens,
        output_tokens: accumulator.output_tokens,
        cache_creation_input_tokens: accumulator.cache_creation_input_tokens,
        cache_read_input_tokens: accumulator.cache_read_input_tokens,
        total_tokens: accumulator.total_tokens(),
    }
}

fn state_snapshot(state: &EngineState, tool_call_count: usize) -> EngineStateSnapshot {
    EngineStateSnapshot {
        turn: state.turn,
        message_count: state.messages.len(),
        tool_call_count,
        usage: usage_from_accumulator(&state.usage),
    }
}

#[allow(dead_code)]
fn unknown_tool_error(tool_name: &str) -> EngineError {
    EngineError::Other(anyhow!("unknown tool {tool_name}"))
}
