//! Hosted session orchestration for remote-code runners.
//!
//! This crate owns control-plane command sync, session process hosting,
//! stream-json relay, approval forwarding, and artifact upload. The
//! `remote-code-runner` binary should stay focused on CLI parsing and
//! startup wiring.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use rc_control_plane::{
    RunnerCommandPullResponse, RunnerQueuedCommandBody, RuntimeEventCreateRequest,
    RuntimeEventDetail, SessionState as ControlPlaneSessionState, SessionStateUpdateRequest,
    runtime_event_detail_from_stream_json_value,
};
use rc_runner::{
    ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionRequest, ApprovalRequestRecord,
    RunnerApi, RunnerApiEvent, RunnerConfig, RunnerSessionCommandRequest, RunnerSessionRecord,
    RunnerSessionStateUpdateRequest, SessionState as RunnerSessionState,
    register_with_control_plane, send_heartbeat,
};
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command as ProcessCommand};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{info, warn};
use uuid::Uuid;

pub fn effective_heartbeat_interval(
    configured_interval_secs: u64,
    lease_ttl_secs: u64,
) -> Duration {
    Duration::from_secs(
        configured_interval_secs
            .max(1)
            .min((lease_ttl_secs / 2).max(1)),
    )
}

pub fn next_retry_delay(current: Duration) -> Duration {
    current.saturating_mul(2).min(Duration::from_secs(30))
}

