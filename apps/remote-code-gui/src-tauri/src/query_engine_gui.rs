//! GUI-specific QueryEngine integration layer.
//!
//! This module provides the unified execution path for all three Agent types
//! (RemoteClaude, RemoteRoo, RemoteCodex) through [`QueryEngine::submit_message()`].
//!
//! # Core Components
//!
//! - [`GuiToolRunner`] — implements [`ToolRunner`] by wrapping `execute_tool_call()`
//!   and the GUI permission broker.
//! - [`GuiQueryObserver`] — implements [`QueryObserver`] by mapping engine events
//!   to Tauri frontend events and persisting session state.
//! - [`run_unified_prompt_with_provider`] — the main entry point that replaces both
//!   `run_gui_prompt()` and `run_agent_prompt()`.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use claude_config::RuntimeConfig;
use claude_core::{
    ConversationEntry, Message, SessionId, SubAgentCompletion, ToolCall, ToolResult, UsageSummary,
};
use claude_permissions::PermissionDecision;
use claude_provider::ConversationBackend;
use claude_provider::context::ContextWindowManager;
use claude_query_engine::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngine, QueryEngineConfig, QueryObserver,
    QueryObserverEvent, ToolRunResult, ToolRunner,
};
use claude_session::SessionStore;
use claude_tools::{
    FileStateCache,
    agent::parse_delegate_progress_event,
    execute_tool_call,
    git::apply_worktree_tool_result_to_runtime,
    runtime_plan_mode::{
        RuntimePlanModeController, inject_plan_mode_runtime_messages, install_plan_mode_runtime,
    },
    runtime_provider_tool_spec,
};
use rc_engine_events::EventStream;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

use crate::dto::{
    BatchProgressDto, ContextCompactedDto, ContextOverflowDto, ContextUsageDto, StreamingDeltaDto,
    SubtaskCompletedDto, SubtaskProgressDto, SubtaskStartedDto, ToolProgressDto, ToolResultDto,
};
use crate::state::{
    APP_EVENT_BATCH_PROGRESS, APP_EVENT_CONTEXT_COMPACTED, APP_EVENT_CONTEXT_OVERFLOW,
    APP_EVENT_CONTEXT_USAGE, APP_EVENT_STREAMING_DELTA, APP_EVENT_SUBTASK_COMPLETED,
    APP_EVENT_SUBTASK_PROGRESS, APP_EVENT_SUBTASK_STARTED, APP_EVENT_TOOL_PROGRESS,
    APP_EVENT_TOOL_RESULT, APP_EVENT_TOOL_START, DEFAULT_MAX_TURNS,
};

// ─── GuiToolRunner ──────────────────────────────────────────────────────────

/// GUI-specific [`ToolRunner`] that wraps the existing `execute_tool_call()`
/// infrastructure and emits Tauri events for tool progress.
pub(crate) struct GuiToolRunner {
    app: AppHandle,
    store: Arc<SessionStore>,
    /// RuntimeConfig guarded by an async mutex for interior mutability
    /// (worktree updates mutate the config).
    config: Arc<Mutex<RuntimeConfig>>,
    broker: Arc<dyn claude_permissions::PermissionBroker>,
    session_id: Uuid,
    task_paths: claude_config::AppPaths,
    sub_agent: Arc<dyn SubAgentCompletion>,
    context_manager: ContextWindowManager,
}

impl GuiToolRunner {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        app: AppHandle,
        store: Arc<SessionStore>,
        config: RuntimeConfig,
        broker: Arc<dyn claude_permissions::PermissionBroker>,
        session_id: Uuid,
        task_paths: claude_config::AppPaths,
        sub_agent: Arc<dyn SubAgentCompletion>,
        context_manager: ContextWindowManager,
    ) -> Self {
        Self {
            app,
            store,
            config: Arc::new(Mutex::new(config)),
            broker,
            session_id,
            task_paths,
            sub_agent,
            context_manager,
        }
    }
}

