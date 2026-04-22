use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use anyhow::Result;
use rc_config::{RUNTIME_VERSION, RuntimeConfig};
use rc_core::{InputFormat, OutputFormat, PermissionMode, SessionState};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, PermissionClass, PermissionDecision,
    PermissionRequest, load_layered_rules,
};
use rc_protocol::{
    InitPayload, PermissionRequestPayload, ProtocolEmitter, ProtocolInput, ResultPayload,
    UsagePayload, parse_input_line,
};
use rc_provider::ProviderCompatBackend;
use rc_session::SessionStore;
use rc_tools::runtime_plan_mode::{RuntimePlanModeController, install_plan_mode_runtime};
use rc_tools::runtime_provider_tool_specs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;
use uuid::Uuid;

use crate::conversation::{
    PromptEventSink, PromptStreamEvent, discover_runtime_extensions, initialize_conversation,
    restore_discovered_tool_scope, run_prompt,
};
use crate::hooks::{
    HookRunState, RuntimeHookDiscovery, discover_runtime_hooks, ensure_session_start_hooks,
};
use crate::status::build_runtime_status_snapshot;

#[allow(clippy::too_many_lines)]
pub(crate) async fn run_headless(
    config: &RuntimeConfig,
    inline_prompt: Option<String>,
) -> Result<()> {
    let discovery = discover_runtime_extensions(config);
    let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
        io::stdout(),
        config.session_id,
    )));
    {
        let mut emitter_guard = emitter.lock().await;
        emitter_guard.emit_init(InitPayload {
            api_key_source: config.auth_source.clone().unwrap_or_else(|| {
                if config.provider.api_key.is_some() {
                    "user".to_owned()
                } else {
                    "missing".to_owned()
                }
            }),
            version: RUNTIME_VERSION.to_owned(),
            cwd: config.cwd.display().to_string(),
            tools: runtime_provider_tool_specs()
                .await
                .into_iter()
                .map(|tool| tool.protocol_name)
                .collect(),
            mcp_servers: discovery.mcp_servers,
            model: config.provider.model.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            slash_commands: Vec::new(),
            output_style: "default".to_owned(),
            skills: discovery.skills,
            plugins: discovery.plugins,
        })?;
        emitter_guard.emit_state(SessionState::Idle)?;
        emitter_guard.emit_status_snapshot(&build_runtime_status_snapshot(config))?;
    }

    let pending_permissions = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<PermissionDecision>,
    >::new()));
    let interrupted = Arc::new(AtomicBool::new(false));
    let controller_store = SessionStore::open(config.paths.clone())?;
    let plan_mode_controller = RuntimePlanModeController::load(config, &controller_store)?;
    let _plan_mode_runtime = install_plan_mode_runtime(plan_mode_controller.clone())?;
    let broker: Arc<dyn PermissionBroker> = Arc::new(HeadlessPermissionBroker::new(
        config,
        plan_mode_controller,
        emitter.clone(),
        pending_permissions.clone(),
    ));
    let (prompt_tx, mut prompt_rx) = mpsc::channel::<String>(8);

    if let Some(prompt) = inline_prompt {
        prompt_tx.send(prompt).await?;
    }

    let mut processor_config = config.clone();
    let processor_store = SessionStore::open(config.paths.clone())?;
    let processor_broker = broker.clone();
    let processor_emitter = emitter.clone();
    let processor_interrupted = interrupted.clone();
    let processor = tokio::spawn(async move {
        let backend = ProviderCompatBackend::new(
            Arc::new(rc_provider::ProviderClient::new()?),
            &processor_config.provider,
        );
        let discovered_tool_scope = backend.discovered_tool_scope();
        let discovery = discover_runtime_hooks(&processor_config, &[]);
        let mut conversation = initialize_conversation(&processor_store, &processor_config, None)?;
        restore_discovered_tool_scope(
            &processor_store,
            processor_config.session_id,
            &discovered_tool_scope,
        )?;
        let mut hook_state = HookRunState::load(&processor_store, processor_config.session_id)?;
        ensure_session_start_hooks(
            &discovery,
            &processor_config,
            &processor_store,
            &mut conversation,
            &mut hook_state,
        )
        .await?;
        while let Some(prompt) = prompt_rx.recv().await {
            if processor_interrupted.load(Ordering::Relaxed) {
                processor_interrupted.store(false, Ordering::Relaxed);
                continue;
            }
            run_headless_prompt_once(
                Arc::clone(&processor_emitter),
                &mut processor_config,
                &processor_store,
                Arc::new(backend.clone()),
                discovered_tool_scope.clone(),
                processor_broker.clone(),
                &discovery,
                &mut hook_state,
                &mut conversation,
                &prompt,
            )
            .await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next_line().await? {
        let Some(input) = parse_input_line(&line) else {
            let mut emitter = emitter.lock().await;
            emitter.emit_status(format!("Ignored unsupported input: {line}"))?;
            continue;
        };
        match input {
            ProtocolInput::User { content } => {
                if config.replay_user_messages {
                    let mut emitter = emitter.lock().await;
                    emitter.emit_status(format!("Replayed user prompt: {content}"))?;
                }
                prompt_tx.send(content).await?;
            }
            ProtocolInput::ControlResponse {
                request_id,
                allow,
                message,
                updated_input,
                permission_updates,
                feedback,
                content_blocks,
            } => {
                resolve_pending_permission(
                    &pending_permissions,
                    &emitter,
                    request_id,
                    PermissionDecision {
                        allowed: allow,
                        message,
                        permission_suggestions: Vec::new(),
                        updated_input,
                        permission_updates,
                        feedback,
                        content_blocks,
                    },
                )
                .await?;
            }
            ProtocolInput::Interrupt => {
                interrupted.store(true, Ordering::Relaxed);
                let mut pending = pending_permissions.lock().await;
                for (request_id, sender) in pending.drain() {
                    let _ = sender.send(PermissionDecision::deny("Interrupted by operator."));
                    let mut emitter = emitter.lock().await;
                    let _ = emitter.emit_permission_cancelled(&request_id);
                }
            }
        }
    }
    drop(prompt_tx);
    processor.await??;
    Ok(())
}

pub(crate) fn should_run_headless(config: &RuntimeConfig) -> bool {
    matches!(config.input_format, InputFormat::StreamJson)
        || matches!(config.output_format, OutputFormat::StreamJson)
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct PersistedUsageSnapshot {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl From<PersistedUsageSnapshot> for UsagePayload {
    fn from(value: PersistedUsageSnapshot) -> Self {
        Self {
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct PersistedResultSnapshot {
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    num_turns: u32,
    #[serde(default)]
    stop_reason: String,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    usage: PersistedUsageSnapshot,
    #[serde(default)]
    model_usage: serde_json::Value,
    #[serde(default)]
    permission_denials: Vec<serde_json::Value>,
}

fn load_persisted_result_snapshot(
    store: &SessionStore,
    session_id: Uuid,
) -> Result<Option<PersistedResultSnapshot>> {
    let transcript = store.load_transcript(session_id)?;
    transcript.latest_named_event_as("result")
}

fn prompt_failure_result_payload(
    error: &anyhow::Error,
    duration_ms: u64,
    snapshot: Option<PersistedResultSnapshot>,
) -> ResultPayload {
    let snapshot = snapshot.unwrap_or_default();
    let stop_reason = if snapshot.stop_reason.is_empty() {
        "error".to_owned()
    } else {
        snapshot.stop_reason
    };
    let model_usage = if snapshot.model_usage.is_null() {
        serde_json::json!({})
    } else {
        snapshot.model_usage
    };
    ResultPayload {
        is_error: true,
        duration_ms: if snapshot.duration_ms == 0 {
            duration_ms
        } else {
            snapshot.duration_ms
        },
        duration_api_ms: if snapshot.duration_ms == 0 {
            duration_ms
        } else {
            snapshot.duration_ms
        },
        num_turns: snapshot.num_turns,
        result: error.to_string(),
        stop_reason,
        total_cost_usd: snapshot.total_cost_usd,
        usage: snapshot.usage.into(),
        model_usage,
        permission_denials: snapshot.permission_denials,
        errors: vec![error.to_string()],
    }
}

fn emit_prompt_stream_event<W: Write>(
    emitter: &mut ProtocolEmitter<W>,
    event: PromptStreamEvent,
) -> Result<()> {
    if let Some(detail) = event.runtime_event_detail() {
        emitter.emit_runtime_event(&detail)?;
        return Ok(());
    }
    match event {
        PromptStreamEvent::SubtaskStarted {
            task_id,
            parent_task_id,
            description,
            depth,
        } => emitter.emit_subtask_started(
            &task_id,
            parent_task_id.as_deref(),
            &description,
            depth,
        )?,
        PromptStreamEvent::SubtaskProgress {
            task_id,
            turn,
            max_turns,
            summary,
        } => emitter.emit_subtask_progress(&task_id, turn, max_turns, &summary)?,
        PromptStreamEvent::SubtaskCompleted {
            task_id,
            success,
            output_preview,
            turns_used,
        } => emitter.emit_subtask_completed(&task_id, success, &output_preview, turns_used)?,
        PromptStreamEvent::BatchProgress {
            total,
            completed,
            running,
        } => emitter.emit_batch_progress(total, completed, running)?,
        PromptStreamEvent::ContextUsage {
            estimated_tokens,
            max_input_tokens,
            threshold_tokens,
            ratio,
        } => emitter.emit_context_usage(
            estimated_tokens,
            max_input_tokens,
            threshold_tokens,
            ratio,
        )?,
        PromptStreamEvent::ContextOverflow {
            estimated_tokens,
            max_input_tokens,
            threshold_tokens,
            ratio,
        } => emitter.emit_context_overflow(
            estimated_tokens,
            max_input_tokens,
            threshold_tokens,
            ratio,
        )?,
        PromptStreamEvent::ContextCompacted {
            entries_removed,
            usage_ratio,
        } => emitter.emit_context_compacted(entries_removed, usage_ratio)?,
        PromptStreamEvent::TaskSnapshot { tasks } => emitter.emit_task_snapshot(tasks)?,
        PromptStreamEvent::MessageDelta { .. }
        | PromptStreamEvent::MessageCommitted { .. }
        | PromptStreamEvent::ToolStarted { .. }
        | PromptStreamEvent::ToolProgress { .. }
        | PromptStreamEvent::ToolFinished { .. } => {
            unreachable!("runtime events should have been emitted through the shared runtime path")
        }
    }
    Ok(())
}

async fn forward_prompt_stream_events<W: Write + Send + 'static>(
    emitter: Arc<Mutex<ProtocolEmitter<W>>>,
    mut event_rx: mpsc::UnboundedReceiver<PromptStreamEvent>,
) -> Result<()> {
    while let Some(event) = event_rx.recv().await {
        let mut emitter = emitter.lock().await;
        emit_prompt_stream_event(&mut emitter, event)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_headless_prompt_once<W: Write + Send + 'static>(
    emitter: Arc<Mutex<ProtocolEmitter<W>>>,
    config: &mut RuntimeConfig,
    store: &SessionStore,
    backend: Arc<dyn crate::conversation_backend::ConversationBackend>,
    discovered_tool_scope: rc_provider::DiscoveredToolScope,
    broker: Arc<dyn PermissionBroker>,
    discovery: &RuntimeHookDiscovery,
    hook_state: &mut HookRunState,
    conversation: &mut Vec<rc_core::ConversationEntry>,
    prompt: &str,
) -> Result<()> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<PromptStreamEvent>();
    let forwarder = tokio::spawn(forward_prompt_stream_events(Arc::clone(&emitter), event_rx));
    let sink_tx = event_tx.clone();
    let event_sink: PromptEventSink = Arc::new(move |event| {
        let _ = sink_tx.send(event);
    });

    let started = Instant::now();
    {
        let mut emitter = emitter.lock().await;
        emitter.emit_state(SessionState::Running)?;
    }

    let result = run_prompt(
        config,
        store,
        backend,
        discovered_tool_scope,
        broker,
        Some(event_sink),
        discovery,
        hook_state,
        conversation,
        prompt,
    )
    .await;

    drop(event_tx);
    forwarder.await??;

    let mut emitter = emitter.lock().await;
    match result {
        Ok(outcome) => {
            emitter.emit_assistant(&outcome.text)?;
            emitter.emit_result(ResultPayload {
                is_error: false,
                duration_ms: outcome.duration_ms,
                duration_api_ms: outcome.duration_api_ms,
                num_turns: outcome.num_turns,
                result: outcome.text,
                stop_reason: outcome.stop_reason,
                total_cost_usd: outcome.total_cost_usd,
                usage: outcome.usage,
                model_usage: outcome.model_usage,
                permission_denials: outcome.permission_denials,
                errors: Vec::new(),
            })?;
        }
        Err(error) => {
            #[allow(clippy::cast_possible_truncation)]
            let duration_ms = started.elapsed().as_millis() as u64;
            emitter.emit_runtime_error(error.to_string())?;
            emitter.emit_result(prompt_failure_result_payload(
                &error,
                duration_ms,
                load_persisted_result_snapshot(store, config.session_id)?,
            ))?;
        }
    }
    emitter.emit_state(SessionState::Idle)?;
    Ok(())
}

async fn resolve_pending_permission<W: Write + Send>(
    pending_permissions: &Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    emitter: &Arc<Mutex<ProtocolEmitter<W>>>,
    request_id: String,
    decision: PermissionDecision,
) -> Result<()> {
    if let Some(sender) = pending_permissions.lock().await.remove(&request_id) {
        let _ = sender.send(decision);
        let mut emitter = emitter.lock().await;
        emitter.emit_state(SessionState::Running)?;
    }
    Ok(())
}

#[derive(Clone)]
struct ChannelPermissionFallbackBroker {
    controller: Arc<RuntimePlanModeController>,
    emitter: Arc<Mutex<ProtocolEmitter<io::Stdout>>>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
}

impl std::fmt::Debug for ChannelPermissionFallbackBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelPermissionFallbackBroker")
            .field("mode", &self.controller.current_mode())
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl PermissionBroker for ChannelPermissionFallbackBroker {
    fn mode(&self) -> Option<PermissionMode> {
        Some(self.controller.current_mode())
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        let mode = self.controller.current_mode();

        // Auto-approve in bypass-permissions mode (all tools).
        if matches!(mode, PermissionMode::BypassPermissions) && request.blocked_path.is_none() {
            return PermissionDecision::allow();
        }

        // Auto-approve in dont-ask mode when the tool class is auto-allowed.
        if matches!(mode, PermissionMode::DontAsk) {
            let class = rc_permissions::classify_tool(&request.tool_name);
            if request.blocked_path.is_none() && rc_permissions::auto_allows(mode, class) {
                return PermissionDecision::allow();
            }
        }

        // Auto-approve file edits in accept-edits mode.
        if matches!(mode, PermissionMode::AcceptEdits) {
            let class = rc_permissions::classify_tool(&request.tool_name);
            if request.blocked_path.is_none() && rc_permissions::auto_allows(mode, class) {
                return PermissionDecision::allow();
            }
        }

        if matches!(mode, PermissionMode::Plan) {
            return PermissionDecision::deny(
                "Plan mode is active. Only read-only tools and plan-file edits are allowed.",
            );
        }

        self.prompt(request).await
    }

    async fn decide_forced_prompt(&self, request: PermissionRequest) -> PermissionDecision {
        self.prompt(request).await
    }
}

impl ChannelPermissionFallbackBroker {
    async fn prompt(&self, request: PermissionRequest) -> PermissionDecision {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_permissions
            .lock()
            .await
            .insert(request_id.clone(), tx);
        {
            let mut emitter = self.emitter.lock().await;
            if let Err(error) = emitter.emit_state(SessionState::RequiresAction) {
                warn!("failed to emit state change: {error}");
            }
            if let Err(error) = emitter.emit_permission_request(PermissionRequestPayload {
                request_id: request_id.clone(),
                tool_name: request.tool_name.clone(),
                tool_use_id: request.tool_use_id.unwrap_or_default(),
                title: request.title.unwrap_or_default(),
                description: request.description.unwrap_or_default(),
                input: request.tool_input.clone(),
                blocked_path: request.blocked_path,
                permission_suggestions: request.permission_suggestions,
            }) {
                warn!("failed to emit permission request: {error}");
            }
        }

        match rx.await {
            Ok(decision) => decision,
            Err(_) => PermissionDecision::deny("Permission request channel closed."),
        }
    }
}

struct HeadlessPermissionBroker {
    controller: Arc<RuntimePlanModeController>,
    inner: LayeredPermissionBroker<ChannelPermissionFallbackBroker>,
}

impl HeadlessPermissionBroker {
    fn new(
        config: &RuntimeConfig,
        controller: Arc<RuntimePlanModeController>,
        emitter: Arc<Mutex<ProtocolEmitter<io::Stdout>>>,
        pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    ) -> Self {
        let inner = LayeredPermissionBroker::new(
            ChannelPermissionFallbackBroker {
                controller: controller.clone(),
                emitter,
                pending_permissions,
            },
            load_layered_rules(
                &config.cwd,
                &config.paths.profile_dir,
                &config.settings_files,
                &config.cli_settings_files,
            ),
        );
        Self { controller, inner }
    }

    fn decide_plan_mode(&self, request: PermissionRequest) -> PermissionDecision {
        match request.resolved_permission_class() {
            PermissionClass::Read => PermissionDecision::allow(),
            PermissionClass::Edit if self.controller.plan_file_matches_request(&request) => {
                PermissionDecision::allow()
            }
            PermissionClass::Edit => PermissionDecision::deny(
                "Plan mode is active. Only the current plan file may be edited.",
            ),
            _ => PermissionDecision::deny(
                "Plan mode is active. Only read-only tools and plan-file edits are allowed.",
            ),
        }
    }
}

#[async_trait::async_trait]
impl PermissionBroker for HeadlessPermissionBroker {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if self.controller.current_mode() == PermissionMode::Plan {
            return self.decide_plan_mode(request);
        }
        self.inner.decide(request).await
    }

    async fn decide_forced_prompt(&self, request: PermissionRequest) -> PermissionDecision {
        self.inner.decide_forced_prompt(request).await
    }

    fn mode(&self) -> Option<PermissionMode> {
        if self.controller.current_mode() == PermissionMode::Plan {
            Some(PermissionMode::Plan)
        } else {
            self.inner.mode()
        }
    }

    fn additional_working_directories(&self) -> Vec<std::path::PathBuf> {
        self.inner.additional_working_directories()
    }

    fn add_session_rule(
        &self,
        action: rc_permissions::RuleAction,
        tool_pattern: String,
    ) -> Result<()> {
        self.inner.add_session_rule(action, tool_pattern)
    }

    fn clear_session_rules(&self) -> Result<usize> {
        self.inner.clear_session_rules()
    }

    fn apply_permission_updates(
        &self,
        updates: &[rc_permissions::PermissionUpdate],
    ) -> Result<usize> {
        self.inner.apply_permission_updates(updates)
    }

    fn audit_records(&self) -> Vec<rc_permissions::PermissionAuditRecord> {
        self.inner.audit_records()
    }

    fn layered_rules(&self) -> Vec<rc_permissions::SourceAwarePermissionRule> {
        self.inner.layered_rules()
    }

    fn matching_rule(
        &self,
        request: &PermissionRequest,
    ) -> Option<rc_permissions::SourceAwarePermissionRule> {
        self.inner.matching_rule(request)
    }

    fn matching_rule_action(
        &self,
        request: &PermissionRequest,
    ) -> Option<rc_permissions::RuleAction> {
        self.inner.matching_rule_action(request)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};

    use anyhow::{Result, anyhow};
    use rc_config::{ProviderOverrides, RuntimeConfig, RuntimeOverrides, load_runtime_config};
    use rc_core::{
        ConversationEntry, InputFormat, OutputFormat, PermissionMode, ProviderProtocol,
        ProviderResponse, SubAgentCompletion, UsageSummary,
    };
    use rc_permissions::{
        LayeredPermissionBroker, PermissionBroker, PermissionClass, PermissionDecision,
        PermissionRequest, StaticPermissionBroker,
    };
    use rc_provider::{ConversationBackend, StreamingCallbacks};
    use rc_session::SessionStore;
    use serde_json::Value;
    use tempfile::{NamedTempFile, TempDir, tempdir};
    use tokio::sync::{Mutex, oneshot};

    use super::{HeadlessPermissionBroker, resolve_pending_permission, run_headless_prompt_once};
    use crate::conversation::initialize_conversation;
    use crate::hooks::{HookRunState, RuntimeHookDiscovery};
    use rc_protocol::ProtocolEmitter;
    use rc_tools::runtime_plan_mode::RuntimePlanModeController;

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

    fn read_protocol_events(path: &std::path::Path) -> Vec<Value> {
        let file = fs::File::open(path).expect("open protocol output");
        BufReader::new(file)
            .lines()
            .map(|line| {
                serde_json::from_str::<Value>(&line.expect("line")).expect("json protocol event")
            })
            .collect()
    }

    fn index_of_event(events: &[Value], event_type: &str) -> usize {
        events
            .iter()
            .position(|event| event.get("type").and_then(Value::as_str) == Some(event_type))
            .unwrap_or_else(|| panic!("missing event type `{event_type}`"))
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
                usage: UsageSummary {
                    input_tokens: 9,
                    output_tokens: 2,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
                stop_reason: "end_turn".to_owned(),
            })
        }

        async fn complete_streaming(
            &self,
            _conversation: &[ConversationEntry],
            callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            self.complete_streaming_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(callbacks) = callbacks {
                if let Some(on_text_delta) = callbacks.on_text_delta.as_ref() {
                    on_text_delta("streaming-backend");
                }
                if let Some(on_usage) = callbacks.on_usage.as_ref() {
                    on_usage(12, 3);
                }
            }
            Ok(ProviderResponse {
                text: "streaming-backend".to_owned(),
                history_text: None,
                thinking: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                request_id: None,
                usage: UsageSummary {
                    input_tokens: 12,
                    output_tokens: 3,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
                stop_reason: "end_turn".to_owned(),
            })
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummySubAgentCompletion)
        }
    }

    struct FailingStreamingBackend;

    #[async_trait::async_trait]
    impl ConversationBackend for FailingStreamingBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            Err(anyhow!("streaming backend failed"))
        }

        async fn complete_streaming(
            &self,
            _conversation: &[ConversationEntry],
            _callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            Err(anyhow!("streaming backend failed"))
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummySubAgentCompletion)
        }
    }

    #[derive(Debug)]
    struct SuggestionCaptureBroker {
        captured: Arc<StdMutex<Vec<PermissionRequest>>>,
    }

    #[async_trait::async_trait]
    impl PermissionBroker for SuggestionCaptureBroker {
        async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
            self.captured
                .lock()
                .expect("captured requests")
                .push(request);
            PermissionDecision::deny("permission denied")
        }
    }

    struct ToolCallingBackend {
        outside_path: String,
        turn: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ConversationBackend for ToolCallingBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            let turn = self.turn.fetch_add(1, Ordering::SeqCst);
            if turn == 0 {
                Ok(ProviderResponse {
                    text: String::new(),
                    history_text: None,
                    thinking: None,
                    content_blocks: Vec::new(),
                    tool_calls: vec![rc_core::ToolCall {
                        id: "tool-read-outside".to_owned(),
                        name: "read_file".to_owned(),
                        input: serde_json::json!({"path": self.outside_path}),
                    }],
                    request_id: None,
                    usage: UsageSummary::default(),
                    stop_reason: "tool_use".to_owned(),
                })
            } else {
                Ok(ProviderResponse {
                    text: "done".to_owned(),
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
    async fn headless_default_compat_path_emits_stream_json_message_events_and_result() {
        let (_tempdir, mut config, store) = mock_config_and_store();
        config.include_partial_messages = true;
        let mut conversation =
            initialize_conversation(&store, &config, Some("streaming")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");
        let backend = Arc::new(RecordingStreamingBackend::default());
        let output = NamedTempFile::new().expect("protocol output");
        let broker = mock_broker(&config);
        let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
            output.reopen().expect("reopen output"),
            config.session_id,
        )));

        run_headless_prompt_once(
            Arc::clone(&emitter),
            &mut config,
            &store,
            backend.clone(),
            rc_provider::DiscoveredToolScope::default(),
            broker,
            &RuntimeHookDiscovery::default(),
            &mut hook_state,
            &mut conversation,
            "streaming",
        )
        .await
        .expect("headless prompt should succeed");

        drop(emitter);
        let events = read_protocol_events(output.path());
        assert_eq!(backend.complete_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.complete_streaming_calls.load(Ordering::SeqCst), 1);

        let running_index = events
            .iter()
            .position(|event| {
                event.get("type").and_then(Value::as_str) == Some("system")
                    && event.get("subtype").and_then(Value::as_str) == Some("session_state_changed")
                    && event.get("state").and_then(Value::as_str) == Some("running")
            })
            .expect("running state event");
        let context_index = index_of_event(&events, "context_usage");
        let delta_index = index_of_event(&events, "message_delta");
        let committed_index = index_of_event(&events, "message_committed");
        let assistant_index = index_of_event(&events, "assistant");
        let result_index = index_of_event(&events, "result");
        let idle_index = events
            .iter()
            .position(|event| {
                event.get("type").and_then(Value::as_str) == Some("system")
                    && event.get("subtype").and_then(Value::as_str) == Some("session_state_changed")
                    && event.get("state").and_then(Value::as_str) == Some("idle")
            })
            .expect("idle state event");

        assert!(running_index < context_index);
        assert!(context_index < delta_index);
        assert!(delta_index < committed_index);
        assert!(committed_index < assistant_index);
        assert!(assistant_index < result_index);
        assert!(result_index < idle_index);
        assert_eq!(events[delta_index]["delta"], "streaming-backend");
        assert_eq!(events[committed_index]["text"], "streaming-backend");
        assert_eq!(
            events[assistant_index]["message"]["content"][0]["text"],
            "streaming-backend"
        );
        assert_eq!(events[result_index]["subtype"], "success");
        assert_eq!(events[result_index]["result"], "streaming-backend");
        assert_eq!(
            events[result_index]["permission_denials"],
            Value::Array(Vec::new())
        );
        assert_eq!(events[result_index]["usage"]["input_tokens"], 12);
        assert_eq!(events[result_index]["usage"]["output_tokens"], 3);
    }

    #[tokio::test]
    async fn headless_error_result_reuses_persisted_compat_metadata() {
        let (_tempdir, mut config, store) = mock_config_and_store();
        config.include_partial_messages = true;
        let mut conversation =
            initialize_conversation(&store, &config, Some("streaming")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");
        let output = NamedTempFile::new().expect("protocol output");
        let broker = mock_broker(&config);
        let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
            output.reopen().expect("reopen output"),
            config.session_id,
        )));

        run_headless_prompt_once(
            Arc::clone(&emitter),
            &mut config,
            &store,
            Arc::new(FailingStreamingBackend),
            rc_provider::DiscoveredToolScope::default(),
            broker,
            &RuntimeHookDiscovery::default(),
            &mut hook_state,
            &mut conversation,
            "streaming",
        )
        .await
        .expect("headless prompt should emit error result");

        drop(emitter);
        let events = read_protocol_events(output.path());
        let runtime_error_index = index_of_event(&events, "runtime_error");
        let result_index = index_of_event(&events, "result");
        assert!(runtime_error_index < result_index);
        assert_eq!(events[result_index]["subtype"], "error_during_execution");
        assert_eq!(events[result_index]["is_error"], true);
        assert_eq!(events[result_index]["stop_reason"], "error");
        assert_eq!(events[result_index]["modelUsage"]["provider"], "mock");
        assert_eq!(events[result_index]["modelUsage"]["model"], "mock-model");
        assert_eq!(
            events[result_index]["permission_denials"],
            Value::Array(Vec::new())
        );
    }

    #[tokio::test]
    async fn resolve_pending_permission_re_emits_running_state() {
        let (_tempdir, config, _store) = mock_config_and_store();
        let output = NamedTempFile::new().expect("protocol output");
        let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
            output.reopen().expect("reopen output"),
            config.session_id,
        )));
        let pending_permissions = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending_permissions
            .lock()
            .await
            .insert("req-1".to_owned(), tx);

        resolve_pending_permission(
            &pending_permissions,
            &emitter,
            "req-1".to_owned(),
            PermissionDecision {
                allowed: true,
                message: Some("approved".to_owned()),
                permission_suggestions: Vec::new(),
                updated_input: Some(serde_json::json!({"plan":"edited"})),
                permission_updates: Vec::new(),
                feedback: Some("ship it".to_owned()),
                content_blocks: vec![serde_json::json!({"type":"text","text":"extra"})],
            },
        )
        .await
        .expect("resolve permission");

        let decision = rx.await.expect("decision");
        assert!(decision.allowed);
        assert_eq!(decision.message.as_deref(), Some("approved"));
        assert_eq!(
            decision.updated_input,
            Some(serde_json::json!({"plan":"edited"}))
        );
        assert_eq!(decision.feedback.as_deref(), Some("ship it"));
        assert_eq!(decision.content_blocks.len(), 1);
        drop(emitter);

        let events = read_protocol_events(output.path());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "system");
        assert_eq!(events[0]["subtype"], "session_state_changed");
        assert_eq!(events[0]["state"], "running");
    }

    #[tokio::test]
    async fn headless_permission_request_preserves_suggestions() {
        let (_tempdir, mut config, store) = mock_config_and_store();
        let outside = config.cwd.parent().expect("parent").join("outside.txt");
        fs::write(&outside, "secret").expect("outside file");
        config.include_partial_messages = true;
        let mut conversation =
            initialize_conversation(&store, &config, Some("streaming")).expect("conversation");
        let mut hook_state = HookRunState::load(&store, config.session_id).expect("hook state");
        let output = NamedTempFile::new().expect("protocol output");
        let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
            output.reopen().expect("reopen output"),
            config.session_id,
        )));
        let captured = Arc::new(StdMutex::new(Vec::new()));

        run_headless_prompt_once(
            Arc::clone(&emitter),
            &mut config,
            &store,
            Arc::new(ToolCallingBackend {
                outside_path: outside.to_string_lossy().into_owned(),
                turn: AtomicUsize::new(0),
            }),
            rc_provider::DiscoveredToolScope::default(),
            Arc::new(SuggestionCaptureBroker {
                captured: captured.clone(),
            }),
            &RuntimeHookDiscovery::default(),
            &mut hook_state,
            &mut conversation,
            "streaming",
        )
        .await
        .expect("headless prompt should succeed");

        drop(emitter);
        let requests = captured.lock().expect("captured");
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].permission_suggestions.is_empty());
    }

    #[tokio::test]
    async fn headless_plan_mode_exit_forced_prompt_enqueues_pending_permission() {
        let (_tempdir, config, store) = mock_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .activate_for_slash_command(Some("audit runtime"))
            .expect("enter plan mode");
        let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
            std::io::stdout(),
            config.session_id,
        )));
        let pending_permissions = Arc::new(Mutex::new(HashMap::new()));
        let broker = Arc::new(HeadlessPermissionBroker::new(
            &config,
            controller,
            Arc::clone(&emitter),
            pending_permissions.clone(),
        ));

        let broker_task = {
            let broker = Arc::clone(&broker);
            let cwd = config.cwd.to_string_lossy().into_owned();
            tokio::spawn(async move {
                broker
                    .decide_forced_prompt(PermissionRequest {
                        tool_name: "exit_plan_mode".to_owned(),
                        permission_class: Some(PermissionClass::Read),
                        tool_input: serde_json::json!({}),
                        working_directory: Some(cwd),
                        tool_use_id: Some("tool-exit-plan".to_owned()),
                        title: Some("Allow ExitPlanMode".to_owned()),
                        description: Some(
                            "Prompts the user to exit plan mode and start coding".to_owned(),
                        ),
                        blocked_path: None,
                        permission_suggestions: Vec::new(),
                    })
                    .await
            })
        };

        let request_id = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Some(request_id) = pending_permissions.lock().await.keys().next().cloned() {
                    break request_id;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("permission request should be emitted");

        resolve_pending_permission(
            &pending_permissions,
            &emitter,
            request_id,
            PermissionDecision {
                allowed: true,
                message: Some("approved".to_owned()),
                permission_suggestions: Vec::new(),
                updated_input: None,
                permission_updates: Vec::new(),
                feedback: None,
                content_blocks: Vec::new(),
            },
        )
        .await
        .expect("resolve permission");

        let decision = broker_task.await.expect("broker task");
        assert!(decision.allowed);
        assert_eq!(decision.message.as_deref(), Some("approved"));
    }
}
