//! Remote control service integrated into the Tauri GUI.
//!
//! Runs as a background task alongside the Tauri desktop window.
//! When enabled (default), starts outbound long-polling to the control plane
//! so the mobile app can remotely control all three in-process agents.
//!
//! Security: Password-based pairing. Both the PC GUI and mobile app must
//! have the same password configured. Remote commands are rejected if
//! passwords don't match.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rc_agent_protocol::PermissionDecision as AgentPermissionDecision;
use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::types::{AgentConfig, AgentType};
use rc_claude_adapter::ClaudeInProcessAdapter;
use rc_codex_adapter::CodexInProcessAdapter;
use rc_engine_events::types::{RuntimeEventCreateRequest, RuntimeEventDetail};
use rc_roo_adapter::RooInProcessAdapter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::dto::{GuiSettingsFile, ProjectListFile, ProviderConfig, ProviderConfigList};
use crate::state::{KEYRING_SERVICE, PROJECTS_FILE_NAME, PROVIDERS_FILE_NAME, SETTINGS_FILE_NAME};

use rc_control_plane::RunnerCommandPullResponse;
use rc_runner::{
    ApprovalRequestRecord, ApprovalState, RUNNER_EVENT_CHANNEL_CAPACITY, RunnerApi, RunnerApiEvent,
    RunnerConfig, RunnerConfigOverrides, RunnerSessionCommandRequest, RunnerSessionRecord,
    load_runner_config, register_with_control_plane, send_heartbeat,
};

static REMOTE_SERVICE_STARTED: AtomicBool = AtomicBool::new(false);
static REMOTE_SERVICE_CONNECTED: AtomicBool = AtomicBool::new(false);
static REMOTE_SERVICE_SHUTDOWN: StdMutex<Option<watch::Sender<bool>>> = StdMutex::new(None);
static REMOTE_SERVICE_LAST_ERROR: StdMutex<Option<String>> = StdMutex::new(None);

const REMOTE_PASSWORD_HASH_FILE: &str = "remote_password_hash.txt";
const REMOTE_USER_KEY_FILE: &str = "remote_user_key.txt";
const REMOTE_RUNNER_API_TOKEN_FILE: &str = "remote_runner_api_token.txt";
const REMOTE_USERNAME_FILE: &str = "remote_username.txt";
const REMOTE_PASSWORD_HASH_KEY: &str = "remote-control-password-hash";
const REMOTE_USER_KEY_KEY: &str = "remote-control-user-key";
const REMOTE_RUNNER_API_TOKEN_KEY: &str = "remote-runner-api-token";
const MIN_REMOTE_PASSWORD_LEN: usize = 12;

// ─── Public API ─────────────────────────────────────────────────────────────

/// Check if remote control should be auto-started.
/// Reads from environment variables first, then persisted GUI settings.
pub fn should_auto_start_remote(app: &AppHandle) -> bool {
    if read_nonempty_env("REMOTE_CODE_CONTROL_PLANE_URL").is_some() {
        return true;
    }

    let settings = load_remote_settings(app);
    settings.auto_start
        && settings
            .control_plane_url
            .as_deref()
            .is_some_and(|url| !url.trim().is_empty())
}

/// Start the remote control background service.
/// Call this from Tauri's setup() callback.
pub fn start_remote_service(app: AppHandle) {
    if !should_auto_start_remote(&app) {
        info!("Remote control: no control plane URL configured, skipping");
        return;
    }

    if let Err(error) = start_configured_remote_service(app) {
        warn!("Remote control: failed to start background service: {error}");
    }
}

// ─── Settings ───────────────────────────────────────────────────────────────

fn default_auto_start() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteControlSettings {
    #[serde(default)]
    control_plane_url: Option<String>,
    #[serde(default)]
    runner_id: Option<String>,
    #[serde(default = "default_auto_start")]
    auto_start: bool,
}

impl Default for RemoteControlSettings {
    fn default() -> Self {
        Self {
            control_plane_url: None,
            runner_id: None,
            auto_start: true,
        }
    }
}

fn read_nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_control_plane_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Control plane URL is required"));
    }

    let parsed =
        reqwest::Url::parse(trimmed).map_err(|_| anyhow!("Control plane URL is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow!(
            "Control plane URL must start with http:// or https://"
        ));
    }
    if parsed.host_str().is_none() {
        return Err(anyhow!("Control plane URL must include a host"));
    }

    Ok(trimmed.trim_end_matches('/').to_string())
}

fn normalize_runner_id(runner_id: Option<String>) -> Option<String> {
    runner_id.and_then(|value| normalize_nonempty(&value))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RemoteProviderSelection {
    name: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
}

fn metadata_string(metadata: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        metadata
            .get(*key)
            .and_then(|value| normalize_nonempty(value))
    })
}

fn metadata_base_url_for_provider(
    metadata: &BTreeMap<String, String>,
    provider_name: Option<&str>,
) -> Option<String> {
    let general = metadata_string(
        metadata,
        &[
            "base_url",
            "provider_base_url",
            "provider-base-url",
            "roo_base_url",
            "roo-base-url",
        ],
    );
    if general.is_some() {
        return general;
    }

    match provider_name
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "anthropic" | "minimax" | "roo" => metadata_string(
            metadata,
            &["anthropic_url", "anthropic-url", "anthropic_base_url"],
        ),
        "openai" | "openai-native" | "codex" => {
            metadata_string(metadata, &["openai_url", "openai-url", "openai_base_url"])
        }
        _ => None,
    }
}

fn env_api_key_for_provider(provider_name: &str) -> Option<String> {
    let normalized = provider_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let provider_specific = format!("{normalized}_API_KEY");
    let mut keys = vec![provider_specific];
    match provider_name.to_ascii_lowercase().as_str() {
        "anthropic" => keys.push("ANTHROPIC_API_KEY".to_string()),
        "openai" | "openai-native" => keys.push("OPENAI_API_KEY".to_string()),
        "minimax" => keys.push("MINIMAX_API_KEY".to_string()),
        "roo" => keys.push("ROO_API_KEY".to_string()),
        _ => {}
    }
    keys.iter().find_map(|key| read_nonempty_env(key))
}

fn keyring_api_key(provider_name: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, provider_name)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .and_then(|value| normalize_nonempty(&value))
}