async fn wait_for_shutdown_or_timeout(
    duration: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        () = tokio::time::sleep(duration) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

/// Maximum buffered events per session before dropping oldest to prevent unbounded memory growth.
const MAX_EVENT_BUFFER_PER_SESSION: usize = 500;
const MAX_ARTIFACT_UPLOAD_BYTES: u64 = 10 * 1024 * 1024;
const MAX_SESSION_INPUT_QUEUE: usize = 256;

#[derive(Clone)]
pub struct HostedSessionManager {
    api: RunnerApi,
    control_plane_url: String,
    remote_code_bin: PathBuf,
    profile_dir: PathBuf,
    client: reqwest::Client,
    auth_token: Option<String>,
    sessions: Arc<Mutex<HashMap<Uuid, HostedSessionHandle>>>,
    /// Buffered runtime events that failed to reach the control plane.
    /// Keyed by session_id; flushed on next successful post.
    event_buffer: Arc<Mutex<HashMap<Uuid, VecDeque<RuntimeEventDetail>>>>,
}

#[derive(Clone)]
struct HostedSessionHandle {
    input_tx: mpsc::Sender<String>,
    request_to_approval: Arc<Mutex<HashMap<String, Uuid>>>,
    approval_to_request: Arc<Mutex<HashMap<Uuid, String>>>,
    task_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// Accumulated tool input deltas for file-writing tools, keyed by tool_use_id.
    /// When tool_finished arrives, the accumulated input is parsed to extract the file path.
    pending_tool_inputs: Arc<Mutex<HashMap<String, String>>>,
    /// Workspace root directory for resolving relative file paths.
    workspace_dir: PathBuf,
}

impl HostedSessionManager {
    pub fn new(
        api: RunnerApi,
        control_plane_url: String,
        remote_code_bin: PathBuf,
        profile_dir: PathBuf,
        auth_token: Option<String>,
    ) -> Self {
        Self {
            api,
            control_plane_url,
            remote_code_bin,
            profile_dir,
            client: reqwest::Client::new(),
            auth_token,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(
        self,
        mut event_rx: mpsc::Receiver<RunnerApiEvent>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.handle_runner_event(event).await {
                        warn!("hosted session manager failed to handle runner event: {error:#}");
                    }
                }
            }
        }
    }

    async fn handle_runner_event(&self, event: RunnerApiEvent) -> Result<()> {
        match event {
            RunnerApiEvent::SessionCreated(session) => self.spawn_hosted_session(session).await,
            RunnerApiEvent::ApprovalResolved(approval) => {
                self.forward_approval_resolution(approval).await
            }
            RunnerApiEvent::SessionCommand {
                session_id,
                command,
            } => self.forward_session_command(session_id, command).await,
        }
    }

    async fn spawn_hosted_session(&self, session: RunnerSessionRecord) -> Result<()> {
        if self.sessions.lock().await.contains_key(&session.session_id) {
            return Ok(());
        }
        let workspace = self
            .api
            .meta()
            .snapshot
            .registration
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == session.workspace_id)
            .cloned()
            .ok_or_else(|| anyhow!("workspace `{}` was not found", session.workspace_id))?;
        let mut command = ProcessCommand::new(&self.remote_code_bin);
        command
            .arg("--cwd")
            .arg(&workspace.root_dir)
            .arg("--profile-dir")
            .arg(&self.profile_dir)
            .arg("--session-id")
            .arg(session.session_id.to_string())
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--print")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to spawn hosted remote-code process using `{}`",
                self.remote_code_bin.display()
            )
        })?;
        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("hosted remote-code stdin was not piped"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("hosted remote-code stdout was not piped"))?;
        let child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("hosted remote-code stderr was not piped"))?;

        let (input_tx, input_rx) = mpsc::channel(MAX_SESSION_INPUT_QUEUE);
        let handle = HostedSessionHandle {
            input_tx,
            request_to_approval: Arc::new(Mutex::new(HashMap::new())),
            approval_to_request: Arc::new(Mutex::new(HashMap::new())),
            task_handles: Arc::new(Mutex::new(Vec::new())),
            pending_tool_inputs: Arc::new(Mutex::new(HashMap::new())),
            workspace_dir: workspace.root_dir.clone(),
        };
        self.sessions
            .lock()
            .await
            .insert(session.session_id, handle.clone());

        self.post_runtime_event(
            session.session_id,
            RuntimeEventDetail::DaemonPresenceChanged {
                state: rc_control_plane::DaemonPresenceState::Online,
            },
        )
        .await?;

        let input_join = tokio::spawn(write_session_input(
            session.session_id,
            child_stdin,
            input_rx,
        ));
        handle.task_handles.lock().await.push(input_join);

        let manager = self.clone();
        let stdout_handle_for_spawn = handle.clone();
        let stdout_join: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            if let Err(error) = manager
                .read_hosted_session_stdout(
                    session.session_id,
                    child_stdout,
                    stdout_handle_for_spawn,
                )
                .await
            {
                warn!(
                    "hosted session `{}` stdout loop failed: {error:#}",
                    session.session_id
                );
            }
        });
        handle.task_handles.lock().await.push(stdout_join);

        let manager = self.clone();
        let stderr_join: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            let mut lines = BufReader::new(child_stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!("hosted session `{}` stderr: {line}", session.session_id);
            }
            let _ = manager;
        });
        handle.task_handles.lock().await.push(stderr_join);

        let manager = self.clone();
        let exit_join: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            let exit = child.wait().await;
            if let Err(error) = manager
                .handle_hosted_session_exit(session.session_id, exit)
                .await
            {
                warn!(
                    "hosted session `{}` exit handling failed: {error:#}",
                    session.session_id
                );
            }
        });
        handle.task_handles.lock().await.push(exit_join);

        info!(
            "spawned hosted session `{}` for workspace `{}`",
            session.session_id, session.workspace_id
        );
        Ok(())
    }

    async fn read_hosted_session_stdout(
        &self,
        session_id: Uuid,
        stdout: tokio::process::ChildStdout,
        handle: HostedSessionHandle,
    ) -> Result<()> {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            if let Err(error) = self.handle_protocol_line(session_id, &line, &handle).await {
                warn!("hosted session `{session_id}` ignored protocol line error: {error:#}");
            }
        }
        Ok(())
    }

    async fn handle_protocol_line(
        &self,
        session_id: Uuid,
        line: &str,
        handle: &HostedSessionHandle,
    ) -> Result<()> {
        // Broadcast raw event to any direct-connect WebSocket subscribers
        self.api.publish_stream_event(session_id, line);

        let value: serde_json::Value =
            serde_json::from_str(line).with_context(|| format!("invalid protocol line: {line}"))?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        match kind {
            "system" => {
                let subtype = value
                    .get("subtype")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if subtype == "session_state_changed"
                    && let Some(state) = value.get("state").and_then(serde_json::Value::as_str)
                {
                    self.apply_runtime_session_state(session_id, state).await?;
                }
            }
            "control_request" => {
                self.handle_control_request(session_id, &value, handle)
                    .await?;
            }
            "control_cancel_request" => {
                if let Some(request_id) =
                    value.get("request_id").and_then(serde_json::Value::as_str)
                {
                    self.cancel_pending_approval(session_id, request_id, handle)
                        .await?;
                }
            }
            "tool_started" => {
                self.handle_tool_started(&value, handle).await;
                if let Some(detail) = runtime_event_detail_from_stream_json_value(&value) {
                    self.post_runtime_event(session_id, detail).await?;
                }
            }
            "tool_progress" => {
                self.handle_tool_progress(&value, handle).await;
                if let Some(detail) = runtime_event_detail_from_stream_json_value(&value) {
                    self.post_runtime_event(session_id, detail).await?;
                }
            }
            "tool_finished" => {
                if let Some(detail) = runtime_event_detail_from_stream_json_value(&value) {
                    self.post_runtime_event(session_id, detail.clone()).await?;
                }
                self.handle_tool_finished(session_id, &value, handle)
                    .await?;
            }
            _ => {
                if let Some(detail) = runtime_event_detail_from_stream_json_value(&value) {
                    self.post_runtime_event(session_id, detail).await?;
                }
            }
        }
        Ok(())
    }

    async fn apply_runtime_session_state(
        &self,
        session_id: Uuid,
        runtime_state: &str,
    ) -> Result<()> {
        let Some((runner_state, control_state)) = map_runtime_session_state(runtime_state) else {
            return Ok(());
        };
        self.api
            .apply_session_state_update_direct(
                session_id,
                RunnerSessionStateUpdateRequest {
                    state: runner_state,
                    metadata: HashMap::new().into_iter().collect(),
                },
            )
            .await?;
        if let Err(error) = self
            .post_control_plane_session_state(session_id, control_state)
            .await
        {
            warn!("failed to post control-plane session state for {session_id}: {error:#}");
        }
        Ok(())
    }

    async fn handle_control_request(
        &self,
        session_id: Uuid,
        value: &serde_json::Value,
        handle: &HostedSessionHandle,
    ) -> Result<()> {
        let request_id = value
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("control_request missing request_id"))?;
        let request = value
            .get("request")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow!("control_request missing request payload"))?;
        let subtype = request
            .get("subtype")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if subtype != "can_use_tool" {
            return Ok(());
        }

        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("request_id".to_owned(), request_id.to_owned());
        if let Some(tool_name) = request
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
        {
            metadata.insert("tool_name".to_owned(), tool_name.to_owned());
        }
        if let Some(tool_use_id) = request
            .get("tool_use_id")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
        {
            metadata.insert("tool_use_id".to_owned(), tool_use_id.to_owned());
        }
        if let Some(blocked_path) = request
            .get("blocked_path")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
        {
            metadata.insert("blocked_path".to_owned(), blocked_path.to_owned());
        }

        let approval = match self
            .create_control_plane_approval(
                session_id,
                ApprovalCreateRequest {
                    approval_id: None,
                    title: request
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("Approve tool use")
                        .to_owned(),
                    description: request
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Runtime requested approval.")
                        .to_owned(),
                    metadata,
                },
            )
            .await
        {
            Ok(approval) => approval,
            Err(error) => {
                warn!(
                    "failed to create control-plane approval for session {session_id}: {error:#}"
                );
                let payload = serde_json::json!({
                    "type": "control_response",
                    "request_id": request_id,
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": {
                            "behavior": "deny",
                            "message": format!("Remote approval service is unavailable: {error}"),
                        }
                    }
                });
                self.send_json_line(session_id, payload).await?;
                return Ok(());
            }
        };
        handle
            .request_to_approval
            .lock()
            .await
            .insert(request_id.to_owned(), approval.approval_id);
        handle
            .approval_to_request
            .lock()
            .await
            .insert(approval.approval_id, request_id.to_owned());
        Ok(())
    }

    async fn cancel_pending_approval(
        &self,
        session_id: Uuid,
        request_id: &str,
        handle: &HostedSessionHandle,
    ) -> Result<()> {
        let approval_id = handle.request_to_approval.lock().await.remove(request_id);
        if let Some(approval_id) = approval_id {
            handle.approval_to_request.lock().await.remove(&approval_id);
            if let Err(error) = self
                .resolve_control_plane_approval(
                    approval_id,
                    ApprovalDecisionRequest {
                        decision: ApprovalDecision::Cancelled,
                        responder: Some("runner".to_owned()),
                        note: Some(format!(
                            "Runtime cancelled approval request `{request_id}` for session `{session_id}`."
                        )),
                    },
                )
                .await
            {
                warn!("failed to cancel control-plane approval {approval_id}: {error:#}");
            }
        }
        Ok(())
    }

    async fn forward_approval_resolution(&self, approval: ApprovalRequestRecord) -> Result<()> {
        let Some(handle) = self
            .sessions
            .lock()
            .await
            .get(&approval.session_id)
            .cloned()
        else {
            return Ok(());
        };
        let request_id = handle
            .approval_to_request
            .lock()
            .await
            .remove(&approval.approval_id);
        let Some(request_id) = request_id else {
            return Ok(());
        };
        handle.request_to_approval.lock().await.remove(&request_id);

        let behavior = match approval.state {
            rc_runner::ApprovalState::Approved => "allow",
            rc_runner::ApprovalState::Denied | rc_runner::ApprovalState::Cancelled => {
                "deny"
            }
            rc_runner::ApprovalState::Pending => return Ok(()),
        };
        let note = approval
            .note
            .clone()
            .unwrap_or_else(|| match approval.state {
                rc_runner::ApprovalState::Approved => "Approved remotely.".to_owned(),
                rc_runner::ApprovalState::Denied => "Denied remotely.".to_owned(),
                rc_runner::ApprovalState::Cancelled => "Cancelled remotely.".to_owned(),
                rc_runner::ApprovalState::Pending => String::new(),
            });
        let payload = control_response_payload(&request_id, behavior, &note);
        self.send_json_line(approval.session_id, payload).await
    }

    async fn forward_session_command(
        &self,
        session_id: Uuid,
        command: RunnerSessionCommandRequest,
    ) -> Result<()> {
        let payload = match command {
            RunnerSessionCommandRequest::SendPrompt { content } => serde_json::json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": content,
                }
            }),
            RunnerSessionCommandRequest::Interrupt => serde_json::json!({
                "type": "control_request",
                "request": {
                    "subtype": "interrupt"
                }
            }),
        };
        self.send_json_line(session_id, payload).await
    }

    async fn send_json_line(&self, session_id: Uuid, payload: serde_json::Value) -> Result<()> {
        let sessions = self.sessions.lock().await;
        let handle = sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| anyhow!("hosted session `{session_id}` is not active"))?;
        drop(sessions);
        handle
            .input_tx
            .send(format!("{}\n", serde_json::to_string(&payload)?))
            .await
            .map_err(|_| anyhow!("hosted session `{session_id}` input channel is closed"))
    }

    async fn handle_hosted_session_exit(
        &self,
        session_id: Uuid,
        exit: std::io::Result<std::process::ExitStatus>,
    ) -> Result<()> {
        self.sessions.lock().await.remove(&session_id);
        self.post_runtime_event(
            session_id,
            RuntimeEventDetail::DaemonPresenceChanged {
                state: rc_control_plane::DaemonPresenceState::Offline,
            },
        )
        .await?;
        let (runner_state, control_state, runtime_error) = match exit {
            Ok(status) if status.success() => (
                RunnerSessionState::Completed,
                ControlPlaneSessionState::Completed,
                None,
            ),
            Ok(status) => (
                RunnerSessionState::Failed,
                ControlPlaneSessionState::Failed,
                Some(format!("Hosted session exited with status `{status}`.")),
            ),
            Err(error) => (
                RunnerSessionState::Failed,
                ControlPlaneSessionState::Failed,
                Some(format!("Hosted session wait failed: {error}")),
            ),
        };
        self.api
            .apply_session_state_update_direct(
                session_id,
                RunnerSessionStateUpdateRequest {
                    state: runner_state,
                    metadata: HashMap::new().into_iter().collect(),
                },
            )
            .await?;
        if let Some(message) = runtime_error {
            self.post_runtime_event(session_id, RuntimeEventDetail::RuntimeError { message })
                .await?;
        }
        if let Err(error) = self
            .post_control_plane_session_state(session_id, control_state)
            .await
        {
            warn!(
                "failed to post terminal control-plane session state for {session_id}: {error:#}"
            );
        }
        Ok(())
    }

    async fn post_control_plane_session_state(
        &self,
        session_id: Uuid,
        state: ControlPlaneSessionState,
    ) -> Result<()> {
        let response = self
            .control_plane_post(format!(
                "{}/v1/sessions/{session_id}/state",
                self.control_plane_url.trim_end_matches('/')
            ))
            .json(&SessionStateUpdateRequest {
                state,
                metadata: std::collections::BTreeMap::new(),
            })
            .send()
            .await
            .context("control-plane session state update request failed")?
            .error_for_status()
            .context("control-plane session state update was rejected")?;
        let _ = response.bytes().await?;
        Ok(())
    }

    async fn post_runtime_event(&self, session_id: Uuid, detail: RuntimeEventDetail) -> Result<()> {
        // First, try to flush any buffered events for this session
        self.flush_event_buffer(session_id).await;

        let request = self
            .control_plane_post(format!(
                "{}/v1/sessions/{session_id}/events",
                self.control_plane_url.trim_end_matches('/')
            ))
            .json(&RuntimeEventCreateRequest {
                detail: detail.clone(),
            })
            .send()
            .await;
        match request {
            Ok(response) if response.status().is_success() => {
                if let Err(error) = response.bytes().await {
                    warn!(
                        "runtime event response body read failed for session {session_id}: {error}"
                    );
                }
                Ok(())
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                self.buffer_runtime_event(
                    session_id,
                    detail,
                    format!("control-plane runtime event request was rejected: {status} {body}"),
                )
                .await;
                Ok(())
            }
            Err(error) => {
                self.buffer_runtime_event(
                    session_id,
                    detail,
                    format!("control-plane runtime event request failed: {error}"),
                )
                .await;
                Ok(())
            }
        }
    }

    async fn buffer_runtime_event(
        &self,
        session_id: Uuid,
        detail: RuntimeEventDetail,
        reason: String,
    ) {
        warn!("buffering runtime event for session {session_id}: {reason}");
        let mut buffer = self.event_buffer.lock().await;
        let buf = buffer.entry(session_id).or_default();
        if buf.len() >= MAX_EVENT_BUFFER_PER_SESSION {
            let dropped = buf.drain(..buf.len() / 2).count();
            warn!("event buffer cap hit for session {session_id}, dropped {dropped} oldest events");
        }
        buf.push_back(detail);
    }

    /// Attempt to flush buffered runtime events for a session.
    /// Caps buffer at [`MAX_EVENT_BUFFER_PER_SESSION`] to prevent unbounded memory growth.
    async fn flush_event_buffer(&self, session_id: Uuid) {
        let mut events: VecDeque<RuntimeEventDetail> = {
            let mut buffer = self.event_buffer.lock().await;
            buffer.remove(&session_id).unwrap_or_default()
        };
        while let Some(detail) = events.pop_front() {
            if self
                .control_plane_post(format!(
                    "{}/v1/sessions/{session_id}/events",
                    self.control_plane_url.trim_end_matches('/')
                ))
                .json(&RuntimeEventCreateRequest {
                    detail: detail.clone(),
                })
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                // Flushed one successfully, continue
            } else {
                // Control plane still unreachable — re-buffer remaining and stop
                warn!("control plane still unreachable during flush for session {session_id}");
                events.push_front(detail);
                let mut buffer = self.event_buffer.lock().await;
                if let Some(mut newer_events) = buffer.remove(&session_id) {
                    events.append(&mut newer_events);
                }
                while events.len() > MAX_EVENT_BUFFER_PER_SESSION {
                    events.pop_front();
                }
                buffer.insert(session_id, events);
                return;
            }
        }
    }

    async fn create_control_plane_approval(
        &self,
        session_id: Uuid,
        request: ApprovalCreateRequest,
    ) -> Result<ApprovalRequestRecord> {
        let response = self
            .control_plane_post(format!(
                "{}/v1/sessions/{session_id}/approvals",
                self.control_plane_url.trim_end_matches('/')
            ))
            .json(&request)
            .send()
            .await
            .context("control-plane approval create request failed")?
            .error_for_status()
            .context("control-plane approval create request was rejected")?;
        response
            .json::<ApprovalRequestRecord>()
            .await
            .context("failed to decode control-plane approval response")
    }

    async fn resolve_control_plane_approval(
        &self,
        approval_id: Uuid,
        request: ApprovalDecisionRequest,
    ) -> Result<()> {
        let response = self
            .control_plane_post(format!(
                "{}/v1/approvals/{approval_id}/decision",
                self.control_plane_url.trim_end_matches('/')
            ))
            .json(&request)
            .send()
            .await
            .context("control-plane approval decision request failed")?
            .error_for_status()
            .context("control-plane approval decision request was rejected")?;
        let _ = response.bytes().await?;
        Ok(())
    }

    fn control_plane_post(&self, url: String) -> reqwest::RequestBuilder {
        authorize_control_plane_request(self.client.post(url), self.auth_token.as_deref())
    }

    /// On tool_started for a file-writing tool, start accumulating input deltas.
    async fn handle_tool_started(&self, value: &serde_json::Value, handle: &HostedSessionHandle) {
        let tool_name = value
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !is_file_write_tool(tool_name) {
            return;
        }
        if let Some(tool_use_id) = value.get("tool_use_id").and_then(serde_json::Value::as_str) {
            handle
                .pending_tool_inputs
                .lock()
                .await
                .insert(tool_use_id.to_owned(), String::new());
        }
    }

    /// Accumulate input_delta for tracked file-writing tools.
    async fn handle_tool_progress(&self, value: &serde_json::Value, handle: &HostedSessionHandle) {
        let tool_use_id = match value.get("tool_use_id").and_then(serde_json::Value::as_str) {
            Some(id) => id.to_owned(),
            None => return,
        };
        let delta = match value.get("input_delta").and_then(serde_json::Value::as_str) {
            Some(d) => d,
            None => return,
        };
        let mut inputs = handle.pending_tool_inputs.lock().await;
        if let Some(acc) = inputs.get_mut(&tool_use_id) {
            acc.push_str(delta);
        }
    }

    /// After tool_finished, extract file path from accumulated input and auto-upload.
    async fn handle_tool_finished(
        &self,
        session_id: Uuid,
        value: &serde_json::Value,
        handle: &HostedSessionHandle,
    ) -> Result<()> {
        let tool_name = value
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let is_error = value
            .get("is_error")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        if is_error || !is_file_write_tool(tool_name) {
            // Clean up any accumulated input for this tool.
            if let Some(id) = value.get("tool_use_id").and_then(serde_json::Value::as_str) {
                handle.pending_tool_inputs.lock().await.remove(id);
            }
            return Ok(());
        }
        let tool_use_id = match value.get("tool_use_id").and_then(serde_json::Value::as_str) {
            Some(id) => id.to_owned(),
            None => return Ok(()),
        };
        let accumulated_input = match handle.pending_tool_inputs.lock().await.remove(&tool_use_id) {
            Some(input) => input,
            None => return Ok(()),
        };
        // Parse accumulated input as JSON to extract file path.
        let file_path = extract_file_path_from_tool_input(&accumulated_input);
        let file_path = match file_path {
            Some(p) => p,
            None => return Ok(()),
        };
        // Upload in background to avoid blocking event processing.
        let this = self.clone();
        let workspace_dir = handle.workspace_dir.clone();
        tokio::spawn(async move {
            if let Err(error) = this
                .upload_artifact(session_id, &file_path, &workspace_dir)
                .await
            {
                warn!(
                    "auto-upload artifact failed for session {session_id} file {}: {error:#}",
                    file_path.display()
                );
            }
        });
        Ok(())
    }

    /// Upload a file as an artifact to the control plane.
    async fn upload_artifact(
        &self,
        session_id: Uuid,
        file_path: &Path,
        workspace_dir: &Path,
    ) -> Result<()> {
        let artifact_path = resolve_artifact_path(workspace_dir, file_path).await?;
        let bytes = tokio::fs::read(&artifact_path)
            .await
            .with_context(|| format!("failed to read artifact file {}", artifact_path.display()))?;
        let name = artifact_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("artifact")
            .to_owned();
        let file_name = artifact_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_owned();
        let media_type = guess_media_type(&artifact_path);
        let request_body = serde_json::json!({
            "name": name,
            "file_name": file_name,
            "media_type": media_type,
            "content_base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes),
            "metadata": {
                "source": "auto-upload",
                "tool": "hosted-session",
            }
        });
        let url = format!(
            "{}/v1/sessions/{session_id}/artifacts",
            self.control_plane_url.trim_end_matches('/')
        );
        let response = self
            .control_plane_post(url)
            .json(&request_body)
            .send()
            .await
            .context("artifact upload request failed")?;
        if response.status().is_success() {
            info!(
                "auto-uploaded artifact for session {session_id}: {} ({} bytes)",
                artifact_path.display(),
                bytes.len()
            );
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("artifact upload rejected for session {session_id}: {status} — {body}");
        }
        Ok(())
    }
}

