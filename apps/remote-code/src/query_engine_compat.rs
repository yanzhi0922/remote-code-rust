use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rc_config::{RuntimeConfig, validate_provider_config};
use rc_core::{ConversationEntry, ConversationRole, Message, ToolCall, ToolResult};
use rc_permissions::PermissionBroker;
use rc_protocol::UsagePayload;
use rc_provider::ConversationBackend;
use rc_query_engine::{
    EffortLevel, ProcessUserInputContext, ProviderInvocationMode, QueryCheckpointKind, QueryEngine,
    QueryEngineConfig, QueryObserver, QueryObserverEvent, ToolRunResult, ToolRunner,
};
use rc_session::SessionStore;
use rc_session::resume_state::{PendingToolCall, ResumeState};
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
use tokio::sync::Mutex;

use crate::conversation::{
    PromptEventSink, PromptRunOutcome, PromptStreamEvent, build_prompt_progress_callback,
    discover_runtime_extensions, truncate_preview,
};
use crate::hooks::{
    HookRunState, RuntimeHookDiscovery, apply_post_tool_hooks, apply_pre_tool_use_hooks,
};

struct CompatSharedState {
    conversation: Mutex<Vec<ConversationEntry>>,
    hook_state: Mutex<HookRunState>,
    streamed_tool_calls: Mutex<HashSet<String>>,
    latest_streaming_usage: Mutex<Option<UsagePayload>>,
    latest_request_id: Mutex<Option<String>>,
}

struct CompatObserver {
    config: RuntimeConfig,
    store: Arc<SessionStore>,
    shared: Arc<CompatSharedState>,
    event_sink: Option<PromptEventSink>,
    include_partial_messages: bool,
}

impl CompatObserver {
    async fn mark_tool_started_if_new(&self, tool_call_id: &str) -> bool {
        if tool_call_id.is_empty() {
            return false;
        }
        self.shared
            .streamed_tool_calls
            .lock()
            .await
            .insert(tool_call_id.to_owned())
    }
}

