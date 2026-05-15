//! Remote control service integrated into the Tauri GUI.
//!
//! Runs as a background task alongside the Tauri desktop window.
//! When enabled (default), starts outbound long-polling to the control plane
//! so the mobile app can remotely control all three in-process agents.
//!
//! Security: Password-based pairing. Both the PC GUI and mobile app must
//! have the same password configured. Remote commands are rejected if
//! passwords don't match.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::types::{AgentConfig, AgentType};
use rc_claude_adapter::ClaudeInProcessAdapter;
use rc_codex_adapter::CodexInProcessAdapter;
use rc_engine_events::types::{RuntimeEventCreateRequest, RuntimeEventDetail};
use rc_roo_adapter::RooInProcessAdapter;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{error, info, warn};
use uuid::Uuid;

use claude_control_plane::RunnerCommandPullResponse;
use claude_runner::{
    RUNNER_EVENT_CHANNEL_CAPACITY, RunnerApi, RunnerApiEvent, RunnerConfig, RunnerConfigOverrides,
    RunnerSessionCommandRequest, RunnerSessionRecord, load_runner_config,
    register_with_control_plane, send_heartbeat,
};

// ─── Public API ─────────────────────────────────────────────────────────────

/// Check if remote control should be auto-started.
/// Reads from env var REMOTE_CODE_RUNNER_MODE=outbound or settings.
pub fn should_auto_start_remote() -> bool {
    // Default: auto-start if control plane URL is configured.
    std::env::var("REMOTE_CODE_CONTROL_PLANE_URL").is_ok()
}

/// Start the remote control background service.
/// Call this from Tauri's setup() callback.
pub fn start_remote_service(app: AppHandle) {
    if !should_auto_start_remote() {
        info!("Remote control: no control plane URL configured, skipping");
        return;
    }

    info!("Remote control: starting background service");

    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        if let Err(e) = run_remote_service(app).await {
            error!("Remote control service error: {e:#}");
        }
    });
}

// ─── Settings ───────────────────────────────────────────────────────────────

/// Get the stored remote control password hash (if any).
fn get_remote_password_hash(app: &AppHandle) -> Option<String> {
    let dir: std::path::PathBuf = app.path().app_config_dir().ok()?;
    let path = dir.join("remote_password_hash.txt");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the stored remote control username (if any).
fn get_remote_username(app: &AppHandle) -> Option<String> {
    let dir: std::path::PathBuf = app.path().app_config_dir().ok()?;
    let path = dir.join("remote_username.txt");
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
/// `user_key = sha256(username:password)` — serves as both auth token and tenant ID.
fn derive_user_key(username: &str, password: &str) -> String {
    hash_password(&format!("{username}:{password}"))
}

/// Save the remote control password (stored as SHA-256 hash).
pub fn set_remote_password(app: &AppHandle, password: &str) -> Result<()> {
    let dir: std::path::PathBuf = app
        .path()
        .app_config_dir()
        .context("failed to get app config dir")?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("remote_password_hash.txt");
    std::fs::write(path, hash_password(password))?;
    Ok(())
}

/// Save the remote control username.
pub fn set_remote_username(app: &AppHandle, username: &str) -> Result<()> {
    let dir: std::path::PathBuf = app
        .path()
        .app_config_dir()
        .context("failed to get app config dir")?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("remote_username.txt");
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
            // No password set yet — first-time pairing accepts any password
            // and auto-saves it for future verification.
            if provided.len() >= 4 {
                let _ = set_remote_password(app, provided);
                true
            } else {
                false
            }
        }
    }
}

