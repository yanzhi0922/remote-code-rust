use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{
        Path as AxumPath, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{
        StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::{DateTime, Duration, Utc};
use futures::SinkExt;
use rc_config::AppPaths;
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionRequest, ApprovalRequestRecord,
    ApprovalState, ListResponse, RunnerHeartbeat, RunnerRegistrationRequest,
    RunnerSessionCreateRequest, RunnerSessionRecord, RunnerSnapshot, RunnerState,
    SessionState as RunnerSessionState,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, broadcast};
use uuid::Uuid;

const DEFAULT_BIND: &str = "127.0.0.1:8787";
const DEFAULT_RUNNER_LEASE_TTL_SECS: u64 = 30;
const DEFAULT_EVENT_HISTORY_LIMIT: usize = 256;
const DEFAULT_EVENT_LIST_LIMIT: usize = 50;
const MAX_EVENT_LIST_LIMIT: usize = 200;
const EVENT_STREAM_BUFFER: usize = 256;
const PHASE: &str = "phase3-remote-skeleton";

#[derive(Debug, Clone, Default)]
pub struct ControlPlaneConfigOverrides {
    pub bind: Option<SocketAddr>,
    pub public_base_url: Option<String>,
    pub service_name: Option<String>,
    pub runner_lease_ttl_secs: Option<u64>,
    pub profile_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    pub bind: SocketAddr,
    pub public_base_url: Option<String>,
    pub service_name: String,
    pub runner_lease_ttl_secs: u64,
    pub profile_dir: PathBuf,
    pub artifact_root_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneMeta {
    pub service: String,
    pub version: String,
    pub phase: String,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub runner_lease_ttl_secs: u64,
    pub profile_dir: String,
    pub artifact_root_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneStatus {
    pub ok: bool,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub service_name: String,
    pub runner_lease_ttl_secs: u64,
    pub profile_dir: String,
    pub artifact_root_dir: String,
    pub phase: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneHealth {
    pub ok: bool,
    pub service: String,
    pub phase: String,
    pub runner_count: usize,
    pub available_runner_count: usize,
    pub session_count: usize,
    pub artifact_count: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Pending,
    Assigned,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub workspace_id: String,
    pub owner_runner_id: Option<String>,
    pub state: SessionState,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub session_id: Option<Uuid>,
    pub workspace_id: String,
    pub preferred_runner_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistrationResponse {
    pub runner_id: String,
    pub registered_at: DateTime<Utc>,
    pub lease_ttl_secs: u64,
    pub snapshot: RunnerSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_id: Uuid,
    pub session_id: Uuid,
    pub runner_id: Option<String>,
    pub name: String,
    pub file_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactCreateRequest {
    pub name: String,
    pub file_name: Option<String>,
    pub media_type: Option<String>,
    pub content_base64: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence: u64,
    pub recorded_at: DateTime<Utc>,
    pub runner_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub detail: TimelineEventDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimelineEventDetail {
    RunnerRegistered {
        lease_ttl_secs: u64,
        workspace_ids: Vec<String>,
        state: RunnerState,
    },
    RunnerHeartbeat {
        state: RunnerState,
        active_sessions: usize,
        queued_sessions: usize,
        reported_at: DateTime<Utc>,
    },
    SessionCreated {
        workspace_id: String,
        owner_runner_id: Option<String>,
        state: SessionState,
    },
    ApprovalRequested {
        approval_id: Uuid,
        title: String,
        state: ApprovalState,
    },
    ApprovalResolved {
        approval_id: Uuid,
        state: ApprovalState,
        responder: Option<String>,
    },
    ArtifactCreated {
        artifact_id: Uuid,
        name: String,
        file_name: String,
        media_type: String,
        size_bytes: u64,
    },
}

#[derive(Debug, Clone)]
struct TimelineEventDraft {
    runner_id: Option<String>,
    session_id: Option<Uuid>,
    detail: TimelineEventDetail,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RecentEventsQuery {
    after: Option<u64>,
    limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ControlPlaneService {
    meta: ControlPlaneMeta,
    runner_lease_ttl_secs: u64,
    artifact_root_dir: PathBuf,
    registry: Arc<RwLock<Registry>>,
    timeline: TimelineStore,
}

#[derive(Debug, Default)]
struct Registry {
    runners: BTreeMap<String, RunnerSnapshot>,
    sessions: BTreeMap<Uuid, SessionRecord>,
    approvals: BTreeMap<Uuid, ApprovalRequestRecord>,
    artifacts: BTreeMap<Uuid, ArtifactRecord>,
}

#[derive(Debug, Clone)]
struct PlannedSession {
    record: SessionRecord,
    owner_runner: Option<RunnerSnapshot>,
}

#[derive(Debug, Clone)]
struct TimelineStore {
    history_limit: usize,
    tx: broadcast::Sender<TimelineEvent>,
    inner: Arc<Mutex<TimelineState>>,
}

#[derive(Debug)]
struct TimelineState {
    next_sequence: u64,
    history: VecDeque<TimelineEvent>,
}

impl ControlPlaneService {
    pub fn new(config: ControlPlaneConfig, version: impl Into<String>) -> Self {
        let service_name = config.service_name.clone();
        let artifact_root_dir = config.artifact_root_dir.clone();
        Self {
            meta: ControlPlaneMeta {
                service: service_name,
                version: version.into(),
                phase: PHASE.to_owned(),
                bind: config.bind.to_string(),
                public_base_url: config.public_base_url,
                runner_lease_ttl_secs: config.runner_lease_ttl_secs,
                profile_dir: config.profile_dir.display().to_string(),
                artifact_root_dir: artifact_root_dir.display().to_string(),
            },
            runner_lease_ttl_secs: config.runner_lease_ttl_secs,
            artifact_root_dir,
            registry: Arc::new(RwLock::new(Registry::default())),
            timeline: TimelineStore::new(DEFAULT_EVENT_HISTORY_LIMIT, EVENT_STREAM_BUFFER),
        }
    }

    pub fn meta(&self) -> &ControlPlaneMeta {
        &self.meta
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/healthz", get(get_health))
            .route("/v1/meta", get(get_meta))
            .route("/v1/events", get(list_recent_events))
            .route("/v1/events/stream", get(subscribe_events))
            .route("/v1/sessions/{session_id}/events", get(list_session_events))
            .route(
                "/v1/sessions/{session_id}/events/stream",
                get(subscribe_session_events),
            )
            .route("/v1/approvals/stream", get(subscribe_approvals))
            .route("/v1/approvals", get(list_approvals))
            .route("/v1/approvals/{approval_id}", get(get_approval))
            .route(
                "/v1/approvals/{approval_id}/decision",
                post(apply_approval_decision),
            )
            .route("/v1/artifacts", get(list_artifacts))
            .route("/v1/artifacts/{artifact_id}", get(get_artifact))
            .route(
                "/v1/artifacts/{artifact_id}/download",
                get(download_artifact),
            )
            .route("/v1/runners", get(list_runners))
            .route("/v1/runners/register", post(register_runner))
            .route("/v1/runners/{runner_id}", get(get_runner))
            .route(
                "/v1/runners/{runner_id}/approvals",
                get(list_runner_approvals),
            )
            .route(
                "/v1/runners/{runner_id}/approvals/stream",
                get(subscribe_runner_approvals),
            )
            .route(
                "/v1/runners/{runner_id}/heartbeat",
                post(update_runner_heartbeat),
            )
            .route("/v1/sessions", get(list_sessions).post(create_session))
            .route("/v1/sessions/{session_id}", get(get_session))
            .route(
                "/v1/sessions/{session_id}/approvals",
                get(list_session_approvals).post(create_approval),
            )
            .route(
                "/v1/sessions/{session_id}/artifacts",
                get(list_session_artifacts).post(create_artifact),
            )
            .with_state(self)
    }

    async fn publish_event(&self, draft: TimelineEventDraft) -> TimelineEvent {
        self.timeline.publish(draft).await
    }
}

impl TimelineStore {
    fn new(history_limit: usize, buffer: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer.max(1));
        Self {
            history_limit: history_limit.max(1),
            tx,
            inner: Arc::new(Mutex::new(TimelineState {
                next_sequence: 1,
                history: VecDeque::with_capacity(history_limit.max(1)),
            })),
        }
    }

    async fn publish(&self, draft: TimelineEventDraft) -> TimelineEvent {
        let event = {
            let mut timeline = self.inner.lock().await;
            let event = TimelineEvent {
                sequence: timeline.next_sequence,
                recorded_at: Utc::now(),
                runner_id: draft.runner_id,
                session_id: draft.session_id,
                detail: draft.detail,
            };
            timeline.next_sequence += 1;
            timeline.history.push_back(event.clone());
            while timeline.history.len() > self.history_limit {
                let _ = timeline.history.pop_front();
            }
            event
        };
        let _ = self.tx.send(event.clone());
        event
    }

    async fn recent(&self, after: Option<u64>, limit: Option<usize>) -> Vec<TimelineEvent> {
        self.recent_filtered(after, limit, |_| true).await
    }

    async fn recent_filtered<F>(
        &self,
        after: Option<u64>,
        limit: Option<usize>,
        filter: F,
    ) -> Vec<TimelineEvent>
    where
        F: Fn(&TimelineEvent) -> bool,
    {
        let limit = limit
            .unwrap_or(DEFAULT_EVENT_LIST_LIMIT)
            .clamp(1, MAX_EVENT_LIST_LIMIT);
        let timeline = self.inner.lock().await;
        let mut events = timeline
            .history
            .iter()
            .filter(|event| after.is_none_or(|sequence| event.sequence > sequence))
            .filter(|event| filter(event))
            .cloned()
            .collect::<Vec<_>>();
        if events.len() > limit {
            events.drain(..events.len() - limit);
        }
        events
    }

    fn subscribe(&self) -> broadcast::Receiver<TimelineEvent> {
        self.tx.subscribe()
    }
}

impl Registry {
    fn register_runner(
        &mut self,
        request: RunnerRegistrationRequest,
        lease_ttl_secs: u64,
    ) -> RunnerRegistrationResponse {
        let now = Utc::now();
        let snapshot = RunnerSnapshot {
            registration: request.clone(),
            state: RunnerState::Idle,
            active_sessions: 0,
            queued_sessions: 0,
            registered_at: now,
            last_seen_at: now,
        };
        self.runners
            .insert(request.runner_id.clone(), snapshot.clone());
        RunnerRegistrationResponse {
            runner_id: request.runner_id,
            registered_at: now,
            lease_ttl_secs,
            snapshot,
        }
    }

    fn apply_heartbeat(
        &mut self,
        runner_id: &str,
        heartbeat: RunnerHeartbeat,
    ) -> Result<RunnerSnapshot, ApiError> {
        let snapshot = self
            .runners
            .get_mut(runner_id)
            .ok_or_else(|| ApiError::not_found(format!("runner `{runner_id}` was not found")))?;
        snapshot.state = heartbeat.state;
        snapshot.active_sessions = heartbeat.active_sessions;
        snapshot.queued_sessions = heartbeat.queued_sessions;
        snapshot.last_seen_at = heartbeat.timestamp;
        Ok(snapshot.clone())
    }

    fn plan_session(
        &self,
        request: &CreateSessionRequest,
        lease_ttl_secs: u64,
    ) -> Result<PlannedSession, ApiError> {
        let session_id = request.session_id.unwrap_or_else(Uuid::new_v4);
        if self.sessions.contains_key(&session_id) {
            return Err(ApiError::conflict(format!(
                "session `{session_id}` already exists"
            )));
        }
        let now = Utc::now();
        let owner_runner_id = self.select_runner(
            &request.workspace_id,
            request.preferred_runner_id.as_deref(),
            lease_ttl_secs,
        )?;
        let state = if owner_runner_id.is_some() {
            SessionState::Assigned
        } else {
            SessionState::Pending
        };
        let record = SessionRecord {
            session_id,
            workspace_id: request.workspace_id.clone(),
            owner_runner_id: owner_runner_id.clone(),
            state,
            metadata: request.metadata.clone(),
            created_at: now,
            updated_at: now,
        };
        let owner_runner = owner_runner_id
            .as_ref()
            .and_then(|runner_id| self.runners.get(runner_id))
            .cloned();
        Ok(PlannedSession {
            record,
            owner_runner,
        })
    }

    fn commit_session(&mut self, record: SessionRecord) -> Result<SessionRecord, ApiError> {
        if self.sessions.contains_key(&record.session_id) {
            return Err(ApiError::conflict(format!(
                "session `{}` already exists",
                record.session_id
            )));
        }
        self.sessions.insert(record.session_id, record.clone());
        if let Some(runner_id) = &record.owner_runner_id
            && let Some(snapshot) = self.runners.get_mut(runner_id)
        {
            snapshot.active_sessions += 1;
            snapshot.state = RunnerState::Busy;
            snapshot.last_seen_at = record.updated_at;
        }
        Ok(record)
    }

    fn list_approvals(&self) -> Vec<ApprovalRequestRecord> {
        self.approvals.values().cloned().collect()
    }

    fn list_artifacts(&self) -> Vec<ArtifactRecord> {
        self.artifacts.values().cloned().collect()
    }

    fn list_runner_approvals(
        &self,
        runner_id: &str,
    ) -> Result<Vec<ApprovalRequestRecord>, ApiError> {
        if !self.runners.contains_key(runner_id) {
            return Err(ApiError::not_found(format!(
                "runner `{runner_id}` was not found"
            )));
        }
        Ok(self
            .approvals
            .values()
            .filter(|approval| approval.runner_id == runner_id)
            .cloned()
            .collect())
    }

    fn get_artifact(&self, artifact_id: Uuid) -> Result<ArtifactRecord, ApiError> {
        self.artifacts
            .get(&artifact_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("artifact `{artifact_id}` was not found")))
    }

    fn get_approval(&self, approval_id: Uuid) -> Result<ApprovalRequestRecord, ApiError> {
        self.approvals
            .get(&approval_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("approval `{approval_id}` was not found")))
    }

    fn list_session_approvals(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ApprovalRequestRecord>, ApiError> {
        if !self.sessions.contains_key(&session_id) {
            return Err(ApiError::not_found(format!(
                "session `{session_id}` was not found"
            )));
        }
        Ok(self
            .approvals
            .values()
            .filter(|approval| approval.session_id == session_id)
            .cloned()
            .collect())
    }

    fn list_session_artifacts(&self, session_id: Uuid) -> Result<Vec<ArtifactRecord>, ApiError> {
        if !self.sessions.contains_key(&session_id) {
            return Err(ApiError::not_found(format!(
                "session `{session_id}` was not found"
            )));
        }
        Ok(self
            .artifacts
            .values()
            .filter(|artifact| artifact.session_id == session_id)
            .cloned()
            .collect())
    }

    fn register_artifact(
        &mut self,
        session_id: Uuid,
        request: &ArtifactCreateRequest,
        size_bytes: u64,
    ) -> Result<ArtifactRecord, ApiError> {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(ApiError::bad_request(
                "artifact name cannot be empty".to_owned(),
            ));
        }
        let session =
            self.sessions.get(&session_id).cloned().ok_or_else(|| {
                ApiError::not_found(format!("session `{session_id}` was not found"))
            })?;
        let file_name = sanitize_artifact_component(
            request
                .file_name
                .as_deref()
                .unwrap_or(request.name.as_str()),
            "artifact.bin",
        );
        let media_type = request
            .media_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let artifact = ArtifactRecord {
            artifact_id: Uuid::new_v4(),
            session_id,
            runner_id: session.owner_runner_id.clone(),
            name: name.to_owned(),
            file_name,
            media_type,
            size_bytes,
            metadata: request.metadata.clone(),
            created_at: Utc::now(),
        };
        self.artifacts
            .insert(artifact.artifact_id, artifact.clone());
        Ok(artifact)
    }

    fn create_approval(
        &mut self,
        session_id: Uuid,
        request: ApprovalCreateRequest,
    ) -> Result<ApprovalRequestRecord, ApiError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
        let now = Utc::now();
        session.state = SessionState::WaitingApproval;
        session.updated_at = now;

        let approval = ApprovalRequestRecord {
            approval_id: Uuid::new_v4(),
            session_id,
            runner_id: session.owner_runner_id.clone().unwrap_or_default(),
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
        self.approvals
            .insert(approval.approval_id, approval.clone());
        Ok(approval)
    }

    fn apply_approval_decision(
        &mut self,
        approval_id: Uuid,
        request: ApprovalDecisionRequest,
    ) -> Result<ApprovalRequestRecord, ApiError> {
        let decision = request.decision;
        let approval = self.approvals.get_mut(&approval_id).ok_or_else(|| {
            ApiError::not_found(format!("approval `{approval_id}` was not found"))
        })?;
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
        let has_pending_approvals = self.approvals.values().any(|candidate| {
            candidate.session_id == updated.session_id
                && candidate.approval_id != updated.approval_id
                && matches!(candidate.state, ApprovalState::Pending)
        });

        if let Some(session) = self.sessions.get_mut(&updated.session_id) {
            session.state = session_state_after_approval(decision, has_pending_approvals);
            session.updated_at = now;
        }

        Ok(updated)
    }

    fn select_runner(
        &self,
        workspace_id: &str,
        preferred_runner_id: Option<&str>,
        lease_ttl_secs: u64,
    ) -> Result<Option<String>, ApiError> {
        if let Some(runner_id) = preferred_runner_id {
            let snapshot = self.runners.get(runner_id).ok_or_else(|| {
                ApiError::not_found(format!("runner `{runner_id}` was not found"))
            })?;
            if !runner_can_host(snapshot, workspace_id, lease_ttl_secs) {
                return Err(ApiError::conflict(format!(
                    "runner `{runner_id}` is not eligible for workspace `{workspace_id}`"
                )));
            }
            return Ok(Some(runner_id.to_owned()));
        }

        let selected = self
            .runners
            .values()
            .filter(|snapshot| runner_can_host(snapshot, workspace_id, lease_ttl_secs))
            .min_by_key(|snapshot| {
                (
                    runner_rank(snapshot.state),
                    snapshot.active_sessions,
                    snapshot.registration.runner_id.as_str(),
                )
            })
            .map(|snapshot| snapshot.registration.runner_id.clone());
        Ok(selected)
    }
}

pub fn load_control_plane_config(
    overrides: ControlPlaneConfigOverrides,
) -> Result<ControlPlaneConfig> {
    let bind = match overrides.bind {
        Some(bind) => bind,
        None => parse_socket_addr(
            &read_env("REMOTE_CODE_CONTROL_PLANE_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned()),
        )?,
    };
    let public_base_url = overrides
        .public_base_url
        .or_else(|| read_env("REMOTE_CODE_CONTROL_PLANE_PUBLIC_BASE_URL"));
    let service_name = overrides
        .service_name
        .or_else(|| read_env("REMOTE_CODE_CONTROL_PLANE_SERVICE_NAME"))
        .unwrap_or_else(|| "remote-code-control-plane".to_owned());
    let runner_lease_ttl_secs = overrides
        .runner_lease_ttl_secs
        .or_else(|| parse_env_number("REMOTE_CODE_RUNNER_LEASE_TTL_SECS"))
        .unwrap_or(DEFAULT_RUNNER_LEASE_TTL_SECS)
        .max(1);
    let profile_dir = overrides
        .profile_dir
        .or_else(|| read_env("REMOTE_CODE_PROFILE_DIR").map(PathBuf::from));
    let paths = AppPaths::discover(profile_dir)?;
    paths.ensure_exists()?;
    let artifact_root_dir = paths.artifacts_dir.join("control-plane");
    std::fs::create_dir_all(&artifact_root_dir)
        .with_context(|| format!("failed to create {}", artifact_root_dir.display()))?;

    Ok(ControlPlaneConfig {
        bind,
        public_base_url,
        service_name,
        runner_lease_ttl_secs,
        profile_dir: paths.profile_dir,
        artifact_root_dir,
    })
}

pub fn describe_status(config: &ControlPlaneConfig) -> ControlPlaneStatus {
    ControlPlaneStatus {
        ok: true,
        bind: config.bind.to_string(),
        public_base_url: config.public_base_url.clone(),
        service_name: config.service_name.clone(),
        runner_lease_ttl_secs: config.runner_lease_ttl_secs,
        profile_dir: config.profile_dir.display().to_string(),
        artifact_root_dir: config.artifact_root_dir.display().to_string(),
        phase: PHASE,
    }
}

async fn get_health(State(service): State<ControlPlaneService>) -> Json<ControlPlaneHealth> {
    let registry = service.registry.read().await;
    let available_runner_count = registry
        .runners
        .values()
        .filter(|snapshot| runner_is_available(snapshot, service.runner_lease_ttl_secs))
        .count();

    Json(ControlPlaneHealth {
        ok: true,
        service: service.meta.service.clone(),
        phase: service.meta.phase.clone(),
        runner_count: registry.runners.len(),
        available_runner_count,
        session_count: registry.sessions.len(),
        artifact_count: registry.artifacts.len(),
    })
}

async fn get_meta(State(service): State<ControlPlaneService>) -> Json<ControlPlaneMeta> {
    Json(service.meta.clone())
}

async fn list_recent_events(
    State(service): State<ControlPlaneService>,
    Query(query): Query<RecentEventsQuery>,
) -> Json<ListResponse<TimelineEvent>> {
    Json(ListResponse {
        items: service.timeline.recent(query.after, query.limit).await,
    })
}

async fn list_session_events(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
    Query(query): Query<RecentEventsQuery>,
) -> Result<Json<ListResponse<TimelineEvent>>, ApiError> {
    {
        let registry = service.registry.read().await;
        if !registry.sessions.contains_key(&session_id) {
            return Err(ApiError::not_found(format!(
                "session `{session_id}` was not found"
            )));
        }
    }
    Ok(Json(ListResponse {
        items: service
            .timeline
            .recent_filtered(query.after, query.limit, |event| {
                event.session_id == Some(session_id)
            })
            .await,
    }))
}

async fn subscribe_events(
    ws: WebSocketUpgrade,
    State(service): State<ControlPlaneService>,
) -> Response {
    ws.on_upgrade(move |socket| serve_event_stream(socket, service.timeline.subscribe()))
}

async fn subscribe_session_events(
    ws: WebSocketUpgrade,
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Response {
    if !service
        .registry
        .read()
        .await
        .sessions
        .contains_key(&session_id)
    {
        return ApiError::not_found(format!("session `{session_id}` was not found"))
            .into_response();
    }
    ws.on_upgrade(move |socket| {
        serve_session_event_stream(socket, service.timeline.subscribe(), session_id)
    })
}

async fn subscribe_approvals(
    ws: WebSocketUpgrade,
    State(service): State<ControlPlaneService>,
) -> Response {
    ws.on_upgrade(move |socket| serve_approval_stream(socket, service.timeline.subscribe()))
}

async fn list_runners(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<RunnerSnapshot>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.runners.values().cloned().collect(),
    })
}

async fn list_runner_approvals(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Result<Json<ListResponse<ApprovalRequestRecord>>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(ListResponse {
        items: registry.list_runner_approvals(&runner_id)?,
    }))
}

async fn subscribe_runner_approvals(
    ws: WebSocketUpgrade,
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Response {
    ws.on_upgrade(move |socket| {
        serve_runner_approval_stream(socket, service.timeline.subscribe(), runner_id)
    })
}

async fn get_runner(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Result<Json<RunnerSnapshot>, ApiError> {
    let registry = service.registry.read().await;
    let snapshot = registry
        .runners
        .get(&runner_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("runner `{runner_id}` was not found")))?;
    Ok(Json(snapshot))
}

async fn list_approvals(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<ApprovalRequestRecord>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.list_approvals(),
    })
}

async fn list_artifacts(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<ArtifactRecord>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.list_artifacts(),
    })
}

async fn get_artifact(
    State(service): State<ControlPlaneService>,
    AxumPath(artifact_id): AxumPath<Uuid>,
) -> Result<Json<ArtifactRecord>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(registry.get_artifact(artifact_id)?))
}

