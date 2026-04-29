//! In-memory registry for runners, sessions, approvals, and artifacts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecisionRequest, ApprovalRequestRecord, ApprovalState,
    RunnerHeartbeat, RunnerRegistrationRequest, RunnerSessionRecord, RunnerSnapshot, RunnerState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::helpers::{
    runner_can_host, runner_rank, sanitize_artifact_component, session_state_after_approval,
    session_state_from_runner,
};
use crate::types::{
    ApiError, ArtifactCreateRequest, ArtifactRecord, BootstrapClaimRequest, CreateSessionRequest,
    DEFAULT_PAIRING_TTL_SECS, DeviceKind, ListSessionsQuery, MAX_PAIRING_TTL_SECS,
    PairingAcceptRequest, PairingOfferCreateRequest, PushPlatform, PushTokenRegistrationRequest,
    RunnerQueuedCommand, RunnerQueuedCommandBody, SessionRecord, SessionState,
    SessionStateTransition, TimelineEvent, TimelineEventDraft, TrustedDeviceRecord,
};

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct Registry {
    pub(crate) runners: BTreeMap<String, RunnerSnapshot>,
    pub(crate) sessions: BTreeMap<Uuid, SessionRecord>,
    pub(crate) approvals: BTreeMap<Uuid, ApprovalRequestRecord>,
    pub(crate) artifacts: BTreeMap<Uuid, ArtifactRecord>,
    #[serde(default)]
    pub(crate) trusted_devices: BTreeMap<Uuid, StoredTrustedDevice>,
    #[serde(default)]
    pub(crate) pairing_offers: BTreeMap<Uuid, StoredPairingOffer>,
    #[serde(default)]
    pub(crate) owner_device_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) queued_runner_commands: BTreeMap<String, VecDeque<RunnerQueuedCommand>>,
    /// Push tokens keyed by device_id for sending push notifications.
    #[serde(default)]
    pub(crate) push_tokens: BTreeMap<Uuid, StoredPushToken>,
}

/// Stored push notification token for a trusted device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredPushToken {
    pub(crate) device_id: Uuid,
    pub(crate) push_token: String,
    pub(crate) platform: PushPlatform,
    pub(crate) registered_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredTrustedDevice {
    pub(crate) device_id: Uuid,
    pub(crate) name: String,
    pub(crate) kind: DeviceKind,
    pub(crate) owner: bool,
    pub(crate) created_by_device_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
    pub(crate) token_hash: String,
}