fn is_file_write_tool(tool_name: &str) -> bool {
    let normalized: String = tool_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    matches!(
        normalized.as_str(),
        "write"
            | "edit"
            | "replaceinfile"
            | "writefile"
            | "editfile"
            | "createfile"
            | "notebookedit"
    )
}

async fn resolve_artifact_path(workspace_dir: &Path, requested_path: &Path) -> Result<PathBuf> {
    let workspace = tokio::fs::canonicalize(workspace_dir)
        .await
        .with_context(|| {
            format!(
                "failed to canonicalize workspace {}",
                workspace_dir.display()
            )
        })?;
    let candidate = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        workspace.join(requested_path)
    };
    let artifact_path = tokio::fs::canonicalize(&candidate)
        .await
        .with_context(|| format!("failed to canonicalize artifact {}", candidate.display()))?;
    if !artifact_path.starts_with(&workspace) {
        bail!(
            "refusing to upload artifact outside workspace: {}",
            artifact_path.display()
        );
    }
    let metadata = tokio::fs::metadata(&artifact_path)
        .await
        .with_context(|| format!("failed to stat artifact {}", artifact_path.display()))?;
    if !metadata.is_file() {
        bail!(
            "refusing to upload non-file artifact: {}",
            artifact_path.display()
        );
    }
    if metadata.len() > MAX_ARTIFACT_UPLOAD_BYTES {
        bail!(
            "refusing to upload artifact larger than {} bytes: {}",
            MAX_ARTIFACT_UPLOAD_BYTES,
            artifact_path.display()
        );
    }
    Ok(artifact_path)
}