async fn get_approval(
    State(service): State<ControlPlaneService>,
    AxumPath(approval_id): AxumPath<Uuid>,
) -> Result<Json<ApprovalRequestRecord>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(registry.get_approval(approval_id)?))
}

async fn register_runner(
    State(service): State<ControlPlaneService>,
    Json(request): Json<RunnerRegistrationRequest>,
) -> Json<RunnerRegistrationResponse> {
    let response = {
        let mut registry = service.registry.write().await;
        registry.register_runner(request, service.runner_lease_ttl_secs)
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: Some(response.runner_id.clone()),
            session_id: None,
            detail: TimelineEventDetail::RunnerRegistered {
                lease_ttl_secs: response.lease_ttl_secs,
                workspace_ids: response
                    .snapshot
                    .registration
                    .workspaces
                    .iter()
                    .map(|workspace| workspace.workspace_id.clone())
                    .collect(),
                state: response.snapshot.state,
            },
        })
        .await;
    Json(response)
}

async fn update_runner_heartbeat(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
    Json(heartbeat): Json<RunnerHeartbeat>,
) -> Result<Json<RunnerSnapshot>, ApiError> {
    let snapshot = {
        let mut registry = service.registry.write().await;
        registry.apply_heartbeat(&runner_id, heartbeat)?
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: Some(snapshot.registration.runner_id.clone()),
            session_id: None,
            detail: TimelineEventDetail::RunnerHeartbeat {
                state: snapshot.state,
                active_sessions: snapshot.active_sessions,
                queued_sessions: snapshot.queued_sessions,
                reported_at: snapshot.last_seen_at,
            },
        })
        .await;
    Ok(Json(snapshot))
}