#[async_trait]
impl QueryObserver for CompatObserver {
    async fn on_event(&self, event: QueryObserverEvent) -> Result<()> {
        match event {
            QueryObserverEvent::ContextBudgetEvaluated { turn, context, .. } => {
                if let Some(event_sink) = self.event_sink.as_ref() {
                    event_sink(PromptStreamEvent::ContextUsage {
                        estimated_tokens: context.estimated_tokens,
                        max_input_tokens: context.max_input_tokens,
                        threshold_tokens: context.threshold_tokens,
                        ratio: context.usage_ratio,
                    });
                    if context.needs_compaction {
                        event_sink(PromptStreamEvent::ContextOverflow {
                            estimated_tokens: context.estimated_tokens,
                            max_input_tokens: context.max_input_tokens,
                            threshold_tokens: context.threshold_tokens,
                            ratio: context.usage_ratio,
                        });
                    }
                }
                if context.needs_compaction {
                    self.store.append_named_event(
                        self.config.session_id,
                        "context_overflow",
                        serde_json::json!({
                            "turn": turn,
                            "estimated_tokens": context.estimated_tokens,
                            "max_input_tokens": context.max_input_tokens,
                            "threshold_tokens": context.threshold_tokens,
                            "usage_ratio": context.usage_ratio,
                        }),
                    )?;
                }
            }
            QueryObserverEvent::ContextCompactionApplied {
                turn,
                before_messages,
                after_messages,
                max_input_tokens,
                threshold_tokens,
                usage_ratio_before,
                usage_ratio_after,
                estimated_tokens_before,
                estimated_tokens_after,
            } => {
                let entries_removed = before_messages.saturating_sub(after_messages);
                self.store.append_named_event(
                    self.config.session_id,
                    "context_compacted",
                    serde_json::json!({
                        "turn": turn,
                        "entries_removed": entries_removed,
                        "usage_ratio_before": usage_ratio_before,
                        "usage_ratio_after": usage_ratio_after,
                        "estimated_tokens_before": estimated_tokens_before,
                        "estimated_tokens_after": estimated_tokens_after,
                        "max_input_tokens": max_input_tokens,
                        "threshold_tokens": threshold_tokens,
                    }),
                )?;
                if let Some(event_sink) = self.event_sink.as_ref() {
                    event_sink(PromptStreamEvent::ContextCompacted {
                        entries_removed,
                        usage_ratio: usage_ratio_after,
                    });
                    event_sink(PromptStreamEvent::ContextUsage {
                        estimated_tokens: estimated_tokens_after,
                        max_input_tokens,
                        threshold_tokens,
                        ratio: usage_ratio_after,
                    });
                }
            }
            QueryObserverEvent::StreamingTextDelta { delta, .. } => {
                if self.include_partial_messages
                    && !delta.is_empty()
                    && let Some(event_sink) = self.event_sink.as_ref()
                {
                    event_sink(PromptStreamEvent::MessageDelta { delta });
                }
            }
            QueryObserverEvent::StreamingToolCallStarted {
                tool_call_id,
                tool_name,
                ..
            } => {
                if !tool_name.is_empty()
                    && self.mark_tool_started_if_new(&tool_call_id).await
                    && let Some(event_sink) = self.event_sink.as_ref()
                {
                    event_sink(PromptStreamEvent::ToolStarted {
                        tool_call_id,
                        tool_name,
                    });
                }
            }
            QueryObserverEvent::StreamingToolCallDelta {
                tool_call_id,
                delta,
                ..
            } => {
                if !tool_call_id.is_empty()
                    && !delta.is_empty()
                    && let Some(event_sink) = self.event_sink.as_ref()
                {
                    event_sink(PromptStreamEvent::ToolProgress {
                        tool_call_id: Some(tool_call_id),
                        delta: Some(delta),
                        elapsed_time_seconds: None,
                    });
                }
            }
            QueryObserverEvent::StreamingUsageUpdated { turn, usage } => {
                let usage = UsagePayload {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                };
                {
                    let mut latest_usage = self.shared.latest_streaming_usage.lock().await;
                    *latest_usage = Some(usage.clone());
                }
                self.store.append_named_event(
                    self.config.session_id,
                    "streaming_usage",
                    serde_json::json!({
                        "turn": turn,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                    }),
                )?;
            }
            QueryObserverEvent::AssistantMessageCommitted {
                message,
                stop_reason,
                turn,
                usage,
                request_id,
            } => {
                {
                    let mut latest_request_id = self.shared.latest_request_id.lock().await;
                    *latest_request_id = request_id.clone();
                }
                let assistant_entry = assistant_entry_from_message(&message)?;
                self.store
                    .append_conversation_entry(self.config.session_id, &assistant_entry)?;
                self.store.append_named_event(
                    self.config.session_id,
                    "assistant_turn",
                    serde_json::json!({
                        "turn": turn,
                        "stop_reason": stop_reason,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                        "request_id": request_id,
                        "tool_calls": assistant_entry.tool_calls.len(),
                        "text_preview": truncate_preview(&assistant_entry.text, 160),
                    }),
                )?;
                {
                    let mut conversation = self.shared.conversation.lock().await;
                    conversation.push(assistant_entry.clone());
                }
                if let Some(event_sink) = self.event_sink.as_ref()
                    && !assistant_entry.text.trim().is_empty()
                {
                    event_sink(PromptStreamEvent::MessageCommitted {
                        text: assistant_entry.text.clone(),
                    });
                }
                if assistant_entry.tool_calls.is_empty() {
                    self.store.clear_resume_state(self.config.session_id)?;
                } else {
                    let pending_tool_calls = assistant_entry
                        .tool_calls
                        .iter()
                        .map(|tool_call| PendingToolCall {
                            id: tool_call.id.clone(),
                            name: tool_call.name.clone(),
                            input: tool_call.input.clone(),
                        })
                        .collect::<Vec<_>>();
                    self.store.save_resume_state(
                        self.config.session_id,
                        &ResumeState::from_pending_calls(pending_tool_calls),
                    )?;
                }
            }
            QueryObserverEvent::ToolCallStarted { tool_call, .. } => {
                if !tool_call.name.is_empty()
                    && self.mark_tool_started_if_new(&tool_call.id).await
                    && let Some(event_sink) = self.event_sink.as_ref()
                {
                    event_sink(PromptStreamEvent::ToolStarted {
                        tool_call_id: tool_call.id,
                        tool_name: tool_call.name,
                    });
                }
            }
            QueryObserverEvent::ToolResultCommitted {
                tool_call, result, ..
            } => {
                if let Some(event_sink) = self.event_sink.as_ref() {
                    event_sink(PromptStreamEvent::ToolFinished {
                        tool_call_id: tool_call.id,
                        tool_name: tool_call.name,
                        is_error: result.is_error,
                        summary: Some(truncate_preview(&result.content, 160)),
                    });
                }
            }
            QueryObserverEvent::CheckpointCleared { checkpoint }
                if checkpoint.kind == QueryCheckpointKind::ToolBatch =>
            {
                self.store.clear_resume_state(self.config.session_id)?;
            }
            QueryObserverEvent::BudgetEvaluated { .. }
            | QueryObserverEvent::BudgetExceeded { .. }
            | QueryObserverEvent::CheckpointCreated { .. }
            | QueryObserverEvent::MessagesAppended { .. }
            | QueryObserverEvent::QueryFailed { .. }
            | QueryObserverEvent::QueryFinished { .. }
            | QueryObserverEvent::QueryStarted { .. } => {}
            QueryObserverEvent::CheckpointCleared { .. } => {}
        }
        Ok(())
    }
}

