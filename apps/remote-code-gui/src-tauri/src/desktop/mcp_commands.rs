use super::*;

#[tauri::command]
pub(super) async fn list_mcp_servers(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    connect: bool,
    include_disabled: bool,
    include_secrets: bool,
) -> std::result::Result<McpServerListDto, String> {
    let runtime = state.runtime.lock().await;
    build_mcp_server_list(
        &runtime.config,
        scope,
        project_path.as_deref(),
        &runtime.projects,
        connect,
        include_disabled,
        include_secrets,
    )
    .await
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub(super) async fn list_runtime_mcp_inventory(
    state: State<'_, AppState>,
    project_path: Option<String>,
    connect: bool,
    include_disabled: bool,
) -> std::result::Result<RuntimeMcpInventoryDto, String> {
    let runtime = state.runtime.lock().await;
    build_runtime_mcp_inventory(
        &runtime.config,
        project_path.as_deref(),
        &runtime.projects,
        connect,
        include_disabled,
    )
    .await
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub(super) async fn save_mcp_server(
    state: State<'_, AppState>,
    request: McpServerUpsertRequestDto,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        request.scope,
        request.project_path.as_deref(),
        &runtime.projects,
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    save_managed_mcp_server_at_path(&config_path, request.scope, &request).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub(super) async fn toggle_mcp_server(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    name: String,
    enabled: bool,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        scope,
        project_path.as_deref(),
        &runtime.projects,
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    toggle_managed_mcp_server_at_path(&config_path, scope, &name, enabled, if_exists).map_err(
        |error| {
            let msg = format!("{error:#}");
            tracing::warn!(error = %msg, "command error");
            msg
        },
    )
}

#[tauri::command]
pub(super) async fn remove_mcp_server(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    name: String,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        scope,
        project_path.as_deref(),
        &runtime.projects,
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    remove_managed_mcp_server_at_path(&config_path, scope, &name, if_exists).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub(super) async fn reset_mcp_servers(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    if_exists: bool,
) -> std::result::Result<McpMutationResultDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        scope,
        project_path.as_deref(),
        &runtime.projects,
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    reset_managed_mcp_config_at_path(&config_path, scope, if_exists).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

/// Lightweight health probe for a managed MCP server. For HTTP-family
/// transports we issue a GET to the URL with a short timeout; for stdio
/// we just confirm the command binary resolves on PATH. The classification
/// mirrors the doctor probe logic so the UI can render the same status
/// colors.
#[tauri::command]
pub(super) async fn probe_mcp_server(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    name: String,
) -> std::result::Result<GuiMcpProbeDto, String> {
    let runtime = state.runtime.lock().await;
    let config_path = mcp_config_path_for_scope(
        &runtime.config,
        scope,
        project_path.as_deref(),
        &runtime.projects,
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    let mcp_config = load_managed_mcp_config_or_default(&config_path).map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    let server = mcp_config
        .servers
        .get(&name)
        .ok_or_else(|| format!("unknown MCP server: {name}"))?
        .clone();
    // Drop the lock before doing network I/O.
    drop(runtime);

    let started = Instant::now();
    let probe = run_mcp_probe(&server).await;
    let url = mcp_server_url(&server);
    let transport = mcp_transport_to_display(server.transport.kind());
    let name = server.name;
    Ok(GuiMcpProbeDto {
        name,
        transport,
        url,
        outcome: probe.outcome,
        status_code: probe.status_code,
        latency_ms: started.elapsed().as_millis(),
        detail: probe.detail,
    })
}

/// Trigger OAuth login for an MCP server. Emits a Tauri event that the
/// Codex MCP subsystem already listens for; this is a thin shim so the
/// GUI doesn't have to import Codex crates directly.
#[tauri::command]
pub(super) async fn oauth_login_mcp_server(
    app: AppHandle,
    session_id: Option<String>,
    server: String,
) -> std::result::Result<(), String> {
    let params = serde_json::json!({
        "server": server,
        "sessionId": session_id,
    });
    app.emit("codex_mcp_oauth_login", params).map_err(|e| {
        let msg = format!("failed to emit oauth login event: {e}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}
