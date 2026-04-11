//! HTTP handler functions for the control plane axum router.

use std::collections::BTreeSet;

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionRequest, ApprovalState,
    ListResponse, RunnerHeartbeat, RunnerRegistrationRequest, RunnerSessionCreateRequest,
    RunnerSessionStateUpdateRequest, RunnerSnapshot,
};
use uuid::Uuid;

use crate::helpers::{
    artifact_file_path, approval_event_matches, dispatch_session_to_runner,
    event_matches_kind, relay_approval_decision_to_runner, relay_approval_to_runner,
    runner_is_available, session_state_from_runner, session_state_to_runner,
    update_runner_session_state,
};
use crate::streams::{
    serve_approval_stream, serve_event_stream, serve_runner_approval_stream,
    serve_runner_event_stream, serve_session_approval_stream, serve_session_event_stream,
};
use crate::types::{
    ApiError, ArtifactCreateRequest, ArtifactRecord, ControlPlaneHealth, ControlPlaneMeta,
    CreateSessionRequest, EventStreamQuery, ListSessionsQuery, RecentEventsQuery,
    RunnerRegistrationResponse, SessionRecord, SessionStateUpdateRequest,
    TimelineEvent, TimelineEventDetail, TimelineEventDraft,
};
use crate::ControlPlaneService;

// ---------------------------------------------------------------------------
// Health / meta
// ---------------------------------------------------------------------------

pub(crate) async fn get_health(
    State(service): State<ControlPlaneService>,
) -> Json<ControlPlaneHealth> {
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

pub(crate) async fn get_meta(
    State(service): State<ControlPlaneService>,
) -> Json<ControlPlaneMeta> {
    Json(service.meta.clone())
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_recent_events(
    State(service): State<ControlPlaneService>,
    Query(query): Query<RecentEventsQuery>,
) -> Json<ListResponse<TimelineEvent>> {
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| event_matches_kind(event, query.kind))
        .await;
    Json(ListResponse {
        items: service
            .timeline
            .recent_filtered(query.after, query.limit, |event| {
                event_matches_kind(event, query.kind)
            })
            .await,
        latest_sequence,
    })
}

pub(crate) async fn list_session_events(
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
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| {
            event.session_id == Some(session_id) && event_matches_kind(event, query.kind)
        })
        .await;
    Ok(Json(ListResponse {
        items: service
            .timeline
            .recent_filtered(query.after, query.limit, |event| {
                event.session_id == Some(session_id) && event_matches_kind(event, query.kind)
            })
            .await,
        latest_sequence,
    }))
}

pub(crate) async fn list_runner_events(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
    Query(query): Query<RecentEventsQuery>,
) -> Result<Json<ListResponse<TimelineEvent>>, ApiError> {
    {
        let registry = service.registry.read().await;
        if !registry.runners.contains_key(&runner_id) {
            return Err(ApiError::not_found(format!(
                "runner `{runner_id}` was not found"
            )));
        }
    }
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| {
            event.runner_id.as_deref() == Some(runner_id.as_str())
                && event_matches_kind(event, query.kind)
        })
        .await;
    Ok(Json(ListResponse {
        items: service
            .timeline
            .recent_filtered(query.after, query.limit, |event| {
                event.runner_id.as_deref() == Some(runner_id.as_str())
                    && event_matches_kind(event, query.kind)
            })
            .await,
        latest_sequence,
    }))
}

// ---------------------------------------------------------------------------
// Event stream (WebSocket) handlers
// ---------------------------------------------------------------------------

pub(crate) async fn subscribe_events(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
    State(service): State<ControlPlaneService>,
) -> Response {
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| event_matches_kind(event, query.kind))
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| serve_event_stream(socket, subscription, backlog, query.kind))
}

pub(crate) async fn subscribe_session_events(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
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
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| {
                event.session_id == Some(session_id) && event_matches_kind(event, query.kind)
            })
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| {
        serve_session_event_stream(socket, subscription, backlog, session_id, query.kind)
    })
}

pub(crate) async fn subscribe_runner_events(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Response {
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| {
                event.runner_id.as_deref() == Some(runner_id.as_str())
                    && event_matches_kind(event, query.kind)
            })
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| {
        serve_runner_event_stream(socket, subscription, backlog, runner_id, query.kind)
    })
}

pub(crate) async fn subscribe_approvals(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
    State(service): State<ControlPlaneService>,
) -> Response {
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| {
                approval_event_matches(event, query.kind)
            })
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| serve_approval_stream(socket, subscription, backlog, query.kind))
}

