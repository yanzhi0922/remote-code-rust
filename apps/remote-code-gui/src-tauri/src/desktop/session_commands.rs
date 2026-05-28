use super::*;

#[tauri::command]
pub(super) async fn init_app(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<InitResultDto, String> {
    let runtime = state.runtime.lock().await;
    emit_runtime_status(&app, &runtime.config);
    let sessions_count = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?
        .len();
    Ok(InitResultDto {
        provider: Some(provider_info_from_runtime(&runtime.config.provider)),
        sessions_count,
    })
}

#[tauri::command]
pub(super) async fn list_sessions(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SessionSummaryDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    Ok(sessions
        .into_iter()
        .map(|session| {
            let agent_type = get_session_agent_type(&runtime.session_store, session.session_id);
            session_summary_to_dto(session, agent_type)
        })
        .collect())
}

#[tauri::command]
pub(super) async fn list_archived_sessions(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SessionSummaryDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_archived_sessions()
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    Ok(sessions
        .into_iter()
        .map(|session| {
            let agent_type = get_session_agent_type(&runtime.session_store, session.session_id);
            session_summary_to_dto(session, agent_type)
        })
        .collect())
}

#[tauri::command]
pub(super) async fn get_session_conversation(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<Vec<ConversationEntryDto>, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let conversation = runtime
        .session_store
        .load_conversation(session_id)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    Ok(conversation.iter().map(conversation_entry_to_dto).collect())
}

#[tauri::command]
pub(super) async fn get_session_tasks(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<Vec<SessionTaskDto>, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    load_session_tasks_from_paths(&runtime.config.paths, session_id)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })
}

