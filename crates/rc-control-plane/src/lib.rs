//! Control plane service for orchestrating remote runners and sessions.
//!
//! The control plane manages runner registration, session dispatch, approval
//! relay, artifact storage, and real-time event streaming via WebSocket / SSE.

mod handlers;
mod helpers;
mod registry;
mod streams;
mod types;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{
    Method,
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use rc_config::AppPaths;
use rusqlite::Row;
use rusqlite::params_from_iter;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use handlers::{
    accept_pairing_offer, apply_approval_decision, claim_bootstrap_device, create_approval,
    create_artifact, create_pairing_offer, create_session, create_session_runtime_event,
    download_artifact, get_approval, get_artifact, get_health, get_meta, get_runner, get_session,
    list_approvals, list_artifacts, list_devices, list_recent_events, list_runner_approvals,
    list_runner_artifacts, list_runner_events, list_runner_sessions, list_runners,
    list_session_approvals, list_session_artifacts, list_session_events, list_sessions,
    post_session_command, pull_runner_commands, register_push_token, register_runner,
    subscribe_approvals, subscribe_events, subscribe_runner_approvals, subscribe_runner_events,
    subscribe_session_approvals, subscribe_session_events, update_runner_heartbeat,
    update_session_state,
};
use helpers::{event_kind_name, event_kind_name_for_detail, is_approval_kind};
use registry::{Registry, TimelineSnapshot, TimelineStore};
use types::{
    DEFAULT_BIND, DEFAULT_EVENT_HISTORY_LIMIT, DEFAULT_RUNNER_LEASE_TTL_SECS, EVENT_STREAM_BUFFER,
    MAX_EVENT_LIST_LIMIT, PHASE, TimelineEventDraft, TimelineEventKind,
};

// ---------------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------------

pub use rc_runner::{RunnerSessionCommandRequest, RunnerSessionCommandResponse};
pub use types::{
    ArtifactCreateRequest, ArtifactRecord, BootstrapClaimRequest, BootstrapClaimResponse,
    ControlPlaneConfig, ControlPlaneConfigOverrides, ControlPlaneHealth, ControlPlaneMeta,
    ControlPlaneStatus, CreateSessionRequest, DaemonPresenceState, DeviceKind, MessageRole,
    PairingAcceptRequest, PairingAcceptResponse, PairingOfferCreateRequest,
    PairingOfferCreateResponse, PushPlatform, PushTokenRegistrationRequest,
    PushTokenRegistrationResponse, RunnerCommandPullResponse, RunnerQueuedCommand,
    RunnerQueuedCommandBody, RunnerRegistrationResponse, RuntimeEventCreateRequest,
    RuntimeEventDetail, SessionRecord, SessionState, SessionStateUpdateRequest, TimelineEvent,
    TimelineEventDetail, TrustedDeviceRecord, runtime_event_detail_from_stream_json_value,
};

// ---------------------------------------------------------------------------
// ControlPlaneService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ControlPlaneService {
    meta: ControlPlaneMeta,
    runner_lease_ttl_secs: u64,
    state_db_path: PathBuf,
    artifact_root_dir: PathBuf,
    auth_token: Option<String>,
    bootstrap_secret_hash: Option<String>,
    registry: Arc<RwLock<Registry>>,
    timeline: TimelineStore,
}

#[derive(Debug, Clone)]
pub(crate) enum AuthPrincipal {
    SharedToken,
    Device(TrustedDeviceRecord),
}

