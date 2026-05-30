/// Minimal mobile entry point — avoids pulling in desktop-only crates
/// (codex, deno, v8, claude-core, claude-provider, etc.) which don't
/// compile for Android targets.

#[cfg_attr(
    any(target_os = "android", target_os = "ios"),
    tauri::mobile_entry_point
)]
pub fn run() {
    // Initialize tracing for mobile.
    // On Android, stderr is captured by logcat automatically.
    // Filter to info+ by default; set RUST_LOG env to override.
    {
        use tracing_subscriber::EnvFilter;
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("remote_code_gui=info,wry=info,tao=info"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .try_init();
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("Unhandled panic in GUI (mobile): {info}");
        default_hook(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            crate::mobile::register_mobile_plugins(app.handle());
            Ok(())
        })
        .manage(crate::quic_bridge::QuicBridgeState::new())
        .invoke_handler(tauri::generate_handler![
            crate::mobile::init_app,
            crate::mobile::mobile_is_mobile,
            crate::mobile::mobile_haptic_notification,
            crate::mobile::mobile_biometric_check_availability,
            crate::mobile::mobile_biometric_authenticate,
            crate::mobile::mobile_secure_store_get,
            crate::mobile::mobile_secure_store_set,
            crate::mobile::mobile_secure_store_remove,
            crate::mobile::mobile_download_artifact,
            crate::mobile::mobile_share_file,
            crate::mobile::mobile_push_request_permission,
            crate::mobile::mobile_push_show,
            crate::mobile::mobile_push_get_token,
            crate::mobile::mobile_push_register_token,
            crate::mobile::mobile_check_file_downloaded,
            crate::mobile::mobile_read_downloaded_text,
            crate::mobile::mobile_delete_downloaded_file,
            crate::mobile::mobile_list_downloaded_files,
            crate::mobile::mobile_quic_status,
            crate::mobile::record_frontend_log,
            crate::mobile::mobile_connection_strategy,
            crate::quic_bridge::quic_connect,
            crate::quic_bridge::quic_send_command,
            crate::quic_bridge::quic_disconnect,
            crate::quic_bridge::quic_state,
            crate::quic_bridge::quic_health_probe,
            crate::quic_bridge::quic_get_metrics,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while running tauri application: {error}"));
}