struct CompatToolRunner {
    config: RuntimeConfig,
    store: Arc<SessionStore>,
    discovery: RuntimeHookDiscovery,
    shared: Arc<CompatSharedState>,
    broker: Arc<dyn PermissionBroker>,
    tool_context: ToolExecutionContext,
}

#[async_trait]
impl ToolRunner for CompatToolRunner {
    async fn run_tool(
        &self,
        tool_call: &ToolCall,
        _context: &ProcessUserInputContext,
    ) -> Result<ToolRunResult> {
        let _ = builtin_tool_specs()
            .into_iter()
            .find(|spec| spec.name == tool_call.name)
            .ok_or_else(|| anyhow!("unknown tool {}", tool_call.name))?;

        let (prepared, pre_messages) = {
            let mut conversation = self.shared.conversation.lock().await;
            let mut hook_state = self.shared.hook_state.lock().await;
            let before_messages = conversation.len();
            let prepared = apply_pre_tool_use_hooks(
                &self.discovery,
                &self.config,
                self.store.as_ref(),
                &mut conversation,
                &mut hook_state,
                tool_call,
            )
            .await?;
            (
                prepared,
                conversation[before_messages..]
                    .iter()
                    .cloned()
                    .map(Message::from)
                    .collect::<Vec<_>>(),
            )
        };

        let effective_tool_call = prepared.call;
        let audit_count_before = self.broker.audit_records().len();
        let raw_result = if let Some(blocked_reason) = &prepared.blocked_reason {
            ToolResult {
                content: blocked_reason.clone(),
                is_error: true,
            }
        } else {
            match execute_tool_call(
                &effective_tool_call,
                &self.tool_context,
                self.broker.as_ref(),
            )
            .await
            {
                Ok(result) => result,
                Err(error) => {
                    tracing::warn!(
                        "tool execution error for {}: {error}",
                        effective_tool_call.name
                    );
                    self.store.append_named_event(
                        self.config.session_id,
                        "tool_error",
                        serde_json::json!({
                            "tool_name": effective_tool_call.name,
                            "tool_use_id": effective_tool_call.id,
                            "error": format!("{error:#}"),
                        }),
                    )?;
                    ToolResult {
                        content: format!("Tool execution error: {error}"),
                        is_error: true,
                    }
                }
            }
        };

        for audit in self
            .broker
            .audit_records()
            .into_iter()
            .skip(audit_count_before)
        {
            self.store.append_named_event(
                self.config.session_id,
                "permission_decision",
                serde_json::to_value(&audit)?,
            )?;
        }

        let is_permission_denied =
            raw_result.is_error && is_permission_denied_message(&raw_result.content);
        let permission_denial =
            (is_permission_denied || prepared.blocked_reason.is_some()).then(|| {
                serde_json::json!({
                    "tool_name": effective_tool_call.name,
                    "tool_use_id": effective_tool_call.id,
                    "tool_input": effective_tool_call.input.clone(),
                    "message": raw_result.content.clone(),
                })
            });

        let tool_preview = truncate_preview(&raw_result.content, 160);
        let model_name = self.config.provider.model.as_deref().unwrap_or("unknown");
        let truncated_content = rc_provider::context::ContextWindowManager::for_model(model_name)
            .truncate_tool_output_default(&raw_result.content);
        let result = ToolResult {
            content: truncated_content.clone(),
            is_error: raw_result.is_error,
        };

        {
            let mut conversation = self.shared.conversation.lock().await;
            let tool_entry = ConversationEntry::tool(
                effective_tool_call.id.clone(),
                effective_tool_call.name.clone(),
                truncated_content,
                raw_result.is_error,
            );
            self.store
                .append_conversation_entry(self.config.session_id, &tool_entry)?;
            self.store.append_named_event(
                self.config.session_id,
                "tool_result",
                serde_json::json!({
                    "tool_name": effective_tool_call.name,
                    "tool_use_id": effective_tool_call.id,
                    "is_error": tool_entry.is_error,
                    "content_preview": tool_preview,
                }),
            )?;
            conversation.push(tool_entry);
        }

        let post_messages = {
            let mut conversation = self.shared.conversation.lock().await;
            let mut hook_state = self.shared.hook_state.lock().await;
            let before_messages = conversation.len();
            apply_post_tool_hooks(
                &self.discovery,
                &self.config,
                self.store.as_ref(),
                &mut conversation,
                &mut hook_state,
                &effective_tool_call,
                &raw_result,
            )
            .await?;
            conversation[before_messages..]
                .iter()
                .cloned()
                .map(Message::from)
                .collect::<Vec<_>>()
        };

        Ok(ToolRunResult {
            result,
            pre_messages,
            post_messages,
            permission_denial,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_prompt_with_query_engine_compat(
    config: &RuntimeConfig,
    store: &SessionStore,
    backend: Arc<dyn ConversationBackend>,
    broker: Arc<dyn PermissionBroker>,
    event_sink: Option<PromptEventSink>,
    discovery: &RuntimeHookDiscovery,
    hook_state: &mut HookRunState,
    conversation: &mut Vec<ConversationEntry>,
    prompt: &str,
) -> Result<PromptRunOutcome> {
    let readiness = validate_provider_config(&config.provider);
    if !readiness.ok {
        return Err(anyhow!(readiness.issues.join(" ")));
    }

    let started = Instant::now();
    let existing_messages = conversation
        .iter()
        .cloned()
        .map(Message::from)
        .collect::<Vec<_>>();
    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        config.session_name.as_deref().or(Some(prompt)),
    )?;
    store.append_named_event(
        config.session_id,
        "prompt_started",
        serde_json::json!({
            "prompt": prompt,
            "provider": config.provider.name.clone(),
            "model": config.provider.model.clone(),
            "protocol": config.provider.protocol.as_str(),
        }),
    )?;
    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(config.session_id, &user_entry)?;
    conversation.push(user_entry);

    let compat_store = Arc::new(SessionStore::open(config.paths.clone())?);
    let shared = Arc::new(CompatSharedState {
        conversation: Mutex::new(conversation.clone()),
        hook_state: Mutex::new(std::mem::take(hook_state)),
        streamed_tool_calls: Mutex::new(HashSet::new()),
        latest_streaming_usage: Mutex::new(None),
        latest_request_id: Mutex::new(None),
    });
    let observer = Arc::new(CompatObserver {
        config: config.clone(),
        store: compat_store.clone(),
        shared: shared.clone(),
        event_sink: event_sink.clone(),
        include_partial_messages: config.include_partial_messages,
    });
    let tool_runner = Arc::new(CompatToolRunner {
        config: config.clone(),
        store: compat_store.clone(),
        discovery: discovery.clone(),
        shared: shared.clone(),
        broker,
        tool_context: ToolExecutionContext {
            cwd: config.cwd.clone(),
            timeout_ms: config.provider.timeout_ms,
            sub_agent: Some(backend.sub_agent_completion()),
            progress_cb: event_sink
                .as_ref()
                .map(|event_sink| build_prompt_progress_callback(config, event_sink)),
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        },
    });

    let model_name = config
        .provider
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());
    let runtime_extensions = discover_runtime_extensions(config);
    let mut process_context = ProcessUserInputContext::new(
        config.session_id.into(),
        config.permission_mode,
        &model_name,
    );
    process_context.effort = parse_effort(config.effort.as_deref());
    process_context.discovered_skills = runtime_extensions
        .skills
        .into_iter()
        .collect::<HashSet<_>>();

    let mut query_config = QueryEngineConfig::new(
        config.session_id.into(),
        &model_name,
        backend,
        tool_runner,
        rc_engine_events::EventStream::new(64),
    )
    .with_observer(observer);
    if event_sink.is_some() {
        query_config =
            query_config.with_provider_invocation_mode(ProviderInvocationMode::Streaming);
    }
    query_config.max_turns = u32::try_from(config.max_turns).unwrap_or(u32::MAX);

    let mut engine = QueryEngine::new(query_config, existing_messages);
    let result = engine
        .submit_message(
            vec![Message::from(ConversationEntry::user(prompt))],
            process_context,
        )
        .await;

    *conversation = legacy_conversation_for_result(&engine, result.as_ref().err());
    {
        let mut shared_hook_state = shared.hook_state.lock().await;
        *hook_state = std::mem::take(&mut *shared_hook_state);
    }

    let latest_request_id = shared.latest_request_id.lock().await.clone();
    let usage = effective_usage(
        UsagePayload {
            input_tokens: engine.state().usage.input_tokens,
            output_tokens: engine.state().usage.output_tokens,
        },
        shared.latest_streaming_usage.lock().await.clone(),
    );
    let total_tool_calls = conversation
        .iter()
        .map(|entry| entry.tool_calls.len())
        .sum::<usize>();
    let permission_denials = match &result {
        Ok(query_result) => query_result.permission_denials.clone(),
        Err(_) => engine.state().permission_denials.clone(),
    };
    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = started.elapsed().as_millis() as u64;
    let model_usage = serde_json::json!({
        "provider": config.provider.name.clone(),
        "model": config.provider.model.clone(),
        "protocol": config.provider.protocol.as_str(),
        "turns": engine.state().turn,
        "tool_calls": total_tool_calls,
        "request_id": latest_request_id,
    });

    match result {
        Ok(query_result) => {
            let outcome = PromptRunOutcome {
                text: query_result.final_text.unwrap_or_default(),
                duration_ms,
                duration_api_ms: duration_ms,
                num_turns: query_result.turns,
                stop_reason: query_result.stop_reason.clone(),
                total_cost_usd: 0.0,
                usage,
                model_usage,
                permission_denials,
            };
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "is_error": false,
                    "stop_reason": outcome.stop_reason.clone(),
                    "usage": {
                        "input_tokens": outcome.usage.input_tokens,
                        "output_tokens": outcome.usage.output_tokens,
                    },
                    "duration_ms": duration_ms,
                    "num_turns": outcome.num_turns,
                    "total_cost_usd": outcome.total_cost_usd,
                    "model_usage": outcome.model_usage.clone(),
                    "permission_denials": outcome.permission_denials.clone(),
                    "request_id": outcome.model_usage.get("request_id").cloned().unwrap_or(serde_json::Value::Null),
                }),
            )?;
            Ok(outcome)
        }
        Err(rc_query_engine::EngineError::Stopped(reason))
            if reason == format!("turn budget exceeded ({})", config.max_turns) =>
        {
            let error = anyhow!(
                "Maximum turn budget reached ({}) without a final assistant reply.",
                config.max_turns
            );
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "is_error": true,
                    "stop_reason": "max_turns",
                    "usage": {
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    },
                    "duration_ms": duration_ms,
                    "num_turns": engine.state().turn,
                    "total_cost_usd": 0.0,
                    "model_usage": model_usage.clone(),
                    "permission_denials": permission_denials.clone(),
                    "request_id": model_usage.get("request_id").cloned().unwrap_or(serde_json::Value::Null),
                    "error": error.to_string(),
                }),
            )?;
            Err(error)
        }
        Err(error) => {
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "is_error": true,
                    "stop_reason": "error",
                    "usage": {
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    },
                    "duration_ms": duration_ms,
                    "num_turns": engine.state().turn,
                    "total_cost_usd": 0.0,
                    "model_usage": model_usage,
                    "permission_denials": permission_denials,
                    "request_id": latest_request_id,
                    "error": error.to_string(),
                }),
            )?;
            Err(error.into())
        }
    }
}