fn roo_provider_id_from_parts(
    name: Option<&str>,
    protocol: Option<&str>,
    base_url: Option<&str>,
) -> Option<String> {
    let lowered_name = name.map(|value| value.trim().to_ascii_lowercase());
    let lowered_url = base_url.map(|value| value.trim().to_ascii_lowercase());

    if let Some(name) = lowered_name.as_deref() {
        let compact = name.replace([' ', '_'], "-");
        let exact = [
            "anthropic",
            "openai",
            "openai-native",
            "openrouter",
            "deepseek",
            "gemini",
            "google",
            "ollama",
            "lmstudio",
            "xai",
            "mistral",
            "fireworks",
            "litellm",
            "qwen",
            "qwen-code",
            "minimax",
            "fake-ai",
            "moonshot",
            "zai",
            "sambanova",
            "baseten",
            "poe",
            "requesty",
            "unbound",
            "vercel",
            "vercel-ai-gateway",
            "bedrock",
            "aws",
            "kuaikat",
            "kuai-kat",
            "kat",
            "kat-coder",
            "kat-coder-pro",
            "streamlake",
        ];
        if compact == "deepseek"
            && lowered_url
                .as_deref()
                .map(|url| url.contains("/anthropic"))
                .unwrap_or(false)
        {
            return Some("anthropic".to_string());
        }
        if exact.contains(&compact.as_str()) {
            return Some(compact);
        }
        if name.contains("kuaikat")
            || name.contains("kuai kat")
            || name.contains("kat-coder")
            || name.contains("streamlake")
        {
            return Some("kuaikat".to_string());
        }
        if name.contains("minimax") {
            return Some("minimax".to_string());
        }
        if name.contains("anthropic") || name.contains("claude") {
            return Some("anthropic".to_string());
        }
        if name.contains("openai") || name.contains("gpt") {
            return Some("openai".to_string());
        }
    }

    if let Some(url) = lowered_url.as_deref() {
        if url.contains("minimax") || url.contains("minimaxi") {
            return Some("minimax".to_string());
        }
        if url.contains("streamlakeapi") || url.contains("claude-code-proxy") {
            return Some("kuaikat".to_string());
        }
        if url.contains("/anthropic") && url.contains("deepseek") {
            return Some("anthropic".to_string());
        }
        if url.contains("anthropic") {
            return Some("anthropic".to_string());
        }
        if url.contains("openai") {
            return Some("openai".to_string());
        }
    }

    match protocol.map(|value| value.trim().to_ascii_lowercase()) {
        Some(protocol) if protocol == "anthropic" => Some("anthropic".to_string()),
        Some(protocol) if protocol == "openai" => Some("openai".to_string()),
        _ => name.and_then(normalize_nonempty),
    }
}

fn active_profile_model(provider: &ProviderConfig) -> Option<String> {
    provider.active_profile.as_ref().and_then(|profile_name| {
        provider
            .profiles
            .iter()
            .find(|profile| profile.name == *profile_name)
            .and_then(|profile| profile.model.as_deref())
            .and_then(normalize_nonempty)
    })
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Option<T> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<T>(&contents).ok())
}

fn provider_from_configs(app: &AppHandle) -> Option<RemoteProviderSelection> {
    let providers: ProviderConfigList =
        read_json_file(app_config_path(app, PROVIDERS_FILE_NAME).ok()?)?;
    let settings: GuiSettingsFile =
        read_json_file(app_config_path(app, SETTINGS_FILE_NAME).ok()?).unwrap_or_default();
    let selected_name = settings
        .provider_name
        .as_deref()
        .and_then(normalize_nonempty)
        .or_else(|| {
            providers
                .active_provider
                .as_deref()
                .and_then(normalize_nonempty)
        });
    let selected = selected_name
        .as_deref()
        .and_then(|name| {
            providers
                .providers
                .iter()
                .find(|provider| provider.name == name)
        })
        .or_else(|| providers.providers.first())?;

    let model = settings
        .provider_model
        .as_deref()
        .and_then(normalize_nonempty)
        .or_else(|| active_profile_model(selected))
        .or_else(|| selected.model.as_deref().and_then(normalize_nonempty));
    let base_url = settings
        .provider_base_url
        .as_deref()
        .and_then(normalize_nonempty)
        .or_else(|| selected.base_url.as_deref().and_then(normalize_nonempty));
    let name = roo_provider_id_from_parts(
        selected_name.as_deref().or(Some(selected.name.as_str())),
        Some(selected.protocol.as_str()),
        base_url.as_deref(),
    );
    let api_key = name
        .as_deref()
        .and_then(keyring_api_key)
        .or_else(|| keyring_api_key(&selected.name))
        .or_else(|| selected.api_key.as_deref().and_then(normalize_nonempty));

    Some(RemoteProviderSelection {
        name,
        model,
        base_url,
        api_key,
    })
}

fn resolve_roo_provider_selection(
    metadata: &BTreeMap<String, String>,
    local: Option<RemoteProviderSelection>,
    fallback_model: &str,
) -> RemoteProviderSelection {
    let local = local.unwrap_or_default();
    let requested_name = metadata_string(
        metadata,
        &["provider", "provider_name", "provider-name", "roo_provider"],
    )
    .or(local.name)
    .unwrap_or_else(|| "anthropic".to_string());
    let model = metadata_string(metadata, &["model", "provider_model", "provider-model"])
        .or(local.model)
        .unwrap_or_else(|| fallback_model.to_string());
    let base_url =
        metadata_base_url_for_provider(metadata, Some(&requested_name)).or(local.base_url);
    let name = roo_provider_id_from_parts(Some(&requested_name), None, base_url.as_deref())
        .unwrap_or(requested_name);
    let api_key = local.api_key.or_else(|| env_api_key_for_provider(&name));

    RemoteProviderSelection {
        name: Some(name),
        model: Some(model),
        base_url,
        api_key,
    }
}

fn generate_runner_id() -> String {
    format!("desktop-{}", Uuid::new_v4())
}

fn generate_secret_token() -> String {
    format!("rc-{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn remote_settings_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .context("failed to get app config dir")?;
    Ok(dir.join("remote_control.json"))
}

fn app_config_path(app: &AppHandle, file_name: &str) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .context("failed to get app config dir")?;
    Ok(dir.join(file_name))
}

fn load_remote_settings(app: &AppHandle) -> RemoteControlSettings {
    let Ok(path) = remote_settings_path(app) else {
        return RemoteControlSettings::default();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return RemoteControlSettings::default();
    };
    match serde_json::from_str::<RemoteControlSettings>(&contents) {
        Ok(settings) => settings,
        Err(error) => {
            warn!("Remote control: ignoring invalid settings file: {error}");
            RemoteControlSettings::default()
        }
    }
}

fn save_remote_settings(app: &AppHandle, settings: &RemoteControlSettings) -> Result<()> {
    let path = remote_settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, contents)?;
    Ok(())
}

fn configured_control_plane_url(app: &AppHandle) -> Option<String> {
    read_nonempty_env("REMOTE_CODE_CONTROL_PLANE_URL")
        .or_else(|| load_remote_settings(app).control_plane_url)
}