// ---------------------------------------------------------------------------
// Runner handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_runners(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<RunnerSnapshot>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.runners.values().cloned().collect(),
        latest_sequence: None,
    })
}

pub(crate) async fn list_runner_approvals(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Result<Json<ListResponse<rc_runner::ApprovalRequestRecord>>, ApiError> {
    let registry = service.registry.read().await;
    let items = registry.list_runner_approvals(&runner_id)?;
    drop(registry);
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| {
            event.runner_id.as_deref() == Some(runner_id.as_str())
                && approval_event_matches(event, None)
        })
        .await;
    Ok(Json(ListResponse {
        items,
        latest_sequence,
    }))
}

pub(crate) async fn subscribe_runner_approvals(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Response {
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| {
                event.runner_id.as_deref() == Some(runner_id.as_str())
                    && approval_event_matches(event, query.kind)
            })
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| {
        serve_runner_approval_stream(socket, subscription, backlog, runner_id, query.kind)
    })
}

pub(crate) async fn subscribe_session_approvals(
    ws: WebSocketUpgrade,
    Query(query): Query<EventStreamQuery>,
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
    let subscription = service.timeline.subscribe();
    let backlog = if query.after.is_some() {
        service
            .timeline
            .replay_filtered(query.after, |event| {
                event.session_id == Some(session_id) && approval_event_matches(event, query.kind)
            })
            .await
    } else {
        Vec::new()
    };
    ws.on_upgrade(move |socket| {
        serve_session_approval_stream(socket, subscription, backlog, session_id, query.kind)
    })
}

pub(crate) async fn get_runner(
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

pub(crate) async fn register_runner(
    State(service): State<ControlPlaneService>,
    Json(request): Json<RunnerRegistrationRequest>,
) -> Json<RunnerRegistrationResponse> {
    let mut response = {
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
    dispatch_pending_sessions_for_runner(&service, &response.runner_id).await;
    if let Ok(snapshot) = {
        let registry = service.registry.read().await;
        registry.get_runner_snapshot(&response.runner_id)
    } {
        response.snapshot = snapshot;
    }
    Json(response)
}

pub(crate) async fn update_runner_heartbeat(
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
    dispatch_pending_sessions_for_runner(&service, &runner_id).await;
    let snapshot = {
        let registry = service.registry.read().await;
        registry.get_runner_snapshot(&runner_id)?
    };
    Ok(Json(snapshot))
}

// ---------------------------------------------------------------------------
// Session handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_sessions(
    State(service): State<ControlPlaneService>,
    Query(query): Query<ListSessionsQuery>,
) -> Json<ListResponse<SessionRecord>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.list_sessions_filtered(&query),
        latest_sequence: None,
    })
}

pub(crate) async fn get_session(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<SessionRecord>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(registry.get_session(session_id)?))
}

pub(crate) async fn list_runner_sessions(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
    Query(mut query): Query<ListSessionsQuery>,
) -> Result<Json<ListResponse<SessionRecord>>, ApiError> {
    let registry = service.registry.read().await;
    if !registry.runners.contains_key(&runner_id) {
        return Err(ApiError::not_found(format!(
            "runner `{runner_id}` was not found"
        )));
    }
    query.runner_id = Some(runner_id);
    Ok(Json(ListResponse {
        items: registry.list_sessions_filtered(&query),
        latest_sequence: None,
    }))
}

pub(crate) async fn list_runner_artifacts(
    State(service): State<ControlPlaneService>,
    AxumPath(runner_id): AxumPath<String>,
) -> Result<Json<ListResponse<ArtifactRecord>>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(ListResponse {
        items: registry.list_runner_artifacts(&runner_id)?,
        latest_sequence: None,
    }))
}

