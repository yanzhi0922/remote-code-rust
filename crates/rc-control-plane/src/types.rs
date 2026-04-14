//! Shared types, constants, and configuration for the control plane.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecisionRequest, ApprovalState, RunnerSessionCommandRequest,
    RunnerSessionCreateRequest, RunnerSessionStateUpdateRequest, RunnerSnapshot, RunnerState,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(crate) const DEFAULT_BIND: &str = "127.0.0.1:8787";
pub(crate) const DEFAULT_RUNNER_LEASE_TTL_SECS: u64 = 30;
pub(crate) const DEFAULT_EVENT_HISTORY_LIMIT: usize = 256;
pub(crate) const DEFAULT_EVENT_LIST_LIMIT: usize = 50;
pub(crate) const MAX_EVENT_LIST_LIMIT: usize = 200;
pub(crate) const DEFAULT_PAIRING_TTL_SECS: u64 = 600;
pub(crate) const MAX_PAIRING_TTL_SECS: u64 = 3600;
pub(crate) const EVENT_STREAM_BUFFER: usize = 256;
pub(crate) const PHASE: &str = "phase5-remote-stable";

// ---------------------------------------------------------------------------
// Public configuration types
// ---------------------------------------------------------------------------

/// CLI / env-var overrides for control plane configuration.
#[derive(Debug, Clone, Default)]
pub struct ControlPlaneConfigOverrides {
    /// Bind address override.
    pub bind: Option<SocketAddr>,
    /// Public base URL override.
    pub public_base_url: Option<String>,
    /// Service name override.
    pub service_name: Option<String>,
    /// Runner lease TTL override in seconds.
    pub runner_lease_ttl_secs: Option<u64>,
    /// Profile directory override.
    pub profile_dir: Option<PathBuf>,
    /// Shared bearer token required for remote access.
    pub auth_token: Option<String>,
    /// Bootstrap secret used to claim the first trusted device.
    pub bootstrap_secret: Option<String>,
}

/// Full control plane configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    /// Address to bind the HTTP server to.
    pub bind: SocketAddr,
    /// Publicly reachable URL (for SSE / WebSocket endpoints).
    pub public_base_url: Option<String>,
    /// Service name for identification.
    pub service_name: String,
    /// Runner lease TTL in seconds.
    pub runner_lease_ttl_secs: u64,
    /// Profile directory for persistent data.
    pub profile_dir: PathBuf,
    /// SQLite database path for state persistence.
    pub state_db_path: PathBuf,
    /// Root directory for artifact storage.
    pub artifact_root_dir: PathBuf,
    /// Shared bearer token required for remote access.
    pub auth_token: Option<String>,
    /// Bootstrap secret used to claim the first trusted device.
    pub bootstrap_secret: Option<String>,
}

/// Metadata returned by the `/meta` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneMeta {
    /// Service name.
    pub service: String,
    /// Service version.
    pub version: String,
    /// Development phase identifier.
    pub phase: String,
    /// Bind address.
    pub bind: String,
    /// Public base URL.
    pub public_base_url: Option<String>,
    /// Runner lease TTL.
    pub runner_lease_ttl_secs: u64,
    /// Profile directory path.
    pub profile_dir: String,
    /// SQLite database path.
    pub state_db_path: String,
    /// Artifact root directory path.
    pub artifact_root_dir: String,
    /// Whether the `/v1/*` API requires a bearer token.
    pub auth_required: bool,
    /// Whether a bootstrap secret is configured.
    pub bootstrap_secret_configured: bool,
}

/// Status report for the `doctor` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneStatus {
    /// Whether the configuration is valid.
    pub ok: bool,
    /// Blocking issues that must be resolved before the service is considered safe to expose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
    /// Bind address.
    pub bind: String,
    /// Public base URL.
    pub public_base_url: Option<String>,
    /// Service name.
    pub service_name: String,
    /// Runner lease TTL.
    pub runner_lease_ttl_secs: u64,
    /// Profile directory path.
    pub profile_dir: String,
    /// SQLite database path.
    pub state_db_path: String,
    /// Artifact root directory path.
    pub artifact_root_dir: String,
    /// Whether the `/v1/*` API requires a bearer token.
    pub auth_required: bool,
    /// Whether a bootstrap secret is configured.
    pub bootstrap_secret_configured: bool,
    /// Development phase.
    pub phase: &'static str,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneHealth {
    /// Whether the service is healthy.
    pub ok: bool,
    /// Service name.
    pub service: String,
    /// Development phase.
    pub phase: String,
    /// Total registered runners.
    pub runner_count: usize,
    /// Currently available runners.
    pub available_runner_count: usize,
    /// Total sessions.
    pub session_count: usize,
    /// Total artifacts.
    pub artifact_count: usize,
    /// Number of pending runner pull commands.
    pub queued_runner_command_count: usize,
    /// Whether the `/v1/*` API currently requires authentication.
    pub auth_required: bool,
    /// Whether a bootstrap secret is configured.
    pub bootstrap_secret_configured: bool,
    /// Whether the owner device has already claimed the control plane.
    pub owner_claimed: bool,
    /// Number of trusted devices.
    pub device_count: usize,
}