#[async_trait]
impl ToolRunner for GuiToolRunner {
    async fn run_tool(
        &self,
        tool_call: &claude_core::ToolCall,
        _context: &ProcessUserInputContext,
    ) -> Result<ToolRunResult> {
        // 1. Validate tool spec.
        let _spec = runtime_provider_tool_spec(&tool_call.name)
            .await
            .ok_or_else(|| anyhow!("unknown tool {}", tool_call.name))?;

        // 2. Emit tool-start event.
        let session_id_str = self.session_id.to_string();

        // Extract activeForm from tool input for TodoWrite
        let active_form = if tool_call.name == "TodoWrite" {
            tool_call.input.as_object().and_then(|obj| {
                obj.get("todos")
                    .and_then(|todos| todos.as_array())
                    .and_then(|arr| {
                        arr.iter()
                            .find(|t| {
                                t.get("status").and_then(|s| s.as_str()) == Some("in_progress")
                            })
                            .and_then(|t| {
                                t.get("activeForm")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_owned())
                            })
                    })
            })
        } else {
            None
        };

        let _ = self.app.emit(
            APP_EVENT_TOOL_START,
            ToolProgressDto {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                message: "running".to_owned(),
                active_form,
            },
        );

        // 3. Build ToolExecutionContext with progress callback for subtask events.
        let app = self.app.clone();
        let sid = session_id_str.clone();
        let paths = self.task_paths.clone();
        let session_id = self.session_id;

        let tool_context = {
            let config = self.config.lock().await;
            claude_tools::ToolExecutionContext {
                cwd: config.cwd.clone(),
                original_cwd: config.original_cwd.clone(),
                active_worktree_session: config.active_worktree_session.clone(),
                timeout_ms: config.provider.timeout_ms,
                sub_agent: Some(self.sub_agent.clone()),
                progress_cb: Some(Arc::new(move |message: &str| {
                    emit_delegate_progress(&app, &sid, &paths, session_id, message);
                })),
                task_stack: Arc::new(parking_lot::Mutex::new(
                    claude_core::task_stack::TaskStack::default(),
                )),
                read_file_state: FileStateCache::new(),
                sub_agent_output_tokens: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            }
        };

        // 4. Execute tool.
        let tool_result =
            match execute_tool_call(tool_call, &tool_context, self.broker.as_ref()).await {
                Ok(result) => result,
                Err(error) => ToolResult {
                    content: format!("Tool execution error: {error}"),
                    is_error: true,
                    content_blocks: Vec::new(),
                    follow_up_user_blocks: Vec::new(),
                },
            };

        // 5. Handle worktree updates.
        {
            let mut config = self.config.lock().await;
            let mut temp_context = claude_tools::ToolExecutionContext {
                cwd: config.cwd.clone(),
                original_cwd: config.original_cwd.clone(),
                active_worktree_session: config.active_worktree_session.clone(),
                timeout_ms: config.provider.timeout_ms,
                sub_agent: Some(self.sub_agent.clone()),
                progress_cb: None,
                task_stack: Arc::new(parking_lot::Mutex::new(
                    claude_core::task_stack::TaskStack::default(),
                )),
                read_file_state: FileStateCache::new(),
                sub_agent_output_tokens: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            };

            if apply_worktree_tool_result_to_runtime(
                &tool_call.name,
                &tool_call.input,
                &tool_result,
                &mut config,
                &mut temp_context,
            )? {
                crate::desktop::persist_session_context(self.store.as_ref(), &config)?;
            }
        }

        // 6. Handle follow-up user blocks as post_messages.
        let mut post_messages = Vec::new();
        if !tool_result.follow_up_user_blocks.is_empty() {
            let follow_up_entry = ConversationEntry::user_with_content_blocks(
                tool_result.follow_up_user_blocks.clone(),
            );
            post_messages.push(Message::from(follow_up_entry));
        }

        // 7. Emit tool-result event.
        let _ = self.app.emit(
            APP_EVENT_TOOL_RESULT,
            ToolResultDto {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                is_error: tool_result.is_error,
                output: tool_result.content.clone(),
            },
        );

        // 8. Persist tool result to session store.
        let output_for_context = self
            .context_manager
            .truncate_tool_output_default(&tool_result.content);
        let mut tool_entry = ConversationEntry::tool(
            tool_call.id.clone(),
            tool_call.name.clone(),
            output_for_context,
            tool_result.is_error,
        );
        tool_entry.content_blocks = tool_result.content_blocks.clone();
        self.store
            .append_conversation_entry(self.session_id, &tool_entry)?;

        self.store.append_named_event(
            self.session_id,
            "tool_result",
            json!({
                "tool_name": tool_call.name,
                "tool_use_id": tool_call.id,
                "is_error": tool_entry.is_error,
            }),
        )?;

