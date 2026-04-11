use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use rc_runner::{ApprovalState, RunnerSnapshot, RunnerState};
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
pub(crate) const EVENT_STREAM_BUFFER: usize = 256;
pub(crate) const PHASE: &str = "phase3-remote-skeleton";

// ---------------------------------------------------------------------------
// Public configuration types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public session types
// ---------------------------------------------------------------------------

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
