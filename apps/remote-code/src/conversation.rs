use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use rc_config::{
    RuntimeConfig, SettingSource, import_legacy_profile, normalize_base_url,
    validate_provider_config,
};
use rc_core::{ConversationEntry, ConversationRole, PermissionMode};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, StaticPermissionBroker, load_layered_rules,
    rules::summarize_rule_sources,
};
use rc_protocol::{MessageRole, RuntimeEventDetail, UsagePayload};
use rc_provider::context::ContextWindowManager;
use rc_provider::{ProviderCompatBackend, StreamingCallbacks};
use rc_session::resume_state::{PendingToolCall, ResumeState};
use rc_session::{SessionStore, conversation::ensure_conversation_initialized};
use rc_skills::SkillDocument;
use rc_tools::{
    ProgressCallback, ToolExecutionContext,
    agent::{DelegateProgressEvent, parse_delegate_progress_event},
    execute_tool_call,
    mcp_runtime::discover_runtime_mcp_servers,
    runtime_provider_tool_spec,
    tasks::load_persisted_ui_task_snapshots,
};
use rc_ui_bridge::UiTaskNode;

use crate::agents::build_remote_code_sub_agent_runtime;
use crate::cli::Cli;
use crate::conversation_backend::ConversationBackend;
use crate::hooks::{
    HookRunState, RuntimeHookDiscovery, apply_post_tool_hooks, apply_pre_tool_use_hooks,
    discover_runtime_hooks, ensure_session_start_hooks,
};
use crate::query_engine_compat::run_prompt_with_query_engine_compat;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedProviderContext {
    pub(crate) name: String,
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) protocol: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedSessionContext {
    pub(crate) cwd: PathBuf,
    pub(crate) permission_mode: String,
    pub(crate) provider: PersistedProviderContext,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeExtensionDiscovery {
    pub(crate) skills: Vec<String>,
    pub(crate) plugins: Vec<String>,
    pub(crate) plugin_runtimes: Vec<String>,
    pub(crate) mcp_servers: Vec<String>,
    pub(crate) disabled_mcp_servers: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PromptRunOutcome {
    pub(crate) text: String,
    pub(crate) duration_ms: u64,
    pub(crate) duration_api_ms: u64,
    pub(crate) num_turns: u32,
    pub(crate) stop_reason: String,
    pub(crate) total_cost_usd: f64,
    pub(crate) usage: UsagePayload,
    pub(crate) model_usage: serde_json::Value,
    pub(crate) permission_denials: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct WizardSettingsDocument {
    provider: WizardSettingsProvider,
}

#[derive(Debug, Clone, serde::Serialize)]
struct WizardSettingsProvider {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    protocol: rc_core::ProviderProtocol,
}

#[derive(Debug, Clone)]
struct WizardProviderSelection {
    provider_name: String,
    protocol: rc_core::ProviderProtocol,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
}

pub(crate) type PromptEventSink = Arc<dyn Fn(PromptStreamEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub(crate) enum PromptStreamEvent {
    MessageDelta {
        delta: String,
    },
    MessageCommitted {
        text: String,
    },
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    ToolProgress {
        tool_call_id: Option<String>,
        delta: Option<String>,
        elapsed_time_seconds: Option<u64>,
    },
    ToolFinished {
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        summary: Option<String>,
    },
    SubtaskStarted {
        task_id: String,
        parent_task_id: Option<String>,
        description: String,
        depth: u32,
    },
    SubtaskProgress {
        task_id: String,
        turn: u32,
        max_turns: u32,
        summary: String,
    },
    SubtaskCompleted {
        task_id: String,
        success: bool,
        output_preview: String,
        turns_used: u32,
    },
    BatchProgress {
        total: usize,
        completed: usize,
        running: usize,
    },
    ContextUsage {
        estimated_tokens: u64,
        max_input_tokens: u64,
        threshold_tokens: u64,
        ratio: f64,
    },
    ContextOverflow {
        estimated_tokens: u64,
        max_input_tokens: u64,
        threshold_tokens: u64,
        ratio: f64,
    },
    ContextCompacted {
        entries_removed: usize,
        usage_ratio: f64,
    },
    TaskSnapshot {
        tasks: Vec<UiTaskNode>,
    },
}

impl PromptStreamEvent {
    #[must_use]
    pub(crate) fn runtime_event_detail(&self) -> Option<RuntimeEventDetail> {
        match self {
            Self::MessageDelta { delta } => Some(RuntimeEventDetail::MessageDelta {
                role: MessageRole::Assistant,
                delta: delta.clone(),
                message_id: None,
            }),
            Self::MessageCommitted { text } => Some(RuntimeEventDetail::MessageCommitted {
                role: MessageRole::Assistant,
                text: text.clone(),
                message_id: None,
            }),
            Self::ToolStarted {
                tool_call_id,
                tool_name,
            } => Some(RuntimeEventDetail::ToolStarted {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
            }),
            Self::ToolProgress {
                tool_call_id,
                delta,
                elapsed_time_seconds,
            } => Some(RuntimeEventDetail::ToolProgress {
                tool_call_id: tool_call_id.clone(),
                tool_name: None,
                delta: delta.clone(),
                elapsed_time_seconds: *elapsed_time_seconds,
            }),
            Self::ToolFinished {
                tool_call_id,
                tool_name,
                is_error,
                summary,
            } => Some(RuntimeEventDetail::ToolFinished {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                is_error: *is_error,
                summary: summary.clone(),
            }),
            Self::SubtaskStarted { .. }
            | Self::SubtaskProgress { .. }
            | Self::SubtaskCompleted { .. }
            | Self::BatchProgress { .. }
            | Self::ContextUsage { .. }
            | Self::ContextOverflow { .. }
            | Self::ContextCompacted { .. }
            | Self::TaskSnapshot { .. } => None,
        }
    }
}

pub(crate) fn truncate_preview(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = collapsed.chars().take(max_chars).collect::<String>();
    if collapsed.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
}

fn session_task_dir(config: &RuntimeConfig) -> PathBuf {
    config
        .paths
        .artifacts_dir
        .join("tasks")
        .join(config.session_id.to_string())
}

fn emit_task_snapshot_if_available(event_sink: &PromptEventSink, task_dir: &Path) {
    if let Ok(tasks) = load_persisted_ui_task_snapshots(task_dir) {
        event_sink(PromptStreamEvent::TaskSnapshot { tasks });
    }
}

fn is_permission_denied_message(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    lowered.contains("permission denied")
        || (lowered.contains("permission") && lowered.contains("denied"))
}

pub(crate) fn build_prompt_progress_callback(
    config: &RuntimeConfig,
    event_sink: &PromptEventSink,
) -> Arc<ProgressCallback> {
    let event_sink = event_sink.clone();
    let task_dir = session_task_dir(config);
    Arc::new(move |message: &str| {
        let Some(event) = parse_delegate_progress_event(message) else {
            return;
        };
        match event {
            DelegateProgressEvent::SubtaskStarted {
                task_id,
                parent_task_id,
                description,
                depth,
            } => {
                event_sink(PromptStreamEvent::SubtaskStarted {
                    task_id,
                    parent_task_id,
                    description,
                    depth,
                });
                emit_task_snapshot_if_available(&event_sink, &task_dir);
            }
            DelegateProgressEvent::SubtaskProgress {
                task_id,
                turn,
                max_turns,
                summary,
            } => {
                event_sink(PromptStreamEvent::SubtaskProgress {
                    task_id,
                    turn,
                    max_turns,
                    summary,
                });
                emit_task_snapshot_if_available(&event_sink, &task_dir);
            }
            DelegateProgressEvent::SubtaskCompleted {
                task_id,
                success,
                output_preview,
                turns_used,
            } => {
                event_sink(PromptStreamEvent::SubtaskCompleted {
                    task_id,
                    success,
                    output_preview,
                    turns_used,
                });
                emit_task_snapshot_if_available(&event_sink, &task_dir);
            }
            DelegateProgressEvent::BatchProgress {
                total,
                completed,
                running,
            } => {
                event_sink(PromptStreamEvent::BatchProgress {
                    total,
                    completed,
                    running,
                });
                emit_task_snapshot_if_available(&event_sink, &task_dir);
            }
        }
    })
}

fn build_streaming_callbacks(
    include_partial_messages: bool,
    event_sink: PromptEventSink,
) -> StreamingCallbacks {
    let text_sink = event_sink.clone();
    let start_sink = event_sink.clone();
    let progress_sink = event_sink;
    StreamingCallbacks {
        on_text_delta: include_partial_messages.then(|| {
            Box::new(move |delta: &str| {
                if delta.is_empty() {
                    return;
                }
                text_sink(PromptStreamEvent::MessageDelta {
                    delta: delta.to_owned(),
                });
            }) as Box<dyn Fn(&str) + Send + Sync>
        }),
        on_tool_call_start: Some(Box::new(move |tool_call_id: &str, tool_name: &str| {
            if tool_call_id.is_empty() || tool_name.is_empty() {
                return;
            }
            start_sink(PromptStreamEvent::ToolStarted {
                tool_call_id: tool_call_id.to_owned(),
                tool_name: tool_name.to_owned(),
            });
        })),
        on_tool_call_delta: Some(Box::new(move |tool_call_id: &str, delta: &str| {
            if tool_call_id.is_empty() || delta.is_empty() {
                return;
            }
            progress_sink(PromptStreamEvent::ToolProgress {
                tool_call_id: Some(tool_call_id.to_owned()),
                delta: Some(delta.to_owned()),
                elapsed_time_seconds: None,
            });
        })),
        on_usage: None,
    }
}

pub(crate) fn persist_session_context(store: &SessionStore, config: &RuntimeConfig) -> Result<()> {
    store.append_named_event(
        config.session_id,
        "session_context",
        serde_json::to_value(PersistedSessionContext {
            cwd: config.cwd.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            provider: PersistedProviderContext {
                name: config.provider.name.clone(),
                base_url: config.provider.base_url.clone(),
                model: config.provider.model.clone(),
                protocol: config.provider.protocol.as_str().to_owned(),
            },
        })?,
    )
}

pub(crate) fn restore_session_context(
    store: &SessionStore,
    config: &mut RuntimeConfig,
) -> Result<()> {
    if let Ok(summary) = store.get_session_summary(config.session_id) {
        config.cwd = summary.cwd;
        config.provider.name = summary.provider_name;
        if summary.model.is_some() {
            config.provider.model = summary.model;
        }
    }

    let Ok(transcript) = store.load_transcript(config.session_id) else {
        return Ok(());
    };
    let Some(persisted) =
        transcript.latest_named_event_as::<PersistedSessionContext>("session_context")?
    else {
        return Ok(());
    };
    config.cwd = persisted.cwd;
    if let Some(permission_mode) = parse_permission_mode(&persisted.permission_mode) {
        config.permission_mode = permission_mode;
    }
    config.provider.name = persisted.provider.name;
    config.provider.base_url = persisted.provider.base_url;
    config.provider.model = persisted.provider.model;
    if let Some(protocol) = parse_provider_protocol(&persisted.provider.protocol) {
        config.provider.protocol = protocol;
    }
    Ok(())
}

pub(crate) fn reapply_cli_overrides(cli: &Cli, config: &mut RuntimeConfig) {
    if let Some(cwd) = &cli.cwd {
        cwd.clone_into(&mut config.cwd);
    }
    if let Some(provider) = &cli.provider {
        provider.clone_into(&mut config.provider.name);
    }
    if let Some(model) = &cli.model {
        config.provider.model = Some(model.clone());
    }
    if let Some(api_key) = &cli.api_key {
        config.provider.api_key = Some(api_key.clone());
    }
    if cli.api_key.is_none() && env::var("REMOTE_CODE_API_KEY").is_ok() {
        config.provider.api_key = env::var("REMOTE_CODE_API_KEY").ok();
    }
    if let Some(protocol) = cli.protocol {
        config.provider.protocol = protocol;
    }
    if let Some(base_url) = &cli.base_url {
        config.provider.base_url =
            normalize_base_url(Some(base_url.clone()), config.provider.protocol);
    } else if cli.protocol.is_some() {
        config.provider.base_url =
            normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
    }
}

pub(crate) fn parse_permission_mode(value: &str) -> Option<PermissionMode> {
    match value.trim() {
        "default" => Some(PermissionMode::Default),
        "acceptEdits" => Some(PermissionMode::AcceptEdits),
        "bypassPermissions" => Some(PermissionMode::BypassPermissions),
        "dontAsk" => Some(PermissionMode::DontAsk),
        "plan" => Some(PermissionMode::Plan),
        _ => None,
    }
}

pub(crate) fn parse_provider_protocol(value: &str) -> Option<rc_core::ProviderProtocol> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Some(rc_core::ProviderProtocol::OpenAi),
        "anthropic" => Some(rc_core::ProviderProtocol::Anthropic),
        _ => None,
    }
}

pub(crate) fn discover_runtime_extensions(config: &RuntimeConfig) -> RuntimeExtensionDiscovery {
    let mut skills = BTreeSet::new();
    let mut plugins = BTreeSet::new();
    let mut plugin_runtimes = BTreeSet::new();
    let mut warnings = Vec::new();
    let user_sources_enabled = setting_source_enabled(config, SettingSource::User);

    if user_sources_enabled && config.paths.skills_dir.exists() {
        collect_skill_names(
            rc_skills::discover_skills(&config.paths.skills_dir),
            &mut skills,
            &mut warnings,
            "profile skills",
        );
    }

    if user_sources_enabled && config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(discovered_plugins) => {
                for plugin in discovered_plugins {
                    plugins.insert(plugin.manifest.name.clone());
                    if plugin.runtime_config().is_some() {
                        plugin_runtimes.insert(plugin.manifest.name.clone());
                    }
                    collect_skill_names(
                        plugin.discover_bundled_skills(),
                        &mut skills,
                        &mut warnings,
                        &format!("plugin {}", plugin.manifest.name),
                    );
                }
            }
            Err(error) => warnings.push(format!("Failed to discover plugins: {error}")),
        }
    }

    let mcp_discovery = discover_runtime_mcp_servers(config, &[]);
    let mcp_servers = mcp_discovery.enabled_server_names();
    let disabled_mcp_servers = mcp_discovery.disabled_server_names();
    warnings.extend(mcp_discovery.warnings);

    RuntimeExtensionDiscovery {
        skills: skills.into_iter().collect(),
        plugins: plugins.into_iter().collect(),
        plugin_runtimes: plugin_runtimes.into_iter().collect(),
        mcp_servers,
        disabled_mcp_servers,
        warnings,
    }
}

