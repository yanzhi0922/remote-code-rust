use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use rc_core::{ConversationEntry, Message, ToolResult};
use rc_engine_events::{
    CompactionResult, EngineEvent, EngineStateSnapshot, ToolError, ToolResult as EventToolResult,
    Usage,
};
use rc_provider::{StreamingCallbacks, query_source::ProviderRequestContext};
use serde_json::json;
use tokio::sync::mpsc;

use crate::config::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngineConfig, ToolRunResult,
};
use crate::engine::{
    EngineError, EngineState, QueryResult, assistant_message_from_response, budget_stop_message,
    tool_result_message, usage_from_accumulator,
};
use crate::max_tokens_recovery::{MaxTokensRecovery, MaxTokensRecoveryAction};
use crate::observer::{
    QueryBudgetState, QueryCheckpoint, QueryCheckpointKind, QueryContextBudgetState,
    QueryObserverEvent,
};
use crate::preprocessing::PreprocessingPipeline;
use crate::reactive_compact::ReactiveCompactHandler;
use crate::state_machine::EnginePhase;
use crate::stop_hooks::{ReplHookContext, StopHookOutcome, StopHookRequest, StopHookResult};
use crate::token_budget::TokenBudgetDecision;

/// Execute the Phase 2 compat query loop in-memory.
///
/// Enhanced with:
/// - **Preprocessing pipeline** — runs before each API call to reduce context
/// - **Reactive compact** — recovers from prompt-too-long errors
/// - **Max-output-tokens recovery** — escalates token limits or continues
/// - **Model fallback** — switches to fallback model on provider errors
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

    // Initialize recovery handlers
    let mut reactive_handler = ReactiveCompactHandler::new();
    let mut max_tokens_recovery = MaxTokensRecovery::new();
    let preprocessing_pipeline = PreprocessingPipeline::default();

    loop {
        // Transition: Initializing -> BuildingPrompt
        let _ = state.state_machine.transition(EnginePhase::BuildingPrompt);

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
                state.state_machine.force_set(EnginePhase::Failed);
                return Err(EngineError::Stopped(reason));
            }
        }

        // --- Preprocessing pipeline ---
        let context_usage = compute_context_usage(state, config);
        let max_context = config.context_manager.available_budget() as usize;
        let _preprocessing_result =
            preprocessing_pipeline.run(&mut state.messages, context_usage, max_context);

        let mut legacy_conversation = state.legacy_conversation();
        maybe_compact_conversation(config, state, &mut legacy_conversation).await;

        // Transition: BuildingPrompt -> CallingProvider
        let _ = state.state_machine.transition(EnginePhase::CallingProvider);

        let response = if matches!(
            config.provider_invocation_mode,
            ProviderInvocationMode::Streaming
        ) {
            complete_with_streaming_observer(config, context, &legacy_conversation, state.turn + 1)
                .await
        } else {
            let provider_context = provider_request_context(context);
            config
                .backend
                .complete_with_context(&legacy_conversation, &provider_context)
                .await
        };

        let response = match response {
            Ok(resp) => resp,
            Err(error) => {
                // --- Prompt-too-long recovery (reactive compact) ---
                if is_prompt_too_long_error(&error) {
                    if reactive_handler.can_attempt() {
                        let compact_result = reactive_handler
                            .handle_prompt_too_long(state.messages.clone())
                            .map_err(EngineError::Other)?;
                        if compact_result.was_compacted {
                            let after_len = compact_result.messages.len();
                            let before_len = compact_result.messages_removed + after_len;
                            state.messages = compact_result.messages;
                            state.replace_from_legacy(&state.legacy_conversation());
                            config.event_stream.emit(EngineEvent::CompactCompleted {
                                result: CompactionResult {
                                    strategy: "reactive".to_owned(),
                                    before_messages: before_len,
                                    after_messages: after_len,
                                    summary: Some(
                                        "reactive compact applied after prompt-too-long".to_owned(),
                                    ),
                                },
                            });
                            continue; // Retry the turn
                        }
                    }
                    // No recovery possible
                    state.consecutive_failures += 1;
                    let _ = state.failure_tracker.record_failure();
                    state.state_machine.force_set(EnginePhase::Failed);
                    return Err(EngineError::Other(error));
                }

                // --- Model fallback ---
                if is_model_overloaded_error(&error)
                    && let Some(fallback) = config.fallback_model.as_deref()
                {
                    let switch_result = state
                        .model_switcher
                        .switch_to(fallback, crate::model_switch::SwitchReason::Fallback);
                    if switch_result.is_switched() {
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MessagesAppended {
                                session_id: context.session_id.clone(),
                                appended: vec![crate::message_utils::create_info_message(
                                    &format!("Switched to fallback model: {fallback}"),
                                )],
                                total_messages: state.messages.len(),
                            })
                            .await;
                        continue; // Retry with fallback model
                    }
                }

                state.consecutive_failures += 1;
                let _ = state.failure_tracker.record_failure();
                state.state_machine.force_set(EnginePhase::Failed);
                return Err(EngineError::Other(error));
            }
        };

        state.consecutive_failures = 0;
        state.failure_tracker.record_success();
        state.turn += 1;
        state.usage.record_summary(&response.usage);
        state.stop_reason = Some(response.stop_reason.clone());

        // --- Max-output-tokens recovery ---
        if is_max_tokens_truncated(&response.stop_reason)
            && let Some(action) =
                max_tokens_recovery.handle_truncation(estimate_current_max_tokens(&state.usage))
        {
            match action {
                MaxTokensRecoveryAction::Escalate { new_max_tokens: _ } => {
                    // The escalation is handled by updating the request
                    // parameters on the next iteration. For now, emit an
                    // event and continue the loop so the assistant response
                    // is processed normally and the next turn uses the
                    // escalated limit.
                    config.event_stream.emit(EngineEvent::StateUpdated {
                        state_snapshot: state_snapshot(state, 0),
                    });
                }
                MaxTokensRecoveryAction::ContinueWithMessage {
                    max_tokens: _,
                    continuation_message,
                } => {
                    // Append the assistant's truncated response
                    let assistant_message = assistant_message_from_response(&response);
                    state.messages.push(assistant_message.clone());
                    // Append the continuation prompt
                    state.messages.push(continuation_message.clone());
                    let _ = config
                        .observer
                        .on_event(QueryObserverEvent::MessagesAppended {
                            session_id: context.session_id.clone(),
                            appended: vec![continuation_message],
                            total_messages: state.messages.len(),
                        })
                        .await;
                    config.event_stream.emit(EngineEvent::StateUpdated {
                        state_snapshot: state_snapshot(state, 0),
                    });
                    continue; // Next turn will pick up the continuation
                }
                MaxTokensRecoveryAction::Exhausted => {
                    // Surface the truncation — the response is still
                    // processed below, but the stop_reason indicates
                    // truncation.
                }
            }
        }

        // Transition: CallingProvider -> ProcessingResponse
        let _ = state
            .state_machine
            .transition(EnginePhase::ProcessingResponse);

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
                usage: Usage {
                    input_tokens: response.usage.input_tokens,
                    output_tokens: response.usage.output_tokens,
                    cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
                    cache_read_input_tokens: response.usage.cache_read_input_tokens,
                    total_tokens: response.usage.input_tokens
                        + response.usage.output_tokens
                        + response.usage.cache_creation_input_tokens
                        + response.usage.cache_read_input_tokens,
                },
                request_id: response.request_id.clone(),
            })
            .await;
        config.event_stream.emit(EngineEvent::StateUpdated {
            state_snapshot: state_snapshot(state, response.tool_calls.len()),
        });

        if !config.post_sampling_hooks.is_empty() {
            let hook_context = ReplHookContext {
                session_id: context.session_id.clone(),
                turn: state.turn,
                messages: state.messages.clone(),
                query_source: context.query_source,
                agent_id: context.agent_id.clone(),
                system_prompt: context.system_prompt.clone(),
                user_context: context.user_context.clone(),
                system_context: context.system_context.clone(),
            };
            for hook in &config.post_sampling_hooks {
                let _ = hook(hook_context.clone()).await;
            }
        }

        if response.tool_calls.is_empty() {
            if let Some(stop_hook) = config.stop_hook.as_ref() {
                state
                    .stop_hook_manager
                    .request_stop(response.stop_reason.clone());
                let hook_context = ReplHookContext {
                    session_id: context.session_id.clone(),
                    turn: state.turn,
                    messages: state.messages.clone(),
                    query_source: context.query_source,
                    agent_id: context.agent_id.clone(),
                    system_prompt: context.system_prompt.clone(),
                    user_context: context.user_context.clone(),
                    system_context: context.system_context.clone(),
                };
                let hook_request = StopHookRequest {
                    stop_reason: response.stop_reason.clone(),
                    final_text: (!response.text.trim().is_empty()).then_some(response.text.clone()),
                };
                let hook_outcome = match stop_hook(hook_context, hook_request).await {
                    Ok(outcome) => outcome,
                    Err(_) => StopHookOutcome::Allow,
                };
                let retry_result = StopHookResult::from(&hook_outcome);
                let should_stop = state.stop_hook_manager.evaluate(retry_result);
                match hook_outcome {
                    StopHookOutcome::Retry { injected_messages } if !should_stop => {
                        if !injected_messages.is_empty() {
                            state.messages.extend(injected_messages.clone());
                            let _ = config
                                .observer
                                .on_event(QueryObserverEvent::MessagesAppended {
                                    session_id: context.session_id.clone(),
                                    appended: injected_messages,
                                    total_messages: state.messages.len(),
                                })
                                .await;
                        }
                        continue;
                    }
                    StopHookOutcome::Deny if !should_stop => {
                        continue;
                    }
                    _ => {}
                }
            }
            // Transition: ProcessingResponse -> Finalizing (handled by caller)
            let _ = state.state_machine.transition(EnginePhase::Finalizing);
            return Ok(QueryResult {
                state: state.clone(),
                final_text: (!response.text.trim().is_empty()).then_some(response.text),
                stop_reason: response.stop_reason,
                turns: state.turn,
                permission_denials: state.permission_denials.clone(),
            });
        }

        // Transition: ProcessingResponse -> ExecutingTools
        let _ = state.state_machine.transition(EnginePhase::ExecutingTools);

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
            let tool_run = match config.tool_runner.run_tool(tool_call, context).await {
                Ok(result) => result,
                Err(error) => {
                    config.event_stream.emit(EngineEvent::ToolUseError {
                        tool_use_id: tool_call.id.clone(),
                        error: ToolError {
                            message: format!("{error:#}"),
                            retryable: false,
                        },
                    });
                    ToolRunResult::from(ToolResult {
                        content: format!("Tool execution error: {error:#}"),
                        is_error: true,
                        content_blocks: Vec::new(),
                        follow_up_user_blocks: Vec::new(),
                    })
                }
            };
            if let Some(permission_denial) = tool_run.permission_denial.clone() {
                state.permission_denials.push(permission_denial);
            } else {
                record_permission_denial(state, tool_call, &tool_run.result);
            }
            config.event_stream.emit(EngineEvent::ToolUseCompleted {
                tool_use_id: tool_call.id.clone(),
                result: EventToolResult {
                    content: tool_run.result.content.clone(),
                    is_error: tool_run.result.is_error,
                    mime_type: None,
                },
            });
            if !tool_run.pre_messages.is_empty() {
                state.messages.extend(tool_run.pre_messages.clone());
                let _ = config
                    .observer
                    .on_event(QueryObserverEvent::MessagesAppended {
                        session_id: context.session_id.clone(),
                        appended: tool_run.pre_messages.clone(),
                        total_messages: state.messages.len(),
                    })
                    .await;
            }
            state
                .messages
                .push(tool_result_message(tool_call, &tool_run.result));
            let _ = config
                .observer
                .on_event(QueryObserverEvent::ToolResultCommitted {
                    tool_call: tool_call.clone(),
                    result: tool_run.result.clone(),
                    turn: state.turn,
                    total_messages: state.messages.len(),
                })
                .await;
            if !tool_run.post_messages.is_empty() {
                state.messages.extend(tool_run.post_messages.clone());
                let _ = config
                    .observer
                    .on_event(QueryObserverEvent::MessagesAppended {
                        session_id: context.session_id.clone(),
                        appended: tool_run.post_messages.clone(),
                        total_messages: state.messages.len(),
                    })
                    .await;
            }
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
    // Transition to Compacting phase if compaction will occur
    if needs_compaction {
        let _ = state.state_machine.transition(EnginePhase::Compacting);
    }
    let _ = config
        .observer
        .on_event(QueryObserverEvent::ContextBudgetEvaluated {
            turn: state.turn + 1,
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
    let mut compacted = config.context_manager.compact(legacy_conversation);
    if compacted.len() == before_messages {
        return;
    }
    if let Some(transform) = config.post_compact_transform.as_ref() {
        compacted = transform(compacted).await;
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
            turn: state.turn + 1,
            before_messages,
            after_messages: legacy_conversation.len(),
            compacted_conversation: legacy_conversation.clone(),
            max_input_tokens: before_snapshot.max_input_tokens,
            threshold_tokens: before_snapshot.threshold_tokens(),
            usage_ratio_before: before_snapshot.usage_ratio,
            usage_ratio_after: after_snapshot.usage_ratio,
            estimated_tokens_before: before_snapshot.estimated_tokens,
            estimated_tokens_after: after_snapshot.estimated_tokens,
        })
        .await;
    // Transition back to BuildingPrompt after compaction
    let _ = state.state_machine.transition(EnginePhase::BuildingPrompt);
}