fn parse_effort(effort: Option<&str>) -> EffortLevel {
    match effort.unwrap_or_default().to_ascii_lowercase().as_str() {
        "low" => EffortLevel::Low,
        "high" => EffortLevel::High,
        _ => EffortLevel::Medium,
    }
}

fn effective_usage(
    usage: UsagePayload,
    latest_streaming_usage: Option<UsagePayload>,
) -> UsagePayload {
    if usage.input_tokens == 0 && usage.output_tokens == 0 {
        latest_streaming_usage.unwrap_or(usage)
    } else {
        usage
    }
}

fn is_permission_denied_message(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    lowered.contains("permission denied")
        || (lowered.contains("permission") && lowered.contains("denied"))
}

fn assistant_entry_from_message(message: &Message) -> Result<ConversationEntry> {
    let Message::Assistant(message) = message else {
        return Err(anyhow!(
            "expected assistant message, got {}",
            message_kind(message)
        ));
    };
    let content_blocks = message
        .blocks
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ConversationEntry {
        role: ConversationRole::Assistant,
        text: message.text.clone(),
        history_text: None,
        content_blocks,
        tool_calls: message.tool_calls.clone(),
        attachments: Vec::new(),
        tool_call_id: None,
        name: None,
        is_error: false,
    })
}

