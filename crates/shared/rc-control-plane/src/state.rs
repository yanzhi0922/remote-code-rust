//! ControlPlaneService struct, state persistence, and configuration helpers.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::Row;
use rusqlite::params_from_iter;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::auth::{constant_time_value_eq, hash_secret_value};
use crate::helpers;
use crate::rate_limit::RateLimiter;
use crate::registry::{Registry, TimelineSnapshot, TimelineStore};
use crate::types::{
    ControlPlaneConfig, ControlPlaneConfigOverrides, ControlPlaneMeta, ControlPlaneStatus,
    DEFAULT_BIND, DEFAULT_EVENT_HISTORY_LIMIT, DEFAULT_RUNNER_LEASE_TTL_SECS, EVENT_STREAM_BUFFER,
    MAX_EVENT_LIST_LIMIT, PHASE, TimelineEventDraft, TimelineEventKind, TrustedDeviceRecord,
};
// ---------------------------------------------------------------------------
// PersistedEventQuery (shared between state and handlers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub(crate) struct PersistedEventQuery {
    pub(crate) after: Option<u64>,
    pub(crate) limit: Option<usize>,
    pub(crate) kind: Option<TimelineEventKind>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) runner_id: Option<String>,
    pub(crate) approvals_only: bool,
}

// ---------------------------------------------------------------------------
// AuthPrincipal (shared between auth middleware and handlers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) enum AuthPrincipal {
    /// Legacy shared bearer token — admin access, sees all tenants.
    SharedToken,
    /// Legacy device token from pairing flow.
    Device(TrustedDeviceRecord),
    /// User-derived tenant identity.  `user_id` is accepted only when the
    /// control plane is configured with a matching user-key hash.
    User { user_id: String },
}

impl AuthPrincipal {
    pub(crate) fn created_by_device_id(&self) -> Option<Uuid> {
        match self {
            Self::SharedToken | Self::User { .. } => None,
            Self::Device(device) => Some(device.device_id),
        }
    }

