//! Helper functions for event matching, runner selection, and approval relay.

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionRequest, ApprovalRequestRecord,
    RunnerSessionCreateRequest, RunnerSessionRecord, RunnerSessionStateUpdateRequest, RunnerSnapshot,
    RunnerState, SessionState as RunnerSessionState,
};
use reqwest::Client;
use uuid::Uuid;

use crate::types::{
    ApiError, SessionState, TimelineEvent, TimelineEventDetail, TimelineEventKind,
};

// ---------------------------------------------------------------------------
// Event matching helpers
// ---------------------------------------------------------------------------

pub(crate) fn event_kind(detail: &TimelineEventDetail) -> TimelineEventKind {
    match detail {
        TimelineEventDetail::RunnerRegistered { .. } => TimelineEventKind::RunnerRegistered,
        TimelineEventDetail::RunnerHeartbeat { .. } => TimelineEventKind::RunnerHeartbeat,
        TimelineEventDetail::SessionCreated { .. } => TimelineEventKind::SessionCreated,
        TimelineEventDetail::SessionStateChanged { .. } => TimelineEventKind::SessionStateChanged,
        TimelineEventDetail::ApprovalRequested { .. } => TimelineEventKind::ApprovalRequested,
        TimelineEventDetail::ApprovalResolved { .. } => TimelineEventKind::ApprovalResolved,
        TimelineEventDetail::ArtifactCreated { .. } => TimelineEventKind::ArtifactCreated,
    }
}

pub(crate) fn event_matches_kind(event: &TimelineEvent, kind: Option<TimelineEventKind>) -> bool {
    kind.is_none_or(|kind| event_kind(&event.detail) == kind)
}

pub(crate) fn approval_event_matches(event: &TimelineEvent, kind: Option<TimelineEventKind>) -> bool {
    is_approval_event(event) && event_matches_kind(event, kind)
}

pub(crate) fn is_approval_event(event: &TimelineEvent) -> bool {
    matches!(
        event.detail,
        TimelineEventDetail::ApprovalRequested { .. }
            | TimelineEventDetail::ApprovalResolved { .. }
    )
}

// ---------------------------------------------------------------------------
// Artifact helpers
// ---------------------------------------------------------------------------

pub(crate) fn artifact_file_path(root: &Path, artifact: &crate::types::ArtifactRecord) -> PathBuf {
    root.join(artifact.session_id.to_string())
        .join(format!("{}-{}", artifact.artifact_id, artifact.file_name))
}

pub(crate) fn sanitize_artifact_component(raw: &str, fallback: &str) -> String {
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

// ---------------------------------------------------------------------------
// Runner helpers
// ---------------------------------------------------------------------------

pub(crate) fn runner_can_host(snapshot: &RunnerSnapshot, workspace_id: &str, lease_ttl_secs: u64) -> bool {
    runner_is_available(snapshot, lease_ttl_secs)
        && runner_has_capacity(snapshot)
        && snapshot
            .registration
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_id)
}

pub(crate) fn runner_has_capacity(snapshot: &RunnerSnapshot) -> bool {
    let max_parallel_sessions =
        usize::from(snapshot.registration.capabilities.max_parallel_sessions);
    snapshot.active_sessions + snapshot.queued_sessions < max_parallel_sessions
}

pub(crate) fn runner_is_available(snapshot: &RunnerSnapshot, lease_ttl_secs: u64) -> bool {
    !matches!(
        snapshot.state,
        RunnerState::Draining | RunnerState::Offline | RunnerState::Unhealthy
    ) && snapshot.last_seen_at >= Utc::now() - Duration::seconds(lease_ttl_secs as i64)
}

pub(crate) fn runner_rank(state: RunnerState) -> u8 {
    match state {
        RunnerState::Idle => 0,
        RunnerState::Busy => 1,
        RunnerState::Starting => 2,
        RunnerState::Draining => 3,
        RunnerState::Unhealthy => 4,
        RunnerState::Offline => 5,
    }
}

// ---------------------------------------------------------------------------
// Session state conversion helpers
// ---------------------------------------------------------------------------