fn configured_control_plane_auth_token(app: &AppHandle) -> Option<String> {
    read_nonempty_env("REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN").or_else(|| get_remote_user_key(app))
}

fn configured_runner_id(app: &AppHandle) -> Option<String> {
    read_nonempty_env("REMOTE_CODE_RUNNER_ID").or_else(|| load_remote_settings(app).runner_id)
}

fn ensure_runner_id(app: &AppHandle) -> Result<String> {
    if let Some(runner_id) = configured_runner_id(app) {
        return Ok(runner_id);
    }

    let mut settings = load_remote_settings(app);
    let runner_id = generate_runner_id();
    settings.runner_id = Some(runner_id.clone());
    save_remote_settings(app, &settings)?;
    Ok(runner_id)
}

fn remote_connection_info(app: &AppHandle) -> serde_json::Value {
    let settings = load_remote_settings(app);
    let control_plane_url = configured_control_plane_url(app).unwrap_or_default();
    let runner_id = configured_runner_id(app).unwrap_or_default();
    let configured = !control_plane_url.is_empty();
    let running = REMOTE_SERVICE_STARTED.load(Ordering::SeqCst);
    let connected = REMOTE_SERVICE_CONNECTED.load(Ordering::SeqCst);
    let credentials_configured = configured_control_plane_auth_token(app).is_some();
    let last_error = REMOTE_SERVICE_LAST_ERROR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    serde_json::json!({
        "control_plane_url": control_plane_url,
        "runner_id": runner_id,
        "auto_start": settings.auto_start,
        "configured": configured,
        "credentials_configured": credentials_configured,
        "running": running,
        "connected": connected,
        "last_error": last_error,
    })
}

fn start_configured_remote_service(app: AppHandle) -> std::result::Result<(), String> {
    let control_plane_url = configured_control_plane_url(&app)
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| "Control plane URL is not configured".to_string())?;

    if REMOTE_SERVICE_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        info!("Remote control: background service is already running");
        return Ok(());
    }
    REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
    *REMOTE_SERVICE_LAST_ERROR
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    *REMOTE_SERVICE_SHUTDOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(shutdown_tx.clone());

    info!("Remote control: starting background service for {control_plane_url}");
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_remote_service(app, shutdown_tx, shutdown_rx).await {
            error!("Remote control service error: {error:#}");
            *REMOTE_SERVICE_LAST_ERROR
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(format!("{error:#}"));
        }
        REMOTE_SERVICE_STARTED.store(false, Ordering::SeqCst);
        REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
    });

    Ok(())
}

fn stop_configured_remote_service() -> bool {
    let sender = REMOTE_SERVICE_SHUTDOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(sender) = sender {
        let _ = sender.send(true);
        REMOTE_SERVICE_STARTED.store(false, Ordering::SeqCst);
        REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
        true
    } else {
        false
    }
}

/// Get the stored remote control password hash (if any).
fn get_remote_password_hash(app: &AppHandle) -> Option<String> {
    read_secret_with_legacy_file_migration(app, REMOTE_PASSWORD_HASH_KEY, REMOTE_PASSWORD_HASH_FILE)
}

/// Get the stored remote control username (if any).
fn get_remote_username(app: &AppHandle) -> Option<String> {
    let path = app_config_path(app, REMOTE_USERNAME_FILE).ok()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Hash a password with SHA-256 for storage.
fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Derive the tenant-scoping user_key from username and password.
/// The control plane accepts it only when sha256(user_key) is configured.
fn derive_user_key(username: &str, password: &str) -> String {
    hash_password(&format!("{username}:{password}"))
}

/// Save the remote control password (stored as SHA-256 hash).
pub fn set_remote_password(app: &AppHandle, password: &str) -> Result<()> {
    save_secret_with_file_fallback(
        app,
        REMOTE_PASSWORD_HASH_KEY,
        REMOTE_PASSWORD_HASH_FILE,
        &hash_password(password),
    )
}

/// Save the remote control username.
pub fn set_remote_username(app: &AppHandle, username: &str) -> Result<()> {
    let path = app_config_path(app, REMOTE_USERNAME_FILE)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, username.trim())?;
    Ok(())
}

/// Check if the provided password matches the stored one.
/// Compares SHA-256 hashes with constant-time comparison.
pub fn verify_remote_password(app: &AppHandle, provided: &str) -> bool {
    match get_remote_password_hash(app) {
        Some(stored_hash) => {
            let provided_hash = hash_password(provided);
            constant_time_eq(stored_hash.as_bytes(), provided_hash.as_bytes())
        }
        None => {
            // Pairing must be explicitly enabled from the desktop GUI first.
            // Never trust a remote-provided password as the initial secret.
            false
        }
    }
}

/// Get the derived user_key for tenant isolation.
/// Returns sha256(username:password), or None if not yet derived.
pub fn get_remote_user_key(app: &AppHandle) -> Option<String> {
    read_secret_with_legacy_file_migration(app, REMOTE_USER_KEY_KEY, REMOTE_USER_KEY_FILE)
}

/// Save the derived user_key for use as an explicitly provisioned auth token.
fn save_remote_user_key(app: &AppHandle, user_key: &str) -> Result<()> {
    save_secret_with_file_fallback(app, REMOTE_USER_KEY_KEY, REMOTE_USER_KEY_FILE, user_key)
}

fn get_or_create_runner_api_token(app: &AppHandle) -> Result<String> {
    if let Some(token) = read_nonempty_env("REMOTE_CODE_RUNNER_AUTH_TOKEN") {
        return Ok(token);
    }

    if let Some(token) = read_secret_with_legacy_file_migration(
        app,
        REMOTE_RUNNER_API_TOKEN_KEY,
        REMOTE_RUNNER_API_TOKEN_FILE,
    ) {
        return Ok(token);
    }

    let token = generate_secret_token();
    save_secret_with_file_fallback(
        app,
        REMOTE_RUNNER_API_TOKEN_KEY,
        REMOTE_RUNNER_API_TOKEN_FILE,
        &token,
    )?;
    Ok(token)
}

