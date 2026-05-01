use tauri::{AppHandle, Emitter, Manager, Runtime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected: bool,
    pub connection_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiometricAvailability {
    pub available: bool,
    pub biometry_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepLinkPairing {
    pub offer_id: String,
    pub secret: String,
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
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let impact_style = match style.as_str() {
            "heavy" => tauri_plugin_haptics::ImpactStyle::Heavy,
            "medium" => tauri_plugin_haptics::ImpactStyle::Medium,
            "light" => tauri_plugin_haptics::ImpactStyle::Light,
            _ => tauri_plugin_haptics::ImpactStyle::Light,
        };
        tauri_plugin_haptics::impact(impact_style);
    }
    let _ = style;
    Ok(())
}

#[tauri::command]
pub async fn mobile_haptic_notification(kind: String) -> Result<(), String> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
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
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        tauri_plugin_haptics::selection();
    }
    Ok(())
}

#[tauri::command]
pub async fn mobile_biometric_check_availability(
    app: AppHandle<impl Runtime>,
) -> Result<BiometricAvailability, String> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let result = tauri_plugin_biometric::check_availability(&app, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(BiometricAvailability {
            available: true,
            biometry_type: result,
        })
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
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
    #[cfg(any(target_os = "ios", target_os = "android"))]
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
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
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
    use tauri_plugin_fs::FsExt;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store_path = data_dir.join(".mobile_secure_store.json");
    if !store_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&store_path).map_err(|e| e.to_string())?;
    let map: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).unwrap_or_default();
    let _ = FsExt;
    Ok(map.get(&key).cloned())
}

#[tauri::command]
pub async fn mobile_secure_store_set(
    app: AppHandle<impl Runtime>,
    key: String,
    value: String,
) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store_path = data_dir.join(".mobile_secure_store.json");
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let mut map: std::collections::HashMap<String, String> = if store_path.exists() {
        let content = std::fs::read_to_string(&store_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    map.insert(key, value);
    let json = serde_json::to_string(&map).map_err(|e| e.to_string())?;
    std::fs::write(&store_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn mobile_secure_store_remove(
    app: AppHandle<impl Runtime>,
    key: String,
) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store_path = data_dir.join(".mobile_secure_store.json");
    if !store_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&store_path).map_err(|e| e.to_string())?;
    let mut map: std::collections::HashMap<String, String> =
        serde_json::from_str(&content).unwrap_or_default();
    map.remove(&key);
    let json = serde_json::to_string(&map).map_err(|e| e.to_string())?;
    std::fs::write(&store_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn mobile_download_artifact(
    app: AppHandle<impl Runtime>,
    url: String,
    file_name: String,
    token: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut request = client.get(&url);
    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {t}"));
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

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
pub async fn mobile_share_file(
    app: AppHandle<impl Runtime>,
    file_path: String,
    _title: Option<String>,
) -> Result<(), String> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let path = std::path::Path::new(&file_path);
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        tauri_plugin_share::share_file(
            &app,
            tauri_plugin_share::ShareFileRequest {
                path: file_path.clone(),
                mime_type: mime_guess::from_path(&file_path)
                    .first()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                title: _title.unwrap_or(file_name),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let _ = (app, file_path, _title);
    }
    Ok(())
}

pub fn register_deep_link_listener<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
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
    let _ = app;
}
