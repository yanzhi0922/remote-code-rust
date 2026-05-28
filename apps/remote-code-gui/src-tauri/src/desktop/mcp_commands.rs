use super::*;

#[tauri::command]
pub(super) async fn list_mcp_servers(
    state: State<'_, AppState>,
    scope: ConfigScopeDto,
    project_path: Option<String>,
    connect: bool,
    include_disabled: bool,
) -> std::result::Result<McpServerListDto, String> {
    let runtime = state.runtime.lock().await;
    build_mcp_server_list(
        &runtime.config,
        scope,
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
