use tauri::{AppHandle, Manager, Runtime};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    // Basic scheme check
    let lowered = url.to_ascii_lowercase();
    if !lowered.starts_with("https://") && !lowered.starts_with("http://") {
        return Err("only http:// and https:// URLs are allowed".to_string());
    }
    // Extract host part: after "://" up to first '/' or end
    let after_scheme = &url[url.find("://").map(|i| i + 3).unwrap_or(0)..];
    let host_port = after_scheme.split('/').next().unwrap_or("");
    // Strip port
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
    // 172.16.0.0/12 private range
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
pub async fn mobile_platform() -> String {
    if cfg!(target_os = "ios") {
        "ios".to_string()
    } else if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}

#[tauri::command]
pub async fn mobile_is_mobile() -> bool {
    cfg!(target_os = "ios") || cfg!(target_os = "android")
}

#[tauri::command]
pub async fn mobile_haptic_impact(style: String) -> Result<(), String> {
    #[cfg(feature = "mobile")]
    {
        let impact_style = match style.as_str() {
            "heavy" => tauri_plugin_haptics::ImpactStyle::Heavy,
            "medium" => tauri_plugin_haptics::ImpactStyle::Medium,
            _ => tauri_plugin_haptics::ImpactStyle::Light,
        };
        tauri_plugin_haptics::impact(impact_style);
    }
    let _ = style;
    Ok(())
}

#[tauri::command]
pub async fn mobile_haptic_notification(kind: String) -> Result<(), String> {
    #[cfg(feature = "mobile")]
    {
        let notif_type = match kind.as_str() {
            "success" => tauri_plugin_haptics::NotificationType::Success,
            "warning" => tauri_plugin_haptics::NotificationType::Warning,
            "error" => tauri_plugin_haptics::NotificationType::Error,
            _ => tauri_plugin_haptics::NotificationType::Success,
        };
        tauri_plugin_haptics::notification(notif_type);
    }
    let _ = kind;
    Ok(())
}

#[tauri::command]
pub async fn mobile_haptic_selection() -> Result<(), String> {
    #[cfg(feature = "mobile")]
    {
        tauri_plugin_haptics::selection();
    }
    Ok(())
}

#[tauri::command]
pub async fn mobile_biometric_check_availability(
    app: AppHandle<impl Runtime>,
) -> Result<BiometricAvailability, String> {
    #[cfg(feature = "mobile")]
    {
        let result = tauri_plugin_biometric::check_availability(&app, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(BiometricAvailability {
            available: true,
            biometry_type: result,
        })
    }
    #[cfg(not(feature = "mobile"))]
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
    #[cfg(feature = "mobile")]
    {
        let options = tauri_plugin_biometric::BiometricOptions {
            reason,
            ..Default::default()
        };
        tauri_plugin_biometric::authenticate(&app, options)
            .await
            .map(|_| true)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = (app, reason);
        Ok(false)
    }
}

#[tauri::command]
pub async fn mobile_secure_store_get(
    _app: AppHandle<impl Runtime>,
    key: String,
) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key)
        .map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            // Fallback: try legacy JSON store for migration
            let _ = &_app;
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn mobile_secure_store_set(
    _app: AppHandle<impl Runtime>,
    key: String,
    value: String,
) -> Result<(), String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key)
        .map_err(|e| e.to_string())?;
    entry.set_password(&value).map_err(|e| e.to_string())?;
    let _ = &_app;
    Ok(())
}

#[tauri::command]
pub async fn mobile_secure_store_remove(
    _app: AppHandle<impl Runtime>,
    key: String,
) -> Result<(), String> {
    let entry = keyring::Entry::new("remote-code-secure-store", &key)
        .map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
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
    // Only allow sharing files from the download directory.
    let download_dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("remote-code");
    let canonical_path = std::fs::canonicalize(&file_path)
        .map_err(|e| format!("file not found: {e}"))?;
    let canonical_dir = std::fs::canonicalize(&download_dir)
        .unwrap_or_else(|_| download_dir.clone());
    if !canonical_path.starts_with(&canonical_dir) {
        return Err("file is outside the allowed download directory".to_string());
    }
    #[cfg(feature = "mobile")]
    {
        tauri_plugin_share::share_file(
            &app,
            tauri_plugin_share::ShareFileRequest {
                path: file_path.clone(),
                mime_type: mime_guess::from_path(&file_path)
                    .first()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                title: _title.unwrap_or_default(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = (app, file_path, _title);
    }
    Ok(())
}

#[tauri::command]
pub async fn mobile_push_request_permission(app: AppHandle<impl Runtime>) -> Result<bool, String> {
    #[cfg(feature = "mobile")]
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
            .await
            .map_err(|e| e.to_string())?;
        Ok(result == tauri_plugin_notification::PermissionState::Granted)
    }
    #[cfg(not(feature = "mobile"))]
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
    #[cfg(feature = "mobile")]
    {
        use tauri_plugin_notification::NotificationExt;
        let notification = tauri_plugin_notification::Notification::new(&app)
            .title(&title)
            .body(&body);
        if let Some(d) = data {
            let _ = notification.data(&d);
        }
        notification.show().map_err(|e| e.to_string())?;
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = (app, title, body, data);
    }
    Ok(())
}

#[tauri::command]
pub async fn mobile_push_get_token(app: AppHandle<impl Runtime>) -> Result<Option<String>, String> {
    #[cfg(feature = "mobile")]
    {
        use tauri_plugin_notification::NotificationExt;
        let token = app
            .notification()
            .get_token()
            .map_err(|e| e.to_string())?;
        Ok(token)
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = app;
        Ok(None)
    }
}

#[tauri::command]
pub async fn mobile_push_register_token(
    app: AppHandle<impl Runtime>,
    _base_url: String,
    _access_token: String,
) -> Result<(), String> {
    #[cfg(feature = "mobile")]
    {
        use tauri_plugin_notification::NotificationExt;
        let token = app
            .notification()
            .get_token()
            .map_err(|e| e.to_string())?;
        if let Some(t) = token {
            let client = reqwest::Client::new();
            let result = client
                .post(format!("{}/api/mobile/push-token", _base_url))
                .header("Authorization", format!("Bearer {}", _access_token))
                .json(&serde_json::json!({ "token": t }))
                .send()
                .await;
            if let Err(e) = result {
                tracing::warn!(error = %e, "Failed to register push token with server");
            }
        }
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = (app, _base_url, _access_token);
    }
    Ok(())
}

pub fn register_mobile_plugins<R: Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(feature = "mobile")]
    {
        for (name, result) in [
            ("haptics", app.handle().plugin(tauri_plugin_haptics::init())),
            ("biometric", app.handle().plugin(tauri_plugin_biometric::init())),
            ("network", app.handle().plugin(tauri_plugin_network::init())),
            ("deep_link", app.handle().plugin(tauri_plugin_deep_link::init())),
            ("notification", app.handle().plugin(tauri_plugin_notification::init())),
            ("share", app.handle().plugin(tauri_plugin_share::init())),
        ] {
            if let Err(e) = result {
                tracing::warn!(plugin = name, "Failed to initialize mobile plugin: {e}");
            }
        }
        register_deep_link_listener(&app.handle().clone());
    }
    #[cfg(not(feature = "mobile"))]
    {
        let _ = app;
    }
}

#[cfg(feature = "mobile")]
fn register_deep_link_listener<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    if let Err(e) = tauri_plugin_deep_link::register(
        "remotecode",
        move |urls| {
            for url in urls {
                let _ = handle.emit("mobile://deep-link", &url);
            }
        },
    ) {
        tracing::warn!("deep link registration failed: {e}");
    }
}