fn keyring_get(key: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn keyring_set(key: &str, value: &str) -> bool {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .ok()
        .and_then(|entry| entry.set_password(value).ok())
        .is_some()
}

fn read_legacy_secret_file(app: &AppHandle, file_name: &str) -> Option<String> {
    let path = app_config_path(app, file_name).ok()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn read_secret_with_legacy_file_migration(
    app: &AppHandle,
    keyring_key: &str,
    legacy_file_name: &str,
) -> Option<String> {
    if let Some(value) = keyring_get(keyring_key) {
        return Some(value);
    }

    let value = read_legacy_secret_file(app, legacy_file_name)?;
    if keyring_set(keyring_key, &value) {
        if let Ok(path) = app_config_path(app, legacy_file_name) {
            let _ = std::fs::remove_file(path);
        }
    }
    Some(value)
}

fn save_secret_with_file_fallback(
    app: &AppHandle,
    keyring_key: &str,
    fallback_file_name: &str,
    value: &str,
) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!("secret value cannot be empty"));
    }

    if keyring_set(keyring_key, value) {
        if let Ok(path) = app_config_path(app, fallback_file_name) {
            let _ = std::fs::remove_file(path);
        }
        return Ok(());
    }

    let path = app_config_path(app, fallback_file_name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, value)?;
    restrict_secret_file_permissions(&path);
    Ok(())
}

#[cfg(unix)]
fn restrict_secret_file_permissions(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn restrict_secret_file_permissions(_path: &PathBuf) {
    // Windows ACLs on the per-user app config directory already scope this
    // fallback to the current user. Prefer OS keyring when available.
}

/// Constant-time byte comparison to prevent timing side-channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn remote_get_status(app: AppHandle) -> String {
    if REMOTE_SERVICE_STARTED.load(Ordering::SeqCst) {
        "running".to_string()
    } else if configured_control_plane_url(&app).is_some() {
        "enabled".to_string()
    } else {
        "disabled".to_string()
    }
}

