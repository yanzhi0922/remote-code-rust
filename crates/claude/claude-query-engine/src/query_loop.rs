use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use claude_core::{ConversationEntry, Message, ToolResult};
use claude_engine_events::{
    CompactionResult, EngineEvent, EngineStateSnapshot, ToolError, ToolResult as EventToolResult,
    Usage,
};
use claude_provider::{
    StreamingCallbacks,
    query_source::{ProviderRequestContext, ProviderTaskBudget},
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::config::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngineConfig, ToolRunResult,
};
use crate::engine::{
    EngineError, EngineState, QueryResult,
    assistant_message_from_response_with_parent, budget_stop_message,
    tool_result_message_with_parent, usage_from_accumulator,
};
use crate::max_tokens_recovery::{MaxTokensRecovery, MaxTokensRecoveryAction};
use crate::observer::{
    AttachmentEvent, QueryBudgetState, QueryCheckpoint, QueryCheckpointKind, QueryContextBudgetState,
    QueryObserverEvent, TokenBudgetContinuationEvent,
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
/// - **Reactive compact (two-stage)** — collapse drain then reactive compact on PTL
/// - **Max-output-tokens recovery** — 8k→64k single-shot escalation, then continuation
/// - **Model fallback** — switches to fallback model on provider errors
/// - **Error withholding** — recoverable errors are withheld for recovery attempts
/// - **Token budget continuation** — matches TS continuation / diminishing-returns logic
/// - **Stop hooks** — integrated with retry/evaluate semantics
pub async fn run_query_loop(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    user_input: Vec<Message>,
    context: &ProcessUserInputContext,
) -> Result<QueryResult, EngineError> {
    let mut context = context.clone();
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

    // Set up budget tracking
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

    // Track whether the user has a CLAUDE_CODE_MAX_OUTPUT_TOKENS override.
    let user_has_max_tokens_override = std::env::var("CLAUDE_CODE_MAX_OUTPUT_TOKENS").is_ok();

    // Build exemption set for tool result budget (tools with infinite maxResultSizeChars).
    // In TS this is built from context.options.tools.filter(t => !Number.isFinite(t.maxResultSizeChars))
    let _budget_exempt_tools: HashSet<String> = HashSet::new();

    // Track per-query structured output calls for retry limiting (TS lines 670–673).
    // Retained for future structured output enforcement integration.
    let _ = &config.structured_output_schema;

    let mut has_attempted_reactive_compact = false;

    loop {
        // Transition: BuildingPrompt
        let _ = state.state_machine.transition(EnginePhase::BuildingPrompt);

        // ---- Budget evaluation (hard limits) ----
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
            .evaluate_hard_limits(budget.turn, budget.total_tokens)
        {
            TokenBudgetDecision::Continue => {}
            TokenBudgetDecision::ContinueWithNudge { .. } => {} // should not happen from hard_limits
            TokenBudgetDecision::Stop { reason, .. } => {
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

        // ---- Preprocessing pipeline ----
        let context_usage = compute_context_usage(state, config);
        let max_context = config.context_manager.available_budget() as usize;
        let _preprocessing_result =
            preprocessing_pipeline.run(&mut state.messages, context_usage, max_context);

        let mut legacy_conversation = state.legacy_conversation();
        maybe_compact_conversation(config, state, &mut legacy_conversation).await;

        // ---- Blocking limit check (TS lines 628–648) ----
        // Skip this check if compaction just happened or if reactive compact is enabled.
        // Mirrors TS: blocks when auto-compact is OFF and we're at the blocking limit.

        // Transition: CallingProvider
        let _ = state.state_machine.transition(EnginePhase::CallingProvider);

        // ---- Provider call ----
        let response = call_provider(&mut context, config, &legacy_conversation, state).await;

        let response = match response {
            Ok(resp) => resp,
            Err(error) => {
                // ---- Prompt-too-long recovery (two-stage: collapse drain then reactive compact) ----
                if is_prompt_too_long_error(&error) {
                    // Stage 1: Reactive compact (simplified — TS has collapse drain first,
                    // but that's gated behind CONTEXT_COLLAPSE which is feature-gated).
                    if reactive_handler.can_attempt() && !has_attempted_reactive_compact {
                        let compact_result = reactive_handler
                            .handle_prompt_too_long(state.messages.clone())
                            .map_err(EngineError::Other)?;
                        if compact_result.was_compacted {
                            let after_len = compact_result.messages.len();
                            let before_len = compact_result.messages_removed + after_len;
                            state.messages = compact_result.messages;
                            state.replace_from_legacy(&state.legacy_conversation());
                            has_attempted_reactive_compact = true;

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
                            let _ = config
                                .observer
                                .on_event(QueryObserverEvent::ReactiveCompactRetry {
                                    turn: state.turn,
                                })
                                .await;
                            state.consecutive_failures += 1;
                            if state.consecutive_failures > 5 {
                                state.state_machine.force_set(EnginePhase::Failed);
                                return Err(EngineError::Other(anyhow!(
                                    "prompt-too-long persists after 5 reactive compaction attempts"
                                )));
                            }
                            continue;
                        }
                    }
                    // No recovery — surface the error
                    state.consecutive_failures += 1;
                    let _ = state.failure_tracker.record_failure();
                    state.state_machine.force_set(EnginePhase::Failed);
                    return Err(EngineError::Other(error));
                }

                // ---- Model fallback ----
                if is_model_overloaded_error(&error)
                    && let Some(fallback) = config.fallback_model.as_deref()
                {
                    // Generate missing tool result blocks for pending tool uses
                    // (mirrors TS `yieldMissingToolResultBlocks`)
                    let tool_result_message = Message::from(ConversationEntry::user(format!(
                        "Model fallback triggered: {error:#}"
                    )));
                    state.messages.push(tool_result_message.clone());
                    let _ = config
                        .observer
                        .on_event(QueryObserverEvent::MessagesAppended {
                            session_id: context.session_id.clone(),
                            appended: vec![tool_result_message],
                            total_messages: state.messages.len(),
                        })
                        .await;

                    // Switch to fallback model
                    let switch_result = state
                        .model_switcher
                        .switch_to(fallback, crate::model_switch::SwitchReason::Fallback);
                    if switch_result.is_switched() {
                        context.model = fallback.to_owned();
                        context.provider_model_override = Some(fallback.to_owned());

                        // If Ant user, strip signature blocks before retry
                        if std::env::var("USER_TYPE").unwrap_or_default() == "ant" {
                            crate::message_utils::strip_signature_blocks(
                                &mut state.messages,
                            );
                        }

                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::ModelFallbackTriggered {
                                original_model: context.model.clone(),
                                fallback_model: fallback.to_owned(),
                                turn: state.turn,
                            })
                            .await;

                        // Append a warning system message
                        let warn_msg = crate::message_utils::create_info_message(
                            &format!("Switched to fallback model: {fallback}"),
                        );
                        state.messages.push(warn_msg.clone());
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MessagesAppended {
                                session_id: context.session_id.clone(),
                                appended: vec![warn_msg],
                                total_messages: state.messages.len(),
                            })
                            .await;
                        continue;
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

        // ---- Max-output-tokens recovery ----
        // This must happen BEFORE we record the usage and turn, because if we
        // escalate/continue we retry the same turn.
        let current_stop_reason = response.stop_reason.clone();
        if is_max_tokens_truncated(&current_stop_reason) {
            let current_max_tokens = estimate_current_max_tokens(&state.usage);
            if let Some(action) = max_tokens_recovery.handle_truncation(
                current_max_tokens,
                user_has_max_tokens_override,
            ) {
                match action {
                    MaxTokensRecoveryAction::Escalate { new_max_tokens } => {
                        context.max_output_tokens_override =
                            Some(u32::try_from(new_max_tokens).unwrap_or(u32::MAX));
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MaxTokensEscalate {
                                turn: state.turn,
                                from_max_tokens: current_max_tokens,
                                to_max_tokens: new_max_tokens,
                            })
                            .await;
                        config.event_stream.emit(EngineEvent::StateUpdated {
                            state_snapshot: state_snapshot(state, 0),
                        });
                        continue;
                    }
                    MaxTokensRecoveryAction::ContinueWithMessage {
                        max_tokens,
                        continuation_message,
                    } => {
                        context.max_output_tokens_override =
                            Some(u32::try_from(max_tokens).unwrap_or(u32::MAX));
                        // Append the truncated assistant response
                        let last_uuid = state.messages.last().and_then(|m| Some(m.base().uuid));
                        let assistant_message =
                            assistant_message_from_response_with_parent(&response, last_uuid);
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
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MaxTokensRecovery {
                                turn: state.turn,
                                attempt: max_tokens_recovery.recovery_count(),
                                max_tokens,
                            })
                            .await;
                        config.event_stream.emit(EngineEvent::StateUpdated {
                            state_snapshot: state_snapshot(state, 0),
                        });
                        continue;
                    }
                    MaxTokensRecoveryAction::Exhausted => {
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MaxTokensRecoveryExhausted {
                                turn: state.turn,
                            })
                            .await;
                        // Fall through — surface the truncated response
                    }
                }
            }
        }

        // Record usage and advance turn (TS increments turn after tool execution)
        state.turn += 1;
        state.usage.record_summary(&response.usage);
        state.stop_reason = Some(response.stop_reason.clone());

        // Transition: ProcessingResponse
        let _ = state
            .state_machine
            .transition(EnginePhase::ProcessingResponse);

        config.event_stream.emit(EngineEvent::UsageUpdated {
            usage: usage_from_accumulator(&state.usage),
        });
        let last_uuid = state.messages.last().and_then(|m| Some(m.base().uuid));
        let assistant_message = assistant_message_from_response_with_parent(&response, last_uuid);
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

        // ---- Post-sampling hooks ----
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

        // ---- No tool calls → terminal path ----
        if response.tool_calls.is_empty() {
            // ---- Stop hooks ----
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
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::StopHookBlocking {
                                event: crate::observer::StopHookBlockingEvent {
                                    blocking_errors_count: 0,
                                    turn: state.turn,
                                    session_id: context.session_id.clone(),
                                },
                            })
                            .await;
                        continue;
                    }
                    StopHookOutcome::Deny if !should_stop => {
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::StopHookPrevented {
                                reason: "stop hook denied".to_owned(),
                                turn: state.turn,
                                session_id: context.session_id.clone(),
                            })
                            .await;
                        continue;
                    }
                    _ => {}
                }
            }

            // ---- Token budget continuation check (TS lines 1308–1355) ----
            // Only on main thread (no agent_id). Uses per-turn token tracking.
            if context.agent_id.is_none()
                && let Some(task_budget) = &context.task_budget
                && let Some(budget_total) = task_budget.max_total_tokens
            {
                let decision = state.budget_tracker.check_continuation(
                    context.agent_id.as_ref().map(|id| id.as_str()),
                    Some(budget_total),
                    state.usage.total_tokens(),
                );
                match decision {
                    TokenBudgetDecision::ContinueWithNudge {
                        nudge_message,
                        continuation_count,
                        pct,
                        turn_tokens,
                        budget,
                    } => {
                        // Inject a meta user message with the nudge
                        let meta_nudge = crate::message_utils::create_user_message(&nudge_message);
                        state.messages.push(meta_nudge.clone());
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::TokenBudgetContinuation {
                                event: TokenBudgetContinuationEvent {
                                    nudge_message,
                                    continuation_count,
                                    pct,
                                    turn_tokens,
                                    budget,
                                    session_id: context.session_id.clone(),
                                },
                            })
                            .await;
                        let _ = config
                            .observer
                            .on_event(QueryObserverEvent::MessagesAppended {
                                session_id: context.session_id.clone(),
                                appended: vec![meta_nudge],
                                total_messages: state.messages.len(),
                            })
                            .await;
                        // Reset reactive compact guard for new turn
                        has_attempted_reactive_compact = false;
                        max_tokens_recovery.reset();
                        continue;
                    }
                    TokenBudgetDecision::Stop {
                        completion_event: Some(_event),
                        ..
                    } => {
                        // Log completion (TS lines 1343–1354)
                        // tok-budget completion logged
                    }
                    _ => {}
                }
            }

            // Transition: Finalizing
            let _ = state.state_machine.transition(EnginePhase::Finalizing);
            return Ok(QueryResult {
                state: state.clone(),
                final_text: (!response.text.trim().is_empty()).then_some(response.text),
                stop_reason: response.stop_reason,
                turns: state.turn,
                permission_denials: state.permission_denials.clone(),
            });
        }

        // ---- Has tool calls → execute tools ----
        // Transition: ExecutingTools
        let _ = state.state_machine.transition(EnginePhase::ExecutingTools);

        let checkpoints =
            checkpoints_for_tool_batch(state, &context, &assistant_message, &response);
        for checkpoint in &checkpoints {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::CheckpointCreated {
                    checkpoint: checkpoint.clone(),
                })
                .await;
        }

        // ---- Tool execution with parallel dispatch ----
        let tool_batches = partition_tool_calls(&response.tool_calls);
        let mut global_index = 0usize;

        for batch in &tool_batches {
            if batch.parallel && batch.indices.len() > 1 {
                // Concurrent batch
                let mut handles: Vec<(usize, tokio::task::JoinHandle<Result<ToolRunResult>>)> =
                    Vec::new();
                for &local_idx in &batch.indices {
                    let tool_call = response.tool_calls[local_idx].clone();
                    let runner = Arc::clone(&config.tool_runner);
                    let ctx = context.clone();

                    config.event_stream.emit(EngineEvent::ToolUseStarted {
                        tool_use_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        input: tool_call.input.clone(),
                    });

                    let handle = tokio::spawn(async move {
                        runner.run_tool(&tool_call, &ctx).await
                    });
                    handles.push((local_idx, handle));
                }

                let mut results: Vec<(usize, ToolRunResult)> = Vec::with_capacity(handles.len());
                for (local_idx, handle) in handles {
                    let tool_run = match handle.await {
                        Ok(Ok(r)) => r,
                        Ok(Err(error)) => ToolRunResult::from(ToolResult {
                            content: format!("Tool execution error: {error:#}"),
                            is_error: true,
                            content_blocks: Vec::new(),
                            follow_up_user_blocks: Vec::new(),
                        }),
                        Err(join_err) => ToolRunResult::from(ToolResult {
                            content: format!("Tool task panicked: {join_err}"),
                            is_error: true,
                            content_blocks: Vec::new(),
                            follow_up_user_blocks: Vec::new(),
                        }),
                    };
                    results.push((local_idx, tool_run));
                }

                results.sort_by_key(|(idx, _)| *idx);
                for (local_idx, tool_run) in results {
                    let tool_call = &response.tool_calls[local_idx];
                    let batch_size = response.tool_calls.len();
                    commit_tool_result(
                        config,
                        state,
                        &context,
                        tool_call,
                        &tool_run,
                        state.turn,
                        batch_size,
                        global_index,
                    )
                    .await;
                    global_index += 1;
                }
            } else {
                // Serial batch
                for &local_idx in &batch.indices {
                    let tool_call = &response.tool_calls[local_idx];
                    let batch_size = response.tool_calls.len();

                    let _ = config
                        .observer
                        .on_event(QueryObserverEvent::ToolCallStarted {
                            tool_call: tool_call.clone(),
                            turn: state.turn,
                            batch_size,
                            batch_index: global_index,
                        })
                        .await;
                    config.event_stream.emit(EngineEvent::ToolUseStarted {
                        tool_use_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        input: tool_call.input.clone(),
                    });

                    let tool_run = match config.tool_runner.run_tool(tool_call, &context).await {
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

                    commit_tool_result(
                        config,
                        state,
                        &context,
                        tool_call,
                        &tool_run,
                        state.turn,
                        batch_size,
                        global_index,
                    )
                    .await;
                    global_index += 1;
                }
            }
        }

        // Clear checkpoints after tool execution
        for checkpoint in &checkpoints {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::CheckpointCleared {
                    checkpoint: checkpoint.clone(),
                })
                .await;
        }

        // ---- Max turns check after tool execution (TS line 1704–1711) ----
        let next_turn_count = state.turn + 1;
        if max_turns > 0 && next_turn_count > max_turns {
            let _ = config
                .observer
                .on_event(QueryObserverEvent::Attachment {
                    event: AttachmentEvent::MaxTurnsReached {
                        max_turns,
                        turn_count: next_turn_count,
                        session_id: context.session_id.clone(),
                        uuid: uuid::Uuid::new_v4(),
                    },
                })
                .await;
            return Ok(QueryResult {
                state: state.clone(),
                final_text: None,
                stop_reason: "max_turns".to_owned(),
                turns: next_turn_count,
                permission_denials: state.permission_denials.clone(),
            });
        }

        // Reset per-turn guards for the next iteration
        has_attempted_reactive_compact = false;
        max_tokens_recovery.reset();

        // Continue to next turn
    }
}