pub(crate) async fn update_session_state(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<SessionStateUpdateRequest>,
) -> Result<Json<SessionRecord>, ApiError> {
    let existing = {
        let registry = service.registry.read().await;
        registry.get_session(session_id)?
    };
    let requested_state = request.state;
    let metadata = request.metadata.clone();

    let runner_update = if let Some(runner_id) = existing.owner_runner_id.as_deref() {
        let runner =
            {
                let registry = service.registry.read().await;
                registry.runners.get(runner_id).cloned().ok_or_else(|| {
                    ApiError::not_found(format!("runner `{runner_id}` was not found"))
                })?
            };
        Some(
            update_runner_session_state(
                &runner,
                session_id,
                &RunnerSessionStateUpdateRequest {
                    state: session_state_to_runner(requested_state),
                    metadata: metadata.clone(),
                },
            )
            .await?,
        )
    } else {
        None
    };

    let (updated, previous_state) = {
        let mut registry = service.registry.write().await;
        let updated_at = runner_update
            .as_ref()
            .map_or_else(Utc::now, |record| record.updated_at);
        registry.apply_session_state_update(
            session_id,
            runner_update.as_ref().map_or(requested_state, |record| {
                session_state_from_runner(record.state)
            }),
            runner_update
                .as_ref()
                .map(|record| record.metadata.clone())
                .unwrap_or(metadata),
            updated_at,
        )?
    };
    let _ = service
        .publish_event(TimelineEventDraft {
            runner_id: updated.owner_runner_id.clone(),
            session_id: Some(updated.session_id),
            detail: TimelineEventDetail::SessionStateChanged {
                previous_state,
                state: updated.state,
            },
        })
        .await;
    Ok(Json(updated))
}

pub(crate) async fn list_session_approvals(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<ListResponse<rc_runner::ApprovalRequestRecord>>, ApiError> {
    let registry = service.registry.read().await;
    let items = registry.list_session_approvals(session_id)?;
    drop(registry);
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| {
            event.session_id == Some(session_id) && approval_event_matches(event, None)
        })
        .await;
    Ok(Json(ListResponse {
        items,
        latest_sequence,
    }))
}

pub(crate) async fn list_session_artifacts(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Result<Json<ListResponse<ArtifactRecord>>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(ListResponse {
        items: registry.list_session_artifacts(session_id)?,
        latest_sequence: None,
    }))
}

pub(crate) async fn create_session(
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

// ---------------------------------------------------------------------------
// Artifact handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_artifacts(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<ArtifactRecord>> {
    let registry = service.registry.read().await;
    Json(ListResponse {
        items: registry.list_artifacts(),
        latest_sequence: None,
    })
}

pub(crate) async fn get_artifact(
    State(service): State<ControlPlaneService>,
    AxumPath(artifact_id): AxumPath<Uuid>,
) -> Result<Json<ArtifactRecord>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(registry.get_artifact(artifact_id)?))
}