pub(crate) fn session_state_after_approval(
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

pub(crate) fn session_state_from_runner(state: RunnerSessionState) -> SessionState {
    match state {
        RunnerSessionState::Pending | RunnerSessionState::Starting => SessionState::Assigned,
        RunnerSessionState::Running => SessionState::Running,
        RunnerSessionState::WaitingApproval => SessionState::WaitingApproval,
        RunnerSessionState::Completed => SessionState::Completed,
        RunnerSessionState::Failed => SessionState::Failed,
        RunnerSessionState::Cancelled => SessionState::Cancelled,
    }
}

pub(crate) fn session_state_to_runner(state: SessionState) -> RunnerSessionState {
    match state {
        SessionState::Pending => RunnerSessionState::Pending,
        SessionState::Assigned => RunnerSessionState::Starting,
        SessionState::Running => RunnerSessionState::Running,
        SessionState::WaitingApproval => RunnerSessionState::WaitingApproval,
        SessionState::Completed => RunnerSessionState::Completed,
        SessionState::Failed => RunnerSessionState::Failed,
        SessionState::Cancelled => RunnerSessionState::Cancelled,
    }
}

// ---------------------------------------------------------------------------
// Runner relay helpers
// ---------------------------------------------------------------------------

pub(crate) fn runner_public_base_url(runner: &RunnerSnapshot) -> Result<&str, ApiError> {
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

pub(crate) async fn dispatch_session_to_runner(
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

pub(crate) async fn update_runner_session_state(
    runner: &RunnerSnapshot,
    session_id: Uuid,
    request: &RunnerSessionStateUpdateRequest,
) -> Result<RunnerSessionRecord, ApiError> {
    let base_url = runner_public_base_url(runner)?;
    let client = Client::new();
    let response = client
        .post(format!(
            "{}/v1/sessions/{session_id}/state",
            base_url.trim_end_matches('/')
        ))
        .json(request)
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to update session `{session_id}` on runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })?
        .error_for_status()
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "runner `{}` rejected state update for session `{session_id}`: {error}",
                runner.registration.runner_id
            ))
        })?;
    response
        .json::<RunnerSessionRecord>()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to decode state update response from runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })
}

pub(crate) async fn relay_approval_to_runner(
    runner: &RunnerSnapshot,
    session_id: Uuid,
    request: &ApprovalCreateRequest,
) -> Result<ApprovalRequestRecord, ApiError> {
    let base_url = runner_public_base_url(runner)?;
    let client = Client::new();
    let response = client
        .post(format!(
            "{}/v1/sessions/{session_id}/approvals",
            base_url.trim_end_matches('/')
        ))
        .json(request)
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to relay approval for session `{session_id}` to runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })?
        .error_for_status()
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "runner `{}` rejected approval relay for session `{session_id}`: {error}",
                runner.registration.runner_id
            ))
        })?;
    response
        .json::<ApprovalRequestRecord>()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to decode approval relay response from runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })
}

pub(crate) async fn relay_approval_decision_to_runner(
    runner: &RunnerSnapshot,
    approval_id: Uuid,
    request: &ApprovalDecisionRequest,
) -> Result<ApprovalRequestRecord, ApiError> {
    let base_url = runner_public_base_url(runner)?;
    let client = Client::new();
    let response = client
        .post(format!(
            "{}/v1/approvals/{approval_id}/decision",
            base_url.trim_end_matches('/')
        ))
        .json(request)
        .send()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to relay approval decision `{approval_id}` to runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })?
        .error_for_status()
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "runner `{}` rejected approval decision `{approval_id}`: {error}",
                runner.registration.runner_id
            ))
        })?;
    response
        .json::<ApprovalRequestRecord>()
        .await
        .map_err(|error| {
            ApiError::bad_gateway(format!(
                "failed to decode approval decision response from runner `{}`: {error}",
                runner.registration.runner_id
            ))
        })
}

// ---------------------------------------------------------------------------
// Environment helpers
// ---------------------------------------------------------------------------

pub(crate) fn parse_socket_addr(raw: &str) -> Result<SocketAddr> {
    SocketAddr::from_str(raw).with_context(|| format!("invalid socket address `{raw}`"))
}

pub(crate) fn parse_env_number<T>(key: &str) -> Option<T>
where
    T: FromStr,
{
    read_env(key).and_then(|value| value.parse::<T>().ok())
}

pub(crate) fn read_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}