impl AuthPrincipal {
    pub(crate) fn created_by_device_id(&self) -> Option<Uuid> {
        match self {
            Self::SharedToken => None,
            Self::Device(device) => Some(device.device_id),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedControlPlaneState {
    registry: Registry,
    timeline: TimelineSnapshot,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PersistedEventQuery {
    pub(crate) after: Option<u64>,
    pub(crate) limit: Option<usize>,
    pub(crate) kind: Option<TimelineEventKind>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) runner_id: Option<String>,
    pub(crate) approvals_only: bool,
}

impl ControlPlaneService {
    pub fn new(config: ControlPlaneConfig, version: impl Into<String>) -> Self {
        let service_name = config.service_name.clone();
        let state_db_path = config.state_db_path.clone();
        let artifact_root_dir = config.artifact_root_dir.clone();
        let auth_token = config.auth_token.clone();
        let bootstrap_secret_hash = config.bootstrap_secret.as_deref().map(hash_secret_value);
        let (registry, timeline) = load_persisted_state(&state_db_path).unwrap_or_else(|_| {
            (
                Registry::default(),
                TimelineStore::new(DEFAULT_EVENT_HISTORY_LIMIT, EVENT_STREAM_BUFFER),
            )
        });
        let auth_required =
            auth_token.is_some() || bootstrap_secret_hash.is_some() || registry.owner_claimed();
        Self {
            meta: ControlPlaneMeta {
                service: service_name,
                version: version.into(),
                phase: PHASE.to_owned(),
                bind: config.bind.to_string(),
                public_base_url: config.public_base_url,
                runner_lease_ttl_secs: config.runner_lease_ttl_secs,
                profile_dir: config.profile_dir.display().to_string(),
                state_db_path: state_db_path.display().to_string(),
                artifact_root_dir: artifact_root_dir.display().to_string(),
                auth_required,
                bootstrap_secret_configured: bootstrap_secret_hash.is_some(),
            },
            runner_lease_ttl_secs: config.runner_lease_ttl_secs,
            state_db_path,
            artifact_root_dir,
            auth_token,
            bootstrap_secret_hash,
            registry: Arc::new(RwLock::new(registry)),
            timeline,
        }
    }

    #[must_use]
    pub fn meta(&self) -> &ControlPlaneMeta {
        &self.meta
    }

    pub(crate) async fn auth_required(&self) -> bool {
        if self.auth_token.is_some() || self.bootstrap_secret_hash.is_some() {
            return true;
        }
        self.registry.read().await.owner_claimed()
    }

    pub(crate) async fn list_persisted_events(
        &self,
        query: PersistedEventQuery,
    ) -> Result<Vec<TimelineEvent>> {
        let state_db_path = self.state_db_path.clone();
        tokio::task::spawn_blocking(move || load_persisted_events(&state_db_path, &query))
            .await
            .context("control-plane event query task failed to join")?
    }

    pub(crate) async fn latest_persisted_event_sequence(
        &self,
        query: PersistedEventQuery,
    ) -> Result<Option<u64>> {
        let state_db_path = self.state_db_path.clone();
        tokio::task::spawn_blocking(move || {
            load_latest_persisted_event_sequence(&state_db_path, &query)
        })
        .await
        .context("control-plane latest event query task failed to join")?
    }

    pub fn router(self) -> Router {
        let protected = Router::new()
            .route("/v1/meta", get(get_meta))
            .route("/v1/devices", get(list_devices))
            .route("/v1/events", get(list_recent_events))
            .route("/v1/events/stream", get(subscribe_events))
            .route(
                "/v1/sessions/{session_id}/events/stream",
                get(subscribe_session_events),
            )
            .route("/v1/runners/{runner_id}/events", get(list_runner_events))
            .route(
                "/v1/runners/{runner_id}/events/stream",
                get(subscribe_runner_events),
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
                "/v1/runners/{runner_id}/artifacts",
                get(list_runner_artifacts),
            )
            .route(
                "/v1/runners/{runner_id}/sessions",
                get(list_runner_sessions),
            )
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
            .route(
                "/v1/runners/{runner_id}/commands/pull",
                post(pull_runner_commands),
            )
            .route("/v1/devices/push-token", post(register_push_token))
            .route("/v1/pairing/offers", post(create_pairing_offer))
            .route("/v1/sessions", get(list_sessions).post(create_session))
            .route("/v1/sessions/{session_id}", get(get_session))
            .route(
                "/v1/sessions/{session_id}/state",
                post(update_session_state),
            )
            .route(
                "/v1/sessions/{session_id}/commands",
                post(post_session_command),
            )
            .route(
                "/v1/sessions/{session_id}/events",
                get(list_session_events).post(create_session_runtime_event),
            )
            .route(
                "/v1/sessions/{session_id}/approvals",
                get(list_session_approvals).post(create_approval),
            )
            .route(
                "/v1/sessions/{session_id}/approvals/stream",
                get(subscribe_session_approvals),
            )
            .route(
                "/v1/sessions/{session_id}/artifacts",
                get(list_session_artifacts).post(create_artifact),
            )
            .route_layer(middleware::from_fn_with_state(
                self.clone(),
                require_api_auth,
            ));

        Router::new()
            .route("/healthz", get(get_health))
            .route("/v1/bootstrap/claim", post(claim_bootstrap_device))
            .route("/v1/pairing/accept", post(accept_pairing_offer))
            .merge(protected)
            .layer(build_cors_layer())
            .with_state(self)
    }

    async fn publish_event(&self, draft: TimelineEventDraft) -> types::TimelineEvent {
        let event = self.timeline.publish(draft).await;
        let snapshot = PersistedControlPlaneState {
            registry: self.registry.read().await.clone(),
            timeline: self.timeline.snapshot().await,
        };
        let state_db_path = self.state_db_path.clone();
        let event_to_persist = event.clone();
        let _ = tokio::task::spawn_blocking(move || {
            persist_control_plane_state(&state_db_path, &snapshot, Some(&event_to_persist))
        })
        .await;
        event
    }

    pub(crate) async fn persist_state(&self) -> Result<()> {
        let snapshot = PersistedControlPlaneState {
            registry: self.registry.read().await.clone(),
            timeline: self.timeline.snapshot().await,
        };
        let state_db_path = self.state_db_path.clone();
        tokio::task::spawn_blocking(move || {
            persist_control_plane_state(&state_db_path, &snapshot, None)
        })
        .await
        .context("control-plane snapshot task failed to join")??;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public configuration helpers
// ---------------------------------------------------------------------------

pub fn load_control_plane_config(
    overrides: ControlPlaneConfigOverrides,
) -> Result<ControlPlaneConfig> {
    let bind = match overrides.bind {
        Some(bind) => bind,
        None => helpers::parse_socket_addr(
            &helpers::read_env("REMOTE_CODE_CONTROL_PLANE_BIND")
                .unwrap_or_else(|| DEFAULT_BIND.to_owned()),
        )?,
    };
    let public_base_url = overrides
        .public_base_url
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_PUBLIC_BASE_URL"));
    let service_name = overrides
        .service_name
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_SERVICE_NAME"))
        .unwrap_or_else(|| "remote-code-control-plane".to_owned());
    let runner_lease_ttl_secs = overrides
        .runner_lease_ttl_secs
        .or_else(|| helpers::parse_env_number("REMOTE_CODE_RUNNER_LEASE_TTL_SECS"))
        .unwrap_or(DEFAULT_RUNNER_LEASE_TTL_SECS)
        .max(1);
    let profile_dir = overrides
        .profile_dir
        .or_else(|| helpers::read_env("REMOTE_CODE_PROFILE_DIR").map(PathBuf::from));
    let auth_token = overrides
        .auth_token
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN"));
    let bootstrap_secret = overrides
        .bootstrap_secret
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET"))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
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
        state_db_path: paths.state_db_path,
        artifact_root_dir,
        auth_token,
        bootstrap_secret,
    })
}

#[must_use]
pub fn describe_status(config: &ControlPlaneConfig) -> ControlPlaneStatus {
    let issues = validate_control_plane_config(config);
    ControlPlaneStatus {
        ok: issues.is_empty(),
        issues,
        bind: config.bind.to_string(),
        public_base_url: config.public_base_url.clone(),
        service_name: config.service_name.clone(),
        runner_lease_ttl_secs: config.runner_lease_ttl_secs,
        profile_dir: config.profile_dir.display().to_string(),
        state_db_path: config.state_db_path.display().to_string(),
        artifact_root_dir: config.artifact_root_dir.display().to_string(),
        auth_required: config.auth_token.is_some() || config.bootstrap_secret.is_some(),
        bootstrap_secret_configured: config.bootstrap_secret.is_some(),
        phase: PHASE,
    }
}

fn validate_control_plane_config(config: &ControlPlaneConfig) -> Vec<String> {
    let mut issues = Vec::new();
    let auth_configured = config.auth_token.is_some() || config.bootstrap_secret.is_some();
    let public_url = config.public_base_url.as_deref();
    let remote_public_url = public_url.filter(|url| !is_local_control_plane_url(url));

    if !config.bind.ip().is_loopback() && !auth_configured {
        issues.push(
            "non-loopback control-plane binds require REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN or REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET".to_owned(),
        );
    }

    if remote_public_url.is_some() && !auth_configured {
        issues.push(
            "remote public_base_url requires REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN or REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET".to_owned(),
        );
    }

    if let Some(url) = remote_public_url
        && !url.starts_with("https://")
    {
        issues.push("remote public_base_url must use https".to_owned());
    }

    issues
}

fn is_local_control_plane_url(raw: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(raw) else {
        return false;
    };

    match parsed.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => false,
    }
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .expose_headers([CONTENT_DISPOSITION, CONTENT_TYPE])
}

fn hash_secret_value(raw: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        #[allow(clippy::format_push_string)]
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

async fn require_api_auth(
    State(service): State<ControlPlaneService>,
    mut request: Request,
    next: Next,
) -> axum::response::Response {
    if !service.auth_required().await {
        return next.run(request).await;
    }

    let Some(provided) = extract_request_auth_token(&request) else {
        return types::ApiError::unauthorized(
            "missing or invalid control plane bearer token".to_owned(),
        )
        .into_response();
    };

    if service.auth_token.as_deref() == Some(provided.as_str()) {
        request.extensions_mut().insert(AuthPrincipal::SharedToken);
        return next.run(request).await;
    }

    let authenticated_device = {
        let mut registry = service.registry.write().await;
        registry.authenticate_device_token(&provided)
    };
    if let Some(device) = authenticated_device {
        request
            .extensions_mut()
            .insert(AuthPrincipal::Device(device));
        return next.run(request).await;
    }

    types::ApiError::unauthorized("missing or invalid control plane bearer token".to_owned())
        .into_response()
}

fn extract_request_auth_token(request: &Request) -> Option<String> {
    let bearer = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if bearer.is_some() {
        return bearer;
    }
    request
        .uri()
        .query()
        .and_then(extract_auth_token_from_query)
}

fn extract_auth_token_from_query(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next().unwrap_or_default().trim();
        if matches!(key, "token" | "access_token") && !value.is_empty() {
            return Some(percent_decode_query_value(value));
        }
    }
    None
}

fn percent_decode_query_value(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let high = bytes[index + 1] as char;
                let low = bytes[index + 2] as char;
                if let (Some(high), Some(low)) = (high.to_digit(16), low.to_digit(16)) {
                    decoded.push(((high << 4) | low) as u8);
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn load_persisted_state(state_db_path: &std::path::Path) -> Result<(Registry, TimelineStore)> {
    let connection = open_state_connection(state_db_path)?;
    let payload = connection
        .query_row(
            "SELECT payload FROM control_plane_snapshot WHERE id = 1",
            [],
            |row: &Row<'_>| row.get::<_, String>(0),
        )
        .optional()
        .with_context(|| format!("failed to read {}", state_db_path.display()))?;
    let Some(payload) = payload else {
        return Ok((
            Registry::default(),
            TimelineStore::new(DEFAULT_EVENT_HISTORY_LIMIT, EVENT_STREAM_BUFFER),
        ));
    };
    let snapshot: PersistedControlPlaneState = serde_json::from_str(&payload)
        .with_context(|| format!("failed to decode {}", state_db_path.display()))?;
    backfill_persisted_events(&connection, &snapshot.timeline)
        .with_context(|| format!("failed to backfill events in {}", state_db_path.display()))?;
    Ok((
        snapshot.registry,
        TimelineStore::from_snapshot(
            DEFAULT_EVENT_HISTORY_LIMIT,
            EVENT_STREAM_BUFFER,
            snapshot.timeline,
        ),
    ))
}

fn persist_control_plane_state(
    state_db_path: &std::path::Path,
    snapshot: &PersistedControlPlaneState,
    event: Option<&TimelineEvent>,
) -> Result<()> {
    let mut connection = open_state_connection(state_db_path)?;
    let transaction = connection.transaction()?;
    let payload = serde_json::to_string(snapshot)
        .with_context(|| format!("failed to encode {}", state_db_path.display()))?;
    transaction
        .execute(
            "INSERT INTO control_plane_snapshot (id, payload, updated_at)
             VALUES (1, ?1, CURRENT_TIMESTAMP)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = CURRENT_TIMESTAMP",
            params![payload],
        )
        .with_context(|| format!("failed to persist {}", state_db_path.display()))?;
    if let Some(event) = event {
        persist_timeline_event(&transaction, event).with_context(|| {
            format!(
                "failed to persist event {} in {}",
                event.sequence,
                state_db_path.display()
            )
        })?;
    }
    transaction.commit()?;
    Ok(())
}

fn open_state_connection(state_db_path: &std::path::Path) -> Result<Connection> {
    let connection = Connection::open(state_db_path)
        .with_context(|| format!("failed to open {}", state_db_path.display()))?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS control_plane_snapshot (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            payload TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS control_plane_events (
            sequence INTEGER PRIMARY KEY,
            recorded_at TEXT NOT NULL,
            runner_id TEXT,
            session_id TEXT,
            kind TEXT NOT NULL,
            is_approval INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_control_plane_events_session_sequence
            ON control_plane_events(session_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_control_plane_events_runner_sequence
            ON control_plane_events(runner_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_control_plane_events_kind_sequence
            ON control_plane_events(kind, sequence);
        CREATE INDEX IF NOT EXISTS idx_control_plane_events_approval_sequence
            ON control_plane_events(is_approval, sequence);",
    )?;
    Ok(connection)
}

fn persist_timeline_event(connection: &Connection, event: &TimelineEvent) -> Result<()> {
    let payload = serde_json::to_string(event)?;
    let kind = crate::helpers::event_kind(&event.detail);
    connection.execute(
        "INSERT OR IGNORE INTO control_plane_events (
            sequence,
            recorded_at,
            runner_id,
            session_id,
            kind,
            is_approval,
            payload
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            i64::try_from(event.sequence)?,
            event.recorded_at.to_rfc3339(),
            event.runner_id.as_deref(),
            event.session_id.map(|value| value.to_string()),
            event_kind_name_for_detail(&event.detail),
            i64::from(is_approval_kind(kind)),
            payload,
        ],
    )?;
    Ok(())
}

fn backfill_persisted_events(connection: &Connection, snapshot: &TimelineSnapshot) -> Result<()> {
    for event in snapshot.history() {
        persist_timeline_event(connection, event)?;
    }
    Ok(())
}

fn load_persisted_events(
    state_db_path: &std::path::Path,
    query: &PersistedEventQuery,
) -> Result<Vec<TimelineEvent>> {
    let connection = open_state_connection(state_db_path)?;
    let mut params: Vec<SqlValue> = Vec::new();
    let mut clauses = Vec::new();
    if let Some(after) = query.after {
        clauses.push("sequence > ?".to_owned());
        params.push(SqlValue::Integer(i64::try_from(after)?));
    }
    if let Some(session_id) = query.session_id {
        clauses.push("session_id = ?".to_owned());
        params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(runner_id) = query.runner_id.as_deref() {
        clauses.push("runner_id = ?".to_owned());
        params.push(SqlValue::Text(runner_id.to_owned()));
    }
    if let Some(kind) = query.kind {
        clauses.push("kind = ?".to_owned());
        params.push(SqlValue::Text(event_kind_name(kind).to_owned()));
    }
    if query.approvals_only {
        clauses.push("is_approval = 1".to_owned());
    }

    let mut sql = String::from("SELECT payload FROM control_plane_events");
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY sequence DESC");

    if let Some(limit) = query.limit {
        sql.push_str(" LIMIT ?");
        params.push(SqlValue::Integer(i64::try_from(
            limit.clamp(1, MAX_EVENT_LIST_LIMIT),
        )?));
    }

    let mut statement = connection.prepare(&sql)?;
    let payloads = statement.query_map(params_from_iter(params), |row| row.get::<_, String>(0))?;
    let mut events = Vec::new();
    for payload in payloads {
        let payload = payload?;
        let event = serde_json::from_str::<TimelineEvent>(&payload)?;
        events.push(event);
    }
    events.reverse();
    Ok(events)
}

fn load_latest_persisted_event_sequence(
    state_db_path: &std::path::Path,
    query: &PersistedEventQuery,
) -> Result<Option<u64>> {
    let connection = open_state_connection(state_db_path)?;
    let mut params: Vec<SqlValue> = Vec::new();
    let mut clauses = Vec::new();
    if let Some(session_id) = query.session_id {
        clauses.push("session_id = ?".to_owned());
        params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(runner_id) = query.runner_id.as_deref() {
        clauses.push("runner_id = ?".to_owned());
        params.push(SqlValue::Text(runner_id.to_owned()));
    }
    if let Some(kind) = query.kind {
        clauses.push("kind = ?".to_owned());
        params.push(SqlValue::Text(event_kind_name(kind).to_owned()));
    }
    if query.approvals_only {
        clauses.push("is_approval = 1".to_owned());
    }

    let mut sql = String::from("SELECT sequence FROM control_plane_events");
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY sequence DESC LIMIT 1");
    let sequence = connection
        .query_row(&sql, params_from_iter(params), |row| row.get::<_, i64>(0))
        .optional()?;
    sequence
        .map(u64::try_from)
        .transpose()
        .context("persisted event sequence overflowed u64")
}
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::SocketAddr;
    use std::str::FromStr;

    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
        http::StatusCode,
        http::header::{AUTHORIZATION, CONTENT_TYPE},
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use chrono::Utc;
    use futures::StreamExt;
    use rc_runner::{
        ApprovalRequestRecord, ApprovalState, ListResponse, RunnerApi, RunnerCapabilities,
        RunnerConfigOverrides, RunnerHeartbeat, RunnerPlatform, RunnerRegistrationRequest,
        RunnerSessionCommandRequest, RunnerSessionCommandResponse, RunnerSnapshot, RunnerState,
        RunnerWorkspace, SessionState as RunnerSessionState, load_runner_config,
    };
    use reqwest::Client;
    use serde::{Deserialize, de::DeserializeOwned};
    use tempfile::{TempDir, tempdir};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::{Duration as TokioDuration, timeout};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
    use tower::ServiceExt;
    use uuid::Uuid;

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

    #[derive(Debug, Deserialize)]
    struct ApiSessionView {
        #[serde(flatten)]
        session: SessionRecord,
        owner_runner_available: bool,
        owner_runner_state: Option<RunnerState>,
        owner_runner_last_seen_at: Option<chrono::DateTime<Utc>>,
    }

    fn isolated_test_overrides() -> ControlPlaneConfigOverrides {
        let profile = tempdir().expect("tempdir should exist");
        ControlPlaneConfigOverrides {
            profile_dir: Some(profile.keep().join("profile")),
            ..ControlPlaneConfigOverrides::default()
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
            auth_token: None,
            bootstrap_secret: None,
        })
        .expect("config should load");

        assert_eq!(config.bind.to_string(), "127.0.0.1:9898");
        assert_eq!(config.service_name, "rc-control");
        assert_eq!(config.runner_lease_ttl_secs, 45);
        assert!(config.artifact_root_dir.ends_with("control-plane"));
        assert!(describe_status(&config).ok);
    }

    #[test]
    fn remote_public_config_requires_auth() {
        let profile = tempdir().expect("tempdir should exist");
        let config = load_control_plane_config(ControlPlaneConfigOverrides {
            bind: Some(SocketAddr::from_str("127.0.0.1:9898").expect("bind should parse")),
            public_base_url: Some("https://remote.example.com".to_owned()),
            profile_dir: Some(profile.path().join("profile")),
            auth_token: None,
            bootstrap_secret: None,
            ..ControlPlaneConfigOverrides::default()
        })
        .expect("config should load");

        let status = describe_status(&config);
        assert!(!status.ok);
        assert!(status.issues.iter().any(|issue| issue.contains("requires")));
    }

    #[test]
    fn remote_public_config_requires_https() {
        let profile = tempdir().expect("tempdir should exist");
        let config = load_control_plane_config(ControlPlaneConfigOverrides {
            bind: Some(SocketAddr::from_str("127.0.0.1:9898").expect("bind should parse")),
            public_base_url: Some("http://remote.example.com".to_owned()),
            profile_dir: Some(profile.path().join("profile")),
            auth_token: Some("test-secret".to_owned()),
            ..ControlPlaneConfigOverrides::default()
        })
        .expect("config should load");

        let status = describe_status(&config);
        assert!(!status.ok);
        assert!(
            status
                .issues
                .iter()
                .any(|issue| issue.contains("must use https"))
        );
    }

    #[tokio::test]
    async fn control_plane_requires_auth_for_http_and_accepts_header_or_query_token_for_websocket()
    {
        let profile = tempdir().expect("tempdir should exist");
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile.path().join("profile")),
                auth_token: Some("test-secret".to_owned()),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/v1/meta")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let authorized = app
            .clone()
            .oneshot(
                Request::get("/v1/meta")
                    .header(AUTHORIZATION, "Bearer test-secret")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(authorized.status(), StatusCode::OK);

        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile.path().join("profile-ws")),
                auth_token: Some("test-secret".to_owned()),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;
        let ws_url = base_url.replacen("http://", "ws://", 1) + "/v1/events/stream";
        let mut ws_request = ws_url
            .into_client_request()
            .expect("websocket request should build");
        ws_request.headers_mut().insert(
            AUTHORIZATION,
            "Bearer test-secret".parse().expect("header should parse"),
        );

        let (mut socket, _) = connect_async(ws_request)
            .await
            .expect("authenticated websocket should connect");
        let query_ws_url =
            base_url.replacen("http://", "ws://", 1) + "/v1/events/stream?access_token=test-secret";
        let (query_socket, _) = connect_async(&query_ws_url)
            .await
            .expect("query websocket should remain supported for browser clients");
        drop(query_socket);

        let client = Client::new();
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .bearer_auth("test-secret")
            .json(&runner_registration(
                "runner-auth-stream",
                "default",
                "C:/workspace/auth-stream",
            ))
            .send()
            .await
            .expect("registration should succeed");
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
        assert_eq!(event.runner_id.as_deref(), Some("runner-auth-stream"));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn control_plane_persists_sessions_and_runner_pull_commands() {
        let profile = tempdir().expect("tempdir should exist");
        let profile_dir = profile.path().join("profile");

        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile_dir.clone()),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerRegistrationRequest {
                            public_base_url: None,
                            ..runner_registration("runner-pull", "default", "C:/workspace/pull")
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("register request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CreateSessionRequest {
                            session_id: Some(Uuid::nil()),
                            workspace_id: "default".to_owned(),
                            preferred_runner_id: Some("runner-pull".to_owned()),
                            metadata: BTreeMap::new(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("create request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;
        assert_eq!(session.owner_runner_id.as_deref(), Some("runner-pull"));

        let command_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/commands", session.session_id))
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerSessionCommandRequest::SendPrompt {
                            content: "queued over pull".to_owned(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("command request should succeed");
        assert_eq!(command_response.status(), StatusCode::OK);

        drop(app);

        let restored = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile_dir),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let restored_app = restored.router();

        let restored_session_response = restored_app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("session request should succeed");
        assert_eq!(restored_session_response.status(), StatusCode::OK);
        let restored_session: SessionRecord = read_json(restored_session_response).await;
        assert_eq!(restored_session.session_id, session.session_id);

        let pull_response = restored_app
            .clone()
            .oneshot(
                Request::post("/v1/runners/runner-pull/commands/pull?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("pull request should succeed");
        assert_eq!(pull_response.status(), StatusCode::OK);
        let pulled: RunnerCommandPullResponse = read_json(pull_response).await;
        assert_eq!(pulled.commands.len(), 2);
        assert!(
            pulled.commands.iter().any(|command| matches!(
                command.body,
                RunnerQueuedCommandBody::CreateSession { .. }
            ))
        );
        assert!(
            pulled.commands.iter().any(|command| matches!(
                command.body,
                RunnerQueuedCommandBody::SessionCommand { .. }
            ))
        );

        let second_pull = restored_app
            .oneshot(
                Request::post("/v1/runners/runner-pull/commands/pull?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("second pull request should succeed");
        assert_eq!(second_pull.status(), StatusCode::OK);
        let second_pulled: RunnerCommandPullResponse = read_json(second_pull).await;
        assert!(second_pulled.commands.is_empty());
    }

    #[tokio::test]
    async fn control_plane_registers_runner_and_assigns_session() {
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                service_name: Some("control".to_owned()),
                ..isolated_test_overrides()
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
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
    async fn registering_runner_dispatches_existing_pending_sessions() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

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
        let pending_session: SessionRecord = read_json(create_response).await;
        assert!(pending_session.owner_runner_id.is_none());
        assert_eq!(pending_session.state, SessionState::Pending);

        let runner = spawn_runner_server("runner-late-register", "default").await;
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
        let runner_registration: RunnerRegistrationResponse = read_json(register_response).await;
        assert_eq!(runner_registration.snapshot.state, RunnerState::Busy);
        assert_eq!(runner_registration.snapshot.active_sessions, 1);
        assert_eq!(runner_registration.snapshot.queued_sessions, 0);

        let session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", pending_session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let assigned_session: SessionRecord = read_json(session_response).await;
        assert_eq!(
            assigned_session.owner_runner_id.as_deref(),
            Some("runner-late-register")
        );
        assert_eq!(assigned_session.state, SessionState::Assigned);

        let runner_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == pending_session.session_id)
            .expect("runner should receive the pending session");
        assert_eq!(runner_session.state, RunnerSessionState::Pending);

        let events_response = app
            .oneshot(
                Request::get("/v1/events?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let events: ListResponse<TimelineEvent> = read_json(events_response).await;
        assert_eq!(events.items.len(), 3);
        assert!(matches!(
            events.items[0].detail,
            TimelineEventDetail::SessionCreated { .. }
        ));
        assert!(matches!(
            events.items[1].detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));
        match &events.items[2].detail {
            TimelineEventDetail::SessionStateChanged {
                previous_state,
                state,
            } => {
                assert_eq!(*previous_state, SessionState::Pending);
                assert_eq!(*state, SessionState::Assigned);
            }
            other => panic!("expected pending-to-assigned event, received {other:?}"),
        }
        assert_eq!(
            events.items[2].runner_id.as_deref(),
            Some("runner-late-register")
        );
        assert_eq!(events.items[2].session_id, Some(pending_session.session_id));
    }

    #[tokio::test]
    async fn heartbeat_dispatches_pending_sessions_when_runner_recovers() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-recover", "default").await;
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

        let unhealthy_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/runner-recover/heartbeat")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerHeartbeat {
                            runner_id: "runner-recover".to_owned(),
                            state: RunnerState::Unhealthy,
                            active_sessions: 0,
                            queued_sessions: 0,
                            timestamp: Utc::now(),
                        })
                        .expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(unhealthy_response.status(), StatusCode::OK);

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
        let pending_session: SessionRecord = read_json(create_response).await;
        assert!(pending_session.owner_runner_id.is_none());
        assert_eq!(pending_session.state, SessionState::Pending);

        let recovered_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/runner-recover/heartbeat")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerHeartbeat {
                            runner_id: "runner-recover".to_owned(),
                            state: RunnerState::Idle,
                            active_sessions: 0,
                            queued_sessions: 0,
                            timestamp: Utc::now(),
                        })
                        .expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(recovered_response.status(), StatusCode::OK);
        let recovered_snapshot: RunnerSnapshot = read_json(recovered_response).await;
        assert_eq!(recovered_snapshot.state, RunnerState::Busy);
        assert_eq!(recovered_snapshot.active_sessions, 1);
        assert_eq!(recovered_snapshot.queued_sessions, 0);

        let session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", pending_session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let assigned_session: SessionRecord = read_json(session_response).await;
        assert_eq!(
            assigned_session.owner_runner_id.as_deref(),
            Some("runner-recover")
        );
        assert_eq!(assigned_session.state, SessionState::Assigned);

        let runner_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == pending_session.session_id)
            .expect("runner should receive the recovered session");
        assert_eq!(runner_session.state, RunnerSessionState::Pending);
    }

    #[tokio::test]
    async fn capacity_limited_runner_leaves_additional_sessions_pending() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-capacity", "default").await;
        let mut registration = runner.registration.clone();
        registration.capabilities.max_parallel_sessions = 1;

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
            .expect("request should succeed");
        assert_eq!(first_response.status(), StatusCode::CREATED);
        let first_session: SessionRecord = read_json(first_response).await;
        assert_eq!(
            first_session.owner_runner_id.as_deref(),
            Some("runner-capacity")
        );
        assert_eq!(first_session.state, SessionState::Assigned);

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
            .expect("request should succeed");
        assert_eq!(second_response.status(), StatusCode::CREATED);
        let second_session: SessionRecord = read_json(second_response).await;
        assert!(second_session.owner_runner_id.is_none());
        assert_eq!(second_session.state, SessionState::Pending);

        let runner_sessions = runner.api.list_sessions().await;
        assert_eq!(runner_sessions.len(), 1);
        assert_eq!(runner_sessions[0].session_id, first_session.session_id);
    }

    #[tokio::test]
    async fn list_sessions_supports_runner_workspace_and_state_filters() {
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

        let runner_a = spawn_runner_server("runner-a", "default").await;
        let runner_b = spawn_runner_server("runner-b", "alt").await;
        for runner in [&runner_a, &runner_b] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/v1/runners/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&runner.registration)
                                .expect("json should serialize"),
                        ))
                        .expect("request should build"),
                )
                .await
                .expect("registration should succeed");
            assert_eq!(response.status(), StatusCode::OK);
        }

        let session_a_response = app
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
        assert_eq!(session_a_response.status(), StatusCode::CREATED);
        let session_a: SessionRecord = read_json(session_a_response).await;

        let session_b_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"workspace_id": "alt"}).to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("session create should succeed");
        assert_eq!(session_b_response.status(), StatusCode::CREATED);
        let session_b: SessionRecord = read_json(session_b_response).await;

        let update_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/state", session_a.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "state": "completed",
                            "metadata": {"result": "ok"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("state update should succeed");
        assert_eq!(update_response.status(), StatusCode::OK);

        let by_runner_response = app
            .clone()
            .oneshot(
                Request::get("/v1/sessions?runner_id=runner-a")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("filter request should succeed");
        let by_runner: ListResponse<SessionRecord> = read_json(by_runner_response).await;
        assert_eq!(by_runner.items.len(), 1);
        assert_eq!(by_runner.items[0].session_id, session_a.session_id);

        let by_workspace_response = app
            .clone()
            .oneshot(
                Request::get("/v1/sessions?workspace_id=alt")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("filter request should succeed");
        let by_workspace: ListResponse<SessionRecord> = read_json(by_workspace_response).await;
        assert_eq!(by_workspace.items.len(), 1);
        assert_eq!(by_workspace.items[0].session_id, session_b.session_id);

        let by_state_response = app
            .clone()
            .oneshot(
                Request::get("/v1/sessions?state=completed")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("filter request should succeed");
        let by_state: ListResponse<SessionRecord> = read_json(by_state_response).await;
        assert_eq!(by_state.items.len(), 1);
        assert_eq!(by_state.items[0].session_id, session_a.session_id);

        let runner_scoped_response = app
            .oneshot(
                Request::get("/v1/runners/runner-b/sessions?state=assigned")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("runner-scoped list should succeed");
        let runner_scoped: ListResponse<SessionRecord> = read_json(runner_scoped_response).await;
        assert_eq!(runner_scoped.items.len(), 1);
        assert_eq!(runner_scoped.items[0].session_id, session_b.session_id);
    }

    #[tokio::test]
    async fn session_views_report_owner_runner_availability() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.clone().router();

        let runner = spawn_runner_server("runner-availability", "default").await;
        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&runner.registration).expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("register request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CreateSessionRequest {
                            session_id: None,
                            workspace_id: "default".to_owned(),
                            preferred_runner_id: Some("runner-availability".to_owned()),
                            metadata: BTreeMap::new(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("create request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;

        {
            let mut registry = service.registry.write().await;
            let runner = registry
                .runners
                .get_mut("runner-availability")
                .expect("runner should exist");
            runner.last_seen_at = Utc::now() - chrono::Duration::seconds(120);
            runner.state = RunnerState::Offline;
        }

        let session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("session request should succeed");
        assert_eq!(session_response.status(), StatusCode::OK);
        let session_view: ApiSessionView = read_json(session_response).await;
        assert_eq!(session_view.session.session_id, session.session_id);
        assert!(!session_view.owner_runner_available);
        assert_eq!(session_view.owner_runner_state, Some(RunnerState::Offline));
        assert!(session_view.owner_runner_last_seen_at.is_some());

        let list_response = app
            .oneshot(
                Request::get("/v1/sessions")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("list request should succeed");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list: ListResponse<ApiSessionView> = read_json(list_response).await;
        assert_eq!(list.items.len(), 1);
        assert!(!list.items[0].owner_runner_available);
    }

    #[tokio::test]
    async fn queued_session_commands_report_when_runner_is_unavailable() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.clone().router();

        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerRegistrationRequest {
                            public_base_url: None,
                            ..runner_registration(
                                "runner-pull-unavailable",
                                "default",
                                "C:/workspace/pull",
                            )
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("register request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CreateSessionRequest {
                            session_id: None,
                            workspace_id: "default".to_owned(),
                            preferred_runner_id: Some("runner-pull-unavailable".to_owned()),
                            metadata: BTreeMap::new(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("create request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;

        {
            let mut registry = service.registry.write().await;
            let runner = registry
                .runners
                .get_mut("runner-pull-unavailable")
                .expect("runner should exist");
            runner.last_seen_at = Utc::now() - chrono::Duration::seconds(120);
        }

        let command_response = app
            .oneshot(
                Request::post(format!("/v1/sessions/{}/commands", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerSessionCommandRequest::SendPrompt {
                            content: "hello while offline".to_owned(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("command request should succeed");
        assert_eq!(command_response.status(), StatusCode::OK);
        let response: RunnerSessionCommandResponse = read_json(command_response).await;
        assert!(response.accepted);
        assert_eq!(
            response.message,
            "prompt queued; runner currently unavailable"
        );
    }

    #[tokio::test]
    async fn heartbeat_updates_runner_state() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
    async fn session_events_survive_history_rollover_and_restart() {
        let profile = tempdir().expect("tempdir should exist");
        let profile_dir = profile.path().join("profile");
        let service = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile_dir.clone()),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let app = service.clone().router();

        let runner = spawn_runner_server("runner-rollover", "default").await;
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

        let runtime_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/events", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "detail": {
                                "kind": "message_committed",
                                "role": "assistant",
                                "text": "persist me through rollover"
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("runtime event request should succeed");
        assert_eq!(runtime_response.status(), StatusCode::CREATED);

        for index in 0..(DEFAULT_EVENT_HISTORY_LIMIT + 64) {
            let heartbeat = RunnerHeartbeat {
                runner_id: "runner-rollover".to_owned(),
                state: RunnerState::Busy,
                active_sessions: 1,
                queued_sessions: 0,
                timestamp: Utc::now() + chrono::Duration::seconds(i64::from(index as i32)),
            };
            let response = app
                .clone()
                .oneshot(
                    Request::post("/v1/runners/runner-rollover/heartbeat")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&heartbeat).expect("json should serialize"),
                        ))
                        .expect("request should build"),
                )
                .await
                .expect("heartbeat request should succeed");
            assert_eq!(response.status(), StatusCode::OK);
        }

        let snapshot = service.timeline.snapshot().await;
        assert!(
            !snapshot.history().any(|event| {
                event.session_id == Some(session.session_id)
                    && matches!(event.detail, TimelineEventDetail::MessageCommitted { .. })
            }),
            "session message should have rolled out of the in-memory timeline"
        );

        drop(app);

        let restored = ControlPlaneService::new(
            load_control_plane_config(ControlPlaneConfigOverrides {
                profile_dir: Some(profile_dir),
                ..ControlPlaneConfigOverrides::default()
            })
            .expect("config should load"),
            "0.1.0",
        );
        let restored_app = restored.router();

        let response = restored_app
            .oneshot(
                Request::get(format!(
                    "/v1/sessions/{}/events?limit=20",
                    session.session_id
                ))
                .body(Body::empty())
                .expect("request should build"),
            )
            .await
            .expect("session events request should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let events: ListResponse<TimelineEvent> = read_json(response).await;
        assert!(
            events.items.iter().any(|event| {
                matches!(
                    &event.detail,
                    TimelineEventDetail::MessageCommitted { text, .. }
                        if text == "persist me through rollover"
                )
            }),
            "session events should still be queryable after restart"
        );
    }

    #[tokio::test]
    async fn approval_relay_updates_session_state_and_timeline() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
        let runner_approvals = runner.api.list_approvals().await;
        assert_eq!(runner_approvals.len(), 1);
        assert_eq!(runner_approvals[0].approval_id, approval.approval_id);
        assert_eq!(runner_approvals[0].state, ApprovalState::Pending);
        let runner_waiting_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == session.session_id)
            .expect("runner session should exist after approval relay");
        assert_eq!(
            runner_waiting_session.state,
            RunnerSessionState::WaitingApproval
        );

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
        let runner_resolved_approval = runner
            .api
            .list_approvals()
            .await
            .into_iter()
            .find(|record| record.approval_id == approval.approval_id)
            .expect("runner approval should exist after decision relay");
        assert_eq!(runner_resolved_approval.state, ApprovalState::Approved);
        assert_eq!(
            runner_resolved_approval.responder.as_deref(),
            Some("operator-1")
        );
        assert_eq!(
            runner_resolved_approval.note.as_deref(),
            Some("Approved for this run")
        );

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
        let runner_resumed_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == session.session_id)
            .expect("runner session should exist after decision relay");
        assert_eq!(runner_resumed_session.state, RunnerSessionState::Running);

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
        assert_eq!(events.items.len(), 6);
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
            TimelineEventDetail::SessionStateChanged {
                previous_state,
                state,
            } => {
                assert_eq!(*previous_state, SessionState::Assigned);
                assert_eq!(*state, SessionState::WaitingApproval);
            }
            other => panic!("expected waiting state change event, received {other:?}"),
        }
        match &events.items[4].detail {
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
        match &events.items[5].detail {
            TimelineEventDetail::SessionStateChanged {
                previous_state,
                state,
            } => {
                assert_eq!(*previous_state, SessionState::WaitingApproval);
                assert_eq!(*state, SessionState::Running);
            }
            other => panic!("expected running state change event, received {other:?}"),
        }
        for index in 2..=5 {
            assert_eq!(events.items[index].session_id, Some(session.session_id));
            assert_eq!(
                events.items[index].runner_id.as_deref(),
                Some("runner-approval")
            );
        }
    }

    #[tokio::test]
    async fn failed_approval_relay_does_not_mutate_control_plane_state() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.clone().router();

        let runner = spawn_runner_server("runner-approval-failure", "default").await;
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

        {
            let mut registry = service.registry.write().await;
            let snapshot = registry
                .runners
                .get_mut("runner-approval-failure")
                .expect("runner snapshot should exist");
            snapshot.registration.public_base_url = Some("http://127.0.0.1:1".to_owned());
        }

        let approval_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/approvals", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "title": "Broken relay",
                            "description": "Should fail before commit"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(approval_response.status(), StatusCode::BAD_GATEWAY);

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
        assert!(approvals.items.is_empty());

        let session_response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/sessions/{}", session.session_id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let control_plane_session: SessionRecord = read_json(session_response).await;
        assert_eq!(control_plane_session.state, SessionState::Assigned);

        assert!(runner.api.list_approvals().await.is_empty());
        let runner_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == session.session_id)
            .expect("runner session should still exist");
        assert_eq!(runner_session.state, RunnerSessionState::Pending);
    }

    #[tokio::test]
    async fn session_state_updates_relay_to_runner_and_refresh_counts() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner = spawn_runner_server("runner-state", "default").await;
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
        assert_eq!(session.owner_runner_id.as_deref(), Some("runner-state"));
        assert_eq!(session.state, SessionState::Assigned);

        let running_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/state", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&SessionStateUpdateRequest {
                            state: SessionState::Running,
                            metadata: BTreeMap::from([("phase".to_owned(), "running".to_owned())]),
                        })
                        .expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(running_response.status(), StatusCode::OK);
        let running_session: SessionRecord = read_json(running_response).await;
        assert_eq!(running_session.state, SessionState::Running);
        assert_eq!(
            running_session.metadata.get("phase").map(String::as_str),
            Some("running")
        );

        let runner_snapshot_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-state")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let running_snapshot: RunnerSnapshot = read_json(runner_snapshot_response).await;
        assert_eq!(running_snapshot.state, RunnerState::Busy);
        assert_eq!(running_snapshot.active_sessions, 1);
        assert_eq!(running_snapshot.queued_sessions, 0);

        let runner_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == session.session_id)
            .expect("runner session should exist");
        assert_eq!(runner_session.state, RunnerSessionState::Running);
        assert_eq!(
            runner_session.metadata.get("phase").map(String::as_str),
            Some("running")
        );

        let completed_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/state", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&SessionStateUpdateRequest {
                            state: SessionState::Completed,
                            metadata: BTreeMap::from([("result".to_owned(), "ok".to_owned())]),
                        })
                        .expect("json should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(completed_response.status(), StatusCode::OK);
        let completed_session: SessionRecord = read_json(completed_response).await;
        assert_eq!(completed_session.state, SessionState::Completed);
        assert_eq!(
            completed_session.metadata.get("phase").map(String::as_str),
            Some("running")
        );
        assert_eq!(
            completed_session.metadata.get("result").map(String::as_str),
            Some("ok")
        );

        let completed_runner_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-state")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let completed_snapshot: RunnerSnapshot = read_json(completed_runner_response).await;
        assert_eq!(completed_snapshot.state, RunnerState::Idle);
        assert_eq!(completed_snapshot.active_sessions, 0);
        assert_eq!(completed_snapshot.queued_sessions, 0);

        let completed_runner_session = runner
            .api
            .list_sessions()
            .await
            .into_iter()
            .find(|record| record.session_id == session.session_id)
            .expect("runner session should exist");
        assert_eq!(
            completed_runner_session.state,
            RunnerSessionState::Completed
        );
        assert_eq!(
            completed_runner_session
                .metadata
                .get("result")
                .map(String::as_str),
            Some("ok")
        );

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
            TimelineEventDetail::SessionStateChanged {
                previous_state,
                state,
            } => {
                assert_eq!(*previous_state, SessionState::Assigned);
                assert_eq!(*state, SessionState::Running);
            }
            other => panic!("expected running state change event, received {other:?}"),
        }
        match &events.items[3].detail {
            TimelineEventDetail::SessionStateChanged {
                previous_state,
                state,
            } => {
                assert_eq!(*previous_state, SessionState::Running);
                assert_eq!(*state, SessionState::Completed);
            }
            other => panic!("expected completion state change event, received {other:?}"),
        }
        assert_eq!(events.items[2].session_id, Some(session.session_id));
        assert_eq!(events.items[3].session_id, Some(session.session_id));
        assert_eq!(events.items[2].runner_id.as_deref(), Some("runner-state"));
        assert_eq!(events.items[3].runner_id.as_deref(), Some("runner-state"));
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
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
    async fn runner_artifact_listing_filters_by_runner() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner_a = spawn_runner_server("runner-artifact-a", "default").await;
        let runner_z = spawn_runner_server("runner-artifact-z", "default").await;
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
                        serde_json::json!({
                            "workspace_id": "default",
                            "preferred_runner_id": "runner-artifact-a"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let session: SessionRecord = read_json(create_response).await;
        assert_eq!(
            session.owner_runner_id.as_deref(),
            Some("runner-artifact-a")
        );

        let artifact_response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/sessions/{}/artifacts", session.session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "runner-log",
                            "file_name": "runner.log",
                            "media_type": "text/plain",
                            "content_base64": BASE64_STANDARD.encode("hello runner"),
                            "metadata": {"kind": "runner-log"}
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(artifact_response.status(), StatusCode::CREATED);

        let runner_a_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-artifact-a/artifacts")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_a_artifacts: ListResponse<ArtifactRecord> = read_json(runner_a_response).await;
        assert_eq!(runner_a_artifacts.items.len(), 1);
        assert_eq!(runner_a_artifacts.items[0].file_name, "runner.log");

        let runner_z_response = app
            .oneshot(
                Request::get("/v1/runners/runner-artifact-z/artifacts")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_z_artifacts: ListResponse<ArtifactRecord> = read_json(runner_z_response).await;
        assert!(runner_z_artifacts.items.is_empty());
    }

    #[tokio::test]
    async fn runner_event_listing_filters_by_runner() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let runner_a = spawn_runner_server("runner-event-a", "default").await;
        let runner_b = spawn_runner_server("runner-event-b", "default").await;
        for registration in [&runner_a.registration, &runner_b.registration] {
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
                        serde_json::json!({
                            "workspace_id": "default",
                            "preferred_runner_id": "runner-event-a"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let runner_a_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-event-a/events?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_a_events: ListResponse<TimelineEvent> = read_json(runner_a_response).await;
        assert!(
            runner_a_events.items.len() >= 2,
            "expected runner registration and session-created events"
        );
        assert!(
            runner_a_events
                .items
                .iter()
                .all(|event| event.runner_id.as_deref() == Some("runner-event-a"))
        );
        assert!(
            runner_a_events
                .items
                .iter()
                .any(|event| matches!(event.detail, TimelineEventDetail::SessionCreated { .. }))
        );

        let runner_a_filtered_response = app
            .clone()
            .oneshot(
                Request::get("/v1/runners/runner-event-a/events?limit=10&kind=session_created")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_a_filtered: ListResponse<TimelineEvent> =
            read_json(runner_a_filtered_response).await;
        assert_eq!(runner_a_filtered.items.len(), 1);
        assert!(matches!(
            runner_a_filtered.items[0].detail,
            TimelineEventDetail::SessionCreated { .. }
        ));

        let runner_b_response = app
            .oneshot(
                Request::get("/v1/runners/runner-event-b/events?limit=10")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");
        let runner_b_events: ListResponse<TimelineEvent> = read_json(runner_b_response).await;
        assert_eq!(runner_b_events.items.len(), 1);
        assert!(matches!(
            runner_b_events.items[0].detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));
    }

    #[tokio::test]
    async fn runner_approval_stream_only_emits_matching_approval_events() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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
    async fn runner_event_stream_replays_backlog_for_matching_runner() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;

        let client = Client::new();
        let runner = spawn_runner_server("runner-event-stream", "default").await;
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner.registration)
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let session: SessionRecord = client
            .post(format!("{base_url}/v1/sessions"))
            .json(&serde_json::json!({
                "workspace_id": "default",
                "preferred_runner_id": "runner-event-stream"
            }))
            .send()
            .await
            .expect("session create should succeed")
            .error_for_status()
            .expect("session create should succeed")
            .json()
            .await
            .expect("session payload should decode");

        let ws_url = base_url.replacen("http://", "ws://", 1)
            + "/v1/runners/runner-event-stream/events/stream?after=0&kind=session_created";
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let first_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("first event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");

        let decode = |message| -> TimelineEvent {
            let text = match message {
                TungsteniteMessage::Text(text) => text,
                other => panic!("expected text frame, received {other:?}"),
            };
            serde_json::from_str(&text).expect("event payload should deserialize")
        };
        let first_event = decode(first_message);
        assert!(matches!(
            first_event.detail,
            TimelineEventDetail::SessionCreated { .. }
        ));
        assert_eq!(
            first_event.runner_id.as_deref(),
            Some("runner-event-stream")
        );
        assert_eq!(first_event.session_id, Some(session.session_id));

        socket.close(None).await.expect("socket should close");
        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn approval_stream_replays_backlog_after_query() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;

        let client = Client::new();
        let runner = spawn_runner_server("runner-approval-backlog", "default").await;
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
                "title": "Backlog approval",
                "description": "Needs replay"
            }))
            .send()
            .await
            .expect("approval create should succeed")
            .error_for_status()
            .expect("approval create should succeed")
            .json()
            .await
            .expect("approval payload should decode");

        let ws_url = base_url.replacen("http://", "ws://", 1) + "/v1/approvals/stream?after=0";
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let backlog_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("backlog event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let backlog_text = match backlog_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let backlog_event: TimelineEvent =
            serde_json::from_str(&backlog_text).expect("event payload should deserialize");
        assert!(matches!(
            backlog_event.detail,
            TimelineEventDetail::ApprovalRequested { .. }
        ));
        assert_eq!(backlog_event.session_id, Some(session.session_id));

        let response = client
            .post(format!(
                "{base_url}/v1/approvals/{}/decision",
                approval.approval_id
            ))
            .json(&serde_json::json!({
                "decision": "approved",
                "responder": "approval-backlog"
            }))
            .send()
            .await
            .expect("approval resolve should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let live_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("live event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let live_text = match live_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let live_event: TimelineEvent =
            serde_json::from_str(&live_text).expect("event payload should deserialize");
        assert!(matches!(
            live_event.detail,
            TimelineEventDetail::ApprovalResolved { .. }
        ));
        assert_eq!(live_event.session_id, Some(session.session_id));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn session_approval_stream_replays_only_matching_session_approvals() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;

        let client = Client::new();
        let runner = spawn_runner_server("runner-session-approval-stream", "default").await;
        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner.registration)
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let target_session: SessionRecord = client
            .post(format!("{base_url}/v1/sessions"))
            .json(&serde_json::json!({"workspace_id": "default"}))
            .send()
            .await
            .expect("target session create should succeed")
            .error_for_status()
            .expect("target session create should succeed")
            .json()
            .await
            .expect("target session payload should decode");

        let other_session: SessionRecord = client
            .post(format!("{base_url}/v1/sessions"))
            .json(&serde_json::json!({"workspace_id": "default"}))
            .send()
            .await
            .expect("other session create should succeed")
            .error_for_status()
            .expect("other session create should succeed")
            .json()
            .await
            .expect("other session payload should decode");

        let _other_approval: ApprovalRequestRecord = client
            .post(format!(
                "{base_url}/v1/sessions/{}/approvals",
                other_session.session_id
            ))
            .json(&serde_json::json!({
                "title": "Other approval",
                "description": "Should be filtered out"
            }))
            .send()
            .await
            .expect("other approval create should succeed")
            .error_for_status()
            .expect("other approval create should succeed")
            .json()
            .await
            .expect("other approval payload should decode");

        let target_approval: ApprovalRequestRecord = client
            .post(format!(
                "{base_url}/v1/sessions/{}/approvals",
                target_session.session_id
            ))
            .json(&serde_json::json!({
                "title": "Target approval",
                "description": "Should be replayed"
            }))
            .send()
            .await
            .expect("target approval create should succeed")
            .error_for_status()
            .expect("target approval create should succeed")
            .json()
            .await
            .expect("target approval payload should decode");

        let ws_url = base_url.replacen("http://", "ws://", 1)
            + &format!(
                "/v1/sessions/{}/approvals/stream?after=0",
                target_session.session_id
            );
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let backlog_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("backlog event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let backlog_text = match backlog_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let backlog_event: TimelineEvent =
            serde_json::from_str(&backlog_text).expect("event payload should deserialize");
        assert_eq!(backlog_event.session_id, Some(target_session.session_id));
        match backlog_event.detail {
            TimelineEventDetail::ApprovalRequested { approval_id, .. } => {
                assert_eq!(approval_id, target_approval.approval_id);
            }
            other => panic!("expected approval requested event, received {other:?}"),
        }

        let response = client
            .post(format!(
                "{base_url}/v1/approvals/{}/decision",
                target_approval.approval_id
            ))
            .json(&serde_json::json!({
                "decision": "denied",
                "responder": "session-approval-stream"
            }))
            .send()
            .await
            .expect("target approval resolve should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let live_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("live event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let live_text = match live_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let live_event: TimelineEvent =
            serde_json::from_str(&live_text).expect("event payload should deserialize");
        assert_eq!(live_event.session_id, Some(target_session.session_id));
        assert!(matches!(
            live_event.detail,
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
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
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

    #[tokio::test]
    async fn websocket_stream_replays_backlog_before_live_runner_events() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let (base_url, server_handle) = spawn_control_plane_server(service).await;
        let client = Client::new();

        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner_registration(
                "runner-backlog-a",
                "default",
                "C:/workspace/a",
            ))
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let ws_url = base_url.replacen("http://", "ws://", 1) + "/v1/events/stream?after=0";
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let backlog_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("backlog event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let backlog_text = match backlog_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let backlog_event: TimelineEvent =
            serde_json::from_str(&backlog_text).expect("event payload should deserialize");
        assert_eq!(backlog_event.runner_id.as_deref(), Some("runner-backlog-a"));
        assert!(matches!(
            backlog_event.detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));

        let response = client
            .post(format!("{base_url}/v1/runners/register"))
            .json(&runner_registration(
                "runner-backlog-b",
                "default",
                "C:/workspace/b",
            ))
            .send()
            .await
            .expect("registration request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let live_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("live event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let live_text = match live_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let live_event: TimelineEvent =
            serde_json::from_str(&live_text).expect("event payload should deserialize");
        assert_eq!(live_event.runner_id.as_deref(), Some("runner-backlog-b"));
        assert!(live_event.sequence > backlog_event.sequence);
        assert!(matches!(
            live_event.detail,
            TimelineEventDetail::RunnerRegistered { .. }
        ));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn session_event_stream_replays_backlog_after_query() {
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
        let runner = spawn_runner_server("runner-session-backlog", "default").await;
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

        let first_artifact = client
            .post(format!(
                "{base_url}/v1/sessions/{}/artifacts",
                session.session_id
            ))
            .json(&serde_json::json!({
                "name": "notes-one",
                "file_name": "notes-one.txt",
                "media_type": "text/plain",
                "content_base64": BASE64_STANDARD.encode("hello backlog one")
            }))
            .send()
            .await
            .expect("artifact create should succeed");
        assert_eq!(first_artifact.status(), StatusCode::CREATED);

        let session_events: ListResponse<TimelineEvent> = client
            .get(format!(
                "{base_url}/v1/sessions/{}/events?limit=10",
                session.session_id
            ))
            .send()
            .await
            .expect("session events request should succeed")
            .error_for_status()
            .expect("session events request should succeed")
            .json()
            .await
            .expect("session events payload should decode");
        let session_created_sequence = session_events
            .items
            .iter()
            .find(|event| matches!(event.detail, TimelineEventDetail::SessionCreated { .. }))
            .map(|event| event.sequence)
            .expect("session created event should exist");

        let ws_url = base_url.replacen("http://", "ws://", 1)
            + &format!(
                "/v1/sessions/{}/events/stream?after={session_created_sequence}",
                session.session_id
            );
        let (mut socket, _) = connect_async(&ws_url)
            .await
            .expect("websocket should connect");

        let backlog_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("backlog event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let backlog_text = match backlog_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let backlog_event: TimelineEvent =
            serde_json::from_str(&backlog_text).expect("event payload should deserialize");
        assert_eq!(backlog_event.session_id, Some(session.session_id));
        assert!(matches!(
            backlog_event.detail,
            TimelineEventDetail::ArtifactCreated { .. }
        ));

        let second_artifact = client
            .post(format!(
                "{base_url}/v1/sessions/{}/artifacts",
                session.session_id
            ))
            .json(&serde_json::json!({
                "name": "notes-two",
                "file_name": "notes-two.txt",
                "media_type": "text/plain",
                "content_base64": BASE64_STANDARD.encode("hello backlog two")
            }))
            .send()
            .await
            .expect("artifact create should succeed");
        assert_eq!(second_artifact.status(), StatusCode::CREATED);

        let live_message = timeout(TokioDuration::from_secs(5), socket.next())
            .await
            .expect("live event should arrive before timeout")
            .expect("event stream should stay open")
            .expect("websocket frame should parse");
        let live_text = match live_message {
            TungsteniteMessage::Text(text) => text,
            other => panic!("expected text frame, received {other:?}"),
        };
        let live_event: TimelineEvent =
            serde_json::from_str(&live_text).expect("event payload should deserialize");
        assert_eq!(live_event.session_id, Some(session.session_id));
        assert!(live_event.sequence > backlog_event.sequence);
        assert!(matches!(
            live_event.detail,
            TimelineEventDetail::ArtifactCreated { .. }
        ));

        server_handle.abort();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn session_command_endpoint_relays_to_runner() {
        let service = ControlPlaneService::new(
            load_control_plane_config(isolated_test_overrides()).expect("config should load"),
            "0.1.0",
        );
        let app = service.router();

        let profile = tempdir().expect("tempdir should exist");
        let workspace_root = profile.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace dir should exist");
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let config = load_runner_config(
            Some(profile.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-command".to_owned()),
                public_base_url: Some(format!("http://{address}")),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: workspace_root,
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let registration = config.registration_request();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let runner_api =
            RunnerApi::new(config, "remote-code-runner", "0.1.0").with_event_channel(event_tx);
        let runner_server = {
            let app = runner_api.router();
            tokio::spawn(async move {
                axum::serve(listener, app).await.expect("server should run");
            })
        };

        let register_response = app
            .clone()
            .oneshot(
                Request::post("/v1/runners/register")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&registration).expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("register request should succeed");
        assert_eq!(register_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::post("/v1/sessions")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CreateSessionRequest {
                            session_id: Some(Uuid::nil()),
                            workspace_id: "default".to_owned(),
                            preferred_runner_id: Some("runner-command".to_owned()),
                            metadata: BTreeMap::new(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("create request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let session: SessionRecord = read_json(create_response).await;
        let _ = event_rx.recv().await.expect("session event should arrive");

        let command_response = app
            .oneshot(
                Request::post(format!("/v1/sessions/{}/commands", session.session_id))
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RunnerSessionCommandRequest::SendPrompt {
                            content: "hello over relay".to_owned(),
                        })
                        .expect("request should serialize"),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("command request should succeed");
        assert_eq!(command_response.status(), StatusCode::OK);
        let response: RunnerSessionCommandResponse = read_json(command_response).await;
        assert!(response.accepted);
        assert_eq!(response.session_id, session.session_id);

        match event_rx
            .recv()
            .await
            .expect("runner command event should arrive")
        {
            rc_runner::RunnerApiEvent::SessionCommand {
                session_id,
                command,
            } => {
                assert_eq!(session_id, session.session_id);
                assert_eq!(
                    command,
                    RunnerSessionCommandRequest::SendPrompt {
                        content: "hello over relay".to_owned()
                    }
                );
            }
            other => panic!("unexpected runner event: {other:?}"),
        }

        runner_server.abort();
        let _ = runner_server.await;
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
