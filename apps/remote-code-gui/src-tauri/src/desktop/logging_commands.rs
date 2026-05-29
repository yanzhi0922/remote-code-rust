use super::*;

#[tauri::command]
pub(super) async fn record_frontend_log(
    event: FrontendLogEventDto,
) -> std::result::Result<(), String> {
    let source = crate::logging::sanitize_log_field(&event.source);
    let message = crate::logging::sanitize_log_field(&event.message);
    let details = event
        .details
        .as_deref()
        .map(crate::logging::sanitize_log_field);
    let stack = event
        .stack
        .as_deref()
        .map(crate::logging::sanitize_log_field);
    let url = event.url.as_deref().map(crate::logging::sanitize_log_field);
    let user_agent = event
        .user_agent
        .as_deref()
        .map(crate::logging::sanitize_log_field);

    match event.level.trim().to_ascii_lowercase().as_str() {
        "debug" => tracing::debug!(
            target: "remote_code_gui::frontend",
            source = %source,
            details = ?details,
            stack = ?stack,
            url = ?url,
            line = ?event.line,
            column = ?event.column,
            user_agent = ?user_agent,
            "frontend: {message}"
        ),
        "warn" | "warning" => tracing::warn!(
            target: "remote_code_gui::frontend",
            source = %source,
            details = ?details,
            stack = ?stack,
            url = ?url,
            line = ?event.line,
            column = ?event.column,
            user_agent = ?user_agent,
            "frontend warning: {message}"
        ),
        "error" => tracing::error!(
            target: "remote_code_gui::frontend",
            source = %source,
            details = ?details,
            stack = ?stack,
            url = ?url,
            line = ?event.line,
            column = ?event.column,
            user_agent = ?user_agent,
            "frontend error: {message}"
        ),
        _ => tracing::info!(
            target: "remote_code_gui::frontend",
            source = %source,
            details = ?details,
            stack = ?stack,
            url = ?url,
            line = ?event.line,
            column = ?event.column,
            user_agent = ?user_agent,
            "frontend: {message}"
        ),
    }

    Ok(())
}

#[tauri::command]
pub(super) async fn export_diagnostic_bundle(
    state: State<'_, AppState>,
    request: DiagnosticBundleRequestDto,
) -> std::result::Result<DiagnosticBundleResultDto, String> {
    let runtime = state.runtime.lock().await;
    let layout = crate::paths::RuntimePathLayout::from_app_paths(&runtime.config.paths);
    crate::logging::create_diagnostic_bundle(
        &layout,
        crate::logging::DiagnosticBundleOptions {
            include_logs: request.include_logs,
            include_settings: request.include_settings,
        },
    )
    .map_err(|error| {
        let msg = format!("{error:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}