    /// Return the tenant-scoping user_id, if any.
    ///
    /// Returns `None` for `SharedToken` (admin — sees everything) and for
    /// `Device` (legacy — no tenant isolation).
    pub(crate) fn user_id(&self) -> Option<&str> {
        match self {
            Self::User { user_id } => Some(user_id.as_str()),
            Self::SharedToken | Self::Device(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ControlPlaneService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ControlPlaneService {
    pub(crate) meta: ControlPlaneMeta,
    pub(crate) runner_lease_ttl_secs: u64,
    #[allow(dead_code)] // Kept for debugging and future state re-opening.
    pub(crate) state_db_path: PathBuf,
    pub(crate) artifact_root_dir: PathBuf,
    pub(crate) auth_token: Option<String>,
    pub(crate) bootstrap_secret_hash: Option<String>,
    pub(crate) registry: Arc<RwLock<Registry>>,
    pub(crate) timeline: TimelineStore,
    // NOTE(perf): Write locks dominate (mint/consume both write), so a plain
    // Mutex would be more efficient than RwLock.  Deferred to avoid API churn.
    pub(crate) stream_tickets: Arc<RwLock<BTreeMap<String, StreamTicket>>>,
    pub(crate) allowed_user_key_hashes: Vec<String>,
    /// Shared HTTP client for runner relay requests.  Reusing a single
    /// client keeps TCP connections alive and avoids a TLS handshake
    /// per request.
    pub(crate) http_client: reqwest::Client,
    /// Directory containing downloadable app binaries (APK, dmg, etc.).
    pub(crate) downloads_dir: Option<PathBuf>,
    /// Per-IP rate limiter for authentication endpoints.
    pub(crate) rate_limiter: Arc<RateLimiter>,
    /// Shared SQLite connection, opened once and reused across queries.
    db_connection: Arc<Mutex<Connection>>,
    /// Whether state was successfully loaded from persistent storage.
    /// `false` means the DB was missing, corrupt, or unreadable — the service
    /// is running with an empty registry and callers should check [`Self::state_healthy`].
    state_loaded_from_persistence: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct StreamTicket {
    pub(crate) principal: AuthPrincipal,
    pub(crate) path: String,
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedControlPlaneState {
    pub(crate) registry: Registry,
    pub(crate) timeline: TimelineSnapshot,
}

/// Borrowed serialization wrapper — avoids cloning the entire registry.
#[derive(serde::Serialize)]
struct PersistedControlPlaneStateRef<'a> {
    registry: &'a Registry,
    timeline: &'a TimelineSnapshot,
}

impl<'a> From<PersistedControlPlaneStateRef<'a>> for PersistedControlPlaneState {
    fn from(value: PersistedControlPlaneStateRef<'a>) -> Self {
        Self {
            registry: value.registry.clone(),
            timeline: value.timeline.clone(),
        }
    }
}

impl ControlPlaneService {
    /// Create a new `ControlPlaneService` using a pre-configured HTTP client.
    ///
    /// Prefer this over [`new`](Self::new) when the caller already has a
    /// long-lived `reqwest::Client` so TCP connections and TLS sessions are
    /// reused across components.
    pub fn with_http_client(
        config: ControlPlaneConfig,
        version: impl Into<String>,
        http_client: reqwest::Client,
    ) -> Self {
        let service_name = config.service_name.clone();
        let state_db_path = config.state_db_path.clone();
        let artifact_root_dir = config.artifact_root_dir.clone();
        let auth_token = config.auth_token.clone();
        let bootstrap_secret_hash = config.bootstrap_secret.as_deref().map(hash_secret_value);
        let db_connection = open_state_connection(&state_db_path)
            .map_err(|e| {
                tracing::error!("control-plane: failed to open state DB connection: {e:#}");
                e
            })
            .ok();
        let mut state_loaded_from_persistence = false;
        let (registry, timeline) = if let Some(ref conn) = db_connection {
            match load_persisted_state_with_conn(conn, &state_db_path) {
                Ok(loaded) => {
                    state_loaded_from_persistence = true;
                    loaded
                }
                Err(e) => {
                    tracing::error!(
                        "control-plane: failed to load persisted state, starting fresh: {e:#}"
                    );
                    (
                        Registry::default(),
                        TimelineStore::new(DEFAULT_EVENT_HISTORY_LIMIT, EVENT_STREAM_BUFFER),
                    )
                }
            }
        } else {
            tracing::error!("control-plane: no DB connection available, starting fresh");
            (
                Registry::default(),
                TimelineStore::new(DEFAULT_EVENT_HISTORY_LIMIT, EVENT_STREAM_BUFFER),
            )
        };
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
            stream_tickets: Arc::new(RwLock::new(BTreeMap::new())),
            allowed_user_key_hashes: load_allowed_user_key_hashes_from_env(),
            http_client,
            downloads_dir: config.downloads_dir,
            rate_limiter: Arc::new(RateLimiter::default()),
            db_connection: Arc::new(Mutex::new(db_connection.unwrap_or_else(|| {
                open_state_connection(std::path::Path::new(":memory:"))
                    .expect("in-memory SQLite should always open")
            }))),
            state_loaded_from_persistence,
        }
    }

    pub fn new(config: ControlPlaneConfig, version: impl Into<String>) -> Self {
        Self::with_http_client(config, version, reqwest::Client::new())
    }

    #[must_use]
    pub fn meta(&self) -> &ControlPlaneMeta {
        &self.meta
    }

    /// Returns `true` when persisted state was loaded successfully from the
    /// backing SQLite database.  Returns `false` when the DB was missing,
    /// corrupt, or unreadable — the service is running with an empty registry.
    #[must_use]
    pub fn state_healthy(&self) -> bool {
        self.state_loaded_from_persistence
    }

    pub(crate) async fn auth_required(&self) -> bool {
        if self.auth_token.is_some() || self.bootstrap_secret_hash.is_some() {
            return true;
        }
        self.registry.read().await.owner_claimed()
    }

    pub(crate) fn accepts_derived_user_key(&self, provided: &str) -> bool {
        if self.allowed_user_key_hashes.is_empty() {
            return false;
        }
        let provided_hash = hash_secret_value(provided);
        self.allowed_user_key_hashes
            .iter()
            .any(|expected| constant_time_value_eq(&provided_hash, expected))
    }

    pub(crate) async fn mint_stream_ticket(
        &self,
        principal: AuthPrincipal,
        path: String,
        ttl_secs: u64,
    ) -> String {
        let ticket = format!(
            "rcst_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let expires_at = Utc::now() + Duration::seconds(ttl_secs as i64);
        let mut tickets = self.stream_tickets.write().await;
        prune_expired_stream_tickets(&mut tickets);
        // Cap active tickets to prevent unbounded growth.
        const MAX_STREAM_TICKETS: usize = 1024;
        while tickets.len() >= MAX_STREAM_TICKETS {
            // Remove the ticket closest to expiry.
            if let Some(oldest_key) = tickets
                .iter()
                .min_by_key(|(_, t)| t.expires_at)
                .map(|(k, _)| k.clone())
            {
                tickets.remove(&oldest_key);
            } else {
                break;
            }
        }
        tickets.insert(
            ticket.clone(),
            StreamTicket {
                principal,
                path,
                expires_at,
            },
        );
        ticket
    }

    pub(crate) async fn consume_stream_ticket(
        &self,
        ticket: &str,
        request_path: &str,
    ) -> Option<AuthPrincipal> {
        let mut tickets = self.stream_tickets.write().await;
        prune_expired_stream_tickets(&mut tickets);
        let stored = tickets.remove(ticket)?;
        (stored.path == request_path && stored.expires_at > Utc::now()).then_some(stored.principal)
    }

    pub(crate) async fn list_persisted_events(
        &self,
        query: PersistedEventQuery,
    ) -> Result<Vec<crate::types::TimelineEvent>> {
        let conn = self.db_connection.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            load_persisted_events_with_conn(&conn, &query)
        })
        .await
        .context("control-plane event query task failed to join")?
    }

    pub(crate) async fn latest_persisted_event_sequence(
        &self,
        query: PersistedEventQuery,
    ) -> Result<Option<u64>> {
        let conn = self.db_connection.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            load_latest_persisted_event_sequence_with_conn(&conn, &query)
        })
        .await
        .context("control-plane latest event query task failed to join")?
    }

    pub(crate) async fn publish_event(
        &self,
        draft: TimelineEventDraft,
    ) -> crate::types::TimelineEvent {
        let event = self.timeline.publish(draft).await;
        let conn = self.db_connection.clone();
        let event_to_persist = event.clone();
        let result = tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            persist_timeline_event(&conn, &event_to_persist)
        })
        .await;
        if let Err(e) = &result {
            tracing::error!("Failed to spawn event persistence task: {e}");
        } else if let Ok(Err(e)) = &result {
            tracing::error!("Failed to persist timeline event: {e:#}");
        }
        event
    }

    pub(crate) async fn persist_state(&self) -> Result<()> {
        // Serialize under the read lock to avoid cloning the entire registry.
        let payload = {
            let registry_guard = self.registry.read().await;
            let timeline = self.timeline.snapshot().await;
            let snapshot = PersistedControlPlaneStateRef {
                registry: &registry_guard,
                timeline: &timeline,
            };
            serde_json::to_string(&snapshot).context("failed to encode control plane snapshot")?
        };
        let conn = self.db_connection.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            persist_control_plane_state_from_payload(&conn, &payload, None)
        })
        .await
        .context("control-plane snapshot task failed to join")??;
        Ok(())
    }
}