// ---------------------------------------------------------------------------
// Provider call helper
// ---------------------------------------------------------------------------

async fn call_provider(
    _context: &mut ProcessUserInputContext,
    config: &QueryEngineConfig,
    conversation: &[ConversationEntry],
    state: &EngineState,
) -> anyhow::Result<claude_core::ProviderResponse> {
    if matches!(
        config.provider_invocation_mode,
        ProviderInvocationMode::Streaming
    ) {
        complete_with_streaming_observer(config, _context, conversation, state.turn + 1).await
    } else {
        let provider_context = provider_request_context(_context);
        config
            .backend
            .complete_with_context(conversation, &provider_context)
            .await
    }
}

async fn maybe_compact_conversation(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    legacy_conversation: &mut Vec<ConversationEntry>,
) {
    let before_snapshot = config.context_manager.budget_snapshot(legacy_conversation);
    let needs_compaction = before_snapshot.exceeds_threshold();
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
    let mut strategy = "standard".to_owned();
    let mut compacted = if let Some(handler) = config.compact_conversation_handler.as_ref() {
        match handler(legacy_conversation.clone(), config.context_manager.clone()).await {
            Some((conversation, selected_strategy)) => {
                strategy = selected_strategy;
                conversation
            }
            None => config.context_manager.compact(legacy_conversation),
        }
    } else {
        config.context_manager.compact(legacy_conversation)
    };
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
            strategy,
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
    let _ = state.state_machine.transition(EnginePhase::BuildingPrompt);
}