impl StoredTrustedDevice {
    fn public_record(&self) -> TrustedDeviceRecord {
        TrustedDeviceRecord {
            device_id: self.device_id,
            name: self.name.clone(),
            kind: self.kind,
            owner: self.owner,
            created_by_device_id: self.created_by_device_id,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredPairingOffer {
    pub(crate) offer_id: Uuid,
    pub(crate) created_by_device_id: Option<Uuid>,
    pub(crate) device_name: String,
    pub(crate) device_kind: DeviceKind,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) pairing_secret_hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedSession {
    pub(crate) record: SessionRecord,
    pub(crate) owner_runner: Option<RunnerSnapshot>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingSessionDispatch {
    pub(crate) session_id: Uuid,
    pub(crate) workspace_id: String,
    pub(crate) metadata: BTreeMap<String, String>,
    pub(crate) runner: RunnerSnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedApproval {
    pub(crate) approval: ApprovalRequestRecord,
    pub(crate) owner_runner: Option<RunnerSnapshot>,
    pub(crate) next_session_state: SessionState,
    pub(crate) transition: Option<SessionStateTransition>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedApprovalDecision {
    pub(crate) approval: ApprovalRequestRecord,
    pub(crate) owner_runner: Option<RunnerSnapshot>,
    pub(crate) next_session_state: Option<SessionState>,
    pub(crate) transition: Option<SessionStateTransition>,
}

// ---------------------------------------------------------------------------
// TimelineStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct TimelineStore {
    history_limit: usize,
    tx: broadcast::Sender<TimelineEvent>,
    inner: Arc<Mutex<TimelineSnapshot>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimelineSnapshot {
    next_sequence: u64,
    history: VecDeque<TimelineEvent>,
}

impl TimelineSnapshot {
    /// Return an iterator over the historical timeline events.
    pub(crate) fn history(&self) -> impl Iterator<Item = &TimelineEvent> {
        self.history.iter()
    }
}

impl TimelineStore {
    pub(crate) fn new(history_limit: usize, buffer: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer.max(1));
        Self {
            history_limit: history_limit.max(1),
            tx,
            inner: Arc::new(Mutex::new(TimelineSnapshot {
                next_sequence: 1,
                history: VecDeque::with_capacity(history_limit.max(1)),
            })),
        }
    }

    pub(crate) fn from_snapshot(
        history_limit: usize,
        buffer: usize,
        snapshot: TimelineSnapshot,
    ) -> Self {
        let (tx, _) = broadcast::channel(buffer.max(1));
        Self {
            history_limit: history_limit.max(1),
            tx,
            inner: Arc::new(Mutex::new(snapshot)),
        }
    }

    pub(crate) async fn snapshot(&self) -> TimelineSnapshot {
        self.inner.lock().await.clone()
    }

    pub(crate) async fn publish(&self, draft: TimelineEventDraft) -> TimelineEvent {
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

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<TimelineEvent> {
        self.tx.subscribe()
    }
}

fn sha256_hex(raw: &str) -> String {
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

fn mint_secret(prefix: &str) -> String {
    format!(
        "{prefix}_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn normalize_device_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "device name cannot be empty".to_owned(),
        ));
    }
    let normalized = trimmed.chars().take(128).collect::<String>();
    if normalized.is_empty() {
        return Err(ApiError::bad_request(
            "device name cannot be empty".to_owned(),
        ));
    }
    Ok(normalized)
}

// ---------------------------------------------------------------------------
// Registry impl
// ---------------------------------------------------------------------------

impl Registry {
    pub(crate) fn owner_claimed(&self) -> bool {
        self.owner_device_id.is_some()
    }

    pub(crate) fn trusted_device_count(&self) -> usize {
        self.trusted_devices.len()
    }

    pub(crate) fn list_trusted_devices(&self) -> Vec<TrustedDeviceRecord> {
        self.trusted_devices
            .values()
            .map(StoredTrustedDevice::public_record)
            .collect()
    }

    pub(crate) fn authenticate_device_token(
        &mut self,
        access_token: &str,
    ) -> Option<TrustedDeviceRecord> {
        let token_hash = sha256_hex(access_token);
        let device_id = self
            .trusted_devices
            .values()
            .find(|device| device.token_hash == token_hash)
            .map(|device| device.device_id)?;
        let now = Utc::now();
        let device = self.trusted_devices.get_mut(&device_id)?;
        device.last_seen_at = now;
        Some(device.public_record())
    }

    pub(crate) fn bootstrap_claim(
        &mut self,
        expected_secret_hash: Option<&str>,
        request: BootstrapClaimRequest,
    ) -> Result<(TrustedDeviceRecord, String), ApiError> {
        if self.owner_claimed() {
            return Err(ApiError::conflict(
                "the control plane owner device has already been claimed".to_owned(),
            ));
        }
        let Some(expected_secret_hash) = expected_secret_hash else {
            return Err(ApiError::service_unavailable(
                "bootstrap claiming is disabled because no bootstrap secret is configured"
                    .to_owned(),
            ));
        };
        if sha256_hex(request.bootstrap_secret.trim()) != expected_secret_hash {
            return Err(ApiError::unauthorized(
                "bootstrap secret is missing or invalid".to_owned(),
            ));
        }

        let now = Utc::now();
        let access_token = mint_secret("rcdt");
        let device_id = Uuid::new_v4();
        let record = StoredTrustedDevice {
            device_id,
            name: normalize_device_name(&request.device_name)?,
            kind: request.device_kind,
            owner: true,
            created_by_device_id: None,
            created_at: now,
            last_seen_at: now,
            token_hash: sha256_hex(&access_token),
        };
        self.owner_device_id = Some(device_id);
        self.trusted_devices.insert(device_id, record.clone());
        Ok((record.public_record(), access_token))
    }

    pub(crate) fn create_pairing_offer(
        &mut self,
        created_by_device_id: Option<Uuid>,
        request: PairingOfferCreateRequest,
    ) -> Result<(StoredPairingOffer, String), ApiError> {
        if !self.owner_claimed() {
            return Err(ApiError::conflict(
                "claim the owner device before creating pairing offers".to_owned(),
            ));
        }
        self.prune_expired_pairing_offers();

        let expires_in_secs = request
            .expires_in_secs
            .unwrap_or(DEFAULT_PAIRING_TTL_SECS)
            .clamp(60, MAX_PAIRING_TTL_SECS);
        let created_at = Utc::now();
        let pairing_secret = mint_secret("rcpo");
        let offer = StoredPairingOffer {
            offer_id: Uuid::new_v4(),
            created_by_device_id,
            device_name: normalize_device_name(&request.device_name)?,
            device_kind: request.device_kind,
            created_at,
            expires_at: created_at + Duration::seconds(expires_in_secs as i64),
            pairing_secret_hash: sha256_hex(&pairing_secret),
        };
        self.pairing_offers.insert(offer.offer_id, offer.clone());
        Ok((offer, pairing_secret))
    }

    pub(crate) fn accept_pairing_offer(
        &mut self,
        request: PairingAcceptRequest,
    ) -> Result<(TrustedDeviceRecord, String), ApiError> {
        self.prune_expired_pairing_offers();

        let offer = self
            .pairing_offers
            .remove(&request.offer_id)
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "pairing offer `{}` was not found",
                    request.offer_id
                ))
            })?;
        if offer.expires_at < Utc::now() {
            return Err(ApiError::conflict(format!(
                "pairing offer `{}` has expired",
                request.offer_id
            )));
        }
        if sha256_hex(request.pairing_secret.trim()) != offer.pairing_secret_hash {
            return Err(ApiError::unauthorized(
                "pairing secret is missing or invalid".to_owned(),
            ));
        }

        let now = Utc::now();
        let access_token = mint_secret("rcdt");
        let record = StoredTrustedDevice {
            device_id: Uuid::new_v4(),
            name: normalize_device_name(
                request
                    .device_name
                    .as_deref()
                    .unwrap_or(offer.device_name.as_str()),
            )?,
            kind: request.device_kind.unwrap_or(offer.device_kind),
            owner: false,
            created_by_device_id: offer.created_by_device_id,
            created_at: now,
            last_seen_at: now,
            token_hash: sha256_hex(&access_token),
        };
        let public_record = record.public_record();
        self.trusted_devices.insert(record.device_id, record);
        Ok((public_record, access_token))
    }

    fn prune_expired_pairing_offers(&mut self) {
        let now = Utc::now();
        self.pairing_offers
            .retain(|_, offer| offer.expires_at >= now);
    }

    pub(crate) fn queued_runner_command_count(&self) -> usize {
        self.queued_runner_commands
            .values()
            .map(VecDeque::len)
            .sum::<usize>()
    }

    pub(crate) fn register_runner(
        &mut self,
        request: RunnerRegistrationRequest,
        lease_ttl_secs: u64,
    ) -> crate::types::RunnerRegistrationResponse {
        let now = Utc::now();
        let runner_id = request.runner_id.clone();
        let snapshot = RunnerSnapshot {
            registration: request.clone(),
            state: RunnerState::Idle,
            active_sessions: 0,
            queued_sessions: 0,
            registered_at: now,
            last_seen_at: now,
        };
        self.runners.insert(runner_id.clone(), snapshot);

        // Recalculate session counts from existing sessions so that a
        // runner re-registering (e.g. after a brief restart) does not
        // appear to have zero sessions, which would allow the dispatch
        // loop to over-assign work beyond its capacity.
        self.refresh_runner_session_counts(&runner_id, now);

        let snapshot = self
            .runners
            .get(&runner_id)
            .cloned()
            .expect("runner was just inserted");
        crate::types::RunnerRegistrationResponse {
            runner_id,
            registered_at: now,
            lease_ttl_secs,
            snapshot,
        }
    }

    pub(crate) fn apply_heartbeat(
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

    pub(crate) fn plan_session(
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

    pub(crate) fn commit_session(
        &mut self,
        record: SessionRecord,
    ) -> Result<SessionRecord, ApiError> {
        if self.sessions.contains_key(&record.session_id) {
            return Err(ApiError::conflict(format!(
                "session `{}` already exists",
                record.session_id
            )));
        }
        self.sessions.insert(record.session_id, record.clone());
        if let Some(runner_id) = &record.owner_runner_id {
            self.refresh_runner_session_counts(runner_id, record.updated_at);
        }
        Ok(record)
    }

    pub(crate) fn get_runner_snapshot(&self, runner_id: &str) -> Result<RunnerSnapshot, ApiError> {
        self.runners
            .get(runner_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("runner `{runner_id}` was not found")))
    }

    pub(crate) fn get_session(&self, session_id: Uuid) -> Result<SessionRecord, ApiError> {
        self.sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))
    }