#[tauri::command]
pub fn remote_set_password(app: AppHandle, password: String) -> Result<(), String> {
    if password.len() < MIN_REMOTE_PASSWORD_LEN {
        return Err(format!(
            "Password must be at least {MIN_REMOTE_PASSWORD_LEN} characters"
        ));
    }
    set_remote_password(&app, &password).map_err(|e| e.to_string())?;
    // If username is already set, derive and save the user_key.
    if let Some(username) = get_remote_username(&app) {
        let user_key = derive_user_key(&username, &password);
        save_remote_user_key(&app, &user_key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn remote_set_username(app: AppHandle, username: String) -> Result<(), String> {
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }
    set_remote_username(&app, &username).map_err(|e| e.to_string())?;
    // If password is already set, derive and save the user_key.
    // We need the plaintext password to derive the key, but we only have the hash.
    // The user_key will be derived when both username and password are set together.
    Ok(())
}

/// Set both username and password together, deriving the user_key.
#[tauri::command]
pub fn remote_set_credentials(
    app: AppHandle,
    username: String,
    password: String,
) -> Result<(), String> {
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }
    if password.len() < MIN_REMOTE_PASSWORD_LEN {
        return Err(format!(
            "Password must be at least {MIN_REMOTE_PASSWORD_LEN} characters"
        ));
    }
    set_remote_username(&app, &username).map_err(|e| e.to_string())?;
    set_remote_password(&app, &password).map_err(|e| e.to_string())?;
    let user_key = derive_user_key(&username, &password);
    save_remote_user_key(&app, &user_key).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the stored username for display in settings.
#[tauri::command]
pub fn remote_get_username(app: AppHandle) -> Option<String> {
    get_remote_username(&app)
}

#[tauri::command]
pub fn remote_get_connection_info(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(remote_connection_info(&app))
}

#[tauri::command]
pub fn remote_set_connection(
    app: AppHandle,
    control_plane_url: String,
    runner_id: Option<String>,
    auto_start: Option<bool>,
) -> Result<serde_json::Value, String> {
    let control_plane_url =
        normalize_control_plane_url(&control_plane_url).map_err(|e| e.to_string())?;
    let mut settings = load_remote_settings(&app);

    let runner_id = normalize_runner_id(runner_id)
        .or(settings.runner_id.clone())
        .unwrap_or_else(generate_runner_id);

    settings.control_plane_url = Some(control_plane_url);
    settings.runner_id = Some(runner_id);
    settings.auto_start = auto_start.unwrap_or(settings.auto_start);
    save_remote_settings(&app, &settings).map_err(|e| e.to_string())?;

    let was_running = REMOTE_SERVICE_STARTED.load(Ordering::SeqCst);
    if was_running {
        stop_configured_remote_service();
        start_configured_remote_service(app.clone())?;
    }

    Ok(remote_connection_info(&app))
}

#[tauri::command]
pub fn remote_start_service(app: AppHandle) -> Result<String, String> {
    start_configured_remote_service(app.clone())?;
    Ok(remote_get_status(app))
}

#[tauri::command]
pub fn remote_has_password(app: AppHandle) -> bool {
    get_remote_password_hash(&app).is_some()
}

// ─── Internal service ───────────────────────────────────────────────────────

async fn run_remote_service(
    app: AppHandle,
    shutdown_tx: watch::Sender<bool>,
    _shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let control_plane_auth_token = configured_control_plane_auth_token(&app)
        .context("remote credentials are not configured; set username/password in the GUI first")?;
    let runner_api_auth_token = get_or_create_runner_api_token(&app)?;
    let runner_id = ensure_runner_id(&app)?;
    let control_plane_url =
        configured_control_plane_url(&app).context("control plane URL is not configured")?;
    let profile_override = remote_profile_override_from_env();
    let workspaces = if read_nonempty_env("REMOTE_CODE_RUNNER_WORKSPACES").is_none() {
        match load_gui_runner_workspaces(profile_override.clone()) {
            Ok(workspaces) if !workspaces.is_empty() => Some(workspaces),
            Ok(_) => Some(Vec::new()),
            Err(error) => {
                warn!("Remote control: failed to load GUI workspaces: {error:#}");
                Some(Vec::new())
            }
        }
    } else {
        None
    };

    let mut config = load_runner_config(
        profile_override,
        RunnerConfigOverrides {
            runner_id: Some(runner_id),
            control_plane_url: Some(control_plane_url),
            auth_token: Some(runner_api_auth_token),
            control_plane_auth_token: Some(control_plane_auth_token.clone()),
            workspaces,
            heartbeat_interval_secs: std::env::var("REMOTE_CODE_RUNNER_HEARTBEAT_SECS")
                .ok()
                .and_then(|s| s.parse().ok()),
            max_parallel_sessions: std::env::var("REMOTE_CODE_RUNNER_MAX_PARALLEL_SESSIONS")
                .ok()
                .and_then(|s| s.parse().ok()),
            ..RunnerConfigOverrides::default()
        },
    )?;
    if !allow_direct_runner_public_url() {
        config.public_base_url = None;
    }

    let profile_dir = config.profile_dir.profile_dir.clone();
    let cp_url = config.control_plane_url.clone().unwrap_or_default();
    let auth = config
        .control_plane_auth_token
        .clone()
        .context("remote credentials are not configured")?;

    let (event_tx, event_rx) = mpsc::channel(RUNNER_EVENT_CHANNEL_CAPACITY);
    let api = RunnerApi::new(config.clone(), "remote-code-gui", env!("CARGO_PKG_VERSION"))
        .with_event_channel(event_tx);

    // Event uploader — relays agent events to the control plane.
    let event_uploader = Arc::new(EventUploader::new(cp_url, auth));

    // Control plane sync (registration + heartbeat).
    let cp_sync_shutdown = shutdown_tx.subscribe();
    tokio::spawn(run_control_plane_sync(
        api.clone(),
        config.clone(),
        cp_sync_shutdown,
    ));

    // In-process session manager — uses the same adapters as the desktop GUI.
    let manager =
        InProcessSessionManager::new(app.clone(), api.clone(), profile_dir, event_uploader);
    tokio::spawn(manager.run(event_rx, shutdown_tx.subscribe()));

    // Outbound poll loop.
    let poll_shutdown = shutdown_tx.subscribe();
    tokio::spawn(run_outbound_poll_loop(
        api.clone(),
        config.clone(),
        poll_shutdown,
    ));

    // Notify frontend that remote service is running.
    let _ = app.emit(
        "remote-service-status",
        serde_json::json!({
            "status": "running",
            "runner_id": config.runner_id,
        }),
    );

    // Keep alive until app shutdown.
    let mut wait_shutdown = shutdown_tx.subscribe();
    let _ = wait_shutdown.changed().await;

    Ok(())
}

fn remote_profile_override_from_env() -> Option<PathBuf> {
    read_nonempty_env("REMOTE_CODE_PROFILE_DIR").map(PathBuf::from)
}

fn allow_direct_runner_public_url() -> bool {
    read_nonempty_env("REMOTE_CODE_GUI_ALLOW_DIRECT_RUNNER")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn load_gui_runner_workspaces(
    profile_override: Option<PathBuf>,
) -> Result<Vec<rc_runner::RunnerWorkspace>> {
    let paths = claude_config::AppPaths::discover(profile_override)?;
    let projects_path = paths.profile_dir.join(PROJECTS_FILE_NAME);
    if !projects_path.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&projects_path)
        .with_context(|| format!("failed to read {}", projects_path.display()))?;
    let file: ProjectListFile = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", projects_path.display()))?;

    let mut workspaces = Vec::new();
    for project in file.projects {
        let root_dir = project.path;
        if !root_dir.is_absolute() || !root_dir.is_dir() {
            continue;
        }
        let workspace_id = gui_workspace_id(&root_dir);
        if workspaces
            .iter()
            .any(|workspace: &rc_runner::RunnerWorkspace| workspace.workspace_id == workspace_id)
        {
            continue;
        }
        workspaces.push(rc_runner::RunnerWorkspace {
            workspace_id,
            root_dir,
            writable: true,
        });
    }

    Ok(workspaces)
}

fn gui_workspace_id(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let normalized = canonical.to_string_lossy().replace('\\', "/");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("gui-{}", &digest[..12])
}

// ─── Session manager (in-process) ───────────────────────────────────────────

struct InProcessSessionManager {
    app: AppHandle,
    api: RunnerApi,
    profile_dir: PathBuf,
    event_uploader: Arc<EventUploader>,
    sessions: Arc<Mutex<HashMap<Uuid, InProcessSession>>>,
    claude_adapters: Arc<Mutex<HashMap<String, ClaudeInProcessAdapter>>>,
    roo_adapters: Arc<Mutex<HashMap<String, RooInProcessAdapter>>>,
    codex_adapters: Arc<Mutex<HashMap<String, CodexInProcessAdapter>>>,
}

struct InProcessSession {
    agent_type: AgentType,
    #[allow(dead_code)]
    workspace_dir: PathBuf,
    #[allow(dead_code)]
    model: String,
}

impl InProcessSessionManager {
    fn new(
        app: AppHandle,
        api: RunnerApi,
        profile_dir: PathBuf,
        event_uploader: Arc<EventUploader>,
    ) -> Self {
        Self {
            app,
            api,
            profile_dir,
            event_uploader,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            claude_adapters: Arc::new(Mutex::new(HashMap::new())),
            roo_adapters: Arc::new(Mutex::new(HashMap::new())),
            codex_adapters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn run(
        self,
        mut event_rx: mpsc::Receiver<RunnerApiEvent>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                event = event_rx.recv() => {
                    let Some(event) = event else { break };
                    if let Err(e) = self.handle_event(event).await {
                        warn!("remote session manager error: {e:#}");
                    }
                }
            }
        }
    }

    async fn handle_event(&self, event: RunnerApiEvent) -> Result<()> {
        match event {
            RunnerApiEvent::SessionCreated(session) => self.create_session(session).await,
            RunnerApiEvent::SessionCommand {
                session_id,
                command,
            } => self.forward_command(session_id, command).await,
            RunnerApiEvent::ApprovalResolved(approval) => {
                self.resolve_remote_approval(approval).await
            }
        }
    }

    async fn resolve_remote_approval(&self, approval: ApprovalRequestRecord) -> Result<()> {
        let decision = match approval.state {
            ApprovalState::Approved => AgentPermissionDecision::Allow,
            ApprovalState::Denied | ApprovalState::Cancelled => AgentPermissionDecision::Deny,
            ApprovalState::Pending => return Ok(()),
        };

        let request_id = approval
            .metadata
            .get("request_id")
            .cloned()
            .unwrap_or_else(|| approval.approval_id.to_string());
        let sid = approval.session_id.to_string();
        let agent_type = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&approval.session_id)
                .map(|session| session.agent_type)
                .unwrap_or(AgentType::RemoteClaude)
        };

        info!(
            "Remote approval resolved: {} for request {} ({:?})",
            approval.approval_id, request_id, agent_type
        );

        let result = match agent_type {
            AgentType::RemoteClaude => {
                let mut adapters = self.claude_adapters.lock().await;
                match adapters.get_mut(&sid) {
                    Some(adapter) => adapter
                        .resolve_permission(&sid, &request_id, decision)
                        .await
                        .map_err(|error| anyhow!("Claude resolve_permission: {error}")),
                    None => Err(anyhow!("Claude adapter not started")),
                }
            }
            AgentType::RemoteRoo => {
                let mut adapters = self.roo_adapters.lock().await;
                match adapters.get_mut(&sid) {
                    Some(adapter) => adapter
                        .resolve_permission(&sid, &request_id, decision)
                        .await
                        .map_err(|error| anyhow!("Roo resolve_permission: {error}")),
                    None => Err(anyhow!("Roo adapter not started")),
                }
            }
            AgentType::RemoteCodex => {
                let mut adapters = self.codex_adapters.lock().await;
                match adapters.get_mut(&sid) {
                    Some(adapter) => adapter
                        .resolve_permission(&sid, &request_id, decision)
                        .await
                        .map_err(|error| anyhow!("Codex resolve_permission: {error}")),
                    None => Err(anyhow!("Codex adapter not started")),
                }
            }
            _ => Err(anyhow!("unsupported agent type for resolve_permission")),
        };

        if let Err(error) = result {
            warn!(
                "Failed to resolve remote approval {} for request {}: {error:#}",
                approval.approval_id, request_id
            );
        }

        Ok(())
    }

    async fn create_session(&self, session: RunnerSessionRecord) -> Result<()> {
        if self.sessions.lock().await.contains_key(&session.session_id) {
            return Ok(());
        }

        // Password pairing verification.
        if let Some(provided) = session.metadata.get("pairing_password") {
            if !verify_remote_password(&self.app, provided) {
                warn!(
                    "Remote session {} rejected: password mismatch",
                    session.session_id
                );
                return Err(anyhow!("pairing password mismatch"));
            }
        } else if get_remote_password_hash(&self.app).is_some() {
            // Password is set on this PC but not provided by remote.
            warn!(
                "Remote session {} rejected: no password provided",
                session.session_id
            );
            return Err(anyhow!("pairing password required"));
        } else {
            warn!(
                "Remote session {} rejected: desktop pairing password is not configured",
                session.session_id
            );
            return Err(anyhow!(
                "desktop pairing password is not configured; set credentials in the GUI first"
            ));
        }

        let workspace = self
            .api
            .meta()
            .snapshot
            .registration
            .workspaces
            .iter()
            .find(|w| w.workspace_id == session.workspace_id)
            .cloned()
            .ok_or_else(|| anyhow!("workspace {} not found", session.workspace_id))?;

        let workspace_dir = PathBuf::from(&workspace.root_dir);
        let sid = session.session_id.to_string();

        let agent_type = session
            .metadata
            .get("agent_type")
            .map(|v| v.as_str())
            .map(|s| match s {
                "remote_codex" | "codex" => AgentType::RemoteCodex,
                "remote_roo" | "roo" => AgentType::RemoteRoo,
                _ => AgentType::RemoteClaude,
            })
            .unwrap_or(AgentType::RemoteClaude);

        let mut model = session
            .metadata
            .get("model")
            .map(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-20250514")
            .to_string();

        info!(
            "Remote session {} ({:?}) for {}",
            sid,
            agent_type,
            workspace_dir.display()
        );

        match agent_type {
            AgentType::RemoteClaude => {
                let mut adapters = self.claude_adapters.lock().await;
                if !adapters.contains_key(&sid) {
                    let app_paths =
                        claude_config::AppPaths::discover(Some(self.profile_dir.clone()))?;
                    app_paths.ensure_exists()?;
                    let store = claude_session::SessionStore::open(app_paths.clone())?;
                    let runtime_config = claude_config::load_runtime_config(
                        Some(workspace_dir.clone()),
                        Some(self.profile_dir.clone()),
                        None,
                        claude_core::PermissionMode::default(),
                        Default::default(),
                        Default::default(),
                        false,
                        false,
                        false,
                        false,
                        64,
                        claude_config::ProviderOverrides {
                            model: Some(model.clone()),
                            ..Default::default()
                        },
                        claude_config::RuntimeOverrides::default(),
                    )?;
                    let mut adapter = ClaudeInProcessAdapter::new(runtime_config, Arc::new(store));
                    let agent_config = AgentConfig {
                        agent_type: AgentType::RemoteClaude,
                        binary_path: None,
                        args: Vec::new(),
                        env: Vec::new(),
                        working_dir: Some(workspace_dir.clone()),
                        model: Some(model.clone()),
                        provider: None,
                        api_key: None,
                        base_url: None,
                    };
                    adapter
                        .start(&agent_config)
                        .await
                        .map_err(|e| anyhow!("Claude start: {e}"))?;
                    adapters.insert(sid.clone(), adapter);
                }
            }
            AgentType::RemoteRoo => {
                let mut adapters = self.roo_adapters.lock().await;
                if !adapters.contains_key(&sid) {
                    let provider = resolve_roo_provider_selection(
                        &session.metadata,
                        provider_from_configs(&self.app),
                        &model,
                    );
                    let provider_name = provider
                        .name
                        .clone()
                        .unwrap_or_else(|| "anthropic".to_string());
                    let effective_model = provider.model.clone().unwrap_or_else(|| model.clone());
                    model = effective_model.clone();
                    let api_key = provider.api_key.clone();
                    let base_url = provider.base_url.clone();
                    let mut adapter = RooInProcessAdapter::new();
                    let roo_storage_path = self.profile_dir.join("roo");
                    let agent_config = AgentConfig {
                        agent_type: AgentType::RemoteRoo,
                        binary_path: None,
                        args: Vec::new(),
                        env: vec![
                            (
                                "ROO_TASK_STORAGE_PATH".to_owned(),
                                roo_storage_path.to_string_lossy().to_string(),
                            ),
                            ("ROO_API_CONFIG_NAME".to_owned(), provider_name.clone()),
                        ],
                        working_dir: Some(workspace_dir.clone()),
                        model: Some(effective_model.clone()),
                        provider: Some(provider_name.clone()),
                        api_key,
                        base_url,
                    };
                    adapter
                        .start(&agent_config)
                        .await
                        .map_err(|e| anyhow!("Roo start: {e}"))?;
                    adapters.insert(sid.clone(), adapter);
                    info!(
                        session_id = %sid,
                        provider = %provider_name,
                        model = %effective_model,
                        "Started remote Roo native adapter with local provider configuration"
                    );
                }
            }
            AgentType::RemoteCodex => {
                let mut adapters = self.codex_adapters.lock().await;
                if !adapters.contains_key(&sid) {
                    let options = rc_codex_adapter::CodexAdapterOptions {
                        cwd: workspace_dir.clone(),
                        model: Some(model.clone()),
                        ..Default::default()
                    };
                    let adapter = CodexInProcessAdapter::start_in_process_with_options(options)
                        .await
                        .map_err(|e| anyhow!("Codex start: {e}"))?;
                    adapters.insert(sid.clone(), adapter);
                }
            }
            _ => {
                return Err(anyhow!("unsupported agent type: {:?}", agent_type));
            }
        }

        self.sessions.lock().await.insert(
            session.session_id,
            InProcessSession {
                agent_type,
                workspace_dir,
                model,
            },
        );

        // Notify frontend about new remote session.
        let _ = self.app.emit(
            "remote-session-created",
            serde_json::json!({
                "session_id": sid,
            }),
        );

        Ok(())
    }

    async fn forward_command(
        &self,
        session_id: Uuid,
        command: RunnerSessionCommandRequest,
    ) -> Result<()> {
        let sid = session_id.to_string();
        let prompt = match &command {
            RunnerSessionCommandRequest::SendPrompt { content } => content.clone(),
            RunnerSessionCommandRequest::Interrupt => {
                return self.interrupt_session(session_id).await;
            }
        };

        info!("Remote prompt for {}: {} chars", sid, prompt.len());

        let claude_adapters = self.claude_adapters.clone();
        let roo_adapters = self.roo_adapters.clone();
        let codex_adapters = self.codex_adapters.clone();
        let sessions_map = self.sessions.clone();
        let api = self.api.clone();
        let app = self.app.clone();
        let uploader = self.event_uploader.clone();

        let agent_type = {
            let s = sessions_map.lock().await;
            s.get(&session_id)
                .map(|s| s.agent_type)
                .unwrap_or(AgentType::RemoteClaude)
        };

        tokio::spawn(async move {
            let result: Result<mpsc::Receiver<UnifiedAgentEvent>, anyhow::Error> = match agent_type
            {
                AgentType::RemoteClaude => {
                    let mut adapters = claude_adapters.lock().await;
                    match adapters.get_mut(&sid) {
                        Some(a) => a
                            .send_message(&sid, &prompt)
                            .await
                            .map_err(|e| anyhow!("{e}")),
                        None => Err(anyhow!("adapter not started")),
                    }
                }
                AgentType::RemoteRoo => {
                    let mut adapters = roo_adapters.lock().await;
                    match adapters.get_mut(&sid) {
                        Some(a) => a
                            .send_message(&sid, &prompt)
                            .await
                            .map_err(|e| anyhow!("{e}")),
                        None => Err(anyhow!("adapter not started")),
                    }
                }
                AgentType::RemoteCodex => {
                    let mut adapters = codex_adapters.lock().await;
                    match adapters.get_mut(&sid) {
                        Some(a) => a
                            .send_message(&sid, &prompt)
                            .await
                            .map_err(|e| anyhow!("{e}")),
                        None => Err(anyhow!("adapter not started")),
                    }
                }
                _ => Err(anyhow!("unsupported agent type: {agent_type:?}")),
            };

            match result {
                Ok(mut rx) => {
                    while let Some(event) = rx.recv().await {
                        // Broadcast to local direct-connect subscribers.
                        api.process_agent_event(session_id, &event);
                        // Upload to control plane for mobile relay.
                        if let Some(detail) =
                            rc_agent_protocol::unified_event_to_runtime_detail(&event)
                        {
                            uploader.upload(session_id, detail).await;
                        }
                        // Also emit to Tauri frontend so desktop user sees it too.
                        if let Some(json_val) = api.agent_event_to_runtime_detail(&event) {
                            let _ = app.emit("remote-event", json_val);
                        }
                    }
                }
                Err(e) => warn!("Remote adapter error for {}: {e}", sid),
            }
        });

        Ok(())
    }

    async fn interrupt_session(&self, session_id: Uuid) -> Result<()> {
        let sid = session_id.to_string();
        let agent_type = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&session_id)
                .map(|session| session.agent_type)
                .ok_or_else(|| anyhow!("session {session_id} not found"))?
        };

        match agent_type {
            AgentType::RemoteClaude => {
                let mut adapters = self.claude_adapters.lock().await;
                adapters
                    .get_mut(&sid)
                    .ok_or_else(|| anyhow!("Claude adapter not started"))?
                    .cancel(&sid)
                    .await
                    .map_err(|error| anyhow!("Claude cancel: {error}"))?;
            }
            AgentType::RemoteRoo => {
                let mut adapters = self.roo_adapters.lock().await;
                adapters
                    .get_mut(&sid)
                    .ok_or_else(|| anyhow!("Roo adapter not started"))?
                    .cancel(&sid)
                    .await
                    .map_err(|error| anyhow!("Roo cancel: {error}"))?;
            }
            AgentType::RemoteCodex => {
                let mut adapters = self.codex_adapters.lock().await;
                adapters
                    .get_mut(&sid)
                    .ok_or_else(|| anyhow!("Codex adapter not started"))?
                    .cancel(&sid)
                    .await
                    .map_err(|error| anyhow!("Codex cancel: {error}"))?;
            }
            _ => {
                return Err(anyhow!("unsupported agent type: {agent_type:?}"));
            }
        }

        Ok(())
    }
}