fn guess_media_type(path: &Path) -> Option<String> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some("text/rust".to_owned()),
        Some("ts" | "tsx") => Some("text/typescript".to_owned()),
        Some("js" | "jsx") => Some("text/javascript".to_owned()),
        Some("py") => Some("text/x-python".to_owned()),
        Some("json") => Some("application/json".to_owned()),
        Some("toml") => Some("text/toml".to_owned()),
        Some("yaml" | "yml") => Some("text/yaml".to_owned()),
        Some("md") => Some("text/markdown".to_owned()),
        Some("html") => Some("text/html".to_owned()),
        Some("css") => Some("text/css".to_owned()),
        Some("png") => Some("image/png".to_owned()),
        Some("jpg" | "jpeg") => Some("image/jpeg".to_owned()),
        Some("svg") => Some("image/svg+xml".to_owned()),
        Some("pdf") => Some("application/pdf".to_owned()),
        _ => Some("application/octet-stream".to_owned()),
    }
}

/// Extract file path from accumulated tool input JSON.
/// Handles Claude file tools (`file_path`), NotebookEdit (`notebook_path`),
/// and legacy `path` inputs.
fn extract_file_path_from_tool_input(input: &str) -> Option<PathBuf> {
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    let path_value = parsed
        .get("file_path")
        .or_else(|| parsed.get("notebook_path"))
        .or_else(|| parsed.get("path"))?;
    path_value.as_str().map(PathBuf::from)
}

