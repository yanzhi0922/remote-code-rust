use rc_remote_transport::RemoteTransport;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager, Runtime};

/// Reject file names that could escape the download directory.
fn validate_download_name(file_name: &str) -> Result<std::path::PathBuf, String> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
    {
        return Err("invalid file name".to_string());
    }
    let path = Path::new(file_name);
    if path.is_absolute() {
        return Err("invalid file name".to_string());
    }
    Ok(path.to_path_buf())
}

/// Reject URLs that target internal/private networks (SSRF mitigation).
fn validate_download_url(url: &str) -> Result<(), String> {
    let lowered = url.to_ascii_lowercase();
    if !lowered.starts_with("https://") && !lowered.starts_with("http://") {
        return Err("only http:// and https:// URLs are allowed".to_string());
    }
    let after_scheme = &url[url.find("://").map(|i| i + 3).unwrap_or(0)..];
    let host_port = after_scheme.split('/').next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("");
    if host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("169.254.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
    {
        return Err("URLs targeting internal addresses are not allowed".to_string());
    }
    if let Some(rest) = host.strip_prefix("172.")
        && let Some(octet) = rest.split('.').next()
        && let Ok(n) = octet.parse::<u8>()
        && (16..=31).contains(&n)
    {
        return Err("URLs targeting internal addresses are not allowed".to_string());
    }
    Ok(())
}

const MAX_DOWNLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiometricAvailability {
    pub available: bool,
    pub biometry_type: String,
}

#[tauri::command]
pub async fn mobile_is_mobile() -> bool {
    cfg!(target_os = "ios") || cfg!(target_os = "android")
}

#[tauri::command]
pub async fn mobile_haptic_notification(
    app: AppHandle<impl Runtime>,
    kind: String,
) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_haptics::HapticsExt;
        let notif_type = match kind.as_str() {
            "success" => tauri_plugin_haptics::NotificationFeedbackType::Success,
            "warning" => tauri_plugin_haptics::NotificationFeedbackType::Warning,
            "error" => tauri_plugin_haptics::NotificationFeedbackType::Error,
            _ => tauri_plugin_haptics::NotificationFeedbackType::Success,
        };
        app.haptics()
            .notification_feedback(notif_type)
            .map_err(|e| e.to_string())?;
    }
    let _ = kind;
    let _ = &app;
    Ok(())
}

#[tauri::command]
pub async fn mobile_biometric_check_availability(
    app: AppHandle<impl Runtime>,
) -> Result<BiometricAvailability, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::BiometricExt;
        let status = app.biometric().status().map_err(|e| e.to_string())?;
        let biometry_type = match status.biometry_type {
            tauri_plugin_biometric::BiometryType::TouchID => "touchid",
            tauri_plugin_biometric::BiometryType::FaceID => "faceid",
            _ => "none",
        };
        Ok(BiometricAvailability {
            available: status.is_available,
            biometry_type: biometry_type.to_string(),
        })
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app;
        Ok(BiometricAvailability {
            available: false,
            biometry_type: "unsupported".to_string(),
        })
    }
}

#[tauri::command]
pub async fn mobile_biometric_authenticate(
    app: AppHandle<impl Runtime>,
    reason: String,
) -> Result<bool, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::BiometricExt;
        let options = tauri_plugin_biometric::AuthOptions {
            title: Some(reason.clone()),
            ..Default::default()
        };
        app.biometric()
            .authenticate(reason, options)
            .map(|_| true)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = (app, reason);
        Ok(false)
    }
}