// ─── Event upload to control plane ──────────────────────────────────────────

const MAX_EVENT_BUFFER_PER_SESSION: usize = 128;

struct EventUploader {
    client: reqwest::Client,
    cp_url: String,
    auth: String,
    buffer: Mutex<HashMap<Uuid, VecDeque<RuntimeEventDetail>>>,
}

impl EventUploader {
    fn new(cp_url: String, auth: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            cp_url,
            auth,
            buffer: Mutex::new(HashMap::new()),
        }
    }

    async fn upload(&self, session_id: Uuid, detail: RuntimeEventDetail) {
        // Flush any previously buffered events first.
        self.flush(session_id).await;

        let url = format!(
            "{}/v1/sessions/{session_id}/events",
            self.cp_url.trim_end_matches('/')
        );
        let result = self
            .client
            .post(&url)
            .header("authorization", format!("Bearer {}", self.auth))
            .json(&RuntimeEventCreateRequest {
                detail: detail.clone(),
            })
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("event upload HTTP {}", resp.status());
                    self.buffer_event(session_id, detail).await;
                }
            }
            Err(e) => {
                warn!("event upload failed: {e}");
                self.buffer_event(session_id, detail).await;
            }
        }
    }

    async fn buffer_event(&self, session_id: Uuid, detail: RuntimeEventDetail) {
        let mut buf = self.buffer.lock().await;
        let queue = buf.entry(session_id).or_default();
        if queue.len() >= MAX_EVENT_BUFFER_PER_SESSION {
            let dropped = queue.drain(..queue.len() / 2).count();
            warn!("event buffer cap hit for {session_id}, dropped {dropped} oldest");
        }
        queue.push_back(detail);
    }

    async fn flush(&self, session_id: Uuid) {
        let events: Vec<RuntimeEventDetail> = {
            let mut buf = self.buffer.lock().await;
            buf.remove(&session_id)
                .map(|q| q.into_iter().collect())
                .unwrap_or_default()
        };

        for detail in events {
            let url = format!(
                "{}/v1/sessions/{session_id}/events",
                self.cp_url.trim_end_matches('/')
            );
            let result = self
                .client
                .post(&url)
                .header("authorization", format!("Bearer {}", self.auth))
                .json(&RuntimeEventCreateRequest {
                    detail: detail.clone(),
                })
                .timeout(Duration::from_secs(10))
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {}
                _ => {
                    self.buffer_event(session_id, detail).await;
                    return; // stop flushing, retry later
                }
            }
        }
    }
}