        Ok(ToolRunResult {
            result: tool_result,
            pre_messages: Vec::new(),
            post_messages,
            permission_denial: None,
            output_tokens_consumed: None,
        })
    }
}

/// Emit delegate progress events as Tauri frontend events.
fn emit_delegate_progress(
    app: &AppHandle,
    session_id_str: &str,
    paths: &claude_config::AppPaths,
    session_id: Uuid,
    message: &str,
) {
    let Some(event) = parse_delegate_progress_event(message) else {
        let _ = app.emit(
            APP_EVENT_TOOL_PROGRESS,
            ToolProgressDto {
                tool_call_id: String::new(),
                tool_name: "agent".to_owned(),
                message: message.to_owned(),
                active_form: None,
            },
        );
        return;
    };

    match event {
        claude_tools::agent::DelegateProgressEvent::SubtaskStarted {
            task_id,
            parent_task_id,
            description,
            depth,
        } => {
            let _ = app.emit(
                APP_EVENT_SUBTASK_STARTED,
                SubtaskStartedDto {
                    session_id: session_id_str.to_owned(),
                    task_id,
                    parent_task_id,
                    description,
                    depth,
                },
            );
            crate::desktop::emit_task_snapshot_for_session(app, paths, session_id);
        }
        claude_tools::agent::DelegateProgressEvent::SubtaskProgress {
            task_id,
            turn,
            max_turns,
            summary,
        } => {
            let _ = app.emit(
                APP_EVENT_SUBTASK_PROGRESS,
                SubtaskProgressDto {
                    session_id: session_id_str.to_owned(),
                    task_id: task_id.clone(),
                    turn,
                    max_turns,
                    summary: summary.clone(),
                },
            );
            let _ = app.emit(
                APP_EVENT_TOOL_PROGRESS,
                ToolProgressDto {
                    tool_call_id: task_id,
                    tool_name: "agent".to_owned(),
                    message: summary,
                    active_form: None,
                },
            );
            crate::desktop::emit_task_snapshot_for_session(app, paths, session_id);
        }
        claude_tools::agent::DelegateProgressEvent::SubtaskCompleted {
            task_id,
            success,
            output_preview,
            turns_used,
        } => {
            let _ = app.emit(
                APP_EVENT_SUBTASK_COMPLETED,
                SubtaskCompletedDto {
                    session_id: session_id_str.to_owned(),
                    task_id,
                    success,
                    output_preview,
                    turns_used,
                },
            );
            crate::desktop::emit_task_snapshot_for_session(app, paths, session_id);
        }
        claude_tools::agent::DelegateProgressEvent::BatchProgress {
            total,
            completed,
            running,
        } => {
            let _ = app.emit(
                APP_EVENT_BATCH_PROGRESS,
                BatchProgressDto {
                    session_id: session_id_str.to_owned(),
                    total,
                    completed,
                    running,
                },
            );
            crate::desktop::emit_task_snapshot_for_session(app, paths, session_id);
        }
    }
}

// ─── GuiQueryObserver ───────────────────────────────────────────────────────

/// GUI-specific [`QueryObserver`] that maps engine lifecycle events to Tauri
/// frontend events and persists session state via [`SessionStore`].
pub(crate) struct GuiQueryObserver {
    app: AppHandle,
    store: Arc<SessionStore>,
    session_id: Uuid,
}

impl GuiQueryObserver {
    pub(crate) fn new(app: AppHandle, store: Arc<SessionStore>, session_id: Uuid) -> Self {
        Self {
            app,
            store,
            session_id,
        }
    }
}