fn load_allowed_user_key_hashes_from_env() -> Vec<String> {
    let Ok(raw) = std::env::var("REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES") else {
        return Vec::new();
    };
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let normalized = value
                .strip_prefix("sha256:")
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase();
            if is_sha256_hex(&normalized) {
                Some(normalized)
            } else {
                tracing::warn!(
                    "Ignoring invalid REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES entry; expected sha256 hex"
                );
                None
            }
        })
        .collect()
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn prune_expired_stream_tickets(tickets: &mut BTreeMap<String, StreamTicket>) {
    let now = Utc::now();
    tickets.retain(|_, ticket| ticket.expires_at > now);
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
    let downloads_dir = overrides
        .downloads_dir
        .or_else(|| helpers::read_env("REMOTE_CODE_DOWNLOADS_DIR").map(PathBuf::from));
    let quic_bind = overrides.quic_bind.or_else(|| {
        helpers::read_env("REMOTE_CODE_CONTROL_PLANE_QUIC_BIND")
            .and_then(|s| helpers::parse_socket_addr(&s).ok())
    });
    let quic_cert_pem = overrides
        .quic_cert_pem
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_QUIC_CERT").map(PathBuf::from));
    let quic_key_pem = overrides
        .quic_key_pem
        .or_else(|| helpers::read_env("REMOTE_CODE_CONTROL_PLANE_QUIC_KEY").map(PathBuf::from));
    let paths = claude_config::AppPaths::discover(profile_dir)?;
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
        downloads_dir,
        quic_bind,
        quic_cert_pem,
        quic_key_pem,
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

// ---------------------------------------------------------------------------
// State persistence (SQLite)
// ---------------------------------------------------------------------------

fn load_persisted_state_with_conn(
    connection: &Connection,
    state_db_path: &std::path::Path,
) -> Result<(Registry, TimelineStore)> {
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
    backfill_persisted_events(connection, &snapshot.timeline)
        .with_context(|| format!("failed to backfill events in {}", state_db_path.display()))?;
    let mut registry = snapshot.registry;
    // Rebuild the reverse indices since they are `#[serde(skip)]`.
    registry.rebuild_session_runner_index();
    registry.rebuild_token_hash_index();
    Ok((
        registry,
        TimelineStore::from_snapshot(
            DEFAULT_EVENT_HISTORY_LIMIT,
            EVENT_STREAM_BUFFER,
            snapshot.timeline,
        ),
    ))
}

/// Persist a pre-serialized control plane snapshot payload to SQLite.
fn persist_control_plane_state_from_payload(
    connection: &Connection,
    payload: &str,
    event: Option<&crate::types::TimelineEvent>,
) -> Result<()> {
    let transaction = connection.unchecked_transaction()?;
    persist_control_plane_state_from_payload_inner(&transaction, payload, event)?;
    transaction.commit()?;
    Ok(())
}

fn persist_control_plane_state_from_payload_inner(
    transaction: &Connection,
    payload: &str,
    event: Option<&crate::types::TimelineEvent>,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO control_plane_snapshot (id, payload, updated_at)
             VALUES (1, ?1, CURRENT_TIMESTAMP)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = CURRENT_TIMESTAMP",
            params![payload],
        )
        .context("failed to persist control plane snapshot")?;
    if let Some(event) = event {
        persist_timeline_event(transaction, event).with_context(|| {
            format!(
                "failed to persist event {} in control plane state",
                event.sequence,
            )
        })?;
    }
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

fn persist_timeline_event(
    connection: &Connection,
    event: &crate::types::TimelineEvent,
) -> Result<()> {
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
            i64::try_from(event.sequence).unwrap_or(i64::MAX),
            event.recorded_at.to_rfc3339(),
            event.runner_id.as_deref(),
            event.session_id.map(|value| value.to_string()),
            crate::helpers::event_kind_name_for_detail(&event.detail),
            i64::from(crate::helpers::is_approval_kind(kind)),
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

fn load_persisted_events_with_conn(
    connection: &Connection,
    query: &PersistedEventQuery,
) -> Result<Vec<crate::types::TimelineEvent>> {
    let mut sql_params: Vec<SqlValue> = Vec::new();
    let mut clauses = Vec::new();
    if let Some(after) = query.after {
        clauses.push("sequence > ?".to_owned());
        sql_params.push(SqlValue::Integer(i64::try_from(after)?));
    }
    if let Some(session_id) = query.session_id {
        clauses.push("session_id = ?".to_owned());
        sql_params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(runner_id) = query.runner_id.as_deref() {
        clauses.push("runner_id = ?".to_owned());
        sql_params.push(SqlValue::Text(runner_id.to_owned()));
    }
    if let Some(kind) = query.kind {
        clauses.push("kind = ?".to_owned());
        sql_params.push(SqlValue::Text(
            crate::helpers::event_kind_name(kind).to_owned(),
        ));
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
        sql_params.push(SqlValue::Integer(i64::try_from(
            limit.clamp(1, MAX_EVENT_LIST_LIMIT),
        )?));
    }

    let mut statement = connection.prepare(&sql)?;
    let payloads =
        statement.query_map(params_from_iter(sql_params), |row| row.get::<_, String>(0))?;
    let mut events = Vec::new();
    for payload in payloads {
        let payload = payload?;
        let event = serde_json::from_str::<crate::types::TimelineEvent>(&payload)?;
        events.push(event);
    }
    events.reverse();
    Ok(events)
}

fn load_latest_persisted_event_sequence_with_conn(
    connection: &Connection,
    query: &PersistedEventQuery,
) -> Result<Option<u64>> {
    let mut sql_params: Vec<SqlValue> = Vec::new();
    let mut clauses = Vec::new();
    if let Some(session_id) = query.session_id {
        clauses.push("session_id = ?".to_owned());
        sql_params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(runner_id) = query.runner_id.as_deref() {
        clauses.push("runner_id = ?".to_owned());
        sql_params.push(SqlValue::Text(runner_id.to_owned()));
    }
    if let Some(kind) = query.kind {
        clauses.push("kind = ?".to_owned());
        sql_params.push(SqlValue::Text(
            crate::helpers::event_kind_name(kind).to_owned(),
        ));
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
        .query_row(&sql, params_from_iter(sql_params), |row| {
            row.get::<_, i64>(0)
        })
        .optional()?;
    sequence
        .map(u64::try_from)
        .transpose()
        .context("persisted event sequence overflowed u64")
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- AuthPrincipal tests --

    #[test]
    fn auth_principal_shared_token_has_no_device_id() {
        let principal = AuthPrincipal::SharedToken;
        assert!(principal.created_by_device_id().is_none());
        assert!(principal.user_id().is_none());
    }

    #[test]
    fn auth_principal_device_exposes_device_id() {
        let device_id = Uuid::new_v4();
        let principal = AuthPrincipal::Device(TrustedDeviceRecord {
            device_id,
            name: "test-device".into(),
            kind: crate::types::DeviceKind::Cli,
            owner: false,
            created_by_device_id: None,
            created_at: Utc::now(),
            last_seen_at: Utc::now(),
        });
        assert_eq!(principal.created_by_device_id(), Some(device_id));
        assert!(principal.user_id().is_none());
    }

    #[test]
    fn auth_principal_user_exposes_user_id() {
        let principal = AuthPrincipal::User {
            user_id: "user-abc".into(),
        };
        assert!(principal.created_by_device_id().is_none());
        assert_eq!(principal.user_id(), Some("user-abc"));
    }

    // -- is_sha256_hex tests --

    #[test]
    fn is_sha256_hex_accepts_valid_hex() {
        let valid = "a".repeat(64);
        assert!(is_sha256_hex(&valid));
    }

    #[test]
    fn is_sha256_hex_rejects_wrong_length() {
        assert!(!is_sha256_hex("abc"));
        assert!(!is_sha256_hex(&"a".repeat(63)));
        assert!(!is_sha256_hex(&"a".repeat(65)));
    }

    #[test]
    fn is_sha256_hex_rejects_non_hex() {
        assert!(!is_sha256_hex(&"g".repeat(64)));
        assert!(!is_sha256_hex(&"z".repeat(64)));
    }

    // -- PersistedEventQuery default tests --

    #[test]
    fn persisted_event_query_defaults_are_none_or_false() {
        let query = PersistedEventQuery::default();
        assert!(query.after.is_none());
        assert!(query.limit.is_none());
        assert!(query.kind.is_none());
        assert!(query.session_id.is_none());
        assert!(query.runner_id.is_none());
        assert!(!query.approvals_only);
    }

    // -- Stream ticket mint/consume tests --

    async fn make_minimal_service() -> ControlPlaneService {
        let dir = tempfile::tempdir().unwrap();
        let config = ControlPlaneConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            public_base_url: None,
            service_name: "test".into(),
            runner_lease_ttl_secs: 30,
            profile_dir: dir.path().to_path_buf(),
            state_db_path: dir.path().join("state.db"),
            artifact_root_dir: dir.path().join("artifacts"),
            auth_token: None,
            bootstrap_secret: None,
            downloads_dir: None,
            quic_bind: None,
            quic_cert_pem: None,
            quic_key_pem: None,
        };
        ControlPlaneService::new(config, "test-version")
    }

    #[tokio::test]
    async fn stream_ticket_mint_and_consume_roundtrip() {
        let service = make_minimal_service().await;
        let path = "/v1/sessions/abc/events/stream".to_owned();

        let ticket = service
            .mint_stream_ticket(AuthPrincipal::SharedToken, path.clone(), 45)
            .await;

        assert!(ticket.starts_with("rcst_"));
        assert_eq!(ticket.len(), "rcst_".len() + 32 + 32); // "rcst_" + two uuid simples

        let principal = service
            .consume_stream_ticket(&ticket, &path)
            .await
            .expect("ticket should be consumed");
        assert!(matches!(principal, AuthPrincipal::SharedToken));
    }

    #[tokio::test]
    async fn stream_ticket_single_use() {
        let service = make_minimal_service().await;
        let path = "/v1/events".to_owned();

        let ticket = service
            .mint_stream_ticket(AuthPrincipal::SharedToken, path.clone(), 45)
            .await;

        let first = service.consume_stream_ticket(&ticket, &path).await;
        assert!(first.is_some());

        let second = service.consume_stream_ticket(&ticket, &path).await;
        assert!(second.is_none(), "ticket should be single-use");
    }

    #[tokio::test]
    async fn stream_ticket_rejects_wrong_path() {
        let service = make_minimal_service().await;

        let ticket = service
            .mint_stream_ticket(AuthPrincipal::SharedToken, "/v1/events".to_owned(), 45)
            .await;

        let result = service
            .consume_stream_ticket(&ticket, "/v1/sessions/x/events")
            .await;
        assert!(result.is_none(), "wrong path should reject ticket");
    }

    #[tokio::test]
    async fn stream_ticket_expired_is_rejected() {
        let service = make_minimal_service().await;
        let path = "/v1/events".to_owned();

        // TTL of 0 seconds = immediately expired
        let ticket = service
            .mint_stream_ticket(AuthPrincipal::SharedToken, path.clone(), 0)
            .await;

        // Give a tiny window for the clock to tick past expiry
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let result = service.consume_stream_ticket(&ticket, &path).await;
        assert!(result.is_none(), "expired ticket should be rejected");
    }

    #[tokio::test]
    async fn stream_ticket_preserves_user_principal() {
        let service = make_minimal_service().await;
        let path = "/v1/events".to_owned();

        let ticket = service
            .mint_stream_ticket(
                AuthPrincipal::User {
                    user_id: "user-42".into(),
                },
                path.clone(),
                45,
            )
            .await;

        let principal = service.consume_stream_ticket(&ticket, &path).await.unwrap();
        assert!(matches!(
            principal,
            AuthPrincipal::User { ref user_id } if user_id == "user-42"
        ));
    }

    // -- prune_expired_stream_tickets tests --

    #[test]
    fn prune_removes_expired_tickets() {
        let mut tickets = BTreeMap::new();

        tickets.insert(
            "ticket-expired".into(),
            StreamTicket {
                principal: AuthPrincipal::SharedToken,
                path: "/expired".to_owned(),
                expires_at: Utc::now() - Duration::seconds(10),
            },
        );
        tickets.insert(
            "ticket-valid".into(),
            StreamTicket {
                principal: AuthPrincipal::SharedToken,
                path: "/valid".to_owned(),
                expires_at: Utc::now() + Duration::seconds(60),
            },
        );

        prune_expired_stream_tickets(&mut tickets);
        assert_eq!(tickets.len(), 1);
        assert!(tickets.contains_key("ticket-valid"));
    }

    // -- SQLite schema tests --

    #[test]
    fn open_state_connection_creates_tables() {
        let conn = open_state_connection(std::path::Path::new(":memory:"))
            .expect("in-memory SQLite should open");

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"control_plane_snapshot".to_owned()));
        assert!(tables.contains(&"control_plane_events".to_owned()));
    }

    // -- Validation tests --

    #[test]
    fn validate_flags_non_loopback_without_auth() {
        let config = ControlPlaneConfig {
            bind: "0.0.0.0:8787".parse().unwrap(),
            public_base_url: None,
            service_name: "test".into(),
            runner_lease_ttl_secs: 30,
            profile_dir: std::path::PathBuf::from("/tmp/profile"),
            state_db_path: std::path::PathBuf::from("/tmp/state.db"),
            artifact_root_dir: std::path::PathBuf::from("/tmp/artifacts"),
            auth_token: None,
            bootstrap_secret: None,
            downloads_dir: None,
            quic_bind: None,
            quic_cert_pem: None,
            quic_key_pem: None,
        };
        let issues = validate_control_plane_config(&config);
        assert!(
            issues.iter().any(|i| i.contains("non-loopback")),
            "should flag non-loopback bind: {issues:?}"
        );
    }

    #[test]
    fn validate_ok_with_loopback_and_no_auth() {
        let config = ControlPlaneConfig {
            bind: "127.0.0.1:8787".parse().unwrap(),
            public_base_url: None,
            service_name: "test".into(),
            runner_lease_ttl_secs: 30,
            profile_dir: std::path::PathBuf::from("/tmp/profile"),
            state_db_path: std::path::PathBuf::from("/tmp/state.db"),
            artifact_root_dir: std::path::PathBuf::from("/tmp/artifacts"),
            auth_token: None,
            bootstrap_secret: None,
            downloads_dir: None,
            quic_bind: None,
            quic_cert_pem: None,
            quic_key_pem: None,
        };
        let issues = validate_control_plane_config(&config);
        assert!(
            issues.is_empty(),
            "loopback without auth should be fine: {issues:?}"
        );
    }

    #[test]
    fn is_local_control_plane_url_detects_localhost() {
        assert!(is_local_control_plane_url("http://localhost:8787"));
        assert!(is_local_control_plane_url("http://127.0.0.1:8787"));
        assert!(!is_local_control_plane_url("http://192.168.1.1:8787"));
        assert!(!is_local_control_plane_url("not-a-url"));
    }
}
