use std::collections::HashMap;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use anyhow::Result;
use rc_config::{RUNTIME_VERSION, RuntimeConfig};
use rc_core::{InputFormat, OutputFormat, PermissionMode, SessionState};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, PermissionDecision, PermissionRequest,
    load_layered_rules,
};
use rc_protocol::{
    InitPayload, PermissionRequestPayload, ProtocolEmitter, ProtocolInput, ResultPayload,
    UsagePayload, parse_input_line,
};
use rc_provider::ProviderCompatBackend;
use rc_session::SessionStore;
use rc_tools::runtime_builtin_tool_specs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;
use uuid::Uuid;

use crate::conversation::{
    PromptEventSink, PromptStreamEvent, discover_runtime_extensions, initialize_conversation,
    run_prompt,
};
use crate::hooks::{HookRunState, discover_runtime_hooks, ensure_session_start_hooks};
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
            api_key_source: if config.provider.api_key.is_some() {
                "user".to_owned()
            } else {
                "missing".to_owned()
            },
            version: RUNTIME_VERSION.to_owned(),
            cwd: config.cwd.display().to_string(),
            tools: runtime_builtin_tool_specs()
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
    let broker = Arc::new(LayeredPermissionBroker::new(
        ChannelPermissionBroker {
            mode: config.permission_mode,
            emitter: emitter.clone(),
            pending_permissions: pending_permissions.clone(),
        },
        load_layered_rules(
            &config.cwd,
            &config.paths.profile_dir,
            &config.settings_files,
        )?,
    ));
    let (prompt_tx, mut prompt_rx) = mpsc::channel::<String>(8);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PromptStreamEvent>();

    if let Some(prompt) = inline_prompt {
        prompt_tx.send(prompt).await?;
    }

    let processor_config = config.clone();
    let processor_store = SessionStore::open(config.paths.clone())?;
    let processor_broker = broker.clone();
    let processor_emitter = emitter.clone();
    let processor_interrupted = interrupted.clone();
    let event_emitter = emitter.clone();
    let event_forwarder = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let mut emitter = event_emitter.lock().await;
            if let Some(detail) = event.runtime_event_detail() {
                emitter.emit_runtime_event(&detail)?;
                continue;
            }
            match event {
                PromptStreamEvent::SubtaskStarted {
                    task_id,
                    parent_task_id,
                    description,
                    depth,
                } => {
                    emitter.emit_subtask_started(
                        &task_id,
                        parent_task_id.as_deref(),
                        &description,
                        depth,
                    )?;
                }
                PromptStreamEvent::SubtaskProgress {
                    task_id,
                    turn,
                    max_turns,
                    summary,
                } => {
                    emitter.emit_subtask_progress(&task_id, turn, max_turns, &summary)?;
                }
                PromptStreamEvent::SubtaskCompleted {
                    task_id,
                    success,
                    output_preview,
                    turns_used,
                } => {
                    emitter.emit_subtask_completed(
                        &task_id,
                        success,
                        &output_preview,
                        turns_used,
                    )?;
                }
                PromptStreamEvent::BatchProgress {
                    total,
                    completed,
                    running,
                } => {
                    emitter.emit_batch_progress(total, completed, running)?;
                }
                PromptStreamEvent::ContextUsage {
                    estimated_tokens,
                    max_input_tokens,
                    threshold_tokens,
                    ratio,
                } => {
                    emitter.emit_context_usage(
                        estimated_tokens,
                        max_input_tokens,
                        threshold_tokens,
                        ratio,
                    )?;
                }
                PromptStreamEvent::ContextOverflow {
                    estimated_tokens,
                    max_input_tokens,
                    threshold_tokens,
                    ratio,
                } => {
                    emitter.emit_context_overflow(
                        estimated_tokens,
                        max_input_tokens,
                        threshold_tokens,
                        ratio,
                    )?;
                }
                PromptStreamEvent::ContextCompacted {
                    entries_removed,
                    usage_ratio,
                } => {
                    emitter.emit_context_compacted(entries_removed, usage_ratio)?;
                }
                PromptStreamEvent::TaskSnapshot { tasks } => {
                    emitter.emit_task_snapshot(tasks)?;
                }
                PromptStreamEvent::MessageDelta { .. }
                | PromptStreamEvent::MessageCommitted { .. }
                | PromptStreamEvent::ToolStarted { .. }
                | PromptStreamEvent::ToolProgress { .. }
                | PromptStreamEvent::ToolFinished { .. } => unreachable!(
                    "runtime events should have been emitted through the shared runtime path"
                ),
            }
        }
        Ok::<(), anyhow::Error>(())
    });
    let processor_event_tx = event_tx.clone();
    let processor = tokio::spawn(async move {
        let backend = ProviderCompatBackend::new(
            Arc::new(rc_provider::ProviderClient::new()?),
            &processor_config.provider,
        );
        let discovery = discover_runtime_hooks(&processor_config, &[]);
        let mut conversation = initialize_conversation(&processor_store, &processor_config, None)?;
        let mut hook_state = HookRunState::load(&processor_store, processor_config.session_id)?;
        let event_sink: PromptEventSink = Arc::new(move |event| {
            let _ = processor_event_tx.send(event);
        });
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
            let started = Instant::now();
            {
                let mut emitter = processor_emitter.lock().await;
                emitter.emit_state(SessionState::Running)?;
            }
            let result = run_prompt(
                &processor_config,
                &processor_store,
                &backend,
                processor_broker.as_ref(),
                Some(event_sink.clone()),
                &discovery,
                &mut hook_state,
                &mut conversation,
                &prompt,
            )
            .await;
            let mut emitter = processor_emitter.lock().await;
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
                    emitter.emit_result(ResultPayload {
                        is_error: true,
                        duration_ms,
                        duration_api_ms: duration_ms,
                        num_turns: 1,
                        result: error.to_string(),
                        stop_reason: "error".to_owned(),
                        total_cost_usd: 0.0,
                        usage: UsagePayload::default(),
                        model_usage: serde_json::json!({}),
                        permission_denials: Vec::new(),
                        errors: vec![error.to_string()],
                    })?;
                }
            }
            emitter.emit_state(SessionState::Idle)?;
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
            } => {
                if let Some(sender) = pending_permissions.lock().await.remove(&request_id) {
                    let _ = sender.send(PermissionDecision {
                        allowed: allow,
                        message,
                    });
                }
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
    drop(event_tx);
    event_forwarder.await??;
    Ok(())
}

pub(crate) fn should_run_headless(config: &RuntimeConfig) -> bool {
    config.print_mode
        || matches!(config.input_format, InputFormat::StreamJson)
        || matches!(config.output_format, OutputFormat::StreamJson)
}

#[derive(Clone)]
struct ChannelPermissionBroker {
    mode: PermissionMode,
    emitter: Arc<Mutex<ProtocolEmitter<io::Stdout>>>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
}

#[async_trait::async_trait]
impl PermissionBroker for ChannelPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
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
                tool_use_id: request.tool_use_id.clone(),
                title: request.title.clone(),
                description: request.description.clone(),
                input: request.input.clone(),
                blocked_path: request.blocked_path.clone(),
                permission_suggestions: Vec::new(),
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