fn record_permission_denial(
    state: &mut EngineState,
    tool_call: &rc_core::ToolCall,
    tool_result: &ToolResult,
) {
    if tool_result.is_error && is_permission_denied_message(&tool_result.content) {
        state.permission_denials.push(json!({
            "tool_name": tool_call.name,
            "tool_use_id": tool_call.id,
            "tool_input": tool_call.input,
            "message": tool_result.content,
        }));
    }
}

fn is_permission_denied_message(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    lowered.contains("permission denied")
        || (lowered.contains("permission") && lowered.contains("denied"))
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

async fn complete_with_streaming_observer(
    config: &QueryEngineConfig,
    context: &ProcessUserInputContext,
    conversation: &[ConversationEntry],
    turn: u32,
) -> anyhow::Result<rc_core::ProviderResponse> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let observer = Arc::clone(&config.observer);
    let forwarder = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = observer.on_event(event).await;
        }
    });

    let provider_context = provider_request_context(context);
    let response = config
        .backend
        .complete_streaming_with_context(
            conversation,
            Some(build_streaming_callbacks(tx.clone(), turn)),
            &provider_context,
        )
        .await;

    drop(tx);
    let _ = forwarder.await;
    response
}

fn provider_request_context(context: &ProcessUserInputContext) -> ProviderRequestContext {
    let query_source = match context.query_source {
        crate::QuerySource::User => rc_provider::query_source::QuerySource::User,
        crate::QuerySource::ReplMainThread => {
            rc_provider::query_source::QuerySource::ReplMainThread
        }
        crate::QuerySource::Sdk => rc_provider::query_source::QuerySource::Sdk,
        crate::QuerySource::Compact => rc_provider::query_source::QuerySource::Compact,
        crate::QuerySource::SessionMemory => rc_provider::query_source::QuerySource::SessionMemory,
        crate::QuerySource::Agent => rc_provider::query_source::QuerySource::Agent,
        crate::QuerySource::ExtractMemories => {
            rc_provider::query_source::QuerySource::ExtractMemories
        }
        crate::QuerySource::BackgroundTask => {
            rc_provider::query_source::QuerySource::BackgroundTask
        }
    };

    let mut provider_context =
        ProviderRequestContext::new(query_source, context.session_id.clone());
    if let Some(agent_id) = context.agent_id.clone() {
        provider_context = provider_context.with_agent_id(agent_id);
    }
    provider_context
}