async fn write_session_input(
    session_id: Uuid,
    mut child_stdin: ChildStdin,
    mut input_rx: mpsc::Receiver<String>,
) {
    while let Some(line) = input_rx.recv().await {
        if let Err(error) = child_stdin.write_all(line.as_bytes()).await {
            warn!("failed to write to hosted session `{session_id}` stdin: {error}");
            break;
        }
        if let Err(error) = child_stdin.flush().await {
            warn!("failed to flush hosted session `{session_id}` stdin: {error}");
            break;
        }
    }
}

fn control_response_payload(request_id: &str, behavior: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "control_response",
        "request_id": request_id,
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": {
                "behavior": behavior,
                "message": message,
            }
        }
    })
}

fn map_runtime_session_state(
    runtime_state: &str,
) -> Option<(RunnerSessionState, ControlPlaneSessionState)> {
    match runtime_state {
        "running" | "idle" => Some((
            RunnerSessionState::Running,
            ControlPlaneSessionState::Running,
        )),
        "completed" | "end_turn" | "stop" => Some((
            RunnerSessionState::Completed,
            ControlPlaneSessionState::Completed,
        )),
        "failed" | "error" => Some((RunnerSessionState::Failed, ControlPlaneSessionState::Failed)),
        "requires_action" => None,
        _ => None,
    }
}