#[tauri::command]
pub(super) async fn create_session(
    state: State<'_, AppState>,
    title: Option<String>,
    project_path: Option<String>,
    agent_type: Option<String>,
) -> std::result::Result<String, String> {
    // Fix #9: extract all needed data from runtime, then drop the lock before
    // acquiring agent_router — avoids potential deadlock from nested locks.
    let (mut config, session_store, projects) = {
        let runtime = state.runtime.lock().await;
        let config = runtime.config.clone();
        (
            config,
            Arc::clone(&runtime.session_store),
            runtime.projects.clone(),
        )
    };
    config.session_id = Uuid::new_v4();
    let project_path = project_path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请选择项目文件夹后再新建会话。".to_owned())?;
    let normalized_project_path =
        normalize_existing_path(Path::new(&project_path)).map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    let path_key = path_identity(&normalized_project_path);
    if !projects
        .iter()
        .any(|project| path_identity(&project.path) == path_key)
    {
        return Err("会话必须创建在已管理的项目文件夹下。".to_owned());
    }
    config.cwd = normalized_project_path.clone();

    // Parse and validate agent_type; default to "remote_claude" when not provided.
    let agent_type_str = agent_type.as_deref().unwrap_or("remote_claude").to_owned();
    let _parsed_agent_type: ProtocolAgentType =
        serde_json::from_str(&format!("\"{}\"", agent_type_str))
            .map_err(|e| format!("无效的 agent_type: {e}"))?;

    as_error(initialize_session_conversation(
        &session_store,
        &config,
        title.as_deref(),
    ))?;

    // Persist agent_type into session transcript as a named event.
    as_error(session_store.append_named_event(
        config.session_id,
        "agent_type",
        serde_json::json!({ "agent_type": agent_type_str }),
    ))?;

    // All Agent types use native in-process integration:
    // - Codex → CodexInProcessAdapter (rc-codex-adapter)
    // - Claude → QueryEngine (rc-query-engine)
    // - Roo → RooInProcessAdapter (rc-roo-adapter)
    // No subprocess/bridge mode is used.

    Ok(config.session_id.to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SupervisedTurnResult<T> {
    Completed(T),
    Failed(String),
    Cancelled,
}

pub(super) async fn supervised_turn_result<T, E>(
    agent_name: &'static str,
    inner: tokio::task::JoinHandle<std::result::Result<T, E>>,
    format_error: impl FnOnce(E) -> String,
) -> SupervisedTurnResult<T>
where
    T: Send + 'static,
    E: Send + 'static,
{
    match inner.await {
        Ok(Ok(value)) => SupervisedTurnResult::Completed(value),
        Ok(Err(error)) => SupervisedTurnResult::Failed(format_error(error)),
        Err(join_err) if join_err.is_cancelled() => SupervisedTurnResult::Cancelled,
        Err(join_err) if join_err.is_panic() => SupervisedTurnResult::Failed(format!(
            "{agent_name} agent panicked unexpectedly: {join_err}"
        )),
        Err(join_err) => SupervisedTurnResult::Failed(format!(
            "{agent_name} agent task failed unexpectedly: {join_err}"
        )),
    }
}

fn spawn_prompt_supervisor<T, E>(
    app: AppHandle,
    running_prompts: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    session_id: String,
    agent_name: &'static str,
    inner: tokio::task::JoinHandle<std::result::Result<T, E>>,
    on_success: impl FnOnce(&AppHandle, &str, T) + Send + 'static,
    format_error: impl FnOnce(E) -> String + Send + 'static,
) -> tokio::task::JoinHandle<()>
where
    T: Send + 'static,
    E: Send + 'static,
{
    tokio::spawn(async move {
        match supervised_turn_result(agent_name, inner, format_error).await {
            SupervisedTurnResult::Completed(value) => on_success(&app, &session_id, value),
            SupervisedTurnResult::Failed(error) => {
                tracing::error!(
                    session_id = %session_id,
                    agent_name,
                    error = %error,
                    "agent turn supervisor observed failure"
                );
                let _ = app.emit(
                    APP_EVENT_PROMPT_DONE,
                    PromptDoneDto {
                        session_id: session_id.clone(),
                        is_error: true,
                        error: Some(error),
                        result: None,
                    },
                );
            }
            SupervisedTurnResult::Cancelled => {
                // cancel_prompt() owns the user-visible cancellation event.
            }
        }

        let mut running = running_prompts.lock().await;
        running.remove(&session_id);
    })
}

#[tauri::command]
pub(super) async fn send_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    prompt: String,
    session_id: Option<String>,
) -> std::result::Result<String, String> {
    let prompt = prompt.trim().to_owned();
    if prompt.is_empty() {
        return Err("prompt cannot be empty".to_owned());
    }
    const MAX_PROMPT_LEN: usize = 1_000_000; // 1 MB
    if prompt.len() > MAX_PROMPT_LEN {
        return Err(format!(
            "prompt too large ({} bytes, max {} bytes)",
            prompt.len(),
            MAX_PROMPT_LEN
        ));
    }

    let (
        mut config,
        provider,
        session_store,
        pending_permissions,
        provider_configs,
        gui_settings,
        agent_type_str,
    ) = {
        let runtime = state.runtime.lock().await;
        let mut config = runtime.config.clone();
        let selected_provider = config.provider.clone();
        let selected_permission_mode = config.permission_mode;
        let session_id =
            session_id.ok_or_else(|| "请先选择项目文件夹并创建会话，再发送消息。".to_owned())?;
        config.session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
        restore_session_context(&runtime.session_store, &mut config)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
        let agent_type_str = get_session_agent_type(&runtime.session_store, config.session_id);
        config.provider = selected_provider;
        config.permission_mode = selected_permission_mode;
        (
            config,
            Arc::clone(&runtime.provider),
            Arc::clone(&runtime.session_store),
            Arc::clone(&state.pending_permissions),
            runtime.provider_configs.clone(),
            runtime.gui_settings.clone(),
            agent_type_str,
        )
    };

    apply_provider_credentials_from_configs(&mut config.provider, &provider_configs);
    configure_runtime_policy_for_config(&config).map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;

    let sid = config.session_id.to_string();

    // Atomically check for duplicate and reserve the slot to prevent TOCTOU races.
    {
        let mut running = state.running_prompts.lock().await;
        if running.contains_key(&sid) {
            return Err("该会话已有正在运行的提示，请等待完成或取消后再试。".to_owned());
        }

        // ── Branch based on agent_type ──────────────────────────────────
        match agent_type_str.as_str() {
            "remote_codex" => {
                // Native in-process path: Codex uses CodexInProcessAdapter
                // with isolated storage (no bridge binary needed).
                let codex_adapters = Arc::clone(&state.active_codex_adapters);
                let pending_codex_permissions = Arc::clone(&state.pending_codex_permissions);
                let sid_clone = sid.clone();
                let prompt_owned = prompt.clone();
                let codex_options = codex_adapter_options_from_runtime(&config, &gui_settings);
                let codex_session_store = Arc::clone(&session_store);

                let app_for_cleanup = app.clone();
                let inner = tokio::spawn(async move {
                    run_codex_in_process_prompt(
                        &app,
                        &codex_adapters,
                        &pending_codex_permissions,
                        &sid_clone,
                        &prompt_owned,
                        codex_options,
                        codex_session_store,
                    )
                    .await
                });
                let handle = spawn_prompt_supervisor(
                    app_for_cleanup,
                    Arc::clone(&state.running_prompts),
                    sid.clone(),
                    "Codex",
                    inner,
                    |_, _, _| {},
                    |error| error,
                );
                running.insert(sid.clone(), handle);
            }
            "remote_roo" => {
                // Native in-process path: Roo uses RooInProcessAdapter
                // (no bridge binary needed).
                let roo_adapters = Arc::clone(&state.active_roo_adapters);
                let pending_roo_permissions = Arc::clone(&state.pending_roo_permissions);
                let sid_clone = sid.clone();
                let prompt_owned = prompt.clone();
                let working_dir = config.cwd.clone();
                let model = config.provider.model.clone();
                let api_key = config.provider.api_key.clone();
                let provider_name = roo_provider_id_from_runtime(&config.provider);
                let base_url = config.provider.base_url.clone();
                let roo_mcp_servers = roo_mcp_server_overrides(&config);
                let roo_storage_path = config.paths.profile_dir.join("roo");
                let roo_session_store = Arc::clone(&session_store);

                let app_for_cleanup = app.clone();
                let inner = tokio::spawn(async move {
                    run_roo_in_process_prompt(
                        &app,
                        &roo_adapters,
                        &pending_roo_permissions,
                        &sid_clone,
                        &prompt_owned,
                        working_dir,
                        model,
                        api_key,
                        provider_name,
                        base_url,
                        roo_mcp_servers,
                        roo_storage_path,
                        roo_session_store,
                    )
                    .await
                });
                let handle = spawn_prompt_supervisor(
                    app_for_cleanup,
                    Arc::clone(&state.running_prompts),
                    sid.clone(),
                    "Roo",
                    inner,
                    |_, _, _| {},
                    |error| error,
                );
                running.insert(sid.clone(), handle);
            }
            "remote_claude" => {
                // Native in-process path: Claude uses ClaudeInProcessAdapter
                // (wraps QueryEngine via AgentAdapter trait).
                let claude_adapters = Arc::clone(&state.active_claude_adapters);
                let pending_claude_permissions = Arc::clone(&state.pending_claude_permissions);
                let sid_clone = sid.clone();
                let prompt_owned = prompt.clone();

                let app_for_cleanup = app.clone();
                let inner = tokio::spawn(async move {
                    run_claude_in_process_prompt(
                        &app,
                        &claude_adapters,
                        &pending_claude_permissions,
                        &sid_clone,
                        &prompt_owned,
                        config.clone(),
                        session_store.clone(),
                    )
                    .await
                });
                let handle = spawn_prompt_supervisor(
                    app_for_cleanup,
                    Arc::clone(&state.running_prompts),
                    sid.clone(),
                    "Claude",
                    inner,
                    |_, _, _| {},
                    |error| error,
                );
                running.insert(sid.clone(), handle);
            }
            _ => {
                // Fallback path: uses the in-process QueryEngine directly.
                let app_for_cleanup = app.clone();
                let inner = tokio::spawn(async move {
                    crate::query_engine_gui::run_unified_prompt_with_provider(
                        &app,
                        config.clone(),
                        provider,
                        session_store,
                        pending_permissions,
                        &prompt,
                    )
                    .await
                });
                let handle = spawn_prompt_supervisor(
                    app_for_cleanup,
                    Arc::clone(&state.running_prompts),
                    sid.clone(),
                    "QueryEngine",
                    inner,
                    |app, session_id, outcome| {
                        let _ = app.emit(
                            APP_EVENT_PROMPT_DONE,
                            PromptDoneDto {
                                session_id: session_id.to_owned(),
                                is_error: false,
                                error: None,
                                result: Some(PromptResultDto {
                                    session_id: session_id.to_owned(),
                                    text: outcome.text,
                                    tool_calls: outcome
                                        .tool_calls
                                        .iter()
                                        .map(tool_call_to_dto)
                                        .collect(),
                                    usage: usage_to_dto(&outcome.usage),
                                    num_turns: outcome.num_turns,
                                    stop_reason: outcome.stop_reason,
                                }),
                            },
                        );
                    },
                    |error| format!("{error:#}"),
                );
                running.insert(sid.clone(), handle);
            }
        }
    }

    Ok(sid)
}

#[tauri::command]
pub(super) async fn cancel_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<bool, String> {
    let mut running = state.running_prompts.lock().await;
    if let Some(handle) = running.remove(&session_id) {
        handle.abort();
        drop(running);

        // Cascade cancel to the adapter so internal resources are cleaned up.
        let sid = &session_id;
        {
            let mut adapters = state.active_claude_adapters.lock().await;
            if let Some(adapter) = adapters.get_mut(sid) {
                if let Err(e) = adapter.cancel(sid).await {
                    tracing::warn!(error = %e, session_id = %sid, "Claude adapter cancel failed");
                }
            }
        }
        {
            let mut adapters = state.active_codex_adapters.lock().await;
            if let Some(adapter) = adapters.get_mut(sid) {
                if let Err(e) = adapter.cancel(sid).await {
                    tracing::warn!(error = %e, session_id = %sid, "Codex adapter cancel failed");
                }
            }
        }
        {
            let mut adapters = state.active_roo_adapters.lock().await;
            if let Some(adapter) = adapters.get_mut(sid) {
                if let Err(e) = adapter.cancel(sid).await {
                    tracing::warn!(error = %e, session_id = %sid, "Roo adapter cancel failed");
                }
            }
        }

        let _ = app.emit(
            APP_EVENT_PROMPT_DONE,
            PromptDoneDto {
                session_id,
                is_error: true,
                error: Some("已取消".to_owned()),
                result: None,
            },
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub(super) async fn export_session_bundle(
    state: State<'_, AppState>,
    session_id: String,
    format: SessionExportFormatDto,
) -> std::result::Result<SessionExportResultDto, String> {
    let runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    export_session_bundle_for_store(&runtime.session_store, session_id, format)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })
}

#[tauri::command]
pub(super) async fn archive_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<(), String> {
    // 1. Cancel any running prompt for this session.
    {
        let mut running = state.running_prompts.lock().await;
        if let Some(handle) = running.remove(&session_id) {
            handle.abort();
            let _ = app.emit(
                APP_EVENT_PROMPT_DONE,
                PromptDoneDto {
                    session_id: session_id.clone(),
                    is_error: true,
                    error: Some("Session archived".to_owned()),
                    result: None,
                },
            );
        }
    }

    // 2. Stop and remove adapters for this session.
    {
        let mut adapters = state.active_claude_adapters.lock().await;
        if let Some(mut adapter) = adapters.remove(&session_id) {
            if let Err(e) = adapter.stop().await {
                tracing::warn!(error = %e, session_id = %session_id, "Claude adapter stop failed during archive");
            }
        }
    }
    {
        let mut adapters = state.active_codex_adapters.lock().await;
        if let Some(mut adapter) = adapters.remove(&session_id) {
            if let Err(e) = adapter.stop().await {
                tracing::warn!(error = %e, session_id = %session_id, "Codex adapter stop failed during archive");
            }
        }
    }
    {
        let mut adapters = state.active_roo_adapters.lock().await;
        if let Some(mut adapter) = adapters.remove(&session_id) {
            if let Err(e) = adapter.stop().await {
                tracing::warn!(error = %e, session_id = %session_id, "Roo adapter stop failed during archive");
            }
        }
    }

    // 3. Clean up any orphaned pending permissions.
    // The native Claude oneshot senders are keyed by request_id (not session_id),
    // so we can only prune senders whose receiver has been dropped.
    {
        let mut pending = state.pending_permissions.lock().await;
        pending.retain(|_id, tx| !tx.is_closed());
    }
    {
        let mut pending = state.pending_codex_permissions.lock().await;
        pending.retain(|_, v| v.session_id != session_id);
    }
    {
        let mut pending = state.pending_roo_permissions.lock().await;
        pending.retain(|_, v| v.session_id != session_id);
    }
    {
        let mut pending = state.pending_claude_permissions.lock().await;
        pending.retain(|_, v| v.session_id != session_id);
    }

    // 4. Mark session as archived in storage.
    let runtime = state.runtime.lock().await;
    let uuid = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    runtime
        .session_store
        .set_archived(uuid, true)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    Ok(())
}

#[tauri::command]
pub(super) async fn restore_session(
    state: State<'_, AppState>,
    session_id: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let summary = runtime
        .session_store
        .get_session_summary(session_id)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    runtime
        .session_store
        .set_archived(session_id, false)
        .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    if ensure_project_entry(&mut runtime.projects, &summary.cwd) {
        persist_runtime_files(&runtime).map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    }
    Ok(())
}