fn legacy_conversation_for_result(
    engine: &QueryEngine,
    error: Option<&rc_query_engine::EngineError>,
) -> Vec<ConversationEntry> {
    let mut conversation = engine.state().legacy_conversation();
    if let Some(rc_query_engine::EngineError::Stopped(reason)) = error
        && conversation
            .last()
            .is_some_and(|entry| entry.role == ConversationRole::System && entry.text == *reason)
    {
        conversation.pop();
    }
    conversation
}

fn message_kind(message: &Message) -> &'static str {
    match message {
        Message::User(_) => "user",
        Message::Assistant(_) => "assistant",
        Message::Progress(_) => "progress",
        Message::System(_) => "system",
        Message::Attachment(_) => "attachment",
        Message::HookResult(_) => "hook_result",
        Message::ToolUseSummary(_) => "tool_use_summary",
        Message::Tombstone(_) => "tombstone",
        Message::GroupedToolUse(_) => "grouped_tool_use",
        Message::CollapsedReadSearch(_) => "collapsed_read_search",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};

    use anyhow::Result;
    use rc_config::{ProviderOverrides, RuntimeConfig, RuntimeOverrides, load_runtime_config};
    use rc_core::{
        ConversationEntry, ConversationRole, InputFormat, OutputFormat, PermissionMode,
        ProviderProtocol, ProviderResponse, SubAgentCompletion, ToolCall, UsageSummary,
    };
    use rc_permissions::{
        LayeredPermissionBroker, PermissionBroker, PermissionDecision, PermissionRequest,
        StaticPermissionBroker,
    };
    use rc_provider::{ConversationBackend, ProviderCompatBackend, StreamingCallbacks};
    use rc_query_engine::{QueryObserver, QueryObserverEvent};
    use rc_session::SessionStore;
    use tempfile::{TempDir, tempdir};

    use super::{CompatObserver, CompatSharedState, run_prompt_with_query_engine_compat};
    use crate::conversation::{PromptEventSink, PromptStreamEvent, initialize_conversation};
    use crate::hooks::{HookRunState, RuntimeHookDiscovery};

    fn mock_config_and_store() -> (TempDir, RuntimeConfig, SessionStore) {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides {
                provider: Some("mock".to_owned()),
                base_url: Some("mock://provider".to_owned()),
                api_key: Some("mock".to_owned()),
                model: Some("mock-model".to_owned()),
                protocol: Some(ProviderProtocol::Anthropic),
            },
            RuntimeOverrides::default(),
        )
        .expect("config");
        let store = SessionStore::open(config.paths.clone()).expect("store");
        (tempdir, config, store)
    }

    fn mock_broker(config: &RuntimeConfig) -> Arc<dyn PermissionBroker> {
        Arc::new(LayeredPermissionBroker::new(
            StaticPermissionBroker::from_mode(config.permission_mode),
            Vec::new(),
        ))
    }

    #[derive(Default)]
    struct DenyCommandBroker;

    #[async_trait::async_trait]
    impl PermissionBroker for DenyCommandBroker {
        fn mode(&self) -> Option<PermissionMode> {
            Some(PermissionMode::Default)
        }

        async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
            if request.tool_name == "bash_command" {
                PermissionDecision::deny("Permission denied for bash_command.")
            } else {
                PermissionDecision::allow()
            }
        }
    }

    fn mock_provider_backend(config: &RuntimeConfig) -> Arc<dyn ConversationBackend> {
        Arc::new(ProviderCompatBackend::new(
            Arc::new(rc_provider::ProviderClient::new().expect("provider client")),
            &config.provider,
        ))
    }

    struct DummySubAgentCompletion;

    #[async_trait::async_trait]
    impl SubAgentCompletion for DummySubAgentCompletion {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            Ok(ProviderResponse {
                text: "subagent".to_owned(),
                history_text: None,
                thinking: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                request_id: None,
                usage: UsageSummary::default(),
                stop_reason: "end_turn".to_owned(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingStreamingBackend {
        complete_calls: AtomicUsize,
        complete_streaming_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ConversationBackend for RecordingStreamingBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            self.complete_calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderResponse {
                text: "buffered".to_owned(),
                history_text: None,
                thinking: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                request_id: None,
                usage: UsageSummary::default(),
                stop_reason: "end_turn".to_owned(),
            })
        }

        async fn complete_streaming(
            &self,
            _conversation: &[ConversationEntry],
            callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            self.complete_streaming_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(callbacks) = callbacks
                && let Some(on_text_delta) = callbacks.on_text_delta.as_ref()
            {
                on_text_delta("streaming-backend");
            }
            Ok(ProviderResponse {
                text: "streaming-backend".to_owned(),
                history_text: None,
                thinking: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                request_id: None,
                usage: UsageSummary::default(),
                stop_reason: "end_turn".to_owned(),
            })
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummySubAgentCompletion)
        }
    }

    struct FailingUsageStreamingBackend;

    #[async_trait::async_trait]
    impl ConversationBackend for FailingUsageStreamingBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            Err(anyhow::anyhow!("streaming backend failed"))
        }

        async fn complete_streaming(
            &self,
            _conversation: &[ConversationEntry],
            callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            if let Some(callbacks) = callbacks
                && let Some(on_usage) = callbacks.on_usage.as_ref()
            {
                on_usage(7, 4);
            }
            Err(anyhow::anyhow!("streaming backend failed"))
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummySubAgentCompletion)
        }
    }

    struct PermissionDeniedCommandBackend;

    #[async_trait::async_trait]
    impl ConversationBackend for PermissionDeniedCommandBackend {
        async fn complete(&self, conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            let has_tool_result_after_latest_user = conversation
                .iter()
                .rev()
                .take_while(|entry| entry.role != ConversationRole::User)
                .any(|entry| entry.role == ConversationRole::Tool);
            Ok(ProviderResponse {
                text: if has_tool_result_after_latest_user {
                    "mock provider observed the denial".to_owned()
                } else {
                    "attempting command".to_owned()
                },
                history_text: None,
                thinking: None,
                content_blocks: Vec::new(),
                tool_calls: if has_tool_result_after_latest_user {
                    Vec::new()
                } else {
                    vec![ToolCall {
                        id: "tool-denied-1".to_owned(),
                        name: "bash_command".to_owned(),
                        input: serde_json::json!({"command": "echo hi"}),
                    }]
                },
                request_id: None,
                usage: UsageSummary::default(),
                stop_reason: "end_turn".to_owned(),
            })
        }

        async fn complete_streaming(
            &self,
            conversation: &[ConversationEntry],
            _callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            self.complete(conversation).await
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummySubAgentCompletion)
        }
    }

    #[tokio::test]
    async fn compat_run_persists_basic_mock_result() {
        let (_tempdir, config, store) = mock_config_and_store();
        let discovery = RuntimeHookDiscovery::default();
        let mut conversation =
            initialize_conversation(&store, &config, Some("hello compat")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");

        let outcome = run_prompt_with_query_engine_compat(
            &config,
            &store,
            mock_provider_backend(&config),
            mock_broker(&config),
            None,
            &discovery,
            &mut hook_state,
            &mut conversation,
            "hello compat",
        )
        .await
        .expect("compat run should succeed");

        assert!(outcome.text.contains("mock provider response"));
        let events = store.load_events(config.session_id).expect("events");
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "prompt_started")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "assistant_turn")
        );
        assert!(events.iter().any(|event| event.event_type == "result"));
        let assistant_turn = events
            .iter()
            .find(|event| event.event_type == "assistant_turn")
            .expect("assistant_turn event");
        assert_eq!(
            assistant_turn
                .payload
                .as_ref()
                .and_then(|payload| payload.get("request_id"))
                .and_then(serde_json::Value::as_str),
            Some("mock-request-id")
        );
        let result = events
            .iter()
            .find(|event| event.event_type == "result")
            .expect("result event");
        assert_eq!(
            result
                .payload
                .as_ref()
                .and_then(|payload| payload.get("request_id"))
                .and_then(serde_json::Value::as_str),
            Some("mock-request-id")
        );
        assert_eq!(
            outcome
                .model_usage
                .get("request_id")
                .and_then(serde_json::Value::as_str),
            Some("mock-request-id")
        );
        assert!(
            conversation
                .iter()
                .any(|entry| entry.role == ConversationRole::Assistant)
        );
    }

    #[tokio::test]
    async fn compat_run_executes_tool_round_trip_and_clears_resume_state() {
        let (_tempdir, config, store) = mock_config_and_store();
        let discovery = RuntimeHookDiscovery::default();
        let mut conversation =
            initialize_conversation(&store, &config, Some("list files")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");

        let outcome = run_prompt_with_query_engine_compat(
            &config,
            &store,
            mock_provider_backend(&config),
            mock_broker(&config),
            None,
            &discovery,
            &mut hook_state,
            &mut conversation,
            "list files",
        )
        .await
        .expect("compat run should succeed");

        assert!(outcome.text.contains("tool result"));
        assert!(
            conversation
                .iter()
                .any(|entry| entry.role == ConversationRole::Tool)
        );
        let resume_state = store
            .load_resume_state(config.session_id)
            .expect("resume state")
            .expect("resume state row");
        assert!(resume_state.pending_tool_calls.is_empty());
        let events = store.load_events(config.session_id).expect("events");
        assert!(events.iter().any(|event| event.event_type == "tool_result"));
    }

    #[tokio::test]
    async fn compat_observer_translates_streaming_events_without_duplicate_tool_started() {
        let (_tempdir, config, _store) = mock_config_and_store();
        let captured = Arc::new(StdMutex::new(Vec::<PromptStreamEvent>::new()));
        let captured_sink = Arc::clone(&captured);
        let event_sink: PromptEventSink = Arc::new(move |event| {
            captured_sink
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
        });
        let observer = CompatObserver {
            config: config.clone(),
            store: Arc::new(SessionStore::open(config.paths.clone()).expect("store")),
            shared: Arc::new(CompatSharedState {
                conversation: tokio::sync::Mutex::new(Vec::new()),
                hook_state: tokio::sync::Mutex::new(HookRunState::default()),
                streamed_tool_calls: tokio::sync::Mutex::new(std::collections::HashSet::new()),
                latest_streaming_usage: tokio::sync::Mutex::new(None),
                latest_request_id: tokio::sync::Mutex::new(None),
            }),
            event_sink: Some(event_sink),
            include_partial_messages: true,
        };

        observer
            .on_event(QueryObserverEvent::StreamingToolCallStarted {
                turn: 1,
                tool_call_id: "tool-1".to_owned(),
                tool_name: "bash_command".to_owned(),
            })
            .await
            .expect("streaming tool start");
        observer
            .on_event(QueryObserverEvent::ToolCallStarted {
                turn: 1,
                batch_size: 1,
                batch_index: 0,
                tool_call: ToolCall {
                    id: "tool-1".to_owned(),
                    name: "bash_command".to_owned(),
                    input: serde_json::json!({"command": "echo hi"}),
                },
            })
            .await
            .expect("buffered tool start");
        observer
            .on_event(QueryObserverEvent::StreamingToolCallDelta {
                turn: 1,
                tool_call_id: "tool-1".to_owned(),
                delta: "{\"command\":\"echo hi\"}".to_owned(),
            })
            .await
            .expect("streaming tool delta");
        observer
            .on_event(QueryObserverEvent::StreamingTextDelta {
                turn: 1,
                delta: "OK".to_owned(),
                accumulated_text: "OK".to_owned(),
            })
            .await
            .expect("streaming text delta");

        let events = captured
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    PromptStreamEvent::ToolStarted { tool_call_id, tool_name }
                        if tool_call_id == "tool-1" && tool_name == "bash_command"
                ))
                .count(),
            1
        );
        assert!(events.iter().any(|event| matches!(
            event,
            PromptStreamEvent::ToolProgress {
                tool_call_id: Some(tool_call_id),
                delta: Some(delta),
                elapsed_time_seconds: None,
            } if tool_call_id == "tool-1" && delta.contains("echo hi")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            PromptStreamEvent::MessageDelta { delta } if delta == "OK"
        )));
    }

    #[tokio::test]
    async fn compat_run_reuses_caller_backend_for_streaming_event_sink_path() {
        let (_tempdir, mut config, store) = mock_config_and_store();
        config.include_partial_messages = true;
        let discovery = RuntimeHookDiscovery::default();
        let mut conversation =
            initialize_conversation(&store, &config, Some("streaming")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");
        let backend = Arc::new(RecordingStreamingBackend::default());
        let captured = Arc::new(StdMutex::new(Vec::<PromptStreamEvent>::new()));
        let captured_sink = Arc::clone(&captured);
        let event_sink: PromptEventSink = Arc::new(move |event| {
            captured_sink
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
        });

        let outcome = run_prompt_with_query_engine_compat(
            &config,
            &store,
            backend.clone(),
            mock_broker(&config),
            Some(event_sink),
            &discovery,
            &mut hook_state,
            &mut conversation,
            "streaming",
        )
        .await
        .expect("compat streaming run should succeed");

        assert_eq!(outcome.text, "streaming-backend");
        assert_eq!(backend.complete_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.complete_streaming_calls.load(Ordering::SeqCst), 1);
        let events = captured
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        assert!(events.iter().any(|event| matches!(
            event,
            PromptStreamEvent::MessageDelta { delta } if delta == "streaming-backend"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            PromptStreamEvent::MessageCommitted { text } if text == "streaming-backend"
        )));
    }

    #[tokio::test]
    async fn compat_run_persists_latest_streaming_usage_on_error() {
        let (_tempdir, mut config, store) = mock_config_and_store();
        config.include_partial_messages = true;
        let discovery = RuntimeHookDiscovery::default();
        let mut conversation =
            initialize_conversation(&store, &config, Some("streaming")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");
        let event_sink: PromptEventSink = Arc::new(|_| {});

        let error = run_prompt_with_query_engine_compat(
            &config,
            &store,
            Arc::new(FailingUsageStreamingBackend),
            mock_broker(&config),
            Some(event_sink),
            &discovery,
            &mut hook_state,
            &mut conversation,
            "streaming",
        )
        .await
        .expect_err("compat streaming run should fail");

        assert!(error.to_string().contains("streaming backend failed"));
        let transcript = store
            .load_transcript(config.session_id)
            .expect("load transcript");
        let result = transcript
            .latest_named_event_payload("result")
            .expect("result payload");
        assert_eq!(result["usage"]["input_tokens"], 7);
        assert_eq!(result["usage"]["output_tokens"], 4);
        let streaming_usage = transcript
            .latest_named_event_payload("streaming_usage")
            .expect("streaming usage payload");
        assert_eq!(streaming_usage["usage"]["input_tokens"], 7);
        assert_eq!(streaming_usage["usage"]["output_tokens"], 4);
    }

    #[tokio::test]
    async fn compat_run_permission_denials_include_tool_input() {
        let (_tempdir, config, store) = mock_config_and_store();
        let discovery = RuntimeHookDiscovery::default();
        let mut conversation =
            initialize_conversation(&store, &config, Some("run command")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");

        let outcome = run_prompt_with_query_engine_compat(
            &config,
            &store,
            Arc::new(PermissionDeniedCommandBackend),
            Arc::new(DenyCommandBroker),
            None,
            &discovery,
            &mut hook_state,
            &mut conversation,
            "run command",
        )
        .await
        .expect("compat run should recover from denied command");

        assert_eq!(outcome.permission_denials.len(), 1);
        assert_eq!(outcome.permission_denials[0]["tool_name"], "bash_command");
        assert_eq!(
            outcome.permission_denials[0]["tool_input"]["command"],
            "echo hi"
        );
    }
}
