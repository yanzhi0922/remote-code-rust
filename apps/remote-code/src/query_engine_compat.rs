use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rc_config::{RuntimeConfig, validate_provider_config};
use rc_core::{ConversationEntry, ConversationRole, Message, ToolCall, ToolResult};
use rc_permissions::{LayeredPermissionBroker, StaticPermissionBroker, load_layered_rules};
use rc_protocol::UsagePayload;
use rc_provider::{ConversationBackend, ProviderCompatBackend};
use rc_query_engine::{
    EffortLevel, ProcessUserInputContext, QueryCheckpointKind, QueryEngine, QueryEngineConfig,
    QueryObserver, QueryObserverEvent, ToolRunResult, ToolRunner,
};
use rc_session::SessionStore;
use rc_session::resume_state::{PendingToolCall, ResumeState};
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
use tokio::sync::Mutex;

use crate::conversation::{
    PromptEventSink, PromptRunOutcome, PromptStreamEvent, discover_runtime_extensions,
    truncate_preview,
};
use crate::hooks::{
    HookRunState, RuntimeHookDiscovery, apply_post_tool_hooks, apply_pre_tool_use_hooks,
};

struct CompatSharedState {
    conversation: Mutex<Vec<ConversationEntry>>,
    hook_state: Mutex<HookRunState>,
}

struct CompatObserver {
    config: RuntimeConfig,
    store: Arc<SessionStore>,
    shared: Arc<CompatSharedState>,
    event_sink: Option<PromptEventSink>,
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
            QueryObserverEvent::AssistantMessageCommitted {
                message,
                stop_reason,
                turn,
                usage,
            } => {
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
                if let Some(event_sink) = self.event_sink.as_ref() {
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
    broker: LayeredPermissionBroker<StaticPermissionBroker>,
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
            match execute_tool_call(&effective_tool_call, &self.tool_context, &self.broker).await {
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

        let is_permission_denied = raw_result.is_error
            && raw_result
                .content
                .to_ascii_lowercase()
                .contains("permission denied");
        let permission_denial =
            (is_permission_denied || prepared.blocked_reason.is_some()).then(|| {
                serde_json::json!({
                    "tool_name": effective_tool_call.name,
                    "tool_use_id": effective_tool_call.id,
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

pub(crate) async fn run_prompt_with_query_engine_compat(
    config: &RuntimeConfig,
    store: &SessionStore,
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
    let backend = Arc::new(ProviderCompatBackend::new(
        Arc::new(rc_provider::ProviderClient::new()?),
        &config.provider,
    ));
    let shared = Arc::new(CompatSharedState {
        conversation: Mutex::new(conversation.clone()),
        hook_state: Mutex::new(std::mem::take(hook_state)),
    });
    let observer = Arc::new(CompatObserver {
        config: config.clone(),
        store: compat_store.clone(),
        shared: shared.clone(),
        event_sink: None,
    });
    let tool_runner = Arc::new(CompatToolRunner {
        config: config.clone(),
        store: compat_store.clone(),
        discovery: discovery.clone(),
        shared: shared.clone(),
        broker: LayeredPermissionBroker::new(
            StaticPermissionBroker::new(config.permission_mode),
            load_layered_rules(
                &config.cwd,
                &config.paths.profile_dir,
                &config.settings_files,
            )?,
        ),
        tool_context: ToolExecutionContext {
            cwd: config.cwd.clone(),
            timeout_ms: config.provider.timeout_ms,
            sub_agent: Some(backend.sub_agent_completion()),
            progress_cb: None,
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

    let usage = UsagePayload {
        input_tokens: engine.state().usage.input_tokens,
        output_tokens: engine.state().usage.output_tokens,
    };
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

    use rc_config::{ProviderOverrides, RuntimeConfig, RuntimeOverrides, load_runtime_config};
    use rc_core::{ConversationRole, InputFormat, OutputFormat, PermissionMode, ProviderProtocol};
    use rc_session::SessionStore;
    use tempfile::{TempDir, tempdir};

    use super::run_prompt_with_query_engine_compat;
    use crate::conversation::initialize_conversation;
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
}