pub(crate) async fn create_artifact(
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

pub(crate) async fn download_artifact(
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

// ---------------------------------------------------------------------------
// Approval handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_approvals(
    State(service): State<ControlPlaneService>,
) -> Json<ListResponse<rc_runner::ApprovalRequestRecord>> {
    let registry = service.registry.read().await;
    let items = registry.list_approvals();
    drop(registry);
    let latest_sequence = service
        .timeline
        .latest_filtered(|event| approval_event_matches(event, None))
        .await;
    Json(ListResponse {
        items,
        latest_sequence,
    })
}

pub(crate) async fn get_approval(
    State(service): State<ControlPlaneService>,
    AxumPath(approval_id): AxumPath<Uuid>,
) -> Result<Json<rc_runner::ApprovalRequestRecord>, ApiError> {
    let registry = service.registry.read().await;
    Ok(Json(registry.get_approval(approval_id)?))
}

pub(crate) async fn create_approval(
    State(service): State<ControlPlaneService>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalCreateRequest>,
) -> Result<(StatusCode, Json<rc_runner::ApprovalRequestRecord>), ApiError> {
    let planned = {
        let registry = service.registry.read().await;
        registry.plan_approval(session_id, request)?
    };
    if let Some(runner) = planned.owner_runner.as_ref() {
        let relay_request = ApprovalCreateRequest {
            approval_id: Some(planned.approval.approval_id),
            title: planned.approval.title.clone(),
            description: planned.approval.description.clone(),
            metadata: planned.approval.metadata.clone(),
        };
        let relayed = relay_approval_to_runner(runner, session_id, &relay_request).await?;
        if relayed.approval_id != planned.approval.approval_id {
            return Err(ApiError::bad_gateway(format!(
                "runner `{}` acknowledged approval `{}` instead of `{}`",
                runner.registration.runner_id, relayed.approval_id, planned.approval.approval_id
            )));
        }
        if relayed.session_id != session_id || relayed.runner_id != runner.registration.runner_id {
            return Err(ApiError::bad_gateway(format!(
                "runner `{}` returned mismatched approval routing for session `{session_id}`",
                runner.registration.runner_id
            )));
        }
    }
    let (approval, transition) = {
        let mut registry = service.registry.write().await;
        registry.commit_planned_approval(planned)?
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
    if let Some(transition) = transition {
        let _ = service
            .publish_event(TimelineEventDraft {
                runner_id: transition.runner_id,
                session_id: Some(transition.session_id),
                detail: TimelineEventDetail::SessionStateChanged {
                    previous_state: transition.previous_state,
                    state: transition.state,
                },
            })
            .await;
    }
    Ok((StatusCode::CREATED, Json(approval)))
}

pub(crate) async fn apply_approval_decision(
    State(service): State<ControlPlaneService>,
    AxumPath(approval_id): AxumPath<Uuid>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<rc_runner::ApprovalRequestRecord>, ApiError> {
    let planned = {
        let registry = service.registry.read().await;
        registry.plan_approval_decision(approval_id, request)?
    };
    if let Some(runner) = planned.owner_runner.as_ref() {
        let relay_request = ApprovalDecisionRequest {
            decision: match planned.approval.state {
                ApprovalState::Approved => ApprovalDecision::Approved,
                ApprovalState::Denied => ApprovalDecision::Denied,
                ApprovalState::Cancelled => ApprovalDecision::Cancelled,
                ApprovalState::Pending => {
                    return Err(ApiError::internal(format!(
                        "approval `{approval_id}` remained pending during decision relay"
                    )));
                }
            },
            responder: planned.approval.responder.clone(),
            note: planned.approval.note.clone(),
        };
        let relayed =
            relay_approval_decision_to_runner(runner, planned.approval.approval_id, &relay_request)
                .await?;
        if relayed.approval_id != planned.approval.approval_id {
            return Err(ApiError::bad_gateway(format!(
                "runner `{}` acknowledged approval decision for `{}` instead of `{}`",
                runner.registration.runner_id, relayed.approval_id, planned.approval.approval_id
            )));
        }
        if relayed.state != planned.approval.state {
            return Err(ApiError::bad_gateway(format!(
                "runner `{}` returned approval state `{:?}` instead of `{:?}` for `{}`",
                runner.registration.runner_id,
                relayed.state,
                planned.approval.state,
                planned.approval.approval_id
            )));
        }
    }
    let (approval, transition) = {
        let mut registry = service.registry.write().await;
        registry.commit_planned_approval_decision(planned)?
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
    if let Some(transition) = transition {
        let _ = service
            .publish_event(TimelineEventDraft {
                runner_id: transition.runner_id,
                session_id: Some(transition.session_id),
                detail: TimelineEventDetail::SessionStateChanged {
                    previous_state: transition.previous_state,
                    state: transition.state,
                },
            })
            .await;
    }
    Ok(Json(approval))
}

// ---------------------------------------------------------------------------
// Internal dispatch helper
// ---------------------------------------------------------------------------

async fn dispatch_pending_sessions_for_runner(service: &ControlPlaneService, runner_id: &str) {
    let mut skipped_session_ids = BTreeSet::new();

    loop {
        let planned = {
            let registry = service.registry.read().await;
            registry
                .plan_next_pending_session_for_runner(
                    runner_id,
                    service.runner_lease_ttl_secs,
                    &skipped_session_ids,
                )
                .ok()
                .flatten()
        };
        let Some(planned) = planned else {
            break;
        };

        let request = RunnerSessionCreateRequest {
            session_id: Some(planned.session_id),
            workspace_id: planned.workspace_id.clone(),
            metadata: planned.metadata.clone(),
        };
        let dispatched =
            if let Ok(dispatched) = dispatch_session_to_runner(&planned.runner, &request).await {
                dispatched
            } else {
                skipped_session_ids.insert(planned.session_id);
                continue;
            };

        let committed = {
            let mut registry = service.registry.write().await;
            registry
                .commit_pending_session_dispatch(
                    planned.session_id,
                    &planned.runner.registration.runner_id,
                    &dispatched,
                )
                .ok()
        };
        let Some((record, previous_state)) = committed else {
            skipped_session_ids.insert(planned.session_id);
            continue;
        };

        let _ = service
            .publish_event(TimelineEventDraft {
                runner_id: record.owner_runner_id.clone(),
                session_id: Some(record.session_id),
                detail: TimelineEventDetail::SessionStateChanged {
                    previous_state,
                    state: record.state,
                },
            })
            .await;
    }
}
