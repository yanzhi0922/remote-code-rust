use anyhow::{Result, anyhow};
use rc_core::{ConversationEntry, Message, ToolResult};
use rc_engine_events::{
    CompactionResult, EngineEvent, EngineStateSnapshot, ToolError, ToolResult as EventToolResult,
};
use serde_json::json;

use crate::config::{ProcessUserInputContext, QueryEngineConfig};
use crate::engine::{
    EngineError, EngineState, QueryResult, assistant_message_from_response, budget_stop_message,
    tool_result_message, usage_from_accumulator,
};
use crate::observer::{
    QueryBudgetState, QueryCheckpoint, QueryCheckpointKind, QueryContextBudgetState,
    QueryObserverEvent,
};
use crate::token_budget::TokenBudgetDecision;

/// Execute the Phase 2 compat query loop in-memory.
pub async fn run_query_loop(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    user_input: Vec<Message>,
    context: &ProcessUserInputContext,
) -> Result<QueryResult, EngineError> {
    let appended_messages = user_input;
    state.messages.extend(appended_messages.clone());
    let _ = config
        .observer
        .on_event(QueryObserverEvent::MessagesAppended {
            session_id: context.session_id.clone(),
            appended: appended_messages,
            total_messages: state.messages.len(),
        })
        .await;
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
        let budget = QueryBudgetState {
            turn: state.turn,
            total_tokens: state.usage.total_tokens(),
            max_turns: state.budget_tracker.max_turns,
            max_total_tokens: state.budget_tracker.max_total_tokens,
        };
        let _ = config
            .observer
            .on_event(QueryObserverEvent::BudgetEvaluated {
                budget: budget.clone(),
            })
            .await;
        match state
            .budget_tracker
            .evaluate(budget.turn, budget.total_tokens)
        {
            TokenBudgetDecision::Continue => {}
            TokenBudgetDecision::Stop { reason } => {
                state.stop_reason = Some("budget_exceeded".to_owned());
                let stop_message = budget_stop_message(reason.clone());
                state.messages.push(stop_message.clone());
                let _ = config
                    .observer
                    .on_event(QueryObserverEvent::BudgetExceeded {
                        budget,
                        reason: reason.clone(),
                    })
                    .await;
                let _ = config
                    .observer
                    .on_event(QueryObserverEvent::MessagesAppended {
                        session_id: context.session_id.clone(),
                        appended: vec![stop_message],
                        total_messages: state.messages.len(),
                    })
                    .await;
                return Err(EngineError::Stopped(reason));
            }
        }

        let mut legacy_conversation = state.legacy_conversation();
        maybe_compact_conversation(config, state, &mut legacy_conversation).await;

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
        let assistant_message = assistant_message_from_response(&response);
        state.messages.push(assistant_message.clone());
        let _ = config
            .observer
            .on_event(QueryObserverEvent::AssistantMessageCommitted {
                message: assistant_message.clone(),
                stop_reason: response.stop_reason.clone(),
                turn: state.turn,
            })
            .await;
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

        let checkpoints = checkpoints_for_tool_batch(state, context, &assistant_message, &response);
        for checkpoint in &checkpoints {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::CheckpointCreated {
                    checkpoint: checkpoint.clone(),
                })
                .await;
        }

        for (batch_index, tool_call) in response.tool_calls.iter().enumerate() {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::ToolCallStarted {
                    tool_call: tool_call.clone(),
                    turn: state.turn,
                    batch_size: response.tool_calls.len(),
                    batch_index,
                })
                .await;
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
            let _ = config
                .observer
                .on_event(QueryObserverEvent::ToolResultCommitted {
                    tool_call: tool_call.clone(),
                    result: tool_result.clone(),
                    turn: state.turn,
                    total_messages: state.messages.len(),
                })
                .await;
            config.event_stream.emit(EngineEvent::StateUpdated {
                state_snapshot: state_snapshot(state, 1),
            });
        }

        for checkpoint in checkpoints {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::CheckpointCleared { checkpoint })
                .await;
        }
    }
}

async fn maybe_compact_conversation(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    legacy_conversation: &mut Vec<ConversationEntry>,
) {
    let before_snapshot = config.context_manager.budget_snapshot(legacy_conversation);
    let needs_compaction = before_snapshot.exceeds_threshold();
    let _ = config
        .observer
        .on_event(QueryObserverEvent::ContextBudgetEvaluated {
            context: QueryContextBudgetState {
                estimated_tokens: before_snapshot.estimated_tokens,
                max_input_tokens: before_snapshot.max_input_tokens,
                threshold_tokens: before_snapshot.threshold_tokens(),
                usage_ratio: before_snapshot.usage_ratio,
                needs_compaction,
            },
            message_count: legacy_conversation.len(),
        })
        .await;
    if !needs_compaction {
        return;
    }
    config.event_stream.emit(EngineEvent::CompactStarted {
        strategy: "standard".to_owned(),
    });
    let before_messages = legacy_conversation.len();
    let compacted = config.context_manager.compact(legacy_conversation);
    if compacted.len() == before_messages {
        return;
    }
    let after_snapshot = config.context_manager.budget_snapshot(&compacted);
    *legacy_conversation = compacted;
    state.replace_from_legacy(legacy_conversation);
    config.event_stream.emit(EngineEvent::CompactCompleted {
        result: CompactionResult {
            strategy: "standard".to_owned(),
            before_messages,
            after_messages: legacy_conversation.len(),
            summary: Some("compat context compaction applied".to_owned()),
        },
    });
    let _ = config
        .observer
        .on_event(QueryObserverEvent::ContextCompactionApplied {
            before_messages,
            after_messages: legacy_conversation.len(),
            estimated_tokens_before: before_snapshot.estimated_tokens,
            estimated_tokens_after: after_snapshot.estimated_tokens,
        })
        .await;
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

fn state_snapshot(state: &EngineState, tool_call_count: usize) -> EngineStateSnapshot {
    EngineStateSnapshot {
        turn: state.turn,
        message_count: state.messages.len(),
        tool_call_count,
        usage: usage_from_accumulator(&state.usage),
    }
}

fn checkpoints_for_tool_batch(
    state: &EngineState,
    context: &ProcessUserInputContext,
    assistant_message: &Message,
    response: &rc_core::ProviderResponse,
) -> Vec<QueryCheckpoint> {
    let tool_use_ids = response
        .tool_calls
        .iter()
        .map(|tool_call| tool_call.id.clone())
        .collect::<Vec<_>>();
    let assistant_message_id = Some(assistant_message.uuid());

    vec![
        QueryCheckpoint::new(
            QueryCheckpointKind::ResumeBoundary,
            context.session_id.clone(),
            state.turn,
            assistant_message_id,
            tool_use_ids.clone(),
            state.messages.len(),
        ),
        QueryCheckpoint::new(
            QueryCheckpointKind::ToolBatch,
            context.session_id.clone(),
            state.turn,
            assistant_message_id,
            tool_use_ids,
            state.messages.len(),
        ),
    ]
}

#[allow(dead_code)]
fn unknown_tool_error(tool_name: &str) -> EngineError {
    EngineError::Other(anyhow!("unknown tool {tool_name}"))
}