pub fn default_remote_code_bin() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to discover current executable")?;
    let file_name = current_exe
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if file_name.starts_with("remote-code-runner") {
        let sibling = if cfg!(windows) {
            current_exe.with_file_name("remote-code.exe")
        } else {
            current_exe.with_file_name("remote-code")
        };
        return Ok(sibling);
    }
    Ok(PathBuf::from("remote-code"))
}

fn authorize_control_plane_request(
    builder: reqwest::RequestBuilder,
    auth_token: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(token) = auth_token {
        builder.bearer_auth(token)
    } else {
        builder
    }
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

pub async fn pull_runner_commands_from_control_plane(
    client: &reqwest::Client,
    control_plane_url: &str,
    runner_id: &str,
    auth_token: Option<&str>,
) -> Result<RunnerCommandPullResponse> {
    let response = authorize_control_plane_request(
        client.post(format!(
            "{}/v1/runners/{}/commands/pull?limit=16",
            control_plane_url.trim_end_matches('/'),
            encode_path_segment(runner_id)
        )),
        auth_token,
    )
    .send()
    .await
    .context("control-plane runner command pull request failed")?
    .error_for_status()
    .context("control-plane runner command pull request was rejected")?;
    response
        .json::<RunnerCommandPullResponse>()
        .await
        .context("failed to decode control-plane runner command pull response")
}

pub async fn apply_pulled_runner_commands(
    api: &RunnerApi,
    response: RunnerCommandPullResponse,
) -> Result<()> {
    for command in response.commands {
        match command.body {
            RunnerQueuedCommandBody::CreateSession { request } => {
                let _ = api.create_session_direct(request).await?;
            }
            RunnerQueuedCommandBody::UpdateSessionState {
                session_id,
                request,
            } => {
                let _ = api
                    .apply_session_state_update_direct(session_id, request)
                    .await?;
            }
            RunnerQueuedCommandBody::SessionCommand {
                session_id,
                request,
            } => {
                let _ = api.post_session_command_direct(session_id, request).await?;
            }
            RunnerQueuedCommandBody::CreateApproval {
                session_id,
                request,
            } => {
                let _ = api.create_approval_direct(session_id, request).await?;
            }
            RunnerQueuedCommandBody::ApplyApprovalDecision {
                approval_id,
                request,
            } => {
                let _ = api
                    .apply_approval_decision_direct(approval_id, request)
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn run_control_plane_sync(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let Some(control_plane_url) = config.control_plane_url.clone() else {
        return;
    };
    let registration = config.registration_request();
    let configured_interval_secs = config.heartbeat_interval_secs;
    let control_plane_auth_token = config.control_plane_auth_token.clone();
    let mut retry_delay = Duration::from_secs(1);
    let client = reqwest::Client::new();

    loop {
        if *shutdown.borrow() {
            return;
        }

        match register_with_control_plane(
            &client,
            &control_plane_url,
            &registration,
            control_plane_auth_token.as_deref(),
        )
        .await
        {
            Ok(lease) => {
                retry_delay = Duration::from_secs(1);
                let mut heartbeat_interval = tokio::time::interval(effective_heartbeat_interval(
                    configured_interval_secs,
                    lease.lease_ttl_secs,
                ));

                loop {
                    tokio::select! {
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                return;
                            }
                        }
                        _ = heartbeat_interval.tick() => {
                            let heartbeat = api.heartbeat().await;
                            if let Err(error) = send_heartbeat(&client, &control_plane_url, &heartbeat, control_plane_auth_token.as_deref()).await {
                                warn!("failed to send heartbeat to control plane: {error}");
                                break;
                            }
                        }
                    }
                }
            }
            Err(error) => warn!("failed to register runner with control plane: {error}"),
        }

        if wait_for_shutdown_or_timeout(retry_delay, &mut shutdown).await {
            return;
        }
        retry_delay = next_retry_delay(retry_delay);
    }
}

/// Outbound polling mode: long-polls the control plane for queued commands.
/// Works behind any firewall/NAT — no inbound port needed.
pub async fn run_outbound_poll_loop(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let cp_url = match &config.control_plane_url {
        Some(url) => url.clone(),
        None => {
            tracing::error!("outbound mode requires control_plane_url");
            return;
        }
    };
    let runner_id = &config.runner_id;
    let client = reqwest::Client::new();
    let poll_timeout = Duration::from_secs(30);

    let mut retry_delay = Duration::from_secs(1);

    loop {
        if *shutdown.borrow() {
            break;
        }

        let url = format!(
            "{}/v1/runners/{}/commands/pull?timeout={}",
            cp_url.trim_end_matches('/'),
            encode_path_segment(runner_id),
            poll_timeout.as_secs(),
        );

        let result = authorize_control_plane_request(
            client
                .post(&url)
                .timeout(poll_timeout + Duration::from_secs(5)),
            config.control_plane_auth_token.as_deref(),
        )
        .send()
        .await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    retry_delay = Duration::from_secs(1);
                    match response.json::<RunnerCommandPullResponse>().await {
                        Ok(cmd_response) => {
                            if let Err(e) = apply_pulled_runner_commands(&api, cmd_response).await {
                                tracing::warn!("outbound command processing failed: {e:#}");
                            }
                        }
                        Err(error) => {
                            tracing::warn!("failed to decode outbound command response: {error}");
                        }
                    }
                } else if response.status().as_u16() == 404 {
                    tracing::warn!("runner not registered, will retry");
                } else {
                    tracing::warn!("poll returned HTTP {}", response.status());
                }
            }
            Err(e) => {
                tracing::warn!("poll request failed: {e}");
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(retry_delay) => {},
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
        retry_delay = next_retry_delay(retry_delay);
    }
}

/// WebSocket-based command streaming with automatic fallback to HTTP polling.
///
/// Tries to establish a WebSocket connection to the control plane for real-time
/// command delivery. Falls back to [`run_outbound_poll_loop`] on connection failure.
///
/// The WebSocket path reduces command latency from the poll interval (30 s) to
/// frame-at-a-time delivery (~100 ms) while sharing the same auth and shutdown
/// wiring as the polling path.
pub async fn run_control_plane_command_stream(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let cp_url = match &config.control_plane_url {
        Some(url) => url.clone(),
        None => {
            tracing::error!("command stream requires control_plane_url");
            return;
        }
    };

    // Try WebSocket first; fall back to polling.
    let use_ws = try_ws_connect(&cp_url, &config).await;
    if let Some(ws) = use_ws {
        tracing::info!("WebSocket command stream established, running WS mode");
        run_ws_command_stream(api, config, shutdown, ws).await;
    } else {
        tracing::info!("WebSocket unavailable, falling back to HTTP polling");
        run_outbound_poll_loop(api, config, shutdown).await;
    }
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

async fn try_ws_connect(
    cp_url: &str,
    config: &RunnerConfig,
) -> Option<WsStream> {
    let base_url = cp_url.trim_end_matches('/');
    let ws_url = if let Some(rest) = base_url.strip_prefix("https://") {
        format!("wss://{rest}/v1/runners/{}/commands/stream", config.runner_id)
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        format!("ws://{rest}/v1/runners/{}/commands/stream", config.runner_id)
    } else {
        return None;
    };

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut request: tokio_tungstenite::tungstenite::http::Request<()> =
        IntoClientRequest::into_client_request(&ws_url).ok()?;

    if let Some(token) = config.control_plane_auth_token.as_deref() {
        if !token.trim().is_empty() {
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {}", token.trim()).parse().ok()?,
            );
        }
    }

    match tokio_tungstenite::connect_async(request).await {
        Ok((ws, _)) => {
            tracing::debug!("WebSocket command stream connected");
            Some(ws)
        }
        Err(e) => {
            tracing::warn!("WebSocket command stream connect failed: {e}");
            None
        }
    }
}

async fn run_ws_command_stream(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
    mut ws: WsStream,
) {
    let latency_tracker = crate::LatencyTracker::new("ws_command");
    let mut reconnect_delay = Duration::from_secs(1);

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        let _latency = latency_tracker.start();
                        match serde_json::from_str::<RunnerCommandPullResponse>(&text) {
                            Ok(cmd_response) => {
                                if let Err(e) = apply_pulled_runner_commands(&api, cmd_response).await {
                                    tracing::warn!("WS command processing failed: {e:#}");
                                }
                            }
                            Err(e) => {
                                tracing::warn!("WS command decode failed: {e}");
                            }
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(data))) => {
                        let _ = ws.send(tokio_tungstenite::tungstenite::Message::Pong(data)).await;
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                        tracing::warn!("WS command stream closed by server");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("WS command stream error: {e}");
                        break;
                    }
                    None => {
                        tracing::warn!("WS command stream ended");
                        break;
                    }
                    _ => {}
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    let _ = ws.close(None).await;
                    return;
                }
            }
        }
    }

    // Fall back to polling.
    tracing::info!("WebSocket command stream lost, falling back to HTTP polling");
    tokio::time::sleep(reconnect_delay).await;
    run_outbound_poll_loop(api, config, shutdown).await;
}