async fn list_sessions(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<SessionRecord>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.sessions.values().cloned().collect(),
    })
}

async fn get_session(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<SessionRecord>, ApiError> {
    let registry = service.registry.read().await;
    let record = registry
        .sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
    Ok(Json(record))
}

async fn list_session_approvals(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<ListResponse<ApprovalRequestRecord>>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(ListResponse {
        items: registry.list_session_approvals(session_id)?,
    }))
}

async fn list_session_artifacts(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<ListResponse<ArtifactRecord>>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(ListResponse {
        items: registry.list_session_artifacts(session_id)?,
    }))
}

async fn create_session(
    State(service): State<ControlPlaneService>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionRecord>), ApiError> {
    let planned = {
        let registry = service.registry.read().await;
        registry.plan_session(&request, service.runner_lease_ttl_secs)?
    };
    let mut record = planned.record;

    if let Some(owner_runner) = planned.owner_runner {
        let dispatched = dispatch_session_to_runner(
            &owner_runner,
            &RunnerSessionCreateRequest {
                session_id: Some(record.session_id),
                workspace_id: record.workspace_id.clone(),
                metadata: record.metadata.clone(),
            },
        )
        .await?;
        record.state = session_state_from_runner(dispatched.state);
        record.updated_at = dispatched.updated_at;
    }

    let record = {
        let mut registry = service.registry.write().await;
        registry.commit_session(record)?
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: record.owner_runner_id.clone(),
            session_id: Some(record.session_id),
            detail: TimelineEventDetail::SessionCreated {
                workspace_id: record.workspace_id.clone(),
                owner_runner_id: record.owner_runner_id.clone(),
                state: record.state,
            },
        })
        .await;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn create_artifact(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<ArtifactCreateRequest>,
) -> Result<(StatusCode, Json<ArtifactRecord>), ApiError> {
    let contents = BASE64_STANDARD
        .decode(request.content_base64.as_bytes())
        .map_err(|error| {
            ApiError::bad_request(format!("artifact content is not valid base64: {error}"))
        })?;
    let artifact = {
        let mut registry = service.registry.write().await;
        registry.register_artifact(session_id, &request, contents.len() as u64)?
    };
    let path = artifact_file_path(&service.artifact_root_dir, &artifact);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            ApiError::internal(format!("failed to create {}: {error}", parent.display()))
        })?;
    }
    tokio::fs::write(&path, &contents).await.map_err(|error| {
        ApiError::internal(format!("failed to write {}: {error}", path.display()))
    })?;
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: artifact.runner_id.clone(),
            session_id: Some(artifact.session_id),
            detail: TimelineEventDetail::ArtifactCreated {
                artifact_id: artifact.artifact_id,
                name: artifact.name.clone(),
                file_name: artifact.file_name.clone(),
                media_type: artifact.media_type.clone(),
                size_bytes: artifact.size_bytes,
            },
        })
        .await;
    Ok((StatusCode::CREATED, Json(artifact)))
}