// ─── Outbound poll loop ─────────────────────────────────────────────────────

async fn run_outbound_poll_loop(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let cp_url = match &config.control_plane_url {
        Some(url) => url.clone(),
        None => {
            error!("no control_plane_url");
            return;
        }
    };
    let runner_id = &config.runner_id;
    let client = reqwest::Client::new();
    let auth = config.control_plane_auth_token.as_deref().unwrap_or("");
    let poll_timeout = Duration::from_secs(30);
    let mut retry_delay = Duration::from_secs(1);

    loop {
        if shutdown.has_changed().unwrap_or(true) {
            break;
        }

        let url = format!(
            "{cp_url}/v1/runners/{runner_id}/commands/pull?limit=16&timeout={}",
            poll_timeout.as_secs(),
        );

        let result = client
            .post(&url)
            .header("authorization", format!("Bearer {auth}"))
            .timeout(poll_timeout + Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    retry_delay = Duration::from_secs(1);
                    REMOTE_SERVICE_CONNECTED.store(true, Ordering::SeqCst);
                    let mut pulled_count = 0usize;
                    if let Ok(body) = response.text().await {
                        if !body.is_empty() {
                            if let Ok(cmd_response) =
                                serde_json::from_str::<RunnerCommandPullResponse>(&body)
                            {
                                pulled_count = cmd_response.commands.len();
                                if let Err(e) = apply_pulled_commands(&api, cmd_response).await {
                                    warn!("command processing failed: {e:#}");
                                }
                            }
                        }
                    }
                    if pulled_count == 0 {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                            _ = shutdown.changed() => break,
                        }
                    }
                } else {
                    REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
                    warn!("poll HTTP {}", response.status());
                }
            }
            Err(e) => {
                REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
                warn!("poll failed: {e}");
                tokio::select! {
                    _ = tokio::time::sleep(retry_delay) => {}
                    _ = shutdown.changed() => break,
                }
                retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
            }
        }
    }
}