/// Tracks command processing latency for observability and diagnostics.
#[derive(Clone)]
pub struct LatencyTracker {
    label: &'static str,
}

impl LatencyTracker {
    pub fn new(label: &'static str) -> Self {
        Self { label }
    }

    /// Record the start of a command and return a guard that records the
    /// duration when dropped.
    pub fn start(&self) -> LatencyGuard<'_> {
        LatencyGuard {
            tracker: self,
            started_at: std::time::Instant::now(),
        }
    }
}

pub struct LatencyGuard<'a> {
    tracker: &'a LatencyTracker,
    started_at: std::time::Instant,
}

impl<'a> Drop for LatencyGuard<'a> {
    fn drop(&mut self) {
        let elapsed = self.started_at.elapsed();
        tracing::debug!(
            target: "runner_latency",
            label = self.tracker.label,
            elapsed_ms = elapsed.as_millis() as u64,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_test_dir() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("remote-code-runner-host-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        root
    }

    #[tokio::test]
    async fn resolve_artifact_path_accepts_workspace_file() {
        let workspace = make_test_dir().await;
        let file = workspace.join("src").join("main.rs");
        tokio::fs::create_dir_all(file.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&file, "fn main() {}\n").await.unwrap();

        let resolved = resolve_artifact_path(&workspace, Path::new("src/main.rs"))
            .await
            .unwrap();

        assert_eq!(resolved, tokio::fs::canonicalize(&file).await.unwrap());
        let _ = tokio::fs::remove_dir_all(workspace).await;
    }

    #[tokio::test]
    async fn resolve_artifact_path_rejects_parent_escape() {
        let root = make_test_dir().await;
        let workspace = root.join("workspace");
        let outside = root.join("outside.txt");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        tokio::fs::write(&outside, "secret").await.unwrap();

        let error = resolve_artifact_path(&workspace, Path::new("../outside.txt"))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("outside workspace"));
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn resolve_artifact_path_rejects_large_file() {
        let workspace = make_test_dir().await;
        let file = workspace.join("large.bin");
        let large = tokio::fs::File::create(&file).await.unwrap();
        large.set_len(MAX_ARTIFACT_UPLOAD_BYTES + 1).await.unwrap();

        let error = resolve_artifact_path(&workspace, Path::new("large.bin"))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("larger than"));
        let _ = tokio::fs::remove_dir_all(workspace).await;
    }

    #[test]
    fn file_write_tool_detection_covers_claude_aliases() {
        for name in [
            "Write",
            "Edit",
            "ReplaceInFile",
            "write_file",
            "edit_file",
            "replace_in_file",
            "replace-in-file",
            "create_file",
            "NotebookEdit",
            "notebook_edit",
        ] {
            assert!(
                is_file_write_tool(name),
                "{name} should trigger artifact upload"
            );
        }
        assert!(!is_file_write_tool("Read"));
        assert!(!is_file_write_tool("Bash"));
    }

    #[test]
    fn extract_file_path_supports_notebook_edit_input() {
        assert_eq!(
            extract_file_path_from_tool_input(r#"{"notebook_path":"notebooks/demo.ipynb"}"#),
            Some(PathBuf::from("notebooks/demo.ipynb"))
        );
    }

    #[test]
    fn control_response_payload_matches_sdk_success_shape() {
        let payload = control_response_payload("req-1", "allow", "approved");

        assert_eq!(payload["type"], "control_response");
        assert_eq!(payload["request_id"], "req-1");
        assert_eq!(payload["response"]["subtype"], "success");
        assert_eq!(payload["response"]["request_id"], "req-1");
        assert_eq!(payload["response"]["response"]["behavior"], "allow");
        assert_eq!(payload["response"]["response"]["message"], "approved");
    }
}