fn setting_source_enabled(config: &RuntimeConfig, source: SettingSource) -> bool {
    config.allowed_setting_sources.contains(&source)
}

fn collect_skill_names(
    result: std::result::Result<Vec<SkillDocument>, rc_skills::SkillError>,
    skills: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
    source: &str,
) {
    match result {
        Ok(discovered) => {
            skills.extend(
                discovered
                    .into_iter()
                    .map(|skill| skill.metadata.slug)
                    .collect::<Vec<_>>(),
            );
        }
        Err(error) => warnings.push(format!("Failed to discover {source}: {error}")),
    }
}

pub(crate) fn initialize_conversation(
    store: &SessionStore,
    config: &RuntimeConfig,
    title_hint: Option<&str>,
) -> Result<Vec<ConversationEntry>> {
    let title_hint = config
        .session_name
        .as_deref()
        .or(title_hint)
        .or(config.provider.model.as_deref());
    persist_session_context(store, config)?;
    ensure_conversation_initialized(
        store,
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        title_hint,
    )
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_prompt(
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
    if env::var_os("REMOTE_CODE_FORCE_LEGACY_PROMPT_LOOP").is_some() {
        return run_prompt_legacy(
            config,
            store,
            backend,
            broker,
            event_sink,
            discovery,
            hook_state,
            conversation,
            prompt,
        )
        .await;
    }
    run_prompt_with_query_engine_compat(
        config,
        store,
        backend,
        broker,
        event_sink,
        discovery,
        hook_state,
        conversation,
        prompt,
    )
    .await
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
async fn run_prompt_legacy(
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

    let progress_cb = event_sink
        .as_ref()
        .map(|event_sink| build_prompt_progress_callback(config, event_sink));

    let tool_context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
        sub_agent: Some(build_remote_code_sub_agent_runtime(
            config,
            backend.sub_agent_completion(),
        )),
        progress_cb,
        task_stack: std::sync::Arc::new(std::sync::Mutex::new(
            rc_core::task_stack::TaskStack::default(),
        )),
    };
    let mut usage = UsagePayload::default();
    let mut num_turns = 0u32;
    let mut permission_denials = Vec::new();
    let mut total_tool_calls = 0usize;
    let model_name = config.provider.model.as_deref().unwrap_or("unknown");
    let context_manager = ContextWindowManager::for_model(model_name);
    for turn_index in 0..config.max_turns {
        num_turns += 1;

        let budget_snapshot = context_manager.budget_snapshot(conversation);
        if let Some(event_sink) = event_sink.as_ref() {
            event_sink(PromptStreamEvent::ContextUsage {
                estimated_tokens: budget_snapshot.estimated_tokens,
                max_input_tokens: budget_snapshot.max_input_tokens,
                threshold_tokens: budget_snapshot.threshold_tokens(),
                ratio: budget_snapshot.usage_ratio,
            });
        }

        // Compact conversation if context window is getting full.
        if budget_snapshot.exceeds_threshold() {
            if let Some(event_sink) = event_sink.as_ref() {
                event_sink(PromptStreamEvent::ContextOverflow {
                    estimated_tokens: budget_snapshot.estimated_tokens,
                    max_input_tokens: budget_snapshot.max_input_tokens,
                    threshold_tokens: budget_snapshot.threshold_tokens(),
                    ratio: budget_snapshot.usage_ratio,
                });
            }
            store.append_named_event(
                config.session_id,
                "context_overflow",
                serde_json::json!({
                    "turn": turn_index + 1,
                    "estimated_tokens": budget_snapshot.estimated_tokens,
                    "max_input_tokens": budget_snapshot.max_input_tokens,
                    "threshold_tokens": budget_snapshot.threshold_tokens(),
                    "usage_ratio": budget_snapshot.usage_ratio,
                }),
            )?;

            let compacted = context_manager.compact(conversation);
            let removed = conversation.len().saturating_sub(compacted.len());
            *conversation = compacted;
            let compacted_snapshot = context_manager.budget_snapshot(conversation);

            if removed > 0 {
                store.append_named_event(
                    config.session_id,
                    "context_compacted",
                    serde_json::json!({
                        "turn": turn_index + 1,
                        "entries_removed": removed,
                        "usage_ratio_before": budget_snapshot.usage_ratio,
                        "usage_ratio_after": compacted_snapshot.usage_ratio,
                        "estimated_tokens_before": budget_snapshot.estimated_tokens,
                        "estimated_tokens_after": compacted_snapshot.estimated_tokens,
                    }),
                )?;
                if let Some(event_sink) = event_sink.as_ref() {
                    event_sink(PromptStreamEvent::ContextCompacted {
                        entries_removed: removed,
                        usage_ratio: compacted_snapshot.usage_ratio,
                    });
                    event_sink(PromptStreamEvent::ContextUsage {
                        estimated_tokens: compacted_snapshot.estimated_tokens,
                        max_input_tokens: compacted_snapshot.max_input_tokens,
                        threshold_tokens: compacted_snapshot.threshold_tokens(),
                        ratio: compacted_snapshot.usage_ratio,
                    });
                }
            }
        }

        let response = if let Some(event_sink) = event_sink.clone() {
            backend
                .complete_streaming(
                    conversation,
                    Some(build_streaming_callbacks(
                        config.include_partial_messages,
                        event_sink,
                    )),
                )
                .await?
        } else {
            backend.complete(conversation).await?
        };
        usage.input_tokens += response.usage.input_tokens;
        usage.output_tokens += response.usage.output_tokens;
        total_tool_calls += response.tool_calls.len();
        let assistant_entry = ConversationEntry {
            role: ConversationRole::Assistant,
            text: response.text.clone(),
            history_text: response.history_text.clone(),
            content_blocks: response.content_blocks.clone(),
            tool_calls: response.tool_calls.clone(),
            attachments: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        };
        store.append_conversation_entry(config.session_id, &assistant_entry)?;
        conversation.push(assistant_entry);
        store.append_named_event(
            config.session_id,
            "assistant_turn",
            serde_json::json!({
                "turn": turn_index + 1,
                "stop_reason": response.stop_reason,
                "usage": {
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                },
                "tool_calls": response.tool_calls.len(),
                "text_preview": truncate_preview(&response.text, 160),
            }),
        )?;
        if let Some(event_sink) = event_sink.as_ref()
            && !response.text.trim().is_empty()
        {
            event_sink(PromptStreamEvent::MessageCommitted {
                text: response.text.clone(),
            });
        }

        if response.tool_calls.is_empty() {
            store.clear_resume_state(config.session_id)?;
            #[allow(clippy::cast_possible_truncation)]
            let duration_ms = started.elapsed().as_millis() as u64;
            let outcome = PromptRunOutcome {
                text: response.text,
                duration_ms,
                duration_api_ms: duration_ms,
                num_turns,
                stop_reason: response.stop_reason.clone(),
                total_cost_usd: 0.0,
                usage,
                model_usage: serde_json::json!({
                    "provider": config.provider.name.clone(),
                    "model": config.provider.model.clone(),
                    "protocol": config.provider.protocol.as_str(),
                    "turns": num_turns,
                    "tool_calls": total_tool_calls,
                }),
                permission_denials,
            };
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "is_error": false,
                    "stop_reason": response.stop_reason,
                    "usage": {
                        "input_tokens": outcome.usage.input_tokens,
                        "output_tokens": outcome.usage.output_tokens,
                    },
                    "duration_ms": duration_ms,
                    "num_turns": outcome.num_turns,
                }),
            )?;
            return Ok(outcome);
        }

        let pending_tool_calls = response
            .tool_calls
            .iter()
            .map(|tool_call| PendingToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                input: tool_call.input.clone(),
            })
            .collect::<Vec<_>>();
        store.save_resume_state(
            config.session_id,
            &ResumeState::from_pending_calls(pending_tool_calls),
        )?;

        for tool_call in &response.tool_calls {
            let _ = runtime_provider_tool_spec(&tool_call.name)
                .await
                .ok_or_else(|| anyhow!("unknown tool {}", tool_call.name))?;

            let prepared = apply_pre_tool_use_hooks(
                discovery,
                config,
                store,
                conversation,
                hook_state,
                tool_call,
            )
            .await?;

            let effective_tool_call = prepared.call;
            let audit_count_before = broker.audit_records().len();
            let tool_result = if let Some(blocked_reason) = &prepared.blocked_reason {
                rc_core::ToolResult {
                    content: blocked_reason.clone(),
                    is_error: true,
                }
            } else {
                // Capture tool execution errors as error tool results instead of
                // propagating, to keep conversation state consistent for the next
                // provider call.  This matches the TUI error-recovery pattern.
                match execute_tool_call(&effective_tool_call, &tool_context, broker.as_ref()).await
                {
                    Ok(result) => result,
                    Err(error) => {
                        let tool_name = effective_tool_call.name.clone();
                        let tool_id = effective_tool_call.id.clone();
                        tracing::warn!("tool execution error for {tool_name}: {error}");
                        store.append_named_event(
                            config.session_id,
                            "tool_error",
                            serde_json::json!({
                                "tool_name": tool_name,
                                "tool_use_id": tool_id,
                                "error": format!("{error:#}"),
                            }),
                        )?;
                        rc_core::ToolResult {
                            content: format!("Tool execution error: {error}"),
                            is_error: true,
                        }
                    }
                }
            };
            let new_audits = broker
                .audit_records()
                .into_iter()
                .skip(audit_count_before)
                .collect::<Vec<_>>();
            for audit in new_audits {
                store.append_named_event(
                    config.session_id,
                    "permission_decision",
                    serde_json::to_value(&audit)?,
                )?;
            }
            let is_permission_denied =
                tool_result.is_error && is_permission_denied_message(&tool_result.content);
            if is_permission_denied || prepared.blocked_reason.is_some() {
                permission_denials.push(serde_json::json!({
                    "tool_name": effective_tool_call.name,
                    "tool_use_id": effective_tool_call.id,
                    "tool_input": effective_tool_call.input.clone(),
                    "message": tool_result.content.clone(),
                }));
            }
            let tool_preview = truncate_preview(&tool_result.content, 160);
            if let Some(event_sink) = event_sink.as_ref() {
                event_sink(PromptStreamEvent::ToolFinished {
                    tool_call_id: effective_tool_call.id.clone(),
                    tool_name: effective_tool_call.name.clone(),
                    is_error: tool_result.is_error,
                    summary: Some(tool_preview.clone()),
                });
            }
            let truncated_content =
                context_manager.truncate_tool_output_default(&tool_result.content);
            let tool_entry = ConversationEntry::tool(
                effective_tool_call.id.clone(),
                effective_tool_call.name.clone(),
                truncated_content,
                tool_result.is_error,
            );
            store.append_conversation_entry(config.session_id, &tool_entry)?;
            store.append_named_event(
                config.session_id,
                "tool_result",
                serde_json::json!({
                    "tool_name": effective_tool_call.name,
                    "tool_use_id": effective_tool_call.id,
                    "is_error": tool_entry.is_error,
                    "content_preview": tool_preview,
                }),
            )?;
            conversation.push(tool_entry);

            apply_post_tool_hooks(
                discovery,
                config,
                store,
                conversation,
                hook_state,
                &effective_tool_call,
                &tool_result,
            )
            .await?;
        }
        store.clear_resume_state(config.session_id)?;
    }
    let error = anyhow!(
        "Maximum turn budget reached ({}) without a final assistant reply.",
        config.max_turns
    );
    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = started.elapsed().as_millis() as u64;
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
            "num_turns": num_turns,
            "error": error.to_string(),
        }),
    )?;
    Err(error)
}