    pub(crate) fn list_sessions_filtered(&self, query: &ListSessionsQuery) -> Vec<SessionRecord> {
        self.sessions
            .values()
            .filter(|session| {
                query
                    .runner_id
                    .as_deref()
                    .is_none_or(|runner_id| session.owner_runner_id.as_deref() == Some(runner_id))
            })
            .filter(|session| {
                query
                    .workspace_id
                    .as_deref()
                    .is_none_or(|workspace_id| session.workspace_id == workspace_id)
            })
            .filter(|session| query.state.is_none_or(|state| session.state == state))
            .cloned()
            .collect()
    }

    pub(crate) fn apply_session_state_update(
        &mut self,
        session_id: Uuid,
        state: SessionState,
        metadata: BTreeMap<String, String>,
        updated_at: DateTime<Utc>,
    ) -> Result<(SessionRecord, SessionState), ApiError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
        let previous_state = session.state;
        session.state = state;
        session.updated_at = updated_at;
        session.metadata.extend(metadata);
        let updated = session.clone();
        let owner_runner_id = updated.owner_runner_id.clone();
        if let Some(runner_id) = owner_runner_id.as_deref() {
            self.refresh_runner_session_counts(runner_id, updated_at);
        }
        Ok((updated, previous_state))
    }

    pub(crate) fn refresh_runner_session_counts(
        &mut self,
        runner_id: &str,
        timestamp: DateTime<Utc>,
    ) {
        let (active_sessions, queued_sessions) = self
            .sessions
            .values()
            .filter(|session| session.owner_runner_id.as_deref() == Some(runner_id))
            .fold((0usize, 0usize), |(active, queued), session| {
                let active = if matches!(
                    session.state,
                    SessionState::Assigned | SessionState::Running | SessionState::WaitingApproval
                ) {
                    active + 1
                } else {
                    active
                };
                let queued = if matches!(session.state, SessionState::Pending) {
                    queued + 1
                } else {
                    queued
                };
                (active, queued)
            });

        if let Some(snapshot) = self.runners.get_mut(runner_id) {
            snapshot.active_sessions = active_sessions;
            snapshot.queued_sessions = queued_sessions;
            snapshot.state = if active_sessions > 0 {
                RunnerState::Busy
            } else {
                RunnerState::Idle
            };
            snapshot.last_seen_at = snapshot.last_seen_at.max(timestamp);
        }
    }

    pub(crate) fn list_approvals(&self) -> Vec<ApprovalRequestRecord> {
        self.approvals.values().cloned().collect()
    }

    pub(crate) fn list_artifacts(&self) -> Vec<ArtifactRecord> {
        self.artifacts.values().cloned().collect()
    }

    pub(crate) fn list_runner_approvals(
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

    pub(crate) fn list_runner_artifacts(
        &self,
        runner_id: &str,
    ) -> Result<Vec<ArtifactRecord>, ApiError> {
        if !self.runners.contains_key(runner_id) {
            return Err(ApiError::not_found(format!(
                "runner `{runner_id}` was not found"
            )));
        }
        Ok(self
            .artifacts
            .values()
            .filter(|artifact| artifact.runner_id.as_deref() == Some(runner_id))
            .cloned()
            .collect())
    }

    pub(crate) fn get_artifact(&self, artifact_id: Uuid) -> Result<ArtifactRecord, ApiError> {
        self.artifacts
            .get(&artifact_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("artifact `{artifact_id}` was not found")))
    }

    pub(crate) fn get_approval(
        &self,
        approval_id: Uuid,
    ) -> Result<ApprovalRequestRecord, ApiError> {
        self.approvals
            .get(&approval_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("approval `{approval_id}` was not found")))
    }

    pub(crate) fn list_session_approvals(
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

    pub(crate) fn list_session_artifacts(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ArtifactRecord>, ApiError> {
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

    pub(crate) fn register_artifact(
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

    pub(crate) fn plan_approval(
        &self,
        session_id: Uuid,
        request: ApprovalCreateRequest,
    ) -> Result<PlannedApproval, ApiError> {
        let session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
        let now = Utc::now();
        let next_session_state = SessionState::WaitingApproval;
        let owner_runner_id = session.owner_runner_id.clone();
        let approval = ApprovalRequestRecord {
            approval_id: request.approval_id.unwrap_or_else(Uuid::new_v4),
            session_id,
            runner_id: owner_runner_id.clone().unwrap_or_default(),
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
        let transition = (session.state != next_session_state).then(|| SessionStateTransition {
            runner_id: owner_runner_id.clone(),
            session_id,
            previous_state: session.state,
            state: next_session_state,
        });
        let owner_runner = owner_runner_id
            .as_ref()
            .and_then(|runner_id| self.runners.get(runner_id))
            .cloned();
        Ok(PlannedApproval {
            approval,
            owner_runner,
            next_session_state,
            transition,
        })
    }

    pub(crate) fn commit_planned_approval(
        &mut self,
        planned: PlannedApproval,
    ) -> Result<(ApprovalRequestRecord, Option<SessionStateTransition>), ApiError> {
        if self.approvals.contains_key(&planned.approval.approval_id) {
            return Err(ApiError::conflict(format!(
                "approval `{}` already exists",
                planned.approval.approval_id
            )));
        }

        let session = self
            .sessions
            .get_mut(&planned.approval.session_id)
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "session `{}` was not found",
                    planned.approval.session_id
                ))
            })?;
        session.state = planned.next_session_state;
        session.updated_at = planned.approval.updated_at;
        let owner_runner_id = session.owner_runner_id.clone();

        self.approvals
            .insert(planned.approval.approval_id, planned.approval.clone());
        if let Some(runner_id) = owner_runner_id.as_deref() {
            self.refresh_runner_session_counts(runner_id, planned.approval.updated_at);
        }

        Ok((planned.approval, planned.transition))
    }

    pub(crate) fn plan_approval_decision(
        &self,
        approval_id: Uuid,
        request: ApprovalDecisionRequest,
    ) -> Result<PlannedApprovalDecision, ApiError> {
        let approval = self.approvals.get(&approval_id).ok_or_else(|| {
            ApiError::not_found(format!("approval `{approval_id}` was not found"))
        })?;
        if !matches!(approval.state, ApprovalState::Pending) {
            return Err(ApiError::conflict(format!(
                "approval `{approval_id}` is already resolved"
            )));
        }

        let now = Utc::now();
        let mut updated = approval.clone();
        updated.state = request.decision.into();
        updated.updated_at = now;
        updated.responded_at = Some(now);
        updated.responder = request.responder;
        updated.note = request.note;

        let has_pending_approvals = self.approvals.values().any(|candidate| {
            candidate.session_id == updated.session_id
                && candidate.approval_id != updated.approval_id
                && matches!(candidate.state, ApprovalState::Pending)
        });

        let (next_session_state, transition, owner_runner) =
            if let Some(session) = self.sessions.get(&updated.session_id) {
                let state = session_state_after_approval(request.decision, has_pending_approvals);
                let owner_runner = session
                    .owner_runner_id
                    .as_ref()
                    .and_then(|runner_id| self.runners.get(runner_id))
                    .cloned();
                let transition = (session.state != state).then(|| SessionStateTransition {
                    runner_id: session.owner_runner_id.clone(),
                    session_id: session.session_id,
                    previous_state: session.state,
                    state,
                });
                (Some(state), transition, owner_runner)
            } else {
                (None, None, None)
            };

        Ok(PlannedApprovalDecision {
            approval: updated,
            owner_runner,
            next_session_state,
            transition,
        })
    }

    pub(crate) fn commit_planned_approval_decision(
        &mut self,
        planned: PlannedApprovalDecision,
    ) -> Result<(ApprovalRequestRecord, Option<SessionStateTransition>), ApiError> {
        let approval = self
            .approvals
            .get_mut(&planned.approval.approval_id)
            .ok_or_else(|| {
                ApiError::not_found(format!(
                    "approval `{}` was not found",
                    planned.approval.approval_id
                ))
            })?;
        if !matches!(approval.state, ApprovalState::Pending) {
            return Err(ApiError::conflict(format!(
                "approval `{}` is already resolved",
                planned.approval.approval_id
            )));
        }
        *approval = planned.approval.clone();
        let updated = approval.clone();

        let owner_runner_id = if let Some(session) = self.sessions.get_mut(&updated.session_id) {
            if let Some(next_state) = planned.next_session_state {
                session.state = next_state;
            }
            session.updated_at = updated.updated_at;
            session.owner_runner_id.clone()
        } else {
            None
        };
        if let Some(runner_id) = owner_runner_id.as_deref() {
            self.refresh_runner_session_counts(runner_id, updated.updated_at);
        }

        Ok((updated, planned.transition))
    }

    pub(crate) fn plan_next_pending_session_for_runner(
        &self,
        runner_id: &str,
        lease_ttl_secs: u64,
        skipped_session_ids: &BTreeSet<Uuid>,
    ) -> Result<Option<PendingSessionDispatch>, ApiError> {
        let runner = self.get_runner_snapshot(runner_id)?;
        Ok(self
            .sessions
            .values()
            .filter(|session| matches!(session.state, SessionState::Pending))
            .filter(|session| session.owner_runner_id.is_none())
            .filter(|session| !skipped_session_ids.contains(&session.session_id))
            .filter_map(|session| {
                let selected = self
                    .select_runner(&session.workspace_id, None, lease_ttl_secs)
                    .ok()?;
                (selected.as_deref() == Some(runner_id)).then(|| PendingSessionDispatch {
                    session_id: session.session_id,
                    workspace_id: session.workspace_id.clone(),
                    metadata: session.metadata.clone(),
                    runner: runner.clone(),
                })
            })
            .min_by_key(|dispatch| {
                self.sessions
                    .get(&dispatch.session_id)
                    .map(|session| (session.created_at, session.session_id))
            }))
    }

    pub(crate) fn commit_pending_session_dispatch(
        &mut self,
        session_id: Uuid,
        runner_id: &str,
        dispatched: &RunnerSessionRecord,
    ) -> Result<(SessionRecord, SessionState), ApiError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| ApiError::not_found(format!("session `{session_id}` was not found")))?;
        if !matches!(session.state, SessionState::Pending) || session.owner_runner_id.is_some() {
            return Err(ApiError::conflict(format!(
                "session `{session_id}` is no longer pending dispatch"
            )));
        }

        let previous_state = session.state;
        session.owner_runner_id = Some(runner_id.to_owned());
        session.state = session_state_from_runner(dispatched.state);
        session.metadata = dispatched.metadata.clone();
        session.updated_at = dispatched.updated_at;
        let updated = session.clone();
        self.refresh_runner_session_counts(runner_id, updated.updated_at);
        Ok((updated, previous_state))
    }

    pub(crate) fn select_runner(
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

    pub(crate) fn enqueue_runner_command(
        &mut self,
        runner_id: &str,
        body: RunnerQueuedCommandBody,
    ) -> Result<RunnerQueuedCommand, ApiError> {
        if !self.runners.contains_key(runner_id) {
            return Err(ApiError::not_found(format!(
                "runner `{runner_id}` was not found"
            )));
        }
        let command = RunnerQueuedCommand {
            command_id: Uuid::new_v4(),
            runner_id: runner_id.to_owned(),
            created_at: Utc::now(),
            body,
        };
        self.queued_runner_commands
            .entry(runner_id.to_owned())
            .or_default()
            .push_back(command.clone());
        Ok(command)
    }

    pub(crate) fn pull_runner_commands(
        &mut self,
        runner_id: &str,
        limit: usize,
    ) -> Result<Vec<RunnerQueuedCommand>, ApiError> {
        if !self.runners.contains_key(runner_id) {
            return Err(ApiError::not_found(format!(
                "runner `{runner_id}` was not found"
            )));
        }
        let queue = self
            .queued_runner_commands
            .entry(runner_id.to_owned())
            .or_default();
        let mut commands = Vec::new();
        for _ in 0..limit.max(1) {
            let Some(command) = queue.pop_front() else {
                break;
            };
            commands.push(command);
        }
        if queue.is_empty() {
            self.queued_runner_commands.remove(runner_id);
        }
        Ok(commands)
    }

    /// Remove push token for a device (cascade on device removal).
    #[allow(dead_code)]
    pub(crate) fn remove_push_token(&mut self, device_id: Uuid) {
        self.push_tokens.remove(&device_id);
    }

    /// Register or update a push notification token for a trusted device.
    pub(crate) fn register_push_token(
        &mut self,
        device_id: Uuid,
        request: PushTokenRegistrationRequest,
    ) -> Result<bool, ApiError> {
        if !self.trusted_devices.contains_key(&device_id) {
            return Err(ApiError::not_found(format!(
                "device `{device_id}` is not a trusted device"
            )));
        }
        let now = Utc::now();
        let is_new = !self.push_tokens.contains_key(&device_id);
        self.push_tokens.insert(
            device_id,
            StoredPushToken {
                device_id,
                push_token: request.push_token,
                platform: request.platform,
                registered_at: self
                    .push_tokens
                    .get(&device_id)
                    .map(|t| t.registered_at)
                    .unwrap_or(now),
                updated_at: now,
            },
        );
        Ok(is_new)
    }
}