/// Get the derived user_key for tenant isolation.
/// Returns sha256(username:password), or None if not yet derived.
pub fn get_remote_user_key(app: &AppHandle) -> Option<String> {
    let dir: std::path::PathBuf = app.path().app_config_dir().ok()?;
    let path = dir.join("remote_user_key.txt");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Save the derived user_key for use as auth token.
fn save_remote_user_key(app: &AppHandle, user_key: &str) -> Result<()> {
    let dir: std::path::PathBuf = app
        .path()
        .app_config_dir()
        .context("failed to get app config dir")?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("remote_user_key.txt");
    std::fs::write(path, user_key)?;
    Ok(())
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
pub fn remote_get_status() -> String {
    if should_auto_start_remote() {
        "enabled".to_string()
    } else {
        "disabled".to_string()
    }
}

#[tauri::command]
pub fn remote_set_password(app: AppHandle, password: String) -> Result<(), String> {
    if password.len() < 4 {
        return Err("Password must be at least 4 characters".to_string());
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
    if password.len() < 4 {
        return Err("Password must be at least 4 characters".to_string());
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
pub fn remote_get_connection_info() -> Result<serde_json::Value, String> {
    let url = std::env::var("REMOTE_CODE_CONTROL_PLANE_URL").unwrap_or_default();
    let runner_id = std::env::var("REMOTE_CODE_RUNNER_ID").unwrap_or_default();
    Ok(serde_json::json!({
        "control_plane_url": url,
        "runner_id": runner_id,
    }))
}

#[tauri::command]
pub fn remote_has_password(app: AppHandle) -> bool {
    get_remote_password_hash(&app).is_some()
}

// ─── Internal service ───────────────────────────────────────────────────────

async fn run_remote_service(app: AppHandle) -> Result<()> {
    // Use the derived user_key as auth token if available, falling back to env var.
    let user_key = get_remote_user_key(&app);
    let auth_token = user_key
        .or_else(|| std::env::var("REMOTE_CODE_RUNNER_AUTH_TOKEN").ok())
        .filter(|s| !s.is_empty());

    let config = load_runner_config(
        std::env::var("REMOTE_CODE_PROFILE_DIR")
            .ok()
            .map(PathBuf::from),
        RunnerConfigOverrides {
            runner_id: std::env::var("REMOTE_CODE_RUNNER_ID").ok(),
            control_plane_url: std::env::var("REMOTE_CODE_CONTROL_PLANE_URL").ok(),
            auth_token: auth_token.clone(),
            heartbeat_interval_secs: std::env::var("REMOTE_CODE_RUNNER_HEARTBEAT_SECS")
                .ok()
                .and_then(|s| s.parse().ok()),
            max_parallel_sessions: std::env::var("REMOTE_CODE_RUNNER_MAX_PARALLEL_SESSIONS")
                .ok()
                .and_then(|s| s.parse().ok()),
            ..RunnerConfigOverrides::default()
        },
    )?;

    let profile_dir = config.profile_dir.profile_dir.clone();
    let cp_url = config.control_plane_url.clone().unwrap_or_default();
    let auth = config.auth_token.clone().unwrap_or_default();

    let (event_tx, event_rx) = mpsc::channel(RUNNER_EVENT_CHANNEL_CAPACITY);
    let api = RunnerApi::new(config.clone(), "remote-code-gui", env!("CARGO_PKG_VERSION"))
        .with_event_channel(event_tx);

    let (shutdown_tx, _shutdown_rx) = watch::channel(false);

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
                info!("Remote approval resolved: {:?}", approval.approval_id);
                Ok(())
            }
        }
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
            .and_then(|v| Some(v.as_str()))
            .map(|s| match s {
                "remote_codex" | "codex" => AgentType::RemoteCodex,
                "remote_roo" | "roo" => AgentType::RemoteRoo,
                _ => AgentType::RemoteClaude,
            })
            .unwrap_or(AgentType::RemoteClaude);

        let model = session
            .metadata
            .get("model")
            .and_then(|v| Some(v.as_str()))
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
                    let mut adapter = RooInProcessAdapter::new();
                    let agent_config = AgentConfig {
                        agent_type: AgentType::RemoteRoo,
                        binary_path: None,
                        args: Vec::new(),
                        env: Vec::new(),
                        working_dir: Some(workspace_dir.clone()),
                        model: Some(model.clone()),
                        provider: Some("anthropic".to_string()),
                        api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
                        base_url: None,
                    };
                    adapter
                        .start(&agent_config)
                        .await
                        .map_err(|e| anyhow!("Roo start: {e}"))?;
                    adapters.insert(sid.clone(), adapter);
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
            RunnerSessionCommandRequest::Interrupt => return Ok(()),
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
    let auth = config.auth_token.as_deref().unwrap_or("");
    let poll_timeout = Duration::from_secs(30);
    let mut retry_delay = Duration::from_secs(1);

    loop {
        if shutdown.has_changed().unwrap_or(true) {
            break;
        }

        let url = format!(
            "{cp_url}/v1/runners/{runner_id}/commands/pull?timeout={}",
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
                    if let Ok(body) = response.text().await {
                        if !body.is_empty() {
                            if let Ok(cmd_response) =
                                serde_json::from_str::<RunnerCommandPullResponse>(&body)
                            {
                                if let Err(e) = apply_pulled_commands(&api, cmd_response).await {
                                    warn!("command processing failed: {e:#}");
                                }
                            }
                        }
                    }
                } else {
                    warn!("poll HTTP {}", response.status());
                }
            }
            Err(e) => {
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
    use claude_control_plane::RunnerQueuedCommandBody;
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
    let auth_token = config.auth_token.as_deref();
    loop {
        match register_with_control_plane(&client, &cp_url, &registration).await {
            Ok(_) => {
                info!("Registered runner {} with control plane", config.runner_id);
                break;
            }
            Err(e) => {
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
                if let Err(e) = send_heartbeat(&client, &cp_url, &hb, auth_token).await {
                    warn!("Heartbeat failed: {e}");
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
}
