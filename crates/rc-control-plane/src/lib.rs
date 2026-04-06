use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{
        Path as AxumPath, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use futures::SinkExt;
use rc_runner::{
    ListResponse, RunnerHeartbeat, RunnerRegistrationRequest, RunnerSnapshot, RunnerState,
};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    pub bind: SocketAddr,
    pub public_base_url: Option<String>,
    pub service_name: String,
    pub runner_lease_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneMeta {
    pub service: String,
    pub version: String,
    pub phase: String,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub runner_lease_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneStatus {
    pub ok: bool,
    pub bind: String,
    pub public_base_url: Option<String>,
    pub service_name: String,
    pub runner_lease_ttl_secs: u64,
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
    registry: Arc<RwLock<Registry>>,
    timeline: TimelineStore,
}

#[derive(Debug, Default)]
struct Registry {
    runners: BTreeMap<String, RunnerSnapshot>,
    sessions: BTreeMap<Uuid, SessionRecord>,
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
        Self {
            meta: ControlPlaneMeta {
                service: service_name,
                version: version.into(),
                phase: PHASE.to_owned(),
                bind: config.bind.to_string(),
                public_base_url: config.public_base_url,
                runner_lease_ttl_secs: config.runner_lease_ttl_secs,
            },
            runner_lease_ttl_secs: config.runner_lease_ttl_secs,
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
            .route("/v1/runners", get(list_runners))
            .route("/v1/runners/register", post(register_runner))
            .route("/v1/runners/{runner_id}", get(get_runner))
            .route(
                "/v1/runners/{runner_id}/heartbeat",
                post(update_runner_heartbeat),
            )
            .route("/v1/sessions", get(list_sessions).post(create_session))
            .route("/v1/sessions/{session_id}", get(get_session))
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
        let limit = limit
            .unwrap_or(DEFAULT_EVENT_LIST_LIMIT)
            .clamp(1, MAX_EVENT_LIST_LIMIT);
        let timeline = self.inner.lock().await;
        let mut events = timeline
            .history
            .iter()
            .filter(|event| after.is_none_or(|sequence| event.sequence > sequence))
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

    fn create_session(
        &mut self,
        request: CreateSessionRequest,
        lease_ttl_secs: u64,
    ) -> Result<SessionRecord, ApiError> {
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
        let session_id = request.session_id.unwrap_or_else(Uuid::new_v4);
        let record = SessionRecord {
            session_id,
            workspace_id: request.workspace_id,
            owner_runner_id: owner_runner_id.clone(),
            state,
            metadata: request.metadata,
            created_at: now,
            updated_at: now,
        };
        self.sessions.insert(session_id, record.clone());
        if let Some(runner_id) = owner_runner_id
            && let Some(snapshot) = self.runners.get_mut(&runner_id)
        {
            snapshot.active_sessions += 1;
            snapshot.state = RunnerState::Busy;
            snapshot.last_seen_at = now;
        }
        Ok(record)
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

    Ok(ControlPlaneConfig {
        bind,
        public_base_url,
        service_name,
        runner_lease_ttl_secs,
    })
}

pub fn describe_status(config: &ControlPlaneConfig) -> ControlPlaneStatus {
    ControlPlaneStatus {
        ok: true,
        bind: config.bind.to_string(),
        public_base_url: config.public_base_url.clone(),
        service_name: config.service_name.clone(),
        runner_lease_ttl_secs: config.runner_lease_ttl_secs,
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

async fn subscribe_events(
    ws: WebSocketUpgrade,
    State(service): State<ControlPlaneService>,
) -> Response {
    ws.on_upgrade(move |socket| serve_event_stream(socket, service.timeline.subscribe()))
}

async fn list_runners(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<RunnerSnapshot>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.runners.values().cloned().collect(),
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

async fn create_session(
    State(service): State<ControlPlaneService>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionRecord>), ApiError> {
    let record = {
        let mut registry = service.registry.write().await;
        registry.create_session(request, service.runner_lease_ttl_secs)?
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

async fn serve_event_stream(
    mut socket: WebSocket,
    mut subscription: broadcast::Receiver<TimelineEvent>,
) {
    loop {
        let event = match subscription.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                let _ = socket.close().await;
                break;
            }
        };
        let payload = match serde_json::to_string(&event) {
            Ok(payload) => payload,
            Err(_) => break,
        };
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break;
        }
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
        body::{Body, to_bytes},
        http::Request,
    };
    use futures::StreamExt;
    use rc_runner::{RunnerCapabilities, RunnerPlatform, RunnerWorkspace};
    use reqwest::Client;
    use serde::de::DeserializeOwned;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::{Duration as TokioDuration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
    use tower::ServiceExt;

    #[test]
    fn control_plane_config_uses_overrides() {
        let config = load_control_plane_config(ControlPlaneConfigOverrides {
            bind: Some(SocketAddr::from_str("127.0.0.1:9898").expect("bind should parse")),
            public_base_url: Some("http://127.0.0.1:9898".to_owned()),
            service_name: Some("rc-control".to_owned()),
            runner_lease_ttl_secs: Some(45),
        })
        .expect("config should load");

        assert_eq!(config.bind.to_string(), "127.0.0.1:9898");
        assert_eq!(config.service_name, "rc-control");
        assert_eq!(config.runner_lease_ttl_secs, 45);
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

        let registration = runner_registration("runner-a", "default", "C:/workspace");
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

        let registration = runner_registration("runner-c", "default", "C:/workspace");
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