#[async_trait]
impl QueryObserver for GuiQueryObserver {
    async fn on_event(&self, event: QueryObserverEvent) -> Result<()> {
        let session_id_str = self.session_id.to_string();

        match event {
            // ── Streaming text delta ──
            QueryObserverEvent::StreamingTextDelta { delta, .. } => {
                let _ = self.app.emit(
                    APP_EVENT_STREAMING_DELTA,
                    StreamingDeltaDto {
                        session_id: session_id_str,
                        delta,
                    },
                );
            }

            // ── Tool call started (non-streaming) ──
            QueryObserverEvent::ToolCallStarted { tool_call, .. } => {
                let _ = self.app.emit(
                    APP_EVENT_TOOL_START,
                    ToolProgressDto {
                        tool_call_id: tool_call.id,
                        tool_name: tool_call.name,
                        message: "running".to_owned(),
                        active_form: None,
                    },
                );
            }

            // ── Streaming tool call started ──
            QueryObserverEvent::StreamingToolCallStarted {
                tool_name,
                tool_call_id,
                ..
            } => {
                let _ = self.app.emit(
                    APP_EVENT_TOOL_START,
                    ToolProgressDto {
                        tool_call_id,
                        tool_name,
                        message: "running".to_owned(),
                        active_form: None,
                    },
                );
            }

            // ── Assistant message committed → persist to session store ──
            QueryObserverEvent::AssistantMessageCommitted {
                message,
                stop_reason,
                turn,
                usage,
                ..
            } => {
                if let Some(entry) = message.as_conversation_entry() {
                    self.store
                        .append_conversation_entry(self.session_id, &entry)?;
                }
                self.store.append_named_event(
                    self.session_id,
                    "assistant_turn",
                    json!({
                        "turn": turn,
                        "stop_reason": stop_reason,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                    }),
                )?;
            }

            // ── Context budget evaluated → context usage + overflow events ──
            QueryObserverEvent::ContextBudgetEvaluated { context, .. } => {
                let _ = self.app.emit(
                    APP_EVENT_CONTEXT_USAGE,
                    ContextUsageDto {
                        session_id: session_id_str.clone(),
                        estimated_tokens: context.estimated_tokens,
                        max_input_tokens: context.max_input_tokens,
                        threshold_tokens: context.threshold_tokens,
                        ratio: context.usage_ratio,
                    },
                );

                if context.needs_compaction {
                    let _ = self.app.emit(
                        APP_EVENT_CONTEXT_OVERFLOW,
                        ContextOverflowDto {
                            session_id: session_id_str,
                            estimated_tokens: context.estimated_tokens,
                            max_input_tokens: context.max_input_tokens,
                            threshold_tokens: context.threshold_tokens,
                            ratio: context.usage_ratio,
                        },
                    );
                }
            }

            // ── Context compaction applied → persist + emit ──
            QueryObserverEvent::ContextCompactionApplied {
                turn,
                before_messages,
                after_messages,
                usage_ratio_before,
                usage_ratio_after,
                estimated_tokens_before,
                estimated_tokens_after,
                ..
            } => {
                let removed = before_messages.saturating_sub(after_messages);
                if removed > 0 {
                    self.store.append_named_event(
                        self.session_id,
                        "context_compacted",
                        json!({
                            "turn": turn,
                            "entries_removed": removed,
                            "usage_ratio_before": usage_ratio_before,
                            "usage_ratio_after": usage_ratio_after,
                            "estimated_tokens_before": estimated_tokens_before,
                            "estimated_tokens_after": estimated_tokens_after,
                        }),
                    )?;
                    let _ = self.app.emit(
                        APP_EVENT_CONTEXT_COMPACTED,
                        ContextCompactedDto {
                            session_id: self.session_id.to_string(),
                            entries_removed: removed,
                            usage_ratio: usage_ratio_after,
                        },
                    );
                }
            }

            // ── Query finished → persist result (prompt-done is emitted by send_prompt) ──
            QueryObserverEvent::QueryFinished {
                stop_reason,
                turns,
                usage,
                ..
            } => {
                self.store.append_named_event(
                    self.session_id,
                    "result",
                    json!({
                        "is_error": false,
                        "stop_reason": stop_reason,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                        "num_turns": turns,
                    }),
                )?;
            }

            // ── Query failed → persist error result ──
            QueryObserverEvent::QueryFailed { error, turns, .. } => {
                self.store.append_named_event(
                    self.session_id,
                    "result",
                    json!({
                        "is_error": true,
                        "error": error,
                        "num_turns": turns,
                    }),
                )?;
            }

            // ── Other events: no-op for now ──
            _ => {}
        }

        Ok(())
    }
}

// ─── UnifiedPromptOutcome ───────────────────────────────────────────────────

/// Outcome returned by [`run_unified_prompt_with_provider`].
#[derive(Debug)]
pub(crate) struct UnifiedPromptOutcome {
    pub(crate) text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
    pub(crate) usage: UsageSummary,
    pub(crate) num_turns: u32,
    pub(crate) stop_reason: String,
}