async fn create_approval(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalCreateRequest>,
) -> Result<(StatusCode, Json<ApprovalRequestRecord>), ApiError> {
    let approval = {
        let mut registry = service.registry.write().await;
        registry.create_approval(session_id, request)?
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: (!approval.runner_id.is_empty()).then(|| approval.runner_id.clone()),
            session_id: Some(approval.session_id),
            detail: TimelineEventDetail::ApprovalRequested {
                approval_id: approval.approval_id,
                title: approval.title.clone(),
                state: approval.state,
            },
        })
        .await;
    Ok((StatusCode::CREATED, Json(approval)))
}

async fn apply_approval_decision(
    State(service): State<ControlPlaneService>,
    AxumPath(approval_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalRequestRecord>, ApiError> {
    let approval = {
        let mut registry = service.registry.write().await;
        registry.apply_approval_decision(approval_id, request)?
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: (!approval.runner_id.is_empty()).then(|| approval.runner_id.clone()),
            session_id: Some(approval.session_id),
            detail: TimelineEventDetail::ApprovalResolved {
                approval_id: approval.approval_id,
                state: approval.state,
                responder: approval.responder.clone(),
            },
        })
        .await;
    Ok(Json(approval))
}

async fn download_artifact(
    State(service): State<ControlPlaneService>,
    AxumPath(artifact_id): AxumPath<Uuid>,
) -> Result<Response, ApiError> {
    let artifact = {
        let registry = service.registry.read().await;
        registry.get_artifact(artifact_id)?
    };
    let path = artifact_file_path(&service.artifact_root_dir, &artifact);
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        ApiError::internal(format!("failed to read {}: {error}", path.display()))
    })?;
    Ok((
        [
            (CONTENT_TYPE, artifact.media_type.clone()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", artifact.file_name),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn serve_event_stream(
    mut socket: WebSocket,
    mut subscription: broadcast::Receiver<TimelineEvent>,
) {
    serve_filtered_event_stream(&mut socket, &mut subscription, |_| true).await;
}

async fn serve_session_event_stream(
    mut socket: WebSocket,
    mut subscription: broadcast::Receiver<TimelineEvent>,
    session_id: Uuid,
) {
    serve_filtered_event_stream(&mut socket, &mut subscription, move |event| {
        event.session_id == Some(session_id)
    })
    .await;
}

async fn serve_approval_stream(
    mut socket: WebSocket,
    mut subscription: broadcast::Receiver<TimelineEvent>,
) {
    serve_filtered_event_stream(&mut socket, &mut subscription, is_approval_event).await;
}

async fn serve_runner_approval_stream(
    mut socket: WebSocket,
    mut subscription: broadcast::Receiver<TimelineEvent>,
    runner_id: String,
) {
    serve_filtered_event_stream(&mut socket, &mut subscription, move |event| {
        event.runner_id.as_deref() == Some(runner_id.as_str()) && is_approval_event(event)
    })
    .await;
}

async fn serve_filtered_event_stream<F>(
    socket: &mut WebSocket,
    subscription: &mut broadcast::Receiver<TimelineEvent>,
    filter: F,
) where
    F: Fn(&TimelineEvent) -> bool,
{
    loop {
        let event = match subscription.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                let _ = socket.close().await;
                break;
            }
        };
        if !filter(&event) {
            continue;
        }
        let payload = match serde_json::to_string(&event) {
            Ok(payload) => payload,
            Err(_) => break,
        };
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break;
        }
    }
}

