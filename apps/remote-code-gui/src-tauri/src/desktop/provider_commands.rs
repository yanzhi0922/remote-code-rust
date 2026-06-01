use super::*;

#[tauri::command]
pub(super) async fn get_provider_info(
    state: State<'_, AppState>,
) -> std::result::Result<Option<ProviderInfoDto>, String> {
    let runtime = state.runtime.lock().await;
    Ok(Some(provider_info_from_runtime(&runtime.config.provider)))
}

#[tauri::command]
pub(super) async fn get_runtime_status(
    state: State<'_, AppState>,
) -> std::result::Result<UiRuntimeStatusSnapshot, String> {
    let runtime = state.runtime.lock().await;
    Ok(runtime_status_snapshot_from_config(&runtime.config))
}

#[tauri::command]
pub(super) async fn run_doctor_report(
    state: State<'_, AppState>,
    probe_network: bool,
    probe_provider: bool,
    probe_mcp: bool,
    include_env_providers: bool,
) -> std::result::Result<GuiDoctorReportDto, String> {
    let runtime = state.runtime.lock().await;
    build_gui_doctor_report(
        &runtime.config,
        probe_network,
        probe_provider,
        probe_mcp,
        include_env_providers,
    )
    .await
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub(super) async fn get_settings(
    state: State<'_, AppState>,
) -> std::result::Result<FullSettingsDto, String> {
    let runtime = state.runtime.lock().await;
    Ok(full_settings_from_runtime(
        &runtime.config,
        &runtime.gui_settings,
    ))
}

#[tauri::command]
pub(super) async fn update_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpdateProviderRequest,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;

    if let Some(provider_name) = request.provider_name.or(request.name) {
        let provider_name = provider_name.trim().to_owned();
        if let Some(index) = find_provider_config_index(&runtime.provider_configs, &provider_name) {
            let stored = runtime.provider_configs.providers[index].clone();
            runtime.config.provider = provider_config_to_runtime(&stored, &runtime.config.provider)
                .map_err(|error| {
                    let msg = format!("{error:#}");
                    tracing::warn!(error = %msg, "command error");
                    msg
                })?;
            runtime.provider_configs.active_provider = Some(provider_name.clone());
        } else if !provider_name.is_empty() {
            runtime.config.provider.name = provider_name.clone();
        }
        runtime.gui_settings.provider_name = Some(runtime.config.provider.name.clone());
    }

    if let Some(model) = request.provider_model.or(request.model) {
        let model = model.trim().to_owned();
        runtime.config.provider.model = if model.is_empty() {
            None
        } else {
            Some(model.clone())
        };
        runtime.gui_settings.provider_model = runtime.config.provider.model.clone();
    }

    if let Some(protocol) = parse_protocol(
        request
            .provider_protocol
            .as_deref()
            .or(request.protocol.as_deref()),
    ) {
        runtime.config.provider.protocol = protocol;
        runtime.config.provider.base_url =
            normalize_base_url(runtime.config.provider.base_url.clone(), protocol);
        runtime.gui_settings.provider_protocol = Some(protocol.as_str().to_owned());
    }

    if let Some(base_url) = request.provider_base_url.or(request.base_url) {
        runtime.config.provider.base_url =
            normalize_base_url(Some(base_url.clone()), runtime.config.provider.protocol);
        runtime.gui_settings.provider_base_url = runtime.config.provider.base_url.clone();
    }

    if let Some(api_key) = request.api_key {
        runtime.config.provider.api_key = trimmed_option(Some(api_key));
    }
    if let Some(max_output_tokens) = request.max_output_tokens {
        runtime.config.provider.max_output_tokens = max_output_tokens.max(256);
        runtime.gui_settings.max_output_tokens = Some(runtime.config.provider.max_output_tokens);
    }
    if let Some(thinking_budget) = request.thinking_budget {
        runtime.config.provider.thinking_budget = thinking_budget;
        runtime.gui_settings.thinking_budget = Some(thinking_budget);
    }
    if let Some(max_retries) = request.max_retries {
        runtime.config.provider.max_retries = max_retries;
        runtime.gui_settings.max_retries = Some(max_retries);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        runtime.config.provider.timeout_ms = timeout_ms.max(1_000);
        runtime.gui_settings.timeout_ms = Some(runtime.config.provider.timeout_ms);
    }
    if let Some(backoff_ms) = request.retry_initial_backoff_ms {
        runtime.config.provider.retry_initial_backoff_ms = backoff_ms.max(50);
        runtime.gui_settings.retry_initial_backoff_ms =
            Some(runtime.config.provider.retry_initial_backoff_ms);
    }
    if let Some(backoff_ms) = request.retry_max_backoff_ms {
        runtime.config.provider.retry_max_backoff_ms =
            backoff_ms.max(runtime.config.provider.retry_initial_backoff_ms);
        runtime.gui_settings.retry_max_backoff_ms =
            Some(runtime.config.provider.retry_max_backoff_ms);
    }
    if let Some(respect_retry_after) = request.respect_retry_after {
        runtime.config.provider.respect_retry_after = respect_retry_after;
        runtime.gui_settings.respect_retry_after = Some(respect_retry_after);
    }
    if let Some(permission_mode) = parse_permission_mode(request.permission_mode.as_deref()) {
        runtime.config.permission_mode = permission_mode;
        runtime.gui_settings.permission_mode = Some(permission_mode.as_legacy_str().to_owned());
    }
    if let Some(verbose) = request.verbose {
        runtime.config.verbose = verbose;
        runtime.gui_settings.verbose = Some(verbose);
    }
    if request.codex_model_provider.is_some() {
        runtime.gui_settings.codex_model_provider = request
            .codex_model_provider
            .and_then(|value| trimmed_option(Some(value)));
    }
    if request.codex_approval_policy.is_some() {
        runtime.gui_settings.codex_approval_policy = request
            .codex_approval_policy
            .and_then(|value| trimmed_option(Some(value)));
    }
    if request.codex_sandbox_mode.is_some() {
        runtime.gui_settings.codex_sandbox_mode = request
            .codex_sandbox_mode
            .and_then(|value| trimmed_option(Some(value)));
    }
    if let Some(value) = request.codex_persist_extended_history {
        runtime.gui_settings.codex_persist_extended_history = Some(value);
    }
    if let Some(value) = request.codex_memories_enabled {
        runtime.gui_settings.codex_memories_enabled = Some(value);
    }
    if request.codex_thread_store_endpoint.is_some() {
        runtime.gui_settings.codex_thread_store_endpoint = request
            .codex_thread_store_endpoint
            .and_then(|value| trimmed_option(Some(value)));
    }
    if let Some(overrides) = request.codex_config_overrides {
        runtime.gui_settings.codex_config_overrides = overrides;
    }
    if let Some(profile) = request.codex_permission_profile {
        runtime.gui_settings.codex_permission_profile = Some(profile);
    }
    if let Some(tier) = request.codex_service_tier {
        runtime.gui_settings.codex_service_tier = trimmed_option(Some(tier));
    }
    if let Some(ephemeral) = request.codex_ephemeral {
        runtime.gui_settings.codex_ephemeral = Some(ephemeral);
    }

    if let Some(thinking_budget) = runtime.config.provider.thinking_budget
        && thinking_budget >= runtime.config.provider.max_output_tokens
    {
        return Err("thinking budget must be lower than max output tokens".to_owned());
    }

    let selected_provider = runtime.config.provider.clone();
    store_provider_selection(&mut runtime, &selected_provider);
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn list_provider_configs(
    state: State<'_, AppState>,
) -> std::result::Result<ProviderConfigList, String> {
    let runtime = state.runtime.lock().await;
    let mut result = runtime.provider_configs.clone();
    // For each provider, set api_key_stored and mask api_key.
    for provider in &mut result.providers {
        let in_keychain = keyring_retrieve(&provider.name).is_some();
        let in_json = provider.api_key.is_some();
        provider.api_key_stored = in_keychain || in_json;
        // Never expose API keys to the frontend — mask to None.
        provider.api_key = None;
    }
    Ok(result)
}

#[tauri::command]
pub(super) async fn save_provider_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ProviderConfig,
    set_active: bool,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let mut config = normalize_provider_config(config).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;

    // Store API key in OS keychain if provided; clear from JSON payload.
    if let Some(ref api_key) = config.api_key {
        keyring_store(&config.name, api_key);
        config.api_key = None;
    }
    // If api_key was None (frontend didn't change it), keep existing keychain entry.

    let index = find_provider_config_index(&runtime.provider_configs, &config.name);
    if let Some(index) = index {
        runtime.provider_configs.providers[index] = config.clone();
    } else {
        runtime.provider_configs.providers.push(config.clone());
        runtime
            .provider_configs
            .providers
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    }

    if set_active || runtime.provider_configs.active_provider.is_none() {
        runtime.provider_configs.active_provider = Some(config.name.clone());
        runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
        let provider_configs_snapshot = runtime.provider_configs.clone();
        apply_provider_credentials_from_configs(
            &mut runtime.config.provider,
            &provider_configs_snapshot,
        );
        let selected_provider = runtime.config.provider.clone();
        store_provider_selection(&mut runtime, &selected_provider);
    }

    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn delete_provider_config(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;

    // Verify the provider exists before deleting the keychain entry.
    let exists = runtime
        .provider_configs
        .providers
        .iter()
        .any(|provider| provider.name == name);
    if !exists {
        return Err(format!("unknown provider config: {name}"));
    }

    // Built-in providers can be disabled but not deleted.
    let is_builtin = runtime
        .provider_configs
        .providers
        .iter()
        .any(|provider| provider.name == name && matches!(provider.group, ProviderGroup::Builtin));
    if is_builtin {
        return Err(format!(
            "built-in provider {name} cannot be deleted; disable it instead"
        ));
    }

    // Remove API key from OS keychain only after confirming the provider exists.
    keyring_delete(&name);

    let removed_active = runtime.provider_configs.active_provider.as_deref() == Some(name.as_str());
    runtime
        .provider_configs
        .providers
        .retain(|provider| provider.name != name);

    if removed_active {
        runtime.provider_configs.active_provider = runtime
            .provider_configs
            .providers
            .first()
            .map(|provider| provider.name.clone());
        if let Some(active) = active_provider_config(&runtime.provider_configs).cloned() {
            runtime.config.provider = provider_config_to_runtime(&active, &runtime.config.provider)
                .map_err(|error| {
                    let msg = format!("{error:#}");
                    tracing::warn!(error = %msg, "command error");
                    msg
                })?;
            let selected_provider = runtime.config.provider.clone();
            store_provider_selection(&mut runtime, &selected_provider);
        } else {
            runtime.gui_settings.provider_name = None;
            runtime.gui_settings.provider_model = None;
            runtime.gui_settings.provider_base_url = None;
            runtime.gui_settings.provider_protocol = None;
            let mut fresh =
                load_base_runtime_config(profile_override_from_env()).map_err(|error| {
                    let msg = format!("{error:#}");
                    tracing::warn!(error = %msg, "command error");
                    msg
                })?;
            apply_gui_settings_to_runtime(&mut fresh, &runtime.gui_settings).map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
            runtime.config = fresh;
        }
    }

    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn set_active_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    let config = runtime.provider_configs.providers[index].clone();
    runtime.provider_configs.active_provider = Some(config.name.clone());
    runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
        .map_err(|error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    let provider_configs_snapshot = runtime.provider_configs.clone();
    apply_provider_credentials_from_configs(
        &mut runtime.config.provider,
        &provider_configs_snapshot,
    );
    let selected_provider = runtime.config.provider.clone();
    store_provider_selection(&mut runtime, &selected_provider);
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn switch_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_name: String,
    profile_name: Option<String>,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &provider_name)
        .ok_or_else(|| format!("unknown provider config: {provider_name}"))?;

    // Validate profile exists if specified.
    if let Some(ref pname) = profile_name {
        let config = &runtime.provider_configs.providers[index];
        if !config.profiles.iter().any(|p| p.name == *pname) {
            return Err(format!("unknown profile: {pname}"));
        }
    }

    runtime.provider_configs.providers[index].active_profile = profile_name;
    let config = runtime.provider_configs.providers[index].clone();

    // If this is the active provider, re-apply to runtime.
    if runtime.provider_configs.active_provider.as_deref() == Some(&provider_name) {
        runtime.config.provider = provider_config_to_runtime(&config, &runtime.config.provider)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
        let provider_configs_snapshot = runtime.provider_configs.clone();
        apply_provider_credentials_from_configs(
            &mut runtime.config.provider,
            &provider_configs_snapshot,
        );
        let selected_provider = runtime.config.provider.clone();
        store_provider_selection(&mut runtime, &selected_provider);
    }

    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn set_provider_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    enabled: bool,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    runtime.provider_configs.providers[index].enabled = enabled;

    // If the active provider was disabled, clear the active pointer so the
    // next provider in the list becomes the runtime's current provider.
    if !enabled && runtime.provider_configs.active_provider.as_deref() == Some(name.as_str()) {
        runtime.provider_configs.active_provider = runtime
            .provider_configs
            .providers
            .iter()
            .find(|p| p.enabled)
            .map(|p| p.name.clone());
        if let Some(active) = active_provider_config(&runtime.provider_configs).cloned() {
            runtime.config.provider = provider_config_to_runtime(&active, &runtime.config.provider)
                .map_err(|error| {
                    let msg = format!("{error:#}");
                    tracing::warn!(error = %msg, "command error");
                    msg
                })?;
            let provider_configs_snapshot = runtime.provider_configs.clone();
            apply_provider_credentials_from_configs(
                &mut runtime.config.provider,
                &provider_configs_snapshot,
            );
            let selected_provider = runtime.config.provider.clone();
            store_provider_selection(&mut runtime, &selected_provider);
        }
    } else if enabled && runtime.provider_configs.active_provider.is_none() {
        runtime.provider_configs.active_provider = Some(name.clone());
        let snapshot = runtime.provider_configs.providers[index].clone();
        runtime.config.provider = provider_config_to_runtime(&snapshot, &runtime.config.provider)
            .map_err(|error| {
                let msg = format!("{error:#}");
                tracing::warn!(error = %msg, "command error");
                msg
            })?;
        let provider_configs_snapshot = runtime.provider_configs.clone();
        apply_provider_credentials_from_configs(
            &mut runtime.config.provider,
            &provider_configs_snapshot,
        );
        let selected_provider = runtime.config.provider.clone();
        store_provider_selection(&mut runtime, &selected_provider);
    }

    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn set_claude_model_mapping(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    mapping: ClaudeModelMapping,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    runtime.provider_configs.providers[index].claude_model_mapping = mapping;
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn add_provider_model(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    model: ProviderModel,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    let trimmed_id = model.id.trim();
    if trimmed_id.is_empty() {
        return Err("model id cannot be empty".to_owned());
    }
    let entry = ProviderModel {
        id: trimmed_id.to_owned(),
        display_name: model
            .display_name
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty()),
    };
    let provider = &mut runtime.provider_configs.providers[index];
    if let Some(existing) = provider.models.iter_mut().find(|m| m.id == entry.id) {
        existing.display_name = entry.display_name.or(existing.display_name.clone());
    } else {
        provider.models.push(entry);
    }
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn update_provider_model(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    old_id: String,
    model: ProviderModel,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    let trimmed_id = model.id.trim();
    if trimmed_id.is_empty() {
        return Err("model id cannot be empty".to_owned());
    }
    let entry = ProviderModel {
        id: trimmed_id.to_owned(),
        display_name: model
            .display_name
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty()),
    };
    let provider = &mut runtime.provider_configs.providers[index];
    let target_index = provider
        .models
        .iter()
        .position(|m| m.id == old_id)
        .ok_or_else(|| format!("unknown model id: {old_id}"))?;
    provider.models[target_index] = entry;
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn remove_provider_model(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    model_id: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let index = find_provider_config_index(&runtime.provider_configs, &name)
        .ok_or_else(|| format!("unknown provider config: {name}"))?;
    let provider = &mut runtime.provider_configs.providers[index];
    let before = provider.models.len();
    provider.models.retain(|m| m.id != model_id);
    if provider.models.len() == before {
        return Err(format!("unknown model id: {model_id}"));
    }
    persist_runtime_files(&runtime).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    emit_runtime_status(&app, &runtime.config);
    Ok(())
}

#[tauri::command]
pub(super) async fn refresh_provider_configs(
    state: State<'_, AppState>,
) -> std::result::Result<ProviderConfigList, String> {
    // Same masking logic as list_provider_configs — kept as a separate command
    // so the FE 刷新 button has a verb-shaped action to call.
    let runtime = state.runtime.lock().await;
    let mut result = runtime.provider_configs.clone();
    for provider in &mut result.providers {
        let in_keychain = keyring_retrieve(&provider.name).is_some();
        let in_json = provider.api_key.is_some();
        provider.api_key_stored = in_keychain || in_json;
        provider.api_key = None;
    }
    Ok(result)
}

#[tauri::command]
pub(super) async fn transcribe_audio(
    _app: AppHandle,
    state: State<'_, AppState>,
    audio_data: Vec<u8>,
    audio_format: Option<String>,
) -> std::result::Result<String, String> {
    if audio_data.is_empty() {
        return Err("音频数据为空".to_owned());
    }

    // Obtain the API key from the active provider configuration.
    let api_key = {
        let runtime = state.runtime.lock().await;
        runtime.config.provider.api_key.clone()
    };

    let api_key = api_key.ok_or_else(|| "未配置 API key，无法使用语音转录".to_string())?;

    let stt = claude_voice::WhisperStt::new(api_key);
    let format = audio_format.as_deref().unwrap_or("webm");

    match stt.transcribe(&audio_data, format).await {
        Ok(result) => Ok(result.text),
        Err(e) => {
            tracing::error!("STT transcription failed: {e}");
            Err(format!("语音转录失败: {e}"))
        }
    }
}