async fn apply_pulled_commands(api: &RunnerApi, response: RunnerCommandPullResponse) -> Result<()> {
    use rc_control_plane::RunnerQueuedCommandBody;
    for cmd in response.commands {
        match cmd.body {
            RunnerQueuedCommandBody::CreateSession { request } => {
                api.create_session_direct(request).await?;
            }
            RunnerQueuedCommandBody::SessionCommand {
                session_id,
                request,
            } => {
                api.post_session_command_direct(session_id, request).await?;
            }
            RunnerQueuedCommandBody::ApplyApprovalDecision {
                approval_id,
                request,
            } => {
                api.apply_approval_decision_direct(approval_id, request)
                    .await?;
            }
            _ => {}
        }
    }
    Ok(())
}

// ─── Control plane sync ─────────────────────────────────────────────────────

async fn run_control_plane_sync(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let cp_url = match &config.control_plane_url {
        Some(url) => url.clone(),
        None => return,
    };

    let registration = config.registration_request();
    let client = reqwest::Client::new();
    let auth_token = config.control_plane_auth_token.as_deref();
    loop {
        match register_with_control_plane(&client, &cp_url, &registration, auth_token).await {
            Ok(_) => {
                REMOTE_SERVICE_CONNECTED.store(true, Ordering::SeqCst);
                info!("Registered runner {} with control plane", config.runner_id);
                break;
            }
            Err(e) => {
                REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
                warn!("Registration failed: {e}, retrying...");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = shutdown.changed() => return,
                }
            }
        }
    }

    let heartbeat_interval = Duration::from_secs(config.heartbeat_interval_secs.max(1));
    loop {
        tokio::select! {
            _ = tokio::time::sleep(heartbeat_interval) => {
                let hb = api.heartbeat().await;
                match send_heartbeat(&client, &cp_url, &hb, auth_token).await {
                    Ok(_) => {
                        REMOTE_SERVICE_CONNECTED.store(true, Ordering::SeqCst);
                    }
                    Err(e) => {
                        REMOTE_SERVICE_CONNECTED.store(false, Ordering::SeqCst);
                        warn!("Heartbeat failed: {e}");
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roo_provider_selection_prefers_remote_provider_without_remote_secret() {
        let metadata = BTreeMap::from([
            ("provider".to_string(), "minimax".to_string()),
            ("model".to_string(), "minimax-m2.7".to_string()),
            (
                "anthropic-url".to_string(),
                "https://api.minimaxi.com/anthropic".to_string(),
            ),
            (
                "api_key".to_string(),
                "must-not-cross-control-plane".to_string(),
            ),
        ]);
        let local = RemoteProviderSelection {
            name: Some("anthropic".to_string()),
            model: Some("claude-sonnet".to_string()),
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("local-key".to_string()),
        };

        let selected = resolve_roo_provider_selection(&metadata, Some(local), "fallback-model");

        assert_eq!(selected.name.as_deref(), Some("minimax"));
        assert_eq!(selected.model.as_deref(), Some("minimax-m2.7"));
        assert_eq!(
            selected.base_url.as_deref(),
            Some("https://api.minimaxi.com/anthropic")
        );
        assert_eq!(selected.api_key.as_deref(), Some("local-key"));
    }

    #[test]
    fn roo_provider_selection_uses_local_provider_when_remote_is_minimal() {
        let metadata = BTreeMap::new();
        let local = RemoteProviderSelection {
            name: Some("openai".to_string()),
            model: Some("gpt-5.1".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            api_key: Some("local-key".to_string()),
        };

        let selected = resolve_roo_provider_selection(&metadata, Some(local), "fallback-model");

        assert_eq!(selected.name.as_deref(), Some("openai"));
        assert_eq!(selected.model.as_deref(), Some("gpt-5.1"));
        assert_eq!(
            selected.base_url.as_deref(),
            Some("https://api.openai.com/v1")
        );
        assert_eq!(selected.api_key.as_deref(), Some("local-key"));
    }

    #[test]
    fn roo_provider_selection_recognizes_kuaikat_and_anthropic_deepseek_urls() {
        assert_eq!(
            roo_provider_id_from_parts(
                Some("KuaiKAT Coding Plan"),
                Some("anthropic"),
                Some("https://wanqing.streamlakeapi.com/api/gateway/coding/kat-coder-pro-v2/claude-code-proxy"),
            )
            .as_deref(),
            Some("kuaikat")
        );
        assert_eq!(
            roo_provider_id_from_parts(
                Some("DeepSeek"),
                Some("anthropic"),
                Some("https://api.deepseek.com/anthropic"),
            )
            .as_deref(),
            Some("anthropic")
        );
    }
}