// ─── run_unified_prompt_with_provider ───────────────────────────────────────

/// Unified execution path that replaces both `run_gui_prompt()` and
/// `run_agent_prompt()`.
///
/// All three Agent types (RemoteClaude, RemoteRoo, RemoteCodex) now share
/// this single path through [`QueryEngine::submit_message()`].
pub(crate) async fn run_unified_prompt_with_provider(
    app: &AppHandle,
    config: RuntimeConfig,
    provider: Arc<claude_provider::ProviderClient>,
    store: Arc<SessionStore>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    prompt: &str,
) -> Result<UnifiedPromptOutcome> {
    let session_id = config.session_id;
    let model = config
        .provider
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());

    // 1. Session initialization (same as run_gui_prompt).
    let mut conversation =
        crate::desktop::initialize_session_conversation(&store, &config, Some(prompt))?;
    let plan_mode_controller = RuntimePlanModeController::load(&config, store.as_ref())?;
    let _plan_mode_runtime_guard = install_plan_mode_runtime(plan_mode_controller.clone())?;
    inject_plan_mode_runtime_messages(store.as_ref(), session_id, &mut conversation)?;

    store.append_named_event(
        session_id,
        "prompt_started",
        json!({
            "prompt": prompt,
            "provider": config.provider.name,
            "model": config.provider.model,
            "protocol": config.provider.protocol.as_str(),
        }),
    )?;

    // Persist user entry.
    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(session_id, &user_entry)?;
    conversation.push(user_entry);

    // 2. Build permission broker.
    let broker = crate::desktop::GuiRuntimePermissionBroker::new(
        &config,
        plan_mode_controller,
        app.clone(),
        pending_permissions,
    );

    // 3. Build context manager.
    let context_manager = ContextWindowManager::for_model(&model);

    // 4. Create the backend (Arc-wrapped).
    let backend: Arc<dyn ConversationBackend> = Arc::new(
        claude_provider::ProviderCompatBackend::new(Arc::clone(&provider), &config.provider),
    );

    // 5. Build GUI tool runner.
    let tool_runner = Arc::new(GuiToolRunner::new(
        app.clone(),
        store.clone(),
        config.clone(),
        Arc::new(broker),
        session_id,
        config.paths.clone(),
        backend.sub_agent_completion(),
        context_manager.clone(),
    ));

    // 6. Build GUI observer.
    let observer = Arc::new(GuiQueryObserver::new(
        app.clone(),
        store.clone(),
        session_id,
    ));

    // 7. Build QueryEngineConfig.
    let event_stream = EventStream::new(64);
    let max_turns = config.max_turns.max(DEFAULT_MAX_TURNS) as u32;

    let mut query_config = QueryEngineConfig::new(
        SessionId::from(session_id),
        &model,
        backend,
        tool_runner,
        event_stream,
    )
    .with_observer(observer)
    .with_provider_invocation_mode(ProviderInvocationMode::Streaming);
    query_config.max_turns = max_turns;

    // 8. Convert existing conversation to Messages for QueryEngine.
    let existing_messages: Vec<Message> = conversation.into_iter().map(Message::from).collect();

    // 9. Create QueryEngine and submit.
    let mut engine = QueryEngine::new(query_config, existing_messages);

    let context =
        ProcessUserInputContext::new(SessionId::from(session_id), config.permission_mode, &model);

    let user_message = vec![Message::from(ConversationEntry::user(prompt))];
    let result = engine.submit_message(user_message, context).await?;

    // 10. Convert result.
    let usage = UsageSummary {
        input_tokens: result.state.usage.input_tokens,
        output_tokens: result.state.usage.output_tokens,
        ..Default::default()
    };

    Ok(UnifiedPromptOutcome {
        text: result.final_text.unwrap_or_default(),
        tool_calls: extract_tool_calls_from_state(&result.state),
        usage,
        num_turns: result.turns,
        stop_reason: result.stop_reason,
    })
}

/// Extract tool calls that were made during the query from the engine state.
fn extract_tool_calls_from_state(state: &claude_query_engine::EngineState) -> Vec<ToolCall> {
    state
        .messages
        .iter()
        .flat_map(|msg| {
            msg.as_conversation_entry()
                .map(|entry| entry.tool_calls.clone())
                .unwrap_or_default()
        })
        .collect()
}