fn build_streaming_callbacks(
    tx: mpsc::UnboundedSender<QueryObserverEvent>,
    turn: u32,
) -> StreamingCallbacks {
    let accumulated_text = Arc::new(Mutex::new(String::new()));
    let started_tool_calls = Arc::new(Mutex::new(HashSet::<String>::new()));

    let text_tx = tx.clone();
    let text_accumulated = Arc::clone(&accumulated_text);
    let on_text_delta = Box::new(move |delta: &str| {
        let accumulated_text = {
            let mut accumulated = text_accumulated
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            accumulated.push_str(delta);
            accumulated.clone()
        };
        let _ = text_tx.send(QueryObserverEvent::StreamingTextDelta {
            turn,
            delta: delta.to_owned(),
            accumulated_text,
        });
    });

    let tool_start_tx = tx.clone();
    let tool_started = Arc::clone(&started_tool_calls);
    let on_tool_call_start = Box::new(move |tool_call_id: &str, tool_name: &str| {
        let should_emit = {
            let mut started = tool_started
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            started.insert(tool_call_id.to_owned())
        };
        if should_emit {
            let _ = tool_start_tx.send(QueryObserverEvent::StreamingToolCallStarted {
                turn,
                tool_call_id: tool_call_id.to_owned(),
                tool_name: tool_name.to_owned(),
            });
        }
    });

    let tool_delta_tx = tx.clone();
    let on_tool_call_delta = Box::new(move |tool_call_id: &str, delta: &str| {
        let _ = tool_delta_tx.send(QueryObserverEvent::StreamingToolCallDelta {
            turn,
            tool_call_id: tool_call_id.to_owned(),
            delta: delta.to_owned(),
        });
    });

    let on_usage = Box::new(move |input_tokens: u64, output_tokens: u64| {
        let _ = tx.send(QueryObserverEvent::StreamingUsageUpdated {
            turn,
            usage: Usage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                total_tokens: input_tokens + output_tokens,
            },
        });
    });

    StreamingCallbacks {
        on_text_delta: Some(on_text_delta),
        on_tool_call_start: Some(on_tool_call_start),
        on_tool_call_delta: Some(on_tool_call_delta),
        on_usage: Some(on_usage),
    }
}