pub(crate) async fn run_oneshot_text(
    config: &RuntimeConfig,
    store: &SessionStore,
    prompt: String,
) -> Result<()> {
    let backend = ProviderCompatBackend::new(
        Arc::new(rc_provider::ProviderClient::new()?),
        &config.provider,
    );
    let broker: Arc<dyn PermissionBroker> = Arc::new(LayeredPermissionBroker::new(
        StaticPermissionBroker::from_mode(config.permission_mode),
        load_layered_rules(
            &config.cwd,
            &config.paths.profile_dir,
            &config.settings_files,
            &config.cli_settings_files,
        ),
    ));
    let discovery = discover_runtime_hooks(config, &[]);
    let mut conversation = initialize_conversation(store, config, Some(&prompt))?;
    let mut hook_state = HookRunState::load(store, config.session_id)?;
    ensure_session_start_hooks(
        &discovery,
        config,
        store,
        &mut conversation,
        &mut hook_state,
    )
    .await?;
    let response = run_prompt(
        config,
        store,
        Arc::new(backend),
        broker,
        None,
        &discovery,
        &mut hook_state,
        &mut conversation,
        &prompt,
    )
    .await?;
    println!("{}", response.text);
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn run_doctor(config: &RuntimeConfig) -> Result<()> {
    let report = validate_provider_config(&config.provider);
    let discovery = discover_runtime_extensions(config);
    let hooks = crate::runtime_hooks::HookRuntime::discover(config);
    let layered_rules = load_layered_rules(
        &config.cwd,
        &config.paths.profile_dir,
        &config.settings_files,
        &config.cli_settings_files,
    );
    let api_key_state = if config.provider.api_key.is_some() {
        "present"
    } else {
        "missing"
    };

    // Gather additional diagnostic information.
    let tool_count = rc_tools::runtime_builtin_tool_specs().len();
    let model_info = rc_provider::model_info::get_model_info(
        config.provider.model.as_deref().unwrap_or("unknown"),
    );
    let profile_dir = config.paths.profile_dir.display();

    let lines = [
        "=== Remote Code Rust — Doctor Report ===".to_owned(),
        String::new(),
        "[Runtime]".to_owned(),
        format!("  version:        {}", env!("CARGO_PKG_VERSION")),
        format!("  cwd:            {}", config.cwd.display()),
        format!("  profile dir:    {profile_dir}"),
        format!("  session id:     {}", config.session_id),
        format!(
            "  session name:   {}",
            config.session_name.as_deref().unwrap_or("(auto)")
        ),
        format!("  permission mode: {:?}", config.permission_mode),
        String::new(),
        "[Provider]".to_owned(),
        format!("  name:           {}", config.provider.name),
        format!("  protocol:       {}", config.provider.protocol.as_str()),
        format!(
            "  base URL:       {}",
            config.provider.base_url.as_deref().unwrap_or("(missing)")
        ),
        format!(
            "  model:          {}",
            config.provider.model.as_deref().unwrap_or("(missing)")
        ),
        format!("  api key:        {api_key_state}"),
        format!(
            "  auth source:    {}",
            config.auth_source.as_deref().unwrap_or("(missing)")
        ),
        format!(
            "  context window: {} tokens (output reserve: {})",
            model_info.max_context, model_info.max_output
        ),
        format!(
            "  capabilities:   multimodal={}, reasoning={}",
            model_info.multimodal,
            model_info
                .capabilities
                .contains(&rc_provider::model_info::ModelCapability::Reasoning)
        ),
        String::new(),
        "[Tools]".to_owned(),
        format!("  builtin tools:  {tool_count}"),
        format!(
            "  allow-list:     {}",
            if config.allowed_tools.is_empty() {
                "(all)".to_owned()
            } else {
                config.allowed_tools.join(", ")
            }
        ),
        format!(
            "  deny-list:      {}",
            if config.disallowed_tools.is_empty() {
                "(none)".to_owned()
            } else {
                config.disallowed_tools.join(", ")
            }
        ),
        format!("  input format:   {:?}", config.input_format),
        format!("  output format:  {:?}", config.output_format),
        String::new(),
        "[Permissions]".to_owned(),
        format!("  layered rules:  {}", layered_rules.len()),
        format!(
            "  settings files: {}",
            if config.settings_files.is_empty() {
                "(auto discovery only)".to_owned()
            } else {
                config
                    .settings_files
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
        String::new(),
        "[Extensions]".to_owned(),
        format!("  skills:         {}", discovery.skills.len()),
        format!("  plugins:        {}", discovery.plugins.len()),
        format!("  plugin runtimes: {}", discovery.plugin_runtimes.len()),
        format!("  mcp servers:    {}", discovery.mcp_servers.len()),
        format!("  disabled mcp:   {}", discovery.disabled_mcp_servers.len()),
        format!("  hooks:          {}", hooks.list(None).len()),
        String::new(),
        "[Readiness]".to_owned(),
        format!(
            "  status:         {}",
            if report.ok {
                "✓ READY"
            } else {
                "✗ NOT READY"
            }
        ),
    ];
    for line in lines {
        println!("{line}");
    }
    if !layered_rules.is_empty() {
        for (source, count) in summarize_rule_sources(&layered_rules) {
            println!("  - {}: {} rule(s)", source.as_str(), count);
        }
    }
    if !report.issues.is_empty() {
        println!();
        println!("[Issues]");
        for issue in report.issues {
            println!("  ✗ {issue}");
        }
    }
    if !discovery.warnings.is_empty() {
        println!();
        println!("[Warnings]");
        for warning in discovery.warnings {
            println!("  ⚠ {warning}");
        }
    }
    if !hooks.warnings().is_empty() {
        for warning in hooks.warnings() {
            println!("  ⚠ {warning}");
        }
    }
    Ok(())
}

pub(crate) fn run_migrate(
    config: &RuntimeConfig,
    command: crate::cli::MigrateCommand,
) -> Result<()> {
    match command {
        crate::cli::MigrateCommand::Import { source } => {
            let summary = import_legacy_profile(source, &config.paths)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
    }
}

/// Detect whether this is a first run and launch an interactive setup wizard.
///
/// A first run is detected when:
/// - No API key is configured (neither env var nor CLI override)
/// - No active runtime settings files were loaded
///
/// The wizard guides the user through:
/// 1. Provider selection (Anthropic / OpenAI / DeepSeek / GLM / Custom)
/// 2. API key entry
/// 3. Model selection (with sensible defaults per provider)
/// 4. Saves the configuration to the active settings target
pub(crate) fn run_first_run_wizard(config: &mut RuntimeConfig) -> Result<()> {
    if !should_run_first_run_wizard(config) {
        return Ok(());
    }

    // Only run the wizard when connected to a terminal (stdin is tty).
    // In headless / CI environments, skip silently.
    if !atty_check() {
        eprintln!(
            "⚠ No API key configured. Set REMOTE_CODE_API_KEY or run interactively to set up."
        );
        return Ok(());
    }

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          Welcome to Remote Code Rust!                   ║");
    println!("║                                                          ║");
    println!("║  Let's set up your provider configuration.              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Step 1: Provider selection
    println!("Which LLM provider would you like to use?");
    println!("  1) Anthropic (Claude)");
    println!("  2) OpenAI (GPT / o-series)");
    println!("  3) DeepSeek");
    println!("  4) 智谱 AI (GLM)");
    println!("  5) MiniMax");
    println!("  6) Custom (OpenAI-compatible)");
    println!("  7) Custom (Anthropic-compatible)");
    println!();
    let provider_choice = read_line_prompt("Enter choice [1-7]: ")?;

    let (provider_name, protocol, default_base_url, default_model) = match provider_choice.trim() {
        "1" => (
            "anthropic",
            rc_core::ProviderProtocol::Anthropic,
            "https://api.anthropic.com",
            "claude-sonnet-4-20250514",
        ),
        "2" => (
            "openai",
            rc_core::ProviderProtocol::OpenAi,
            "https://api.openai.com",
            "gpt-4o",
        ),
        "3" => (
            "deepseek",
            rc_core::ProviderProtocol::OpenAi,
            "https://api.deepseek.com",
            "deepseek-chat",
        ),
        "4" => (
            "glm",
            rc_core::ProviderProtocol::OpenAi,
            "https://open.bigmodel.cn/api/paas",
            "glm-5.1",
        ),
        "5" => (
            "minimax",
            rc_core::ProviderProtocol::OpenAi,
            "https://api.minimax.chat",
            "MiniMax-M1",
        ),
        "6" => ("custom", rc_core::ProviderProtocol::OpenAi, "", ""),
        "7" => ("custom", rc_core::ProviderProtocol::Anthropic, "", ""),
        _ => {
            println!("  → Using default: OpenAI-compatible");
            ("custom", rc_core::ProviderProtocol::OpenAi, "", "")
        }
    };

    // Step 2: Base URL
    let base_url = if default_base_url.is_empty() {
        let input = read_line_prompt("Enter base URL: ")?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    } else {
        let input = read_line_prompt(&format!("Base URL [{default_base_url}]: "))?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            Some(default_base_url.to_owned())
        } else {
            Some(trimmed)
        }
    };

    // Step 3: API Key
    let api_key = {
        let input = read_line_prompt("Enter your API key: ")?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };

    if api_key.is_none() {
        println!();
        println!("  ⚠ No API key entered. You can set it later via:");
        println!("    export REMOTE_CODE_API_KEY=<your-key>");
        println!();
    }

    // Step 4: Model
    let model = if default_model.is_empty() {
        let input = read_line_prompt("Enter model name: ")?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    } else {
        let input = read_line_prompt(&format!("Model [{default_model}]: "))?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            Some(default_model.to_owned())
        } else {
            Some(trimmed)
        }
    };

    let selection = WizardProviderSelection {
        provider_name: provider_name.to_owned(),
        protocol,
        base_url,
        api_key,
        model,
    };

    // Step 5: Save to the currently-active settings target.
    let settings_path = resolve_first_run_settings_path(config)?;
    write_wizard_settings_file(&settings_path, &selection)?;
    println!();
    println!("  ✓ Configuration saved to {}", settings_path.display());

    // Step 6: Apply to current config
    apply_wizard_settings(config, &settings_path, &selection);

    println!("  ✓ Provider: {}", selection.provider_name);
    if let Some(m) = &selection.model {
        println!("  ✓ Model:    {m}");
    }
    println!();
    println!("  Setup complete! Run `remote-code doctor` to verify your configuration.");
    println!();

    Ok(())
}

fn should_run_first_run_wizard(config: &RuntimeConfig) -> bool {
    config.provider.api_key.is_none() && config.settings_files.is_empty()
}

fn resolve_first_run_settings_path(config: &RuntimeConfig) -> Result<PathBuf> {
    if let Some(path) = config.cli_settings_files.last() {
        return Ok(path.clone());
    }

    for source in [
        SettingSource::Local,
        SettingSource::Project,
        SettingSource::User,
    ] {
        if config.allowed_setting_sources.contains(&source) {
            return Ok(match source {
                SettingSource::User => config.paths.profile_dir.join("settings.json"),
                SettingSource::Project => config.cwd.join(".remote-code").join("settings.json"),
                SettingSource::Local => config.cwd.join(".remote-code").join("settings.local.json"),
            });
        }
    }

    Err(anyhow!(
        "No writable settings target is available; enable at least one of user/project/local or pass --settings"
    ))
}

fn wizard_settings_document(selection: &WizardProviderSelection) -> WizardSettingsDocument {
    WizardSettingsDocument {
        provider: WizardSettingsProvider {
            name: selection.provider_name.clone(),
            base_url: selection.base_url.clone(),
            api_key: selection.api_key.clone(),
            model: selection.model.clone(),
            protocol: selection.protocol,
        },
    }
}

fn write_wizard_settings_file(path: &Path, selection: &WizardProviderSelection) -> Result<()> {
    let document = wizard_settings_document(selection);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "toml" {
        std::fs::write(path, toml::to_string_pretty(&document)?)?;
    } else {
        let settings_file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(settings_file, &document)?;
    }
    Ok(())
}

fn apply_wizard_settings(
    config: &mut RuntimeConfig,
    settings_path: &Path,
    selection: &WizardProviderSelection,
) {
    config.provider.name = selection.provider_name.clone();
    config.provider.protocol = selection.protocol;
    config.provider.base_url = normalize_base_url(selection.base_url.clone(), selection.protocol);
    config.provider.api_key = selection.api_key.clone();
    config.provider.model = selection.model.clone();
    config.auth_source = selection
        .api_key
        .as_ref()
        .map(|_| format!("settings:{}", settings_path.display()));
    config.settings_files = rc_config::settings_layers::resolve_runtime_settings_files(
        &config.cwd,
        &config.paths.profile_dir,
        &config.paths.profiles_dir,
        &config.cli_settings_files,
        &config.allowed_setting_sources,
    );
    config.setting_sources = config
        .settings_files
        .iter()
        .map(|path| format!("settings:{}", path.display()))
        .collect();
}

/// Check if stdin is connected to a terminal (TTY).
fn atty_check() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Read a line from stdin with a prompt.
fn read_line_prompt(prompt: &str) -> Result<String> {
    use std::io::{self, Write};
    print!("{prompt}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rc_config::{ProviderOverrides, RuntimeOverrides, SettingSource, load_runtime_config};
    use rc_protocol::{MessageRole, RuntimeEventDetail};
    use tempfile::tempdir;

    use super::{
        PromptStreamEvent, WizardProviderSelection, apply_wizard_settings,
        discover_runtime_extensions, resolve_first_run_settings_path, should_run_first_run_wizard,
        write_wizard_settings_file,
    };

    fn test_config() -> (tempfile::TempDir, rc_config::RuntimeConfig) {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .expect("config");
        (tempdir, config)
    }

    fn sample_wizard_selection() -> WizardProviderSelection {
        WizardProviderSelection {
            provider_name: "glm".to_owned(),
            protocol: rc_core::ProviderProtocol::OpenAi,
            base_url: Some("https://open.bigmodel.cn/api/paas".to_owned()),
            api_key: Some("secret".to_owned()),
            model: Some("glm-5.1".to_owned()),
        }
    }

    #[test]
    fn prompt_stream_event_maps_message_delta_to_shared_runtime_event() {
        let event = PromptStreamEvent::MessageDelta {
            delta: "hello".to_owned(),
        };

        assert_eq!(
            event.runtime_event_detail(),
            Some(RuntimeEventDetail::MessageDelta {
                role: MessageRole::Assistant,
                delta: "hello".to_owned(),
                message_id: None,
            })
        );
    }

    #[test]
    fn prompt_stream_event_keeps_non_runtime_only_events_local() {
        let event = PromptStreamEvent::ContextUsage {
            estimated_tokens: 10,
            max_input_tokens: 100,
            threshold_tokens: 80,
            ratio: 0.1,
        };

        assert_eq!(event.runtime_event_detail(), None);
    }

    #[test]
    fn wizard_round_trips_loader_compatible_settings_schema() {
        let (_tempdir, config) = test_config();
        let settings_path = config.paths.profile_dir.join("settings.json");
        let selection = sample_wizard_selection();

        write_wizard_settings_file(&settings_path, &selection).expect("wizard settings");
        let resolved = rc_config::settings_layers::load_runtime_settings(&[settings_path])
            .expect("settings should load");

        assert_eq!(resolved.provider_name.as_deref(), Some("glm"));
        assert_eq!(
            resolved.base_url.as_deref(),
            Some("https://open.bigmodel.cn/api/paas")
        );
        assert_eq!(resolved.api_key.as_deref(), Some("secret"));
        assert_eq!(resolved.model.as_deref(), Some("glm-5.1"));
        assert_eq!(resolved.protocol, Some(rc_core::ProviderProtocol::OpenAi));
    }

    #[test]
    fn wizard_prefers_explicit_settings_target_and_updates_runtime_metadata() {
        let (_tempdir, mut config) = test_config();
        let explicit = config.cwd.join("configs").join("wizard.toml");
        config.cli_settings_files = vec![
            config.cwd.join("configs").join("base.toml"),
            explicit.clone(),
        ];
        let selection = sample_wizard_selection();

        let target = resolve_first_run_settings_path(&config).expect("target path");
        assert_eq!(target, explicit);

        write_wizard_settings_file(&target, &selection).expect("write explicit settings");
        apply_wizard_settings(&mut config, &target, &selection);

        let rendered = fs::read_to_string(&target).expect("settings file");
        assert!(rendered.contains("[provider]"));
        assert!(rendered.contains("name = \"glm\""));
        assert_eq!(config.settings_files, config.cli_settings_files);
        assert_eq!(
            config.setting_sources,
            vec![
                format!("settings:{}", config.cli_settings_files[0].display()),
                format!("settings:{}", config.cli_settings_files[1].display())
            ]
        );
        assert_eq!(
            config.auth_source.as_deref(),
            Some(format!("settings:{}", target.display()).as_str())
        );
    }

    #[test]
    fn wizard_target_uses_highest_allowed_scope_and_first_run_gate_is_strict() {
        let (_tempdir, mut config) = test_config();
        config.allowed_setting_sources = vec![SettingSource::Project, SettingSource::Local];

        assert!(should_run_first_run_wizard(&config));
        assert_eq!(
            resolve_first_run_settings_path(&config).expect("target"),
            config.cwd.join(".remote-code").join("settings.local.json")
        );

        config.settings_files = vec![PathBuf::from("visible-settings.json")];
        assert!(!should_run_first_run_wizard(&config));
        config.settings_files.clear();

        config.provider.api_key = Some("env-key".to_owned());
        assert!(!should_run_first_run_wizard(&config));
    }

    #[test]
    fn discover_runtime_extensions_respects_setting_sources() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plugin_root = profile.join("plugins").join("sample");
        fs::create_dir_all(cwd.join(".remote-code")).expect("workspace dir");
        fs::create_dir_all(profile.join("skills").join("demo")).expect("profile skills");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            profile.join("skills").join("demo").join("SKILL.md"),
            "# Demo\n\nSummary.\n",
        )
        .expect("write skill");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.profile]
command = "python"
args = ["profile.py"]

[servers.disabled]
command = "python"
args = ["disabled.py"]
enabled = false"#,
        )
        .expect("write profile mcp");
        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.project]
command = "python"
args = ["project.py"]"#,
        )
        .expect("write project mcp");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{
                "name": "sample",
                "version": "0.1.0",
                "skills": "./skills",
                "mcp": "./mcp.toml",
                "runtime": {
                    "command": "python",
                    "cwd": "."
                }
            }"#,
        )
        .expect("write plugin manifest");
        fs::create_dir_all(plugin_root.join("skills").join("bundled")).expect("plugin skills");
        fs::write(
            plugin_root.join("skills").join("bundled").join("SKILL.md"),
            "# Bundled\n\nSummary.\n",
        )
        .expect("write plugin skill");
        fs::write(
            plugin_root.join("mcp.toml"),
            r#"[servers.plugin]
command = "python"
args = ["plugin.py"]"#,
        )
        .expect("write plugin mcp");

        let project_only = load_runtime_config(
            Some(cwd.clone()),
            Some(profile.clone()),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::Project]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("project config");
        let discovery = discover_runtime_extensions(&project_only);
        assert!(discovery.skills.is_empty());
        assert!(discovery.plugins.is_empty());
        assert!(discovery.plugin_runtimes.is_empty());
        assert_eq!(discovery.mcp_servers, vec!["project".to_owned()]);
        assert!(discovery.disabled_mcp_servers.is_empty());

        let user_only = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::User]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("user config");
        let discovery = discover_runtime_extensions(&user_only);
        assert_eq!(
            discovery.skills,
            vec!["bundled".to_owned(), "demo".to_owned()]
        );
        assert_eq!(discovery.plugins, vec!["sample".to_owned()]);
        assert_eq!(discovery.plugin_runtimes, vec!["sample".to_owned()]);
        assert_eq!(
            discovery.mcp_servers,
            vec!["plugin".to_owned(), "profile".to_owned()]
        );
        assert_eq!(discovery.disabled_mcp_servers, vec!["disabled".to_owned()]);
    }
}