#[tauri::command]
pub async fn mobile_secure_store_get(
    app: AppHandle<impl Runtime>,
    key: String,
) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => {
            if let Ok(dir) = app.path().app_config_dir() {
                let legacy_path = dir.join("secure_store.json");
                if legacy_path.exists() {
                    if let Ok(data) = std::fs::read_to_string(&legacy_path) {
                        if let Ok(map) =
                            serde_json::from_str::<std::collections::HashMap<String, String>>(&data)
                        {
                            if let Some(value) = map.get(&key).cloned() {
                                if entry.set_password(&value).is_ok() {
                                    tracing::info!(
                                        "Migrated key '{key}' from legacy JSON to keyring"
                                    );
                                }
                                return Ok(Some(value));
                            }
                        }
                    }
                }
            }
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn mobile_secure_store_set(key: String, value: String) -> Result<(), String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key).map_err(|e| e.to_string())?;
    entry.set_password(&value).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mobile_secure_store_remove(key: String) -> Result<(), String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn mobile_download_artifact(
    app: AppHandle<impl Runtime>,
    url: String,
    file_name: String,
    token: Option<String>,
) -> Result<String, String> {
    validate_download_url(&url)?;
    validate_download_name(&file_name)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.get(&url);
    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {t}"));
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_DOWNLOAD_SIZE {
        return Err(format!(
            "download too large ({} bytes, max {} bytes)",
            bytes.len(),
            MAX_DOWNLOAD_SIZE
        ));
    }
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    std::fs::create_dir_all(&download_dir).map_err(|e| e.to_string())?;
    let file_path = download_dir.join(&file_name);
    std::fs::write(&file_path, &bytes).map_err(|e| e.to_string())?;
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn mobile_check_file_downloaded(
    app: AppHandle<impl Runtime>,
    file_name: String,
) -> Result<bool, String> {
    validate_download_name(&file_name)?;
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    let file_path = download_dir.join(&file_name);
    Ok(file_path.exists())
}

#[tauri::command]
pub async fn mobile_read_downloaded_text(
    app: AppHandle<impl Runtime>,
    file_name: String,
) -> Result<Option<String>, String> {
    validate_download_name(&file_name)?;
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    let file_path = download_dir.join(&file_name);
    if !file_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    Ok(Some(content))
}

#[tauri::command]
pub async fn mobile_delete_downloaded_file(
    app: AppHandle<impl Runtime>,
    file_name: String,
) -> Result<(), String> {
    validate_download_name(&file_name)?;
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    let file_path = download_dir.join(&file_name);
    if file_path.exists() {
        std::fs::remove_file(&file_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn mobile_list_downloaded_files(
    app: AppHandle<impl Runtime>,
) -> Result<Vec<String>, String> {
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    if !download_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&download_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
    }
    Ok(files)
}

#[tauri::command]
pub async fn mobile_share_file(
    app: AppHandle<impl Runtime>,
    file_path: String,
    _title: Option<String>,
) -> Result<(), String> {
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    let canonical_path =
        std::fs::canonicalize(&file_path).map_err(|e| format!("file not found: {e}"))?;
    let canonical_dir =
        std::fs::canonicalize(&download_dir).unwrap_or_else(|_| download_dir.clone());
    if !canonical_path.starts_with(&canonical_dir) {
        return Err("file is outside the allowed download directory".to_string());
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_share::ShareExt;
        let mime = mime_guess::from_path(&file_path)
            .first()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let request = tauri_plugin_share::ShareRequest {
            path: Some(file_path.clone()),
            mime: Some(mime),
            group: _title.clone(),
        };
        app.share_file()
            .share_file(request)
            .map_err(|e: tauri_plugin_share::Error| e.to_string())?;
    }
    let _ = (app, file_path, _title);
    Ok(())
}

#[tauri::command]
pub async fn mobile_push_request_permission(app: AppHandle<impl Runtime>) -> Result<bool, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_notification::NotificationExt;
        let permission = app
            .notification()
            .permission_state()
            .map_err(|e| e.to_string())?;
        if permission == tauri_plugin_notification::PermissionState::Denied {
            return Ok(false);
        }
        if permission == tauri_plugin_notification::PermissionState::Granted {
            return Ok(true);
        }
        let result = app
            .notification()
            .request_permission()
            .map_err(|e| e.to_string())?;
        Ok(result == tauri_plugin_notification::PermissionState::Granted)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app;
        Ok(false)
    }
}

#[tauri::command]
pub async fn mobile_push_show(
    app: AppHandle<impl Runtime>,
    title: String,
    body: String,
    data: Option<String>,
) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_notification::NotificationExt;
        let mut builder = app.notification().builder().title(&title).body(&body);
        if let Some(ref d) = data {
            builder = builder.extra("data", d);
        }
        builder.show().map_err(|e| e.to_string())?;
    }
    let _ = (app, title, body, data);
    Ok(())
}

#[tauri::command]
pub async fn mobile_push_get_token(app: AppHandle<impl Runtime>) -> Result<Option<String>, String> {
    // Push token retrieval requires platform-specific FCM/APNs integration.
    // Return None for now — will be implemented with a dedicated push plugin.
    let _ = app;
    Ok(None)
}

#[tauri::command]
pub async fn mobile_push_register_token(
    _app: AppHandle<impl Runtime>,
    base_url: String,
    access_token: String,
    push_token: Option<String>,
) -> Result<bool, String> {
    let Some(push_token) = push_token
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };
    let platform = mobile_push_platform();
    let url = format!("{}/v1/devices/push-token", base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "push_token": push_token,
            "platform": platform,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status().is_success() {
        Ok(true)
    } else {
        Err(format!(
            "push token registration failed: HTTP {}",
            response.status()
        ))
    }
}

fn mobile_push_platform() -> &'static str {
    #[cfg(target_os = "ios")]
    {
        "apns"
    }
    #[cfg(not(target_os = "ios"))]
    {
        "fcm"
    }
}

/// Get the current QUIC connection state (idle, connecting, open, etc.).
#[tauri::command]
#[allow(dead_code)]
pub async fn mobile_quic_status(
    state: tauri::State<'_, crate::quic_bridge::QuicBridgeState>,
) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    match guard.as_ref() {
        Some(bridge) => {
            let s = bridge.transport.state();
            serde_json::to_string(&s).map_err(|e| format!("{e}"))
        }
        None => Ok("\"disconnected\"".to_owned()),
    }
}

/// Get the current connection strategy (direct_ws, server_relay, quic, etc.).
#[tauri::command]
#[allow(dead_code)]
pub async fn mobile_connection_strategy(
    state: tauri::State<'_, crate::quic_bridge::QuicBridgeState>,
) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    match guard.as_ref() {
        Some(bridge) => Ok(bridge.transport.active_strategy().to_owned()),
        None => Ok("none".to_owned()),
    }
}

pub fn register_mobile_plugins<R: Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        for (name, result) in [
            ("haptics", app.plugin(tauri_plugin_haptics::init())),
            ("biometric", app.plugin(tauri_plugin_biometric::init())),
            ("deep_link", app.plugin(tauri_plugin_deep_link::init())),
            (
                "notification",
                app.plugin(tauri_plugin_notification::init()),
            ),
            ("share", app.plugin(tauri_plugin_share::init())),
        ] {
            if let Err(e) = result {
                tracing::warn!(plugin = name, "Failed to initialize mobile plugin: {e}");
            }
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app;
    }
}