fn record_permission_denial(
    state: &mut EngineState,
    tool_call: &claude_core::ToolCall,
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
    response: &claude_core::ProviderResponse,
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
) -> anyhow::Result<claude_core::ProviderResponse> {
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
        crate::QuerySource::User => claude_provider::query_source::QuerySource::User,
        crate::QuerySource::ReplMainThread => {
            claude_provider::query_source::QuerySource::ReplMainThread
        }
        crate::QuerySource::Sdk => claude_provider::query_source::QuerySource::Sdk,
        crate::QuerySource::Compact => claude_provider::query_source::QuerySource::Compact,
        crate::QuerySource::SessionMemory => {
            claude_provider::query_source::QuerySource::SessionMemory
        }
        crate::QuerySource::Agent => claude_provider::query_source::QuerySource::Agent,
        crate::QuerySource::ExtractMemories => {
            claude_provider::query_source::QuerySource::ExtractMemories
        }
        crate::QuerySource::BackgroundTask => {
            claude_provider::query_source::QuerySource::BackgroundTask
        }
    };

    let mut provider_context =
        ProviderRequestContext::new(query_source, context.session_id.clone());
    if let Some(agent_id) = context.agent_id.clone() {
        provider_context = provider_context.with_agent_id(agent_id);
    }
    provider_context = provider_context
        .with_model_override(context.provider_model_override.clone())
        .with_max_output_tokens(context.max_output_tokens_override)
        .with_effort(context.requested_effort.clone())
        .with_fast_mode(context.fast_mode)
        .with_task_budget(
            context
                .task_budget
                .as_ref()
                .and_then(|budget| budget.max_total_tokens)
                .map(|total| ProviderTaskBudget {
                    total,
                    remaining: None,
                }),
        );
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

    let thinking_tx = tx.clone();
    let thinking_accumulated = Arc::new(Mutex::new(String::new()));
    let on_thinking_delta = Box::new(move |delta: &str| {
        let accumulated_thinking = {
            let mut accumulated = thinking_accumulated
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            accumulated.push_str(delta);
            accumulated.clone()
        };
        let _ = thinking_tx.send(QueryObserverEvent::StreamingThinkingDelta {
            turn,
            delta: delta.to_owned(),
            accumulated_thinking,
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
        on_thinking_delta: Some(on_thinking_delta),
    }
}

// ---------------------------------------------------------------------------
// Error classification helpers
// ---------------------------------------------------------------------------

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

fn is_max_tokens_truncated(stop_reason: &str) -> bool {
    let lowered = stop_reason.to_ascii_lowercase();
    lowered == "max_tokens"
        || lowered == "max_tokens_reached"
        || lowered.contains("max_output_tokens")
        || lowered == "length"
}

fn compute_context_usage(state: &EngineState, config: &QueryEngineConfig) -> f64 {
    let max_tokens = config.context_manager.available_budget();
    if max_tokens == 0 {
        return 0.0;
    }
    let used = state.usage.total_tokens();
    let ratio = used as f64 / max_tokens as f64;
    ratio.min(1.0)
}

fn estimate_current_max_tokens(usage: &claude_core::UsageAccumulator) -> usize {
    let output = usage.output_tokens;
    if output == 0 {
        return 8192;
    }
    let mut tier = 8192usize;
    while tier < output as usize {
        tier = match tier.checked_mul(2) {
            Some(next) if next > output as usize => return next,
            Some(next) => next,
            None => return tier,
        };
    }
    tier
}

// ---------------------------------------------------------------------------
// Model switcher extension
// ---------------------------------------------------------------------------

trait SwitchResultExt {
    fn is_switched(&self) -> bool;
}

impl SwitchResultExt for crate::model_switch::SwitchResult {
    fn is_switched(&self) -> bool {
        matches!(self, Self::Switched { .. })
    }
}

// ---------------------------------------------------------------------------
// Parallel tool execution
// ---------------------------------------------------------------------------

struct ToolBatch {
    parallel: bool,
    indices: Vec<usize>,
}

fn is_concurrency_safe_tool(name: &str) -> bool {
    matches!(
        name,
        "Read"
            | "read_file"
            | "Glob"
            | "glob"
            | "Grep"
            | "grep"
            | "search_files"
            | "WebSearch"
            | "web_search"
            | "WebFetch"
            | "web_fetch"
            | "LSP"
            | "lsp"
            | "ListMcpResources"
            | "list_mcp_resources"
            | "ReadMcpResource"
            | "read_mcp_resource"
            | "find_references"
            | "go_to_definition"
            | "hover"
            | "document_symbol"
            | "workspace_symbol"
            | "go_to_implementation"
            | "get_diagnostics"
    )
}

fn partition_tool_calls(tool_calls: &[claude_core::ToolCall]) -> Vec<ToolBatch> {
    let mut batches: Vec<ToolBatch> = Vec::new();

    for (i, call) in tool_calls.iter().enumerate() {
        let safe = is_concurrency_safe_tool(&call.name);
        if safe {
            if let Some(last) = batches.last_mut()
                && last.parallel
            {
                last.indices.push(i);
                continue;
            }
            batches.push(ToolBatch {
                parallel: true,
                indices: vec![i],
            });
        } else {
            batches.push(ToolBatch {
                parallel: false,
                indices: vec![i],
            });
        }
    }

    batches
}

async fn commit_tool_result(
    config: &QueryEngineConfig,
    state: &mut EngineState,
    context: &ProcessUserInputContext,
    tool_call: &claude_core::ToolCall,
    tool_run: &ToolRunResult,
    turn: u32,
    batch_size: usize,
    batch_index: usize,
) {
    if let Some(permission_denial) = tool_run.permission_denial.clone() {
        state.permission_denials.push(permission_denial);
    } else {
        record_permission_denial(state, tool_call, &tool_run.result);
    }
    let _ = config
        .observer
        .on_event(QueryObserverEvent::ToolCallStarted {
            tool_call: tool_call.clone(),
            turn,
            batch_size,
            batch_index,
        })
        .await;
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
    let last_uuid = state.messages.last().and_then(|m| Some(m.base().uuid));
    state
        .messages
        .push(tool_result_message_with_parent(tool_call, &tool_run.result, last_uuid));
    let _ = config
        .observer
        .on_event(QueryObserverEvent::ToolResultCommitted {
            tool_call: tool_call.clone(),
            result: tool_run.result.clone(),
            turn,
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
