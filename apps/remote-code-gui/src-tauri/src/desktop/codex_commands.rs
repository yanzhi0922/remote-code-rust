use super::*;

#[tauri::command]
pub(super) async fn codex_list_threads(
    state: State<'_, AppState>,
    params: Option<CodexThreadListRequest>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_threads(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_read_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .read_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_codex_params::<ThreadStartParams>(request.params, ThreadStartParams::default)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_thread_with_params(Some(params)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_resume_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .resume_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_resume_thread_native(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<ThreadResumeParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.resume_thread_with_params(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fork_thread(
    state: State<'_, AppState>,
    request: CodexThreadRefRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .fork_thread(request.thread_id, request.include_turns)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fork_thread_native(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<ThreadForkParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fork_thread_with_params(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_archive_thread(
    state: State<'_, AppState>,
    request: CodexThreadArchiveRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.archive_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_unarchive_thread(
    state: State<'_, AppState>,
    request: CodexThreadArchiveRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.unarchive_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_unsubscribe(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.unsubscribe_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_elicitation_increment(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .increment_thread_elicitation(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_elicitation_decrement(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .decrement_thread_elicitation(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_set_name(
    state: State<'_, AppState>,
    request: CodexThreadSetNameRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_thread_name(request.thread_id, request.name)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_metadata_update(
    state: State<'_, AppState>,
    request: CodexThreadMetadataUpdateRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = ThreadMetadataUpdateParams {
        thread_id: request.thread_id,
        git_info: Some(ThreadMetadataGitInfoUpdateParams {
            sha: request.sha,
            branch: request.branch,
            origin_url: request.origin_url,
        }),
    };
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.update_thread_metadata(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_current_thread_id(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, String> {
    let key = codex_adapter_key(session_id);
    let adapters = state.active_codex_adapters.lock().await;
    let adapter = adapters
        .get(&key)
        .ok_or_else(|| "Codex adapter was not initialized".to_owned())?;
    Ok(adapter.thread_id().map(|s| s.to_owned()))
}

#[tauri::command]
pub(super) async fn codex_thread_goal_set(
    state: State<'_, AppState>,
    request: CodexThreadGoalSetUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    let session_id = request.session_id.clone();
    with_codex_adapter_value(&state, session_id, |adapter| {
        let thread_id = if request.thread_id.is_empty() {
            adapter.thread_id().map(|s| s.to_owned())
        } else {
            Some(request.thread_id.clone())
        };
        let objective = if request.text.is_empty() {
            None
        } else {
            Some(request.text.clone())
        };
        let status = request.status.clone();
        let token_budget = request.token_budget;
        Box::pin(async move {
            let tid = thread_id.ok_or_else(|| anyhow::anyhow!("No active thread"))?;
            adapter
                .set_thread_goal_with_options(tid, objective, status, token_budget)
                .await
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_goal_get(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    let session_id = request.session_id.clone();
    with_codex_adapter_value(&state, session_id, |adapter| {
        let thread_id = if request.thread_id.is_empty() {
            adapter.thread_id().map(|s| s.to_owned())
        } else {
            Some(request.thread_id.clone())
        };
        Box::pin(async move {
            let tid = thread_id.ok_or_else(|| anyhow::anyhow!("No active thread"))?;
            serde_json::to_value(adapter.get_thread_goal(tid).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_goal_clear(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    let session_id = request.session_id.clone();
    with_codex_adapter_value(&state, session_id, |adapter| {
        let thread_id = if request.thread_id.is_empty() {
            adapter.thread_id().map(|s| s.to_owned())
        } else {
            Some(request.thread_id.clone())
        };
        Box::pin(async move {
            let tid = thread_id.ok_or_else(|| anyhow::anyhow!("No active thread"))?;
            serde_json::to_value(adapter.clear_thread_goal(tid).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_compact_start(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.compact_thread(request.thread_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_shell_command(
    state: State<'_, AppState>,
    request: CodexThreadShellCommandRequest,
) -> std::result::Result<serde_json::Value, String> {
    if request.command.trim().is_empty() {
        return Err("Codex thread shell command cannot be empty".to_owned());
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .run_thread_shell_command(request.thread_id, request.command)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_background_terminals_clean(
    state: State<'_, AppState>,
    request: CodexThreadGoalRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .clean_thread_background_terminals(request.thread_id)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_guardian_denied_action_approve(
    state: State<'_, AppState>,
    request: CodexThreadNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let mut params =
        decode_required_codex_params::<ThreadApproveGuardianDeniedActionParams>(request.params)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
    if params.thread_id.trim().is_empty() {
        params.thread_id = request.thread_id;
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.approve_guardian_denied_action(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_rollback(
    state: State<'_, AppState>,
    request: CodexThreadRollbackUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .rollback_thread(CodexThreadRollbackRequest {
                        thread_id: request.thread_id,
                        num_turns: request.num_turns,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_turns_list(
    state: State<'_, AppState>,
    request: CodexThreadTurnsListRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_thread_turns(request.thread_id, request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_loaded_list(
    state: State<'_, AppState>,
    request: CodexThreadLoadedListRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_loaded_threads(request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_thread_inject_items(
    state: State<'_, AppState>,
    request: CodexThreadNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let mut params = decode_required_codex_params::<ThreadInjectItemsParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    if params.thread_id.trim().is_empty() {
        params.thread_id = request.thread_id;
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.inject_thread_items(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_turn_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<TurnStartParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_turn(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_turn_steer(
    state: State<'_, AppState>,
    request: CodexTurnSteerUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .steer_turn(CodexTurnSteerRequest {
                        thread_id: request.thread_id,
                        expected_turn_id: request.expected_turn_id,
                        message: request.message,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_turn_interrupt(
    state: State<'_, AppState>,
    request: CodexTurnInterruptUiRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .interrupt_turn(CodexTurnInterruptRequest {
                        thread_id: request.thread_id,
                        turn_id: request.turn_id,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_model_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_models(Some(true)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_collaboration_mode_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_collaboration_modes().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_experimental_feature_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_experimental_features().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_experimental_feature_set(
    state: State<'_, AppState>,
    request: CodexExperimentalFeatureSetRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_experimental_feature(request.feature, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_read(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_account().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_login(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<LoginAccountParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.login_account(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_login_cancel(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<CancelLoginAccountParams>(request.params).map_err(
        |error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        },
    )?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.cancel_login_account(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_logout(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.logout_account().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_rate_limits_read(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_account_rate_limits().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_account_add_credits_nudge(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<SendAddCreditsNudgeEmailParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.send_add_credits_nudge_email(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_apps_list(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_apps().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_exec(
    state: State<'_, AppState>,
    request: CodexExecRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.exec_command(request).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_app_server_request(
    state: State<'_, AppState>,
    request: CodexAppServerRequest,
) -> std::result::Result<serde_json::Value, String> {
    if request.method.trim().is_empty() {
        return Err("Codex app-server method cannot be empty".to_owned());
    }
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            adapter
                .app_server_request(request.method, request.params)
                .await
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_exec_write(
    state: State<'_, AppState>,
    request: CodexExecWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .exec_write(CommandExecWriteParams {
                        process_id: request.process_id,
                        delta_base64: request.delta_base64,
                        close_stdin: request.close_stdin,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_exec_terminate(
    state: State<'_, AppState>,
    session_id: Option<String>,
    process_id: String,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.exec_terminate(process_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_exec_resize(
    state: State<'_, AppState>,
    request: CodexExecResizeRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .exec_resize(CommandExecResizeParams {
                        process_id: request.process_id,
                        size: CommandExecTerminalSize {
                            rows: request.rows,
                            cols: request.cols,
                        },
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_windows_sandbox_setup_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<WindowsSandboxSetupStartParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_windows_sandbox_setup(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_mcp_refresh(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.refresh_mcp().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_mcp_status(
    state: State<'_, AppState>,
    request: CodexMcpStatusRequest,
) -> std::result::Result<serde_json::Value, String> {
    let detail = parse_codex_mcp_detail(request.detail.as_deref());
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .list_mcp_status(detail, request.cursor, request.limit)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_mcp_read_resource(
    state: State<'_, AppState>,
    request: CodexMcpResourceReadRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .read_mcp_resource(request.server, request.uri)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_mcp_call_tool(
    state: State<'_, AppState>,
    request: CodexMcpToolCallRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .call_mcp_tool(
                        request.thread_id,
                        request.server,
                        request.tool,
                        request.arguments,
                        request.meta,
                    )
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_mcp_oauth_login(
    state: State<'_, AppState>,
    request: CodexMcpOAuthLoginRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.mcp_oauth_login(request.server).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_skills_list(
    state: State<'_, AppState>,
    request: CodexSkillsListRequest,
) -> std::result::Result<serde_json::Value, String> {
    let cwds = request
        .cwds
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_skills(cwds, request.force_reload).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_skills_config_write(
    state: State<'_, AppState>,
    request: CodexSkillsConfigWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    let path_candidate = PathBuf::from(&request.skill_id);
    let (path, name) = if path_candidate.components().count() > 1 || path_candidate.is_absolute() {
        (Some(path_candidate), None)
    } else {
        (None, Some(request.skill_id))
    };
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_skills_config(path, name, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_plugin_list(
    state: State<'_, AppState>,
    request: CodexPluginListRequest,
) -> std::result::Result<serde_json::Value, String> {
    let cwds = request
        .cwds
        .map(|paths| paths.into_iter().map(PathBuf::from).collect());
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_plugins(cwds).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_plugin_read(
    state: State<'_, AppState>,
    request: CodexPluginIdRequest,
) -> std::result::Result<serde_json::Value, String> {
    let request = parse_codex_plugin_ref(request.plugin_id);
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_plugin(request).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_plugin_install(
    state: State<'_, AppState>,
    request: CodexPluginInstallRequest,
) -> std::result::Result<serde_json::Value, String> {
    let request = parse_codex_plugin_ref(request.source);
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.install_plugin(request).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_plugin_uninstall(
    state: State<'_, AppState>,
    request: CodexPluginIdRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.uninstall_plugin(request.plugin_id).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_marketplace_add(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.add_marketplace(request.source).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_marketplace_remove(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.remove_marketplace(request.source).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_marketplace_upgrade(
    state: State<'_, AppState>,
    request: CodexMarketplaceRequest,
) -> std::result::Result<serde_json::Value, String> {
    let marketplace_name = trimmed_option(Some(request.source));
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.upgrade_marketplace(marketplace_name).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_review_start(
    state: State<'_, AppState>,
    request: CodexReviewStartRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .start_review(request.thread_id, request.prompt)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_read_config(
    state: State<'_, AppState>,
    include_layers: Option<bool>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_config(include_layers.unwrap_or(false)).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_config_requirements_read(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.read_config_requirements().await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_external_agent_config_detect(
    state: State<'_, AppState>,
    request: CodexExternalAgentConfigDetectRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = ExternalAgentConfigDetectParams {
        include_home: request.include_home,
        cwds: request
            .cwds
            .map(|paths| paths.into_iter().map(PathBuf::from).collect()),
    };
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.detect_external_agent_config(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_external_agent_config_import(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ExternalAgentConfigImportParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.import_external_agent_config(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_write_config_value(
    state: State<'_, AppState>,
    request: CodexConfigValueWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_config_value(ConfigValueWriteParams {
                        key_path: request.key_path,
                        value: request.value,
                        merge_strategy: parse_codex_merge_strategy(
                            request.merge_strategy.as_deref(),
                        ),
                        file_path: request.file_path,
                        expected_version: request.expected_version,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_write_config_batch(
    state: State<'_, AppState>,
    request: CodexConfigBatchWriteRequest,
) -> std::result::Result<serde_json::Value, String> {
    let edits = request
        .edits
        .into_iter()
        .map(|edit| ConfigEdit {
            key_path: edit.key_path,
            value: edit.value,
            merge_strategy: parse_codex_merge_strategy(edit.merge_strategy.as_deref()),
        })
        .collect();
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .write_config_batch(ConfigBatchWriteParams {
                        edits,
                        file_path: request.file_path,
                        expected_version: request.expected_version,
                        reload_user_config: request.reload_user_config,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_upload_feedback(
    state: State<'_, AppState>,
    request: CodexFeedbackRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.upload_feedback(request).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_set_thread_memory_mode(
    state: State<'_, AppState>,
    request: CodexMemoryModeRequest,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(
                adapter
                    .set_thread_memory_mode(request.thread_id, request.enabled)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_reset_memories(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, None, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.reset_memories().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_realtime_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeStartParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_realtime(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_realtime_append_audio(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeAppendAudioParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.append_realtime_audio(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_realtime_append_text(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeAppendTextParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.append_realtime_text(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_realtime_stop(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<ThreadRealtimeStopParams>(request.params).map_err(
        |error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        },
    )?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.stop_realtime(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_realtime_voices_list(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    with_codex_adapter_value(&state, session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.list_realtime_voices().await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

// Device key commands removed — upstream removed DeviceKey APIs

#[tauri::command]
pub(super) async fn codex_fs_read_file(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsReadFileParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_read_file(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_write_file(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsWriteFileParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_write_file(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_create_directory(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsCreateDirectoryParams>(request.params).map_err(
        |error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        },
    )?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_create_directory(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_get_metadata(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsGetMetadataParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_get_metadata(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_read_directory(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsReadDirectoryParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_read_directory(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_remove(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsRemoveParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_remove(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_copy(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FsCopyParams>(request.params).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_copy(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_watch(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsWatchParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_watch(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fs_unwatch(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FsUnwatchParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fs_unwatch(params).await?).map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fuzzy_file_search(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params =
        decode_required_codex_params::<FuzzyFileSearchParams>(request.params).map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.fuzzy_file_search(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fuzzy_file_search_session_start(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionStartParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.start_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fuzzy_file_search_session_update(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionUpdateParams>(request.params)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.update_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_fuzzy_file_search_session_stop(
    state: State<'_, AppState>,
    request: CodexNativeParamsRequest,
) -> std::result::Result<serde_json::Value, String> {
    let params = decode_required_codex_params::<FuzzyFileSearchSessionStopParams>(request.params)
        .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    with_codex_adapter_value(&state, request.session_id, |adapter| {
        Box::pin(async move {
            serde_json::to_value(adapter.stop_fuzzy_file_search_session(params).await?)
                .map_err(anyhow::Error::from)
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn codex_adapter_stop(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<(), String> {
    let key = codex_adapter_key(session_id);
    let mut adapters = state.active_codex_adapters.lock().await;
    if let Some(mut adapter) = adapters.remove(&key) {
        adapter.stop().await.map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    }
    Ok(())
}

#[tauri::command]
pub(super) async fn codex_adapter_restart(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> std::result::Result<(), String> {
    let key = codex_adapter_key(session_id);
    {
        let mut adapters = state.active_codex_adapters.lock().await;
        if let Some(mut adapter) = adapters.remove(&key) {
            let _ = adapter.stop().await;
        }
    }
    ensure_codex_adapter(&state, &key).await
}
