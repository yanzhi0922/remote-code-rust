use std::collections::BTreeMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use rc_config::AppPaths;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

const DEFAULT_RUNNER_BIND: &str = "127.0.0.1:8788";
const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 15;
const DEFAULT_MAX_PARALLEL_SESSIONS: u16 = 4;
const PHASE: &str = "phase3-remote-skeleton";

#[derive(Debug, Clone, Default)]
pub struct RunnerConfigOverrides {
    pub runner_id: Option<String>,
    pub control_plane_url: Option<String>,
    pub bind: Option<SocketAddr>,
    pub public_base_url: Option<String>,
    pub heartbeat_interval_secs: Option<u64>,
    pub max_parallel_sessions: Option<u16>,
    pub workspaces: Option<Vec<RunnerWorkspace>>,
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub runner_id: String,
    pub control_plane_url: Option<String>,
    pub bind: SocketAddr,
    pub public_base_url: Option<String>,
    pub profile_dir: AppPaths,
    pub workspaces: Vec<RunnerWorkspace>,
    pub heartbeat_interval_secs: u64,
    pub max_parallel_sessions: u16,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub capabilities: RunnerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerWorkspace {
    pub workspace_id: String,
    pub root_dir: PathBuf,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerCapabilities {
    pub interactive_approvals: bool,
    pub background_sessions: bool,
    pub artifact_uploads: bool,
    pub max_parallel_sessions: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerPlatform {
    pub os: String,
    pub arch: String,
    pub family: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RunnerState {
    Starting,
    #[default]
    Idle,
    Busy,
    Draining,
    Unhealthy,
    Offline,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Pending,
    Starting,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistrationRequest {
    pub runner_id: String,
    pub control_plane_url: Option<String>,
    pub public_base_url: Option<String>,
    pub workspaces: Vec<RunnerWorkspace>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub capabilities: RunnerCapabilities,
    pub platform: RunnerPlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerHeartbeat {
    pub runner_id: String,
    pub state: RunnerState,
    pub active_sessions: usize,
    pub queued_sessions: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSnapshot {
    pub registration: RunnerRegistrationRequest,
    pub state: RunnerState,
    pub active_sessions: usize,
    pub queued_sessions: usize,
    pub registered_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistrationLease {
    pub runner_id: String,
    pub registered_at: DateTime<Utc>,
    pub lease_ttl_secs: u64,
    pub snapshot: RunnerSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStatus {
    pub ok: bool,
    pub runner_id: String,
    pub control_plane_url: Option<String>,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub profile_dir: String,
    pub workspace_count: usize,
    pub workspaces: Vec<RunnerWorkspace>,
    pub heartbeat_interval_secs: u64,
    pub max_parallel_sessions: u16,
    pub issues: Vec<String>,
    pub phase: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerMeta {
    pub service: String,
    pub version: String,
    pub phase: String,
    pub snapshot: RunnerSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerHealth {
    pub ok: bool,
    pub runner_id: String,
    pub state: RunnerState,
    pub active_sessions: usize,
    pub queued_sessions: usize,
    pub workspace_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSessionRecord {
    pub session_id: Uuid,
    pub runner_id: String,
    pub workspace_id: String,
    pub state: SessionState,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSessionCreateRequest {
    pub session_id: Option<Uuid>,
    pub workspace_id: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSessionStateUpdateRequest {
    pub state: SessionState,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    #[default]
    Pending,
    Approved,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
    Cancelled,
}

impl From<ApprovalDecision> for ApprovalState {
    fn from(value: ApprovalDecision) -> Self {
        match value {
            ApprovalDecision::Approved => ApprovalState::Approved,
            ApprovalDecision::Denied => ApprovalState::Denied,
            ApprovalDecision::Cancelled => ApprovalState::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestRecord {
    pub approval_id: Uuid,
    pub session_id: Uuid,
    pub runner_id: String,
    pub state: ApprovalState,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub responder: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalCreateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecisionRequest {
    pub decision: ApprovalDecision,
    #[serde(default)]
    pub responder: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
}

#[derive(Debug, Clone)]
pub struct RunnerApi {
    meta: RunnerMeta,
    sessions: Arc<RwLock<BTreeMap<Uuid, RunnerSessionRecord>>>,
    approvals: Arc<RwLock<BTreeMap<Uuid, ApprovalRequestRecord>>>,
}

impl RunnerApi {
    pub fn new(
        config: RunnerConfig,
        service: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let snapshot = config.snapshot();
        Self {
            meta: RunnerMeta {
                service: service.into(),
                version: version.into(),
                phase: PHASE.to_owned(),
                snapshot,
            },
            sessions: Arc::new(RwLock::new(BTreeMap::new())),
            approvals: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn meta(&self) -> &RunnerMeta {
        &self.meta
    }

    pub async fn list_sessions(&self) -> Vec<RunnerSessionRecord> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    pub async fn list_approvals(&self) -> Vec<ApprovalRequestRecord> {
        let approvals = self.approvals.read().await;
        approvals.values().cloned().collect()
    }

    pub async fn heartbeat(&self) -> RunnerHeartbeat {
        let sessions = self.sessions.read().await;
        let (active_sessions, queued_sessions) = session_counts(&sessions);
        RunnerHeartbeat {
            runner_id: self.meta.snapshot.registration.runner_id.clone(),
            state: if active_sessions > 0 {
                RunnerState::Busy
            } else {
                RunnerState::Idle
            },
            active_sessions,
            queued_sessions,
            timestamp: Utc::now(),
        }
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/healthz", get(get_health))
            .route("/v1/meta", get(get_meta))
            .route("/v1/approvals", get(list_approvals))
            .route("/v1/approvals/{approval_id}", get(get_approval))
            .route(
                "/v1/approvals/{approval_id}/decision",
                axum::routing::post(apply_approval_decision),
            )
            .route("/v1/sessions", get(list_sessions).post(create_session))
            .route("/v1/sessions/{session_id}", get(get_session))
            .route(
                "/v1/sessions/{session_id}/state",
                axum::routing::post(update_session_state),
            )
            .route(
                "/v1/sessions/{session_id}/approvals",
                get(list_session_approvals).post(create_approval),
            )
            .with_state(self)
    }
}

impl RunnerConfig {
    pub fn snapshot(&self) -> RunnerSnapshot {
        let now = Utc::now();
        RunnerSnapshot {
            registration: self.registration_request(),
            state: RunnerState::Idle,
            active_sessions: 0,
            queued_sessions: 0,
            registered_at: now,
            last_seen_at: now,
        }
    }

    pub fn registration_request(&self) -> RunnerRegistrationRequest {
        RunnerRegistrationRequest {
            runner_id: self.runner_id.clone(),
            control_plane_url: self.control_plane_url.clone(),
            public_base_url: self.public_base_url.clone(),
            workspaces: self.workspaces.clone(),
            labels: self.labels.clone(),
            capabilities: self.capabilities.clone(),
            platform: RunnerPlatform::detect(),
        }
    }
}

impl RunnerPlatform {
    pub fn detect() -> Self {
        Self {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
            family: env::consts::FAMILY.to_owned(),
        }
    }
}

pub fn load_runner_config(
    profile_dir_override: Option<PathBuf>,
    overrides: RunnerConfigOverrides,
) -> Result<RunnerConfig> {
    let paths = AppPaths::discover(profile_dir_override)?;
    paths.ensure_exists()?;

    let runner_id = overrides
        .runner_id
        .or_else(|| read_env("REMOTE_CODE_RUNNER_ID"))
        .unwrap_or_else(|| "local-runner".to_owned());
    let control_plane_url = overrides
        .control_plane_url
        .or_else(|| read_env("REMOTE_CODE_CONTROL_PLANE_URL"));
    let bind = match overrides.bind {
        Some(bind) => bind,
        None => parse_socket_addr(
            &read_env("REMOTE_CODE_RUNNER_BIND").unwrap_or_else(|| DEFAULT_RUNNER_BIND.to_owned()),
        )?,
    };
    let public_base_url = overrides
        .public_base_url
        .or_else(|| read_env("REMOTE_CODE_RUNNER_PUBLIC_BASE_URL"));
    let heartbeat_interval_secs = overrides
        .heartbeat_interval_secs
        .or_else(|| parse_env_number("REMOTE_CODE_RUNNER_HEARTBEAT_SECS"))
        .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SECS)
        .max(1);
    let max_parallel_sessions = overrides
        .max_parallel_sessions
        .or_else(|| parse_env_number("REMOTE_CODE_RUNNER_MAX_PARALLEL_SESSIONS"))
        .unwrap_or(DEFAULT_MAX_PARALLEL_SESSIONS)
        .max(1);
    let labels = overrides
        .labels
        .or_else(|| read_env("REMOTE_CODE_RUNNER_LABELS").map(|raw| parse_key_value_map(&raw)))
        .unwrap_or_default();
    let workspaces = match overrides.workspaces {
        Some(workspaces) => workspaces,
        None => {
            if let Some(raw) = read_env("REMOTE_CODE_RUNNER_WORKSPACES") {
                parse_runner_workspaces(&raw)?
            } else {
                vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: env::current_dir()
                        .context("failed to discover the current working directory")?,
                    writable: true,
                }]
            }
        }
    };

    Ok(RunnerConfig {
        runner_id,
        control_plane_url,
        bind,
        public_base_url,
        profile_dir: paths,
        workspaces,
        heartbeat_interval_secs,
        max_parallel_sessions,
        labels,
        capabilities: RunnerCapabilities {
            interactive_approvals: true,
            background_sessions: true,
            artifact_uploads: true,
            max_parallel_sessions,
        },
    })
}

pub fn describe_status(config: &RunnerConfig) -> Result<RunnerStatus> {
    let mut issues = Vec::new();
    if config.control_plane_url.is_none() {
        issues.push("REMOTE_CODE_CONTROL_PLANE_URL is not configured.".to_owned());
    }
    if config.workspaces.is_empty() {
        issues.push("No runner workspaces are configured.".to_owned());
    }

    Ok(RunnerStatus {
        ok: issues.is_empty(),
        runner_id: config.runner_id.clone(),
        control_plane_url: config.control_plane_url.clone(),
        bind: config.bind.to_string(),
        public_base_url: config.public_base_url.clone(),
        profile_dir: config.profile_dir.profile_dir.display().to_string(),
        workspace_count: config.workspaces.len(),
        workspaces: config.workspaces.clone(),
        heartbeat_interval_secs: config.heartbeat_interval_secs,
        max_parallel_sessions: config.max_parallel_sessions,
        issues,
        phase: PHASE,
    })
}

pub fn parse_runner_workspaces(raw: &str) -> Result<Vec<RunnerWorkspace>> {
    let mut workspaces = Vec::new();
    for entry in raw
        .split([';', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (workspace_id, remainder) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid workspace entry `{entry}`; expected id=path|rw"))?;
        let workspace_id = workspace_id.trim();
        if workspace_id.is_empty() {
            return Err(anyhow!(
                "invalid workspace entry `{entry}`; workspace id is empty"
            ));
        }

        let mut parts = remainder
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty());
        let root_dir = parts
            .next()
            .ok_or_else(|| anyhow!("invalid workspace entry `{entry}`; path is missing"))?;
        let mut writable = true;
        for part in parts {
            if part.eq_ignore_ascii_case("ro") || part.eq_ignore_ascii_case("read-only") {
                writable = false;
            } else if part.eq_ignore_ascii_case("rw") || part.eq_ignore_ascii_case("read-write") {
                writable = true;
            }
        }

        workspaces.push(RunnerWorkspace {
            workspace_id: workspace_id.to_owned(),
            root_dir: PathBuf::from(root_dir),
            writable,
        });
    }

    if workspaces.is_empty() {
        return Err(anyhow!("at least one runner workspace must be configured"));
    }
    Ok(workspaces)
}

pub fn parse_key_value_map(raw: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for entry in raw
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if let Some((key, value)) = entry.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                values.insert(key.to_owned(), value.to_owned());
            }
        }
    }
    values
}

pub async fn register_with_control_plane(
    control_plane_url: &str,
    registration: &RunnerRegistrationRequest,
) -> Result<RunnerRegistrationLease> {
    let client = Client::new();
    let response = client
        .post(control_plane_endpoint(
            control_plane_url,
            "/v1/runners/register",
        )?)
        .json(registration)
        .send()
        .await
        .context("runner registration request failed")?
        .error_for_status()
        .context("runner registration was rejected by the control plane")?;
    response
        .json::<RunnerRegistrationLease>()
        .await
        .context("failed to decode runner registration response")
}

pub async fn send_heartbeat(
    control_plane_url: &str,
    heartbeat: &RunnerHeartbeat,
) -> Result<RunnerSnapshot> {
    let client = Client::new();
    let path = format!(
        "/v1/runners/{}/heartbeat",
        encode_path_segment(&heartbeat.runner_id)
    );
    let response = client
        .post(control_plane_endpoint(control_plane_url, &path)?)
        .json(heartbeat)
        .send()
        .await
        .context("runner heartbeat request failed")?
        .error_for_status()
        .context("runner heartbeat was rejected by the control plane")?;
    response
        .json::<RunnerSnapshot>()
        .await
        .context("failed to decode runner heartbeat response")
}

fn control_plane_endpoint(base_url: &str, path: &str) -> Result<String> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(anyhow!("control plane URL is empty"));
    }
    Ok(format!("{base_url}{path}"))
}

fn encode_path_segment(raw: &str) -> String {
    let mut encoded = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn session_counts(sessions: &BTreeMap<Uuid, RunnerSessionRecord>) -> (usize, usize) {
    let active_sessions = sessions
        .values()
        .filter(|session| {
            matches!(
                session.state,
                SessionState::Starting | SessionState::Running | SessionState::WaitingApproval
            )
        })
        .count();
    let queued_sessions = sessions
        .values()
        .filter(|session| matches!(session.state, SessionState::Pending))
        .count();
    (active_sessions, queued_sessions)
}

fn session_state_after_approval(
    decision: ApprovalDecision,
    has_pending_approvals: bool,
) -> SessionState {
    if has_pending_approvals {
        SessionState::WaitingApproval
    } else {
        match decision {
            ApprovalDecision::Approved => SessionState::Running,
            ApprovalDecision::Denied => SessionState::Failed,
            ApprovalDecision::Cancelled => SessionState::Cancelled,
        }
    }
}

async fn get_health(State(api): State<RunnerApi>) -> Json<RunnerHealth> {
    let sessions = api.sessions.read().await;
    let (active_sessions, queued_sessions) = session_counts(&sessions);

    Json(RunnerHealth {
        ok: true,
        runner_id: api.meta.snapshot.registration.runner_id.clone(),
        state: if active_sessions > 0 {
            RunnerState::Busy
        } else {
            RunnerState::Idle
        },
        active_sessions,
        queued_sessions,
        workspace_count: api.meta.snapshot.registration.workspaces.len(),
    })
}

async fn get_meta(State(api): State<RunnerApi>) -> Json<RunnerMeta> {
    Json(api.meta.clone())
}

async fn list_sessions(State(api): State<RunnerApi>) -> Json<ListResponse<RunnerSessionRecord>> {
    let sessions = api.sessions.read().await;
    Json(ListResponse {
        items: sessions.values().cloned().collect(),
    })
}

async fn get_session(
    State(api): State<RunnerApi>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<RunnerSessionRecord>, ApiError> {
    let sessions = api.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
    Ok(Json(session))
}

async fn list_approvals(State(api): State<RunnerApi>) -> Json<ListResponse<ApprovalRequestRecord>> {
    let approvals = api.approvals.read().await;
    Json(ListResponse {
        items: approvals.values().cloned().collect(),
    })
}

async fn get_approval(
    State(api): State<RunnerApi>,
    AxumPath(approval_id): AxumPath<Uuid>,
) -> Result<Json<ApprovalRequestRecord>, ApiError> {
    let approvals = api.approvals.read().await;
    let approval = approvals
        .get(&approval_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("approval `{approval_id}` was not found")))?;
    Ok(Json(approval))
}

async fn list_session_approvals(
    State(api): State<RunnerApi>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<ListResponse<ApprovalRequestRecord>>, ApiError> {
    let sessions = api.sessions.read().await;
    if !sessions.contains_key(&session_id) {
        return Err(ApiError::not_found(format!(
            "session `{session_id}` was not found"
        )));
    }
    drop(sessions);

    let approvals = api.approvals.read().await;
    Ok(Json(ListResponse {
        items: approvals
            .values()
            .filter(|approval| approval.session_id == session_id)
            .cloned()
            .collect(),
    }))
}

async fn create_session(
    State(api): State<RunnerApi>,
    Json(request): Json<RunnerSessionCreateRequest>,
) -> Result<(StatusCode, Json<RunnerSessionRecord>), ApiError> {
    let workspace = api
        .meta
        .snapshot
        .registration
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == request.workspace_id)
        .ok_or_else(|| {
            ApiError::not_found(format!(
                "workspace `{}` is not owned by this runner",
                request.workspace_id
            ))
        })?;

    let mut sessions = api.sessions.write().await;
    let (active_sessions, queued_sessions) = session_counts(&sessions);
    let max_parallel_sessions = usize::from(
        api.meta
            .snapshot
            .registration
            .capabilities
            .max_parallel_sessions,
    );
    if active_sessions + queued_sessions >= max_parallel_sessions {
        return Err(ApiError::conflict(format!(
            "runner `{}` is at session capacity ({max_parallel_sessions})",
            api.meta.snapshot.registration.runner_id
        )));
    }

    let session_id = request.session_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();
    let record = RunnerSessionRecord {
        session_id,
        runner_id: api.meta.snapshot.registration.runner_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        state: SessionState::Pending,
        metadata: request.metadata,
        created_at: now,
        updated_at: now,
    };

    sessions.insert(record.session_id, record.clone());
    Ok((StatusCode::CREATED, Json(record)))
}

async fn update_session_state(
    State(api): State<RunnerApi>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<RunnerSessionStateUpdateRequest>,
) -> Result<Json<RunnerSessionRecord>, ApiError> {
    let mut sessions = api.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
    session.state = request.state;
    session.metadata.extend(request.metadata);
    session.updated_at = Utc::now();
    Ok(Json(session.clone()))
}

async fn create_approval(
    State(api): State<RunnerApi>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalCreateRequest>,
) -> Result<(StatusCode, Json<ApprovalRequestRecord>), ApiError> {
    let mut sessions = api.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
    let now = Utc::now();
    session.state = SessionState::WaitingApproval;
    session.updated_at = now;

    let approval = ApprovalRequestRecord {
        approval_id: request.approval_id.unwrap_or_else(Uuid::new_v4),
        session_id,
        runner_id: api.meta.snapshot.registration.runner_id.clone(),
        state: ApprovalState::Pending,
        title: request.title,
        description: request.description,
        metadata: request.metadata,
        created_at: now,
        updated_at: now,
        responded_at: None,
        responder: None,
        note: None,
    };
    drop(sessions);

    let mut approvals = api.approvals.write().await;
    approvals.insert(approval.approval_id, approval.clone());
    Ok((StatusCode::CREATED, Json(approval)))
}

async fn apply_approval_decision(
    State(api): State<RunnerApi>,
    AxumPath(approval_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalRequestRecord>, ApiError> {
    let decision = request.decision;
    let mut approvals = api.approvals.write().await;
    let approval = approvals
        .get_mut(&approval_id)
        .ok_or_else(|| ApiError::not_found(format!("approval `{approval_id}` was not found")))?;
    if !matches!(approval.state, ApprovalState::Pending) {
        return Err(ApiError::conflict(format!(
            "approval `{approval_id}` is already resolved"
        )));
    }

    let now = Utc::now();
    approval.state = decision.into();
    approval.updated_at = now;
    approval.responded_at = Some(now);
    approval.responder = request.responder;
    approval.note = request.note;
    let updated = approval.clone();
    let has_pending_approvals = approvals.values().any(|candidate| {
        candidate.session_id == updated.session_id
            && candidate.approval_id != updated.approval_id
            && matches!(candidate.state, ApprovalState::Pending)
    });
    drop(approvals);

    let mut sessions = api.sessions.write().await;
    if let Some(session) = sessions.get_mut(&updated.session_id) {
        session.state = session_state_after_approval(decision, has_pending_approvals);
        session.updated_at = now;
    }

    Ok(Json(updated))
}

#[derive(Debug, Clone, Serialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

#[derive(Debug, Clone)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message,
        }
    }

    fn conflict(message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

fn parse_socket_addr(raw: &str) -> Result<SocketAddr> {
    SocketAddr::from_str(raw).with_context(|| format!("invalid socket address `{raw}`"))
}

fn parse_env_number<T>(key: &str) -> Option<T>
where
    T: FromStr,
{
    read_env(key).and_then(|value| value.parse::<T>().ok())
}

fn read_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        body::{Body, to_bytes},
        extract::{Path as AxumPath, State},
        http::Request,
        routing::post,
    };
    use serde::de::DeserializeOwned;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::{net::TcpListener, sync::Mutex};
    use tower::ServiceExt;

    #[test]
    fn workspace_parser_supports_multiple_entries() {
        let workspaces = parse_runner_workspaces("default=C:\\repo|rw;docs=C:\\docs|ro")
            .expect("workspaces should parse");
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces[0].writable);
        assert!(!workspaces[1].writable);
    }

    #[test]
    fn load_runner_config_uses_overrides() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-a".to_owned()),
                control_plane_url: Some("http://127.0.0.1:8787".to_owned()),
                bind: Some(SocketAddr::from_str("127.0.0.1:9999").expect("bind should parse")),
                public_base_url: Some("http://127.0.0.1:9999".to_owned()),
                heartbeat_interval_secs: Some(30),
                max_parallel_sessions: Some(8),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: PathBuf::from("C:/workspace"),
                    writable: true,
                }]),
                labels: Some(BTreeMap::from([(
                    String::from("region"),
                    String::from("lab"),
                )])),
            },
        )
        .expect("config should load");

        assert_eq!(config.runner_id, "runner-a");
        assert_eq!(config.bind.to_string(), "127.0.0.1:9999");
        assert_eq!(config.max_parallel_sessions, 8);
        assert_eq!(config.labels.get("region").map(String::as_str), Some("lab"));
    }

    #[test]
    fn encode_path_segment_escapes_reserved_bytes() {
        assert_eq!(encode_path_segment("runner-a"), "runner-a");
        assert_eq!(encode_path_segment("runner/a b?c"), "runner%2Fa%20b%3Fc");
    }

    #[tokio::test]
    async fn runner_router_creates_and_reads_sessions() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-a".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let app = RunnerApi::new(config, "remote-code-runner", "0.1.0").router();

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "workspace_id": "default",
                            "metadata": {"kind": "smoke"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: RunnerSessionRecord = read_json(create_response).await;

        let get_response = app
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(get_response.status(), StatusCode::OK);
        let loaded: RunnerSessionRecord = read_json(get_response).await;
        assert_eq!(loaded.workspace_id, "default");
        assert_eq!(
            loaded.metadata.get("kind").map(String::as_str),
            Some("smoke")
        );
    }

    #[tokio::test]
    async fn health_endpoint_reports_busy_state_when_sessions_exist() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-b".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let app = RunnerApi::new(config, "remote-code-runner", "0.1.0").router();

        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let response = app
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let health: RunnerHealth = read_json(response).await;
        assert_eq!(health.state, RunnerState::Idle);
        assert_eq!(health.queued_sessions, 1);
    }

    #[tokio::test]
    async fn runner_api_heartbeat_reports_session_counts() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-c".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let api = RunnerApi::new(config, "remote-code-runner", "0.1.0");
        let app = api.clone().router();

        let _ = app
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let heartbeat = api.heartbeat().await;
        assert_eq!(heartbeat.runner_id, "runner-c");
        assert_eq!(heartbeat.state, RunnerState::Idle);
        assert_eq!(heartbeat.active_sessions, 0);
        assert_eq!(heartbeat.queued_sessions, 1);
    }

    #[tokio::test]
    async fn session_state_updates_change_health_and_heartbeat_counts() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-state".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let api = RunnerApi::new(config, "remote-code-runner", "0.1.0");
        let app = api.clone().router();

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "workspace_id": "default",
                            "metadata": {"phase": "queued"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("session create should succeed");
        let created: RunnerSessionRecord = read_json(create_response).await;

        let queued_health_response = app
            .clone()
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("queued health request should succeed");
        let queued_health: RunnerHealth = read_json(queued_health_response).await;
        assert_eq!(queued_health.state, RunnerState::Idle);
        assert_eq!(queued_health.active_sessions, 0);
        assert_eq!(queued_health.queued_sessions, 1);

        let running_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/state", created.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!(RunnerSessionStateUpdateRequest {
                            state: SessionState::Running,
                            metadata: BTreeMap::from([("phase".to_owned(), "running".to_owned(),)]),
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("state update should succeed");
        assert_eq!(running_response.status(), StatusCode::OK);
        let running_session: RunnerSessionRecord = read_json(running_response).await;
        assert_eq!(running_session.state, SessionState::Running);
        assert_eq!(
            running_session.metadata.get("phase").map(String::as_str),
            Some("running")
        );

        let running_health_response = app
            .clone()
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("running health request should succeed");
        let running_health: RunnerHealth = read_json(running_health_response).await;
        assert_eq!(running_health.state, RunnerState::Busy);
        assert_eq!(running_health.active_sessions, 1);
        assert_eq!(running_health.queued_sessions, 0);

        let running_heartbeat = api.heartbeat().await;
        assert_eq!(running_heartbeat.state, RunnerState::Busy);
        assert_eq!(running_heartbeat.active_sessions, 1);
        assert_eq!(running_heartbeat.queued_sessions, 0);

        let completed_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/state", created.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!(RunnerSessionStateUpdateRequest {
                            state: SessionState::Completed,
                            metadata: BTreeMap::from([("result".to_owned(), "ok".to_owned())]),
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("completion update should succeed");
        assert_eq!(completed_response.status(), StatusCode::OK);
        let completed_session: RunnerSessionRecord = read_json(completed_response).await;
        assert_eq!(completed_session.state, SessionState::Completed);
        assert_eq!(
            completed_session.metadata.get("phase").map(String::as_str),
            Some("running")
        );
        assert_eq!(
            completed_session.metadata.get("result").map(String::as_str),
            Some("ok")
        );

        let completed_health_response = app
            .clone()
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("completed health request should succeed");
        let completed_health: RunnerHealth = read_json(completed_health_response).await;
        assert_eq!(completed_health.state, RunnerState::Idle);
        assert_eq!(completed_health.active_sessions, 0);
        assert_eq!(completed_health.queued_sessions, 0);

        let completed_heartbeat = api.heartbeat().await;
        assert_eq!(completed_heartbeat.state, RunnerState::Idle);
        assert_eq!(completed_heartbeat.active_sessions, 0);
        assert_eq!(completed_heartbeat.queued_sessions, 0);
    }

    #[tokio::test]
    async fn send_heartbeat_url_encodes_runner_id_segments() {
        #[derive(Clone)]
        struct HeartbeatCapture {
            runner_id: Arc<Mutex<Option<String>>>,
        }

        async fn capture_heartbeat(
            State(state): State<HeartbeatCapture>,
            AxumPath(runner_id): AxumPath<String>,
            Json(heartbeat): Json<RunnerHeartbeat>,
        ) -> Json<RunnerSnapshot> {
            *state.runner_id.lock().await = Some(runner_id.clone());
            Json(RunnerSnapshot {
                registration: RunnerRegistrationRequest {
                    runner_id,
                    control_plane_url: None,
                    public_base_url: Some("http://127.0.0.1:9".to_owned()),
                    workspaces: Vec::new(),
                    labels: BTreeMap::new(),
                    capabilities: RunnerCapabilities {
                        interactive_approvals: true,
                        background_sessions: true,
                        artifact_uploads: true,
                        max_parallel_sessions: 4,
                    },
                    platform: RunnerPlatform {
                        os: "windows".to_owned(),
                        arch: "x86_64".to_owned(),
                        family: "windows".to_owned(),
                    },
                },
                state: heartbeat.state,
                active_sessions: heartbeat.active_sessions,
                queued_sessions: heartbeat.queued_sessions,
                registered_at: heartbeat.timestamp,
                last_seen_at: heartbeat.timestamp,
            })
        }

        let state = HeartbeatCapture {
            runner_id: Arc::new(Mutex::new(None)),
        };
        let app = Router::new()
            .route("/v1/runners/{runner_id}/heartbeat", post(capture_heartbeat))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("server should keep serving");
        });

        let heartbeat = RunnerHeartbeat {
            runner_id: "runner/a b?c".to_owned(),
            state: RunnerState::Busy,
            active_sessions: 2,
            queued_sessions: 1,
            timestamp: Utc::now(),
        };
        let snapshot = send_heartbeat(&format!("http://{address}"), &heartbeat)
            .await
            .expect("heartbeat request should succeed");
        assert_eq!(snapshot.registration.runner_id, "runner/a b?c");
        assert_eq!(
            state.runner_id.lock().await.as_deref(),
            Some("runner/a b?c")
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn runner_router_creates_and_resolves_approvals() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-approval".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let app = RunnerApi::new(config, "remote-code-runner", "0.1.0").router();

        let create_session_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("session create should succeed");
        let session: RunnerSessionRecord = read_json(create_session_response).await;

        let approval_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/approvals", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "approval_id": Uuid::nil(),
                            "title": "Execute shell command",
                            "description": "Needs user confirmation"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("approval create should succeed");
        assert_eq!(approval_response.status(), StatusCode::CREATED);
        let approval: ApprovalRequestRecord = read_json(approval_response).await;
        assert_eq!(approval.approval_id, Uuid::nil());
        assert_eq!(approval.state, ApprovalState::Pending);

        let list_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}/approvals", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("approval list should succeed");
        let approvals: ListResponse<ApprovalRequestRecord> = read_json(list_response).await;
        assert_eq!(approvals.items.len(), 1);

        let decide_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", approval.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "approved",
                            "responder": "tester",
                            "note": "Ship it"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("approval decision should succeed");
        assert_eq!(decide_response.status(), StatusCode::OK);
        let resolved: ApprovalRequestRecord = read_json(decide_response).await;
        assert_eq!(resolved.state, ApprovalState::Approved);
        assert_eq!(resolved.responder.as_deref(), Some("tester"));

        let session_response = app
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("session fetch should succeed");
        let updated_session: RunnerSessionRecord = read_json(session_response).await;
        assert_eq!(updated_session.state, SessionState::Running);
    }

    #[tokio::test]
    async fn runner_router_keeps_waiting_for_remaining_approvals_and_handles_denial() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-approval-multi".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let app = RunnerApi::new(config, "remote-code-runner", "0.1.0").router();

        let create_session_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("session create should succeed");
        let session: RunnerSessionRecord = read_json(create_session_response).await;

        let create_approval = |title: &str| {
            Request::post(format!("/v1/sessions/{}/approvals", session.session_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "title": title,
                        "description": "Needs user confirmation"
                    })
                    .to_string(),
                ))
                .expect("request should build")
        };

        let first_response = app
            .clone()
            .oneshot(create_approval("First approval"))
            .await
            .expect("first approval create should succeed");
        let first: ApprovalRequestRecord = read_json(first_response).await;

        let second_response = app
            .clone()
            .oneshot(create_approval("Second approval"))
            .await
            .expect("second approval create should succeed");
        let second: ApprovalRequestRecord = read_json(second_response).await;

        let approve_first_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", first.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "approved",
                            "responder": "tester"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("first approval decision should succeed");
        assert_eq!(approve_first_response.status(), StatusCode::OK);

        let waiting_session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("session fetch should succeed");
        let waiting_session: RunnerSessionRecord = read_json(waiting_session_response).await;
        assert_eq!(waiting_session.state, SessionState::WaitingApproval);

        let deny_second_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", second.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "denied",
                            "responder": "tester",
                            "note": "Denied for safety"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("second approval decision should succeed");
        assert_eq!(deny_second_response.status(), StatusCode::OK);
        let denied: ApprovalRequestRecord = read_json(deny_second_response).await;
        assert_eq!(denied.state, ApprovalState::Denied);

        let failed_session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("session fetch should succeed");
        let failed_session: RunnerSessionRecord = read_json(failed_session_response).await;
        assert_eq!(failed_session.state, SessionState::Failed);

        let duplicate_decision_response = app
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", second.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "approved",
                            "responder": "tester"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("duplicate decision request should complete");
        assert_eq!(duplicate_decision_response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn runner_router_rejects_sessions_above_capacity() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-capacity".to_owned()),
                max_parallel_sessions: Some(1),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let app = RunnerApi::new(config, "remote-code-runner", "0.1.0").router();

        let first_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("first request should succeed");
        assert_eq!(first_response.status(), StatusCode::CREATED);

        let second_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("second request should complete");
        assert_eq!(second_response.status(), StatusCode::CONFLICT);
    }

    async fn read_json<T>(response: Response<Body>) -> T
    where
        T: DeserializeOwned,
    {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        serde_json::from_slice(&body).expect("json should parse")
    }
}