fn is_approval_event(event: &TimelineEvent) -> bool {
    matches!(
        event.detail,
        TimelineEventDetail::ApprovalRequested { .. }
            | TimelineEventDetail::ApprovalResolved { .. }
    )
}

fn artifact_file_path(root: &Path, artifact: &ArtifactRecord) -> PathBuf {
    root.join(artifact.session_id.to_string())
        .join(format!("{}-{}", artifact.artifact_id, artifact.file_name))
}

fn sanitize_artifact_component(raw: &str, fallback: &str) -> String {
    let candidate = Path::new(raw)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(raw)
        .trim();
    let sanitized = candidate
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        fallback.to_owned()
    } else {
        sanitized
    }
}

fn runner_can_host(snapshot: &RunnerSnapshot, workspace_id: &str, lease_ttl_secs: u64) -> bool {
    runner_is_available(snapshot, lease_ttl_secs)
        && snapshot
            .registration
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_id)
}

fn runner_is_available(snapshot: &RunnerSnapshot, lease_ttl_secs: u64) -> bool {
    !matches!(
        snapshot.state,
        RunnerState::Draining | RunnerState::Offline | RunnerState::Unhealthy
    ) && snapshot.last_seen_at >= Utc::now() - Duration::seconds(lease_ttl_secs as i64)
}