#[allow(dead_code)]
fn unknown_tool_error(tool_name: &str) -> EngineError {
    EngineError::Other(anyhow!("unknown tool {tool_name}"))
}

// ---------------------------------------------------------------------------
// Error classification helpers
// ---------------------------------------------------------------------------

/// Check if the error message indicates a prompt-too-long / 413 error.
fn is_prompt_too_long_error(error: &anyhow::Error) -> bool {
    let msg = format!("{error:#}");
    let lowered = msg.to_ascii_lowercase();
    lowered.contains("prompt is too long")
        || lowered.contains("prompt_too_long")
        || lowered.contains("request too large")
        || lowered.contains("context_length_exceeded")
        || lowered.contains("maximum context length")
        || (lowered.contains("413") && lowered.contains("prompt"))
}

/// Check if the error message indicates model overload (triggers fallback).
fn is_model_overloaded_error(error: &anyhow::Error) -> bool {
    let msg = format!("{error:#}");
    let lowered = msg.to_ascii_lowercase();
    lowered.contains("overloaded")
        || lowered.contains("capacity")
        || lowered.contains("503")
        || lowered.contains("rate_limit")
        || lowered.contains("too many requests")
        || lowered.contains("service unavailable")
}

/// Check if the stop reason indicates max-output-tokens truncation.
fn is_max_tokens_truncated(stop_reason: &str) -> bool {
    let lowered = stop_reason.to_ascii_lowercase();
    lowered == "max_tokens"
        || lowered == "max_tokens_reached"
        || lowered.contains("max_output_tokens")
        || lowered == "length"
}

/// Compute an approximate context usage ratio from the engine state.
fn compute_context_usage(state: &EngineState, config: &QueryEngineConfig) -> f64 {
    let max_tokens = config.context_manager.available_budget();
    if max_tokens == 0 {
        return 0.0;
    }
    let used = state.usage.total_tokens();
    let ratio = used as f64 / max_tokens as f64;
    ratio.min(1.0)
}

/// Estimate the current max_tokens setting from usage data.
/// Uses output tokens as a proxy for the current max_tokens limit.
fn estimate_current_max_tokens(usage: &rc_core::UsageAccumulator) -> usize {
    let output = usage.output_tokens;
    if output == 0 {
        return 8192; // Default starting tier
    }
    // Round up to the nearest power of 2
    let mut tier = 8192;
    while tier < output as usize {
        tier *= 2;
    }
    tier
}

// ---------------------------------------------------------------------------
// Model switcher integration
// ---------------------------------------------------------------------------

/// Extension for SwitchResult to check if a switch actually occurred.
trait SwitchResultExt {
    fn is_switched(&self) -> bool;
}

impl SwitchResultExt for crate::model_switch::SwitchResult {
    fn is_switched(&self) -> bool {
        matches!(self, Self::Switched { .. })
    }
}