// ---------------------------------------------------------------------------
// Trusted-device / pairing types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Runner,
    Browser,
    #[default]
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDeviceRecord {
    pub device_id: Uuid,
    pub name: String,
    pub kind: DeviceKind,
    pub owner: bool,
    pub created_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapClaimRequest {
    pub bootstrap_secret: String,
    pub device_name: String,
    #[serde(default)]
    pub device_kind: DeviceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapClaimResponse {
    pub device: TrustedDeviceRecord,
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingOfferCreateRequest {
    pub device_name: String,
    #[serde(default)]
    pub device_kind: DeviceKind,
    pub expires_in_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingOfferCreateResponse {
    pub offer_id: Uuid,
    pub device_name: String,
    pub device_kind: DeviceKind,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub pairing_secret: String,
    pub pairing_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingAcceptRequest {
    pub offer_id: Uuid,
    pub pairing_secret: String,
    pub device_name: Option<String>,
    pub device_kind: Option<DeviceKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingAcceptResponse {
    pub device: TrustedDeviceRecord,
    pub access_token: String,
}

// ---------------------------------------------------------------------------
// Push-token registration (mobile devices)
// ---------------------------------------------------------------------------

/// Push notification platform.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushPlatform {
    #[default]
    Apns,
    Fcm,
}

/// Request body for `POST /v1/devices/push-token`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushTokenRegistrationRequest {
    pub push_token: String,
    #[serde(default)]
    pub platform: PushPlatform,
}

/// Response body for `POST /v1/devices/push-token`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushTokenRegistrationResponse {
    pub registered: bool,
}

// ---------------------------------------------------------------------------
// Runner pull-command types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunnerQueuedCommandBody {
    CreateSession {
        request: RunnerSessionCreateRequest,
    },
    UpdateSessionState {
        session_id: Uuid,
        request: RunnerSessionStateUpdateRequest,
    },
    SessionCommand {
        session_id: Uuid,
        request: RunnerSessionCommandRequest,
    },
    CreateApproval {
        session_id: Uuid,
        request: ApprovalCreateRequest,
    },
    ApplyApprovalDecision {
        approval_id: Uuid,
        request: ApprovalDecisionRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerQueuedCommand {
    pub command_id: Uuid,
    pub runner_id: String,
    pub created_at: DateTime<Utc>,
    pub body: RunnerQueuedCommandBody,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerCommandPullResponse {
    pub commands: Vec<RunnerQueuedCommand>,
}

// ---------------------------------------------------------------------------
// Public session types
// ---------------------------------------------------------------------------

/// Lifecycle state of a control-plane session.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Waiting for runner assignment.
    #[default]
    Pending,
    /// Assigned to a runner, not yet started.
    Assigned,
    /// Currently running.
    Running,
    /// Waiting for user approval.
    WaitingApproval,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled.
    Cancelled,
}

/// Persistent record of a control-plane session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Session identifier.
    pub session_id: Uuid,
    /// Workspace identifier.
    pub workspace_id: String,
    /// Runner currently owning this session.
    pub owner_runner_id: Option<String>,
    /// Current session state.
    pub state: SessionState,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Session payload exposed by the HTTP API with dynamic runner availability metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionView {
    #[serde(flatten)]
    pub session: SessionRecord,
    #[serde(default)]
    pub owner_runner_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_runner_state: Option<RunnerState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_runner_last_seen_at: Option<DateTime<Utc>>,
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
pub struct SessionStateUpdateRequest {
    pub state: SessionState,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// Public runner types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistrationResponse {
    pub runner_id: String,
    pub registered_at: DateTime<Utc>,
    pub lease_ttl_secs: u64,
    pub snapshot: RunnerSnapshot,
}

// ---------------------------------------------------------------------------
// Public artifact types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public timeline types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence: u64,
    pub recorded_at: DateTime<Utc>,
    pub runner_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub detail: TimelineEventDetail,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    Assistant,
    User,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonPresenceState {
    Online,
    Offline,
    Reconnecting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEventCreateRequest {
    pub detail: RuntimeEventDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeEventDetail {
    MessageDelta {
        role: MessageRole,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    MessageCommitted {
        role: MessageRole,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    ToolProgress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_time_seconds: Option<u64>,
    },
    ToolFinished {
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    ArtifactManifest {
        artifact_ids: Vec<Uuid>,
    },
    RuntimeError {
        message: String,
    },
    DaemonPresenceChanged {
        state: DaemonPresenceState,
    },
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
    SessionStateChanged {
        previous_state: SessionState,
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
    MessageDelta {
        role: MessageRole,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    MessageCommitted {
        role: MessageRole,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    ToolProgress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_time_seconds: Option<u64>,
    },
    ToolFinished {
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    ArtifactManifest {
        artifact_ids: Vec<Uuid>,
    },
    RuntimeError {
        message: String,
    },
    DaemonPresenceChanged {
        state: DaemonPresenceState,
    },
}

// ---------------------------------------------------------------------------
// Internal timeline types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimelineEventKind {
    RunnerRegistered,
    RunnerHeartbeat,
    SessionCreated,
    SessionStateChanged,
    ApprovalRequested,
    ApprovalResolved,
    ArtifactCreated,
    MessageDelta,
    MessageCommitted,
    ToolStarted,
    ToolProgress,
    ToolFinished,
    ArtifactManifest,
    RuntimeError,
    DaemonPresenceChanged,
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineEventDraft {
    pub(crate) runner_id: Option<String>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) detail: TimelineEventDetail,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionStateTransition {
    pub(crate) runner_id: Option<String>,
    pub(crate) session_id: Uuid,
    pub(crate) previous_state: SessionState,
    pub(crate) state: SessionState,
}

impl From<RuntimeEventDetail> for TimelineEventDetail {
    fn from(value: RuntimeEventDetail) -> Self {
        match value {
            RuntimeEventDetail::MessageDelta {
                role,
                delta,
                message_id,
            } => Self::MessageDelta {
                role,
                delta,
                message_id,
            },
            RuntimeEventDetail::MessageCommitted {
                role,
                text,
                message_id,
            } => Self::MessageCommitted {
                role,
                text,
                message_id,
            },
            RuntimeEventDetail::ToolStarted {
                tool_call_id,
                tool_name,
            } => Self::ToolStarted {
                tool_call_id,
                tool_name,
            },
            RuntimeEventDetail::ToolProgress {
                tool_call_id,
                tool_name,
                delta,
                elapsed_time_seconds,
            } => Self::ToolProgress {
                tool_call_id,
                tool_name,
                delta,
                elapsed_time_seconds,
            },
            RuntimeEventDetail::ToolFinished {
                tool_call_id,
                tool_name,
                is_error,
                summary,
            } => Self::ToolFinished {
                tool_call_id,
                tool_name,
                is_error,
                summary,
            },
            RuntimeEventDetail::ArtifactManifest { artifact_ids } => {
                Self::ArtifactManifest { artifact_ids }
            }
            RuntimeEventDetail::RuntimeError { message } => Self::RuntimeError { message },
            RuntimeEventDetail::DaemonPresenceChanged { state } => {
                Self::DaemonPresenceChanged { state }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal query types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RecentEventsQuery {
    pub(crate) after: Option<u64>,
    pub(crate) limit: Option<usize>,
    pub(crate) kind: Option<TimelineEventKind>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ListSessionsQuery {
    pub(crate) runner_id: Option<String>,
    pub(crate) workspace_id: Option<String>,
    pub(crate) state: Option<SessionState>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct EventStreamQuery {
    pub(crate) after: Option<u64>,
    pub(crate) kind: Option<TimelineEventKind>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RunnerCommandPullQuery {
    pub(crate) limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Internal error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ErrorEnvelope {
    pub(crate) error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ErrorDetail {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message,
        }
    }

    pub(crate) fn conflict(message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        }
    }

    pub(crate) fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message,
        }
    }

    pub(crate) fn service_unavailable(message: String) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message,
        }
    }

    pub(crate) fn bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "bad_gateway",
            message,
        }
    }

    pub(crate) fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message,
        }
    }

    pub(crate) fn internal(message: String) -> Self {
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