fn runner_rank(state: RunnerState) -> u8 {
    match state {
        RunnerState::Idle => 0,
        RunnerState::Busy => 1,
        RunnerState::Starting => 2,
        RunnerState::Draining => 3,
        RunnerState::Unhealthy => 4,
        RunnerState::Offline => 5,
    }
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

fn session_state_from_runner(state: RunnerSessionState) -> SessionState {
    match state {
        RunnerSessionState::Pending | RunnerSessionState::Starting => SessionState::Assigned,
        RunnerSessionState::Running => SessionState::Running,
        RunnerSessionState::WaitingApproval => SessionState::WaitingApproval,
        RunnerSessionState::Completed => SessionState::Completed,
        RunnerSessionState::Failed => SessionState::Failed,
        RunnerSessionState::Cancelled => SessionState::Cancelled,
    }
}

fn runner_public_base_url(runner: &RunnerSnapshot) -> Result<&str, ApiError> {
    runner
        .registration
        .public_base_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ApiError::service_unavailable(format!(
                "runner `{}` does not expose a public base URL",
                runner.registration.runner_id
            ))
        })
}

async fn dispatch_session_to_runner(
    runner: &RunnerSnapshot,
    request: &RunnerSessionCreateRequest,
) -> Result<RunnerSessionRecord, ApiError> {
    let base_url = runner_public_base_url(runner)?;
    let client = Client::new();
    let response = client
        .post(format!("{}/v1/sessions", base_url.trim_end_matches('/')))
        .json(request)
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to dispatch session to runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })?
        .error_for_status()
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "runner `{}` rejected session dispatch: {error}",
                runner.registration.runner_id
            ))
        })?;
    response
        .json::<RunnerSessionRecord>()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to decode session dispatch response from runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })
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

    fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message,
        }
    }

    fn service_unavailable(message: String) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message,
        }
    }

    fn bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "bad_gateway",
            message,
        }
    }

    fn internal(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
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
        body::{Body, to_bytes},
        http::Request,
    };
    use futures::StreamExt;
    use rc_runner::{
        RunnerApi, RunnerCapabilities, RunnerConfigOverrides, RunnerPlatform, RunnerWorkspace,
        load_runner_config,
    };
    use reqwest::Client;
    use serde::de::DeserializeOwned;
    use tempfile::{TempDir, tempdir};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::{Duration as TokioDuration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
    use tower::ServiceExt;

    struct SpawnedRunner {
        api: RunnerApi,
        registration: RunnerRegistrationRequest,
        _profile: TempDir,
        server: JoinHandle<()>,
    }

    impl Drop for SpawnedRunner {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    #[test]
    fn control_plane_config_uses_overrides() {
        let profile = tempdir().expect("tempdir should exist");
        let config = load_control_plane_config(ControlPlaneConfigOverrides {
            bind: Some(SocketAddr::from_str("127.0.0.1:9898").expect("bind should parse")),
            public_base_url: Some("http://127.0.0.1:9898".to_owned()),
            service_name: Some("rc-control".to_owned()),
            runner_lease_ttl_secs: Some(45),
            profile_dir: Some(profile.path().join("profile")),
        })
        .expect("config should load");

        assert_eq!(config.bind.to_string(), "127.0.0.1:9898");
        assert_eq!(config.service_name, "rc-control");
        assert_eq!(config.runner_lease_ttl_secs, 45);
        assert!(config.artifact_root_dir.ends_with("control-plane"));
    }

    #[tokio::test]
    async fn control_plane_registers_runner_and_assigns_session() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                service_name: Some("control".to_owned()),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-a", "default").await;
        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&runner.registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "default", "metadata": {"source": "test"}})
                            .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;
        assert_eq!(session.owner_runner_id.as_deref(), Some("runner-a"));
        assert_eq!(session.state, SessionState::Assigned);
        let runner_sessions = runner.api.list_sessions().await;
        assert_eq!(runner_sessions.len(), 1);
        assert_eq!(runner_sessions[0].session_id, session.session_id);

        let health_response = app
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let health: ControlPlaneHealth = read_json(health_response).await;
        assert_eq!(health.runner_count, 1);
        assert_eq!(health.session_count, 1);
    }

    #[tokio::test]
    async fn control_plane_rejects_session_when_runner_dispatch_fails() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let mut registration = runner_registration("runner-dead", "default", "C:/workspace-dead");
        registration.public_base_url = Some("http://127.0.0.1:9".to_owned());
        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
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
            .expect("request should complete");
        assert_eq!(create_response.status(), StatusCode::BAD_GATEWAY);

        let sessions_response = app
            .oneshot(
                Request::get("/v1/sessions")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let sessions: ListResponse<SessionRecord> = read_json(sessions_response).await;
        assert!(sessions.items.is_empty());
    }

    #[tokio::test]
    async fn heartbeat_updates_runner_state() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let registration = runner_registration("runner-b", "default", "C:/workspace");
        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let heartbeat = RunnerHeartbeat {
            runner_id: "runner-b".to_owned(),
            state: RunnerState::Busy,
            active_sessions: 2,
            queued_sessions: 1,
            timestamp: Utc::now(),
        };
        let heartbeat_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/runner-b/heartbeat")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&heartbeat).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(heartbeat_response.status(), StatusCode::OK);

        let runner_response = app
            .oneshot(
                Request::get("/v1/runners/runner-b")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let snapshot: RunnerSnapshot = read_json(runner_response).await;
        assert_eq!(snapshot.state, RunnerState::Busy);
        assert_eq!(snapshot.active_sessions, 2);
        assert_eq!(snapshot.queued_sessions, 1);
    }

    #[tokio::test]
    async fn recent_events_endpoint_lists_emitted_timeline_entries() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-c", "default").await;
        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&runner.registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let heartbeat = RunnerHeartbeat {
            runner_id: "runner-c".to_owned(),
            state: RunnerState::Busy,
            active_sessions: 1,
            queued_sessions: 0,
            timestamp: Utc::now(),
        };
        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/runner-c/heartbeat")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&heartbeat).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let create_response = app
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
        let created_session: SessionRecord = read_json(create_response).await;

        let events_response = app
            .oneshot(
                Request::get("/v1/events?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(events_response.status(), StatusCode::OK);

        let events: ListResponse<TimelineEvent> = read_json(events_response).await;
        assert_eq!(events.items.len(), 3);
        assert_eq!(events.items[0].sequence, 1);
        assert_eq!(events.items[1].sequence, 2);
        assert_eq!(events.items[2].sequence, 3);
        assert!(matches!(
            events.items[0].detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));
        assert!(matches!(
            events.items[1].detail,
            TimelineEventDetail::RunnerHeartbeat { .. }
        ));
        assert!(matches!(
            events.items[2].detail,
            TimelineEventDetail::SessionCreated { .. }
        ));
        assert_eq!(events.items[2].session_id, Some(created_session.session_id));
    }

    #[tokio::test]
    async fn approval_relay_updates_session_state_and_timeline() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-approval", "default").await;
        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&runner.registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let create_response = app
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
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;
        assert_eq!(session.owner_runner_id.as_deref(), Some("runner-approval"));
        assert_eq!(session.state, SessionState::Assigned);
        assert_eq!(runner.api.list_sessions().await.len(), 1);

        let create_approval_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/approvals", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "title": "Run privileged tool",
                            "description": "Needs operator approval",
                            "metadata": {"tool": "shell_command"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_approval_response.status(), StatusCode::CREATED);
        let approval: ApprovalRequestRecord = read_json(create_approval_response).await;
        assert_eq!(approval.session_id, session.session_id);
        assert_eq!(approval.runner_id, "runner-approval");
        assert_eq!(approval.state, ApprovalState::Pending);

        let approvals_response = app
            .clone()
            .oneshot(
                Request::get("/v1/approvals")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let approvals: ListResponse<ApprovalRequestRecord> = read_json(approvals_response).await;
        assert_eq!(approvals.items.len(), 1);
        assert_eq!(approvals.items[0].approval_id, approval.approval_id);

        let session_approvals_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}/approvals", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let session_approvals: ListResponse<ApprovalRequestRecord> =
            read_json(session_approvals_response).await;
        assert_eq!(session_approvals.items.len(), 1);
        assert_eq!(session_approvals.items[0].approval_id, approval.approval_id);

        let pending_approval_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/approvals/{}", approval.approval_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let pending_approval: ApprovalRequestRecord = read_json(pending_approval_response).await;
        assert_eq!(pending_approval.state, ApprovalState::Pending);

        let waiting_session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let waiting_session: SessionRecord = read_json(waiting_session_response).await;
        assert_eq!(waiting_session.state, SessionState::WaitingApproval);

        let resolve_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", approval.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "approved",
                            "responder": "operator-1",
                            "note": "Approved for this run"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(resolve_response.status(), StatusCode::OK);
        let resolved_approval: ApprovalRequestRecord = read_json(resolve_response).await;
        assert_eq!(resolved_approval.state, ApprovalState::Approved);
        assert_eq!(resolved_approval.responder.as_deref(), Some("operator-1"));

        let resumed_session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let resumed_session: SessionRecord = read_json(resumed_session_response).await;
        assert_eq!(resumed_session.state, SessionState::Running);

        let duplicate_resolution_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/approvals/{}/decision", approval.approval_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "decision": "approved",
                            "responder": "operator-2"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(duplicate_resolution_response.status(), StatusCode::CONFLICT);

        let events_response = app
            .oneshot(
                Request::get("/v1/events?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let events: ListResponse<TimelineEvent> = read_json(events_response).await;
        assert_eq!(events.items.len(), 4);
        assert!(matches!(
            events.items[0].detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));
        assert!(matches!(
            events.items[1].detail,
            TimelineEventDetail::SessionCreated { .. }
        ));
        match &events.items[2].detail {
            TimelineEventDetail::ApprovalRequested {
                approval_id,
                title,
                state,
            } => {
                assert_eq!(*approval_id, approval.approval_id);
                assert_eq!(title, "Run privileged tool");
                assert_eq!(*state, ApprovalState::Pending);
            }
            other => panic!("expected approval requested event, received {other:?}"),
        }
        match &events.items[3].detail {
            TimelineEventDetail::ApprovalResolved {
                approval_id,
                state,
                responder,
            } => {
                assert_eq!(*approval_id, approval.approval_id);
                assert_eq!(*state, ApprovalState::Approved);
                assert_eq!(responder.as_deref(), Some("operator-1"));
            }
            other => panic!("expected approval resolved event, received {other:?}"),
        }
        assert_eq!(events.items[2].session_id, Some(session.session_id));
        assert_eq!(events.items[3].session_id, Some(session.session_id));
        assert_eq!(
            events.items[2].runner_id.as_deref(),
            Some("runner-approval")
        );
        assert_eq!(
            events.items[3].runner_id.as_deref(),
            Some("runner-approval")
        );
    }

    #[tokio::test]
    async fn artifact_endpoints_store_list_and_download_session_outputs() {
        let profile = tempdir().expect("tempdir should exist");
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile.path().join("profile")),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-artifact", "default").await;
        let _ = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&runner.registration).expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let create_response = app
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
        let session: SessionRecord = read_json(create_response).await;

        let artifact_payload = "artifact-bytes-123";
        let create_artifact_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/artifacts", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "session export",
                            "file_name": "export.txt",
                            "media_type": "text/plain",
                            "content_base64": BASE64_STANDARD.encode(artifact_payload),
                            "metadata": {"kind": "export"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_artifact_response.status(), StatusCode::CREATED);
        let artifact: ArtifactRecord = read_json(create_artifact_response).await;
        assert_eq!(artifact.session_id, session.session_id);
        assert_eq!(artifact.runner_id.as_deref(), Some("runner-artifact"));
        assert_eq!(artifact.file_name, "export.txt");
        assert_eq!(artifact.media_type, "text/plain");
        assert_eq!(artifact.size_bytes, artifact_payload.len() as u64);

        let artifacts_response = app
            .clone()
            .oneshot(
                Request::get("/v1/artifacts")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let artifacts: ListResponse<ArtifactRecord> = read_json(artifacts_response).await;
        assert_eq!(artifacts.items.len(), 1);

        let session_artifacts_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}/artifacts", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let session_artifacts: ListResponse<ArtifactRecord> =
            read_json(session_artifacts_response).await;
        assert_eq!(session_artifacts.items.len(), 1);
        assert_eq!(session_artifacts.items[0].artifact_id, artifact.artifact_id);

        let get_artifact_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/artifacts/{}", artifact.artifact_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let fetched: ArtifactRecord = read_json(get_artifact_response).await;
        assert_eq!(fetched.artifact_id, artifact.artifact_id);

        let download_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/artifacts/{}/download", artifact.artifact_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(download_response.status(), StatusCode::OK);
        assert_eq!(
            download_response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/plain")
        );
        let download_body = to_bytes(download_response.into_body(), usize::MAX)
            .await
            .expect("download body should read");
        assert_eq!(download_body.as_ref(), artifact_payload.as_bytes());

        let session_events_response = app
            .oneshot(
                Request::get(format!(
                    "/v1/sessions/{}/events?limit=10",
                    session.session_id
                ))
                .body(Body::empty())
                .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let session_events: ListResponse<TimelineEvent> = read_json(session_events_response).await;
        assert_eq!(session_events.items.len(), 2);
        assert!(matches!(
            session_events.items[0].detail,
            TimelineEventDetail::SessionCreated { .. }
        ));
        assert!(matches!(
            session_events.items[1].detail,
            TimelineEventDetail::ArtifactCreated { .. }
        ));
    }

    #[tokio::test]
    async fn runner_approval_listing_filters_by_runner() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner_a = spawn_runner_server("runner-a", "default").await;
        let runner_z = spawn_runner_server("runner-z", "default").await;
        for registration in [&runner_a.registration, &runner_z.registration] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/v1/runners/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(registration).expect("json should serialize"),
                        ))
                        .expect("request should build"),
                )
                .await
                .expect("request should succeed");
            assert_eq!(response.status(), StatusCode::OK);
        }

        let create_response = app
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
        let session: SessionRecord = read_json(create_response).await;
        assert_eq!(session.owner_runner_id.as_deref(), Some("runner-a"));

        let approval_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/approvals", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "title": "Needs approval",
                            "description": "Confirm tool usage"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(approval_response.status(), StatusCode::CREATED);

        let runner_a_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-a/approvals")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_a_approvals: ListResponse<ApprovalRequestRecord> =
            read_json(runner_a_response).await;
        assert_eq!(runner_a_approvals.items.len(), 1);

        let runner_z_response = app
            .oneshot(
                Request::get("/v1/runners/runner-z/approvals")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_z_approvals: ListResponse<ApprovalRequestRecord> =
            read_json(runner_z_response).await;
        assert!(runner_z_approvals.items.is_empty());
    }

    #[tokio::test]
    async fn runner_approval_stream_only_emits_matching_approval_events() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;
        let ws_url =
            base_url.replacen("http://", "ws://", 1) + "/v1/runners/runner-stream/approvals/stream";

        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let client = Client::new();
        let runner = spawn_runner_server("runner-stream", "default").await;
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner.registration)
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let session: SessionRecord = client
            .post(format!("{base_url}/v1/sessions"))
            .json(&serde_json::json!({"workspace_id": "default"}))
            .send()
            .await
            .expect("session create should succeed")
            .error_for_status()
            .expect("session create should succeed")
            .json()
            .await
            .expect("session payload should decode");

        let approval: ApprovalRequestRecord = client
            .post(format!(
                "{base_url}/v1/sessions/{}/approvals",
                session.session_id
            ))
            .json(&serde_json::json!({
                "title": "Run tool",
                "description": "Needs approval"
            }))
            .send()
            .await
            .expect("approval create should succeed")
            .error_for_status()
            .expect("approval create should succeed")
            .json()
            .await
            .expect("approval payload should decode");

        let requested_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("requested event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let requested_text = match requested_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let requested_event: TimelineEvent =
            serde_json::from_str(&requested_text).expect("event payload should deserialize");
        assert_eq!(requested_event.runner_id.as_deref(), Some("runner-stream"));
        assert_eq!(requested_event.session_id, Some(session.session_id));
        assert!(matches!(
            requested_event.detail,
            TimelineEventDetail::ApprovalRequested { .. }
        ));

        let response = client
            .post(format!(
                "{base_url}/v1/approvals/{}/decision",
                approval.approval_id
            ))
            .json(&serde_json::json!({
                "decision": "approved",
                "responder": "stream-tester"
            }))
            .send()
            .await
            .expect("approval resolve should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let resolved_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("resolved event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let resolved_text = match resolved_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let resolved_event: TimelineEvent =
            serde_json::from_str(&resolved_text).expect("event payload should deserialize");
        assert_eq!(resolved_event.runner_id.as_deref(), Some("runner-stream"));
        assert_eq!(resolved_event.session_id, Some(session.session_id));
        assert!(matches!(
            resolved_event.detail,
            TimelineEventDetail::ApprovalResolved { .. }
        ));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn session_event_stream_only_emits_matching_session_events() {
        let profile = tempdir().expect("tempdir should exist");
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile.path().join("profile")),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;

        let client = Client::new();
        let runner = spawn_runner_server("runner-session-stream", "default").await;
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner.registration)
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let session: SessionRecord = client
            .post(format!("{base_url}/v1/sessions"))
            .json(&serde_json::json!({"workspace_id": "default"}))
            .send()
            .await
            .expect("session create should succeed")
            .error_for_status()
            .expect("session create should succeed")
            .json()
            .await
            .expect("session payload should decode");

        let ws_url = base_url.replacen("http://", "ws://", 1)
            + &format!("/v1/sessions/{}/events/stream", session.session_id);
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let artifact_response = client
            .post(format!(
                "{base_url}/v1/sessions/{}/artifacts",
                session.session_id
            ))
            .json(&serde_json::json!({
                "name": "notes",
                "file_name": "notes.txt",
                "media_type": "text/plain",
                "content_base64": BASE64_STANDARD.encode("hello session stream")
            }))
            .send()
            .await
            .expect("artifact create should succeed");
        assert_eq!(artifact_response.status(), StatusCode::CREATED);

        let message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let text = match message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let event: TimelineEvent =
            serde_json::from_str(&text).expect("event payload should deserialize");
        assert_eq!(event.session_id, Some(session.session_id));
        assert!(matches!(
            event.detail,
            TimelineEventDetail::ArtifactCreated { .. }
        ));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn websocket_stream_receives_live_runner_events() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides::default())
                .expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;
        let ws_url = base_url.replacen("http://", "ws://", 1) + "/v1/events/stream";

        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let registration = runner_registration("runner-live", "default", "C:/workspace");
        let client = Client::new();
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&registration)
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let text = match message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let event: TimelineEvent =
            serde_json::from_str(&text).expect("event payload should deserialize");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.runner_id.as_deref(), Some("runner-live"));
        assert!(matches!(
            event.detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));

        server_handle.abort();
        let _ = server_handle.await;
    }

    async fn spawn_runner_server(runner_id: &str, workspace_id: &str) -> SpawnedRunner {
        let profile = tempdir().expect("tempdir should exist");
        let workspace_root = profile.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir should exist");
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let public_base_url = format!("http://{address}");
        let config = load_runner_config(
            Some(profile.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some(runner_id.to_owned()),
                public_base_url: Some(public_base_url.clone()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: workspace_id.to_owned(),
                    root_dir: workspace_root,
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let registration = config.registration_request();
        let api = RunnerApi::new(config, "remote-code-runner", "0.1.0");
        let server = {
            let app = api.clone().router();
            tokio::spawn(async move {
                axum::serve(listener, app).await.expect("server should run");
            })
        };
        SpawnedRunner {
            api,
            registration,
            _profile: profile,
            server,
        }
    }

    fn runner_registration(
        runner_id: &str,
        workspace_id: &str,
        root_dir: &str,
    ) -> RunnerRegistrationRequest {
        RunnerRegistrationRequest {
            runner_id: runner_id.to_owned(),
            control_plane_url: Some("http://127.0.0.1:8787".to_owned()),
            public_base_url: Some("http://127.0.0.1:9900".to_owned()),
            workspaces: vec![RunnerWorkspace {
                workspace_id: workspace_id.to_owned(),
                root_dir: root_dir.into(),
                writable: true,
            }],
            labels: BTreeMap::from([(String::from("region"), String::from("test"))]),
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
        }
    }

    async fn read_json<T>(response: axum::response::Response) -> T
    where
        T: DeserializeOwned,
    {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        serde_json::from_slice(&body).expect("json should parse")
    }

    async fn spawn_control_plane_server(service: ControlPlaneService) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let server = tokio::spawn(async move {
            axum::serve(listener, service.router())
                .await
                .expect("server should run");
        });
        (format!("http://{address}"), server)
    }
}
