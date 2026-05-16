use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use claude_control_plane::{
    RunnerCommandPullResponse, RunnerQueuedCommandBody, RuntimeEventCreateRequest,
    RuntimeEventDetail, SessionState as ControlPlaneSessionState, SessionStateUpdateRequest,
    runtime_event_detail_from_stream_json_value,
};
use claude_runner::{
    ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionRequest, ApprovalRequestRecord,
    RUNNER_EVENT_CHANNEL_CAPACITY, RunnerApi, RunnerApiEvent, RunnerConfig, RunnerConfigOverrides,
    RunnerSessionCommandRequest, RunnerSessionRecord, RunnerSessionStateUpdateRequest,
    SessionState as RunnerSessionState, describe_status, load_runner_config,
    register_with_control_plane, send_heartbeat, validate_runner_config,
};
use claude_telemetry::install_tracing;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command as ProcessCommand};
use tokio::sync::{Mutex, mpsc, watch};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "remote-code-runner", version, about = "Rust runner service")]
struct Cli {
    #[arg(long, env = "REMOTE_CODE_RUNNER_ID")]
    runner_id: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_URL")]
    control_plane_url: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_BIND")]
    bind: Option<SocketAddr>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_PUBLIC_BASE_URL")]
    public_base_url: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_AUTH_TOKEN")]
    auth_token: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN")]
    control_plane_auth_token: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_HEARTBEAT_SECS")]
    heartbeat_interval_secs: Option<u64>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_MAX_PARALLEL_SESSIONS")]
    max_parallel_sessions: Option<u16>,

    #[arg(long, env = "REMOTE_CODE_PROFILE_DIR")]
    profile_dir: Option<PathBuf>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_REMOTE_CODE_BIN")]
    remote_code_bin: Option<PathBuf>,

    /// Runner connection mode: "outbound" (default, long-polls control plane
    /// for commands — no inbound port needed) or "inbound" (explicit direct API).
    #[arg(long, env = "REMOTE_CODE_RUNNER_MODE", default_value = "outbound")]
    mode: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
    PrintConfig,
    Serve,
}

fn effective_heartbeat_interval(configured_interval_secs: u64, lease_ttl_secs: u64) -> Duration {
    Duration::from_secs(
        configured_interval_secs
            .max(1)
            .min((lease_ttl_secs / 2).max(1)),
    )
}

fn next_retry_delay(current: Duration) -> Duration {
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

#[derive(Clone)]
struct HostedSessionManager {
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
    input_tx: mpsc::UnboundedSender<String>,
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
    fn new(
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

    async fn run(
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

        let (input_tx, input_rx) = mpsc::unbounded_channel();
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
                state: claude_control_plane::DaemonPresenceState::Online,
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
            self.handle_protocol_line(session_id, &line, &handle)
                .await?;
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
        self.post_control_plane_session_state(session_id, control_state)
            .await?;
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

        let approval = self
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
            .await?;
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
            self.resolve_control_plane_approval(
                approval_id,
                ApprovalDecisionRequest {
                    decision: ApprovalDecision::Cancelled,
                    responder: Some("runner".to_owned()),
                    note: Some(format!(
                        "Runtime cancelled approval request `{request_id}` for session `{session_id}`."
                    )),
                },
            )
            .await?;
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
            claude_runner::ApprovalState::Approved => "allow",
            claude_runner::ApprovalState::Denied | claude_runner::ApprovalState::Cancelled => {
                "deny"
            }
            claude_runner::ApprovalState::Pending => return Ok(()),
        };
        let note = approval
            .note
            .clone()
            .unwrap_or_else(|| match approval.state {
                claude_runner::ApprovalState::Approved => "Approved remotely.".to_owned(),
                claude_runner::ApprovalState::Denied => "Denied remotely.".to_owned(),
                claude_runner::ApprovalState::Cancelled => "Cancelled remotely.".to_owned(),
                claude_runner::ApprovalState::Pending => String::new(),
            });
        let payload = serde_json::json!({
            "type": "control_response",
            "response": {
                "request_id": request_id,
                "response": {
                    "behavior": behavior,
                    "message": note,
                }
            }
        });
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
                state: claude_control_plane::DaemonPresenceState::Offline,
            },
        )
        .await?;
        let (runner_state, control_state, message) = match exit {
            Ok(status) if status.success() => (
                RunnerSessionState::Completed,
                ControlPlaneSessionState::Completed,
                "Hosted session exited cleanly.".to_owned(),
            ),
            Ok(status) => (
                RunnerSessionState::Failed,
                ControlPlaneSessionState::Failed,
                format!("Hosted session exited with status `{status}`."),
            ),
            Err(error) => (
                RunnerSessionState::Failed,
                ControlPlaneSessionState::Failed,
                format!("Hosted session wait failed: {error}"),
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
        self.post_runtime_event(
            session_id,
            RuntimeEventDetail::RuntimeError {
                message: message.clone(),
            },
        )
        .await?;
        self.post_control_plane_session_state(session_id, control_state)
            .await?;
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

        let result = self
            .control_plane_post(format!(
                "{}/v1/sessions/{session_id}/events",
                self.control_plane_url.trim_end_matches('/')
            ))
            .json(&RuntimeEventCreateRequest {
                detail: detail.clone(),
            })
            .send()
            .await
            .context("control-plane runtime event request failed")?
            .error_for_status()
            .context("control-plane runtime event request was rejected");
        match result {
            Ok(response) => {
                let _ = response.bytes().await?;
                Ok(())
            }
            Err(error) => {
                // Buffer the event instead of killing the session
                warn!("buffering runtime event for session {session_id}: {error:#}");
                let mut buffer = self.event_buffer.lock().await;
                let buf = buffer.entry(session_id).or_default();
                if buf.len() >= MAX_EVENT_BUFFER_PER_SESSION {
                    let dropped = buf.drain(..buf.len() / 2).count();
                    warn!(
                        "event buffer cap hit for session {session_id}, dropped {dropped} oldest events"
                    );
                }
                buf.push_back(detail);
                Ok(())
            }
        }
    }

    /// Attempt to flush buffered runtime events for a session.
    /// Caps buffer at [`MAX_EVENT_BUFFER_PER_SESSION`] to prevent unbounded memory growth.
    async fn flush_event_buffer(&self, session_id: Uuid) {
        let events: Vec<RuntimeEventDetail> = {
            let mut buffer = self.event_buffer.lock().await;
            buffer
                .remove(&session_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        };
        for detail in events {
            if self
                .control_plane_post(format!(
                    "{}/v1/sessions/{session_id}/events",
                    self.control_plane_url.trim_end_matches('/')
                ))
                .json(&RuntimeEventCreateRequest { detail })
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                // Flushed one successfully, continue
            } else {
                // Control plane still unreachable — re-buffer remaining and stop
                warn!("control plane still unreachable during flush for session {session_id}");
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

    /// File-writing tool names that should trigger automatic artifact upload.
    const FILE_WRITE_TOOLS: &[&str] = &["Write", "Edit", "write_file", "edit_file", "create_file"];

    /// On tool_started for a file-writing tool, start accumulating input deltas.
    async fn handle_tool_started(&self, value: &serde_json::Value, handle: &HostedSessionHandle) {
        let tool_name = value
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !Self::FILE_WRITE_TOOLS.contains(&tool_name) {
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
        if is_error || !Self::FILE_WRITE_TOOLS.contains(&tool_name) {
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
        let absolute = if file_path.is_absolute() {
            file_path
        } else {
            handle.workspace_dir.join(&file_path)
        };
        // Upload in background to avoid blocking event processing.
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(error) = this.upload_artifact(session_id, &absolute).await {
                warn!(
                    "auto-upload artifact failed for session {session_id} file {}: {error:#}",
                    absolute.display()
                );
            }
        });
        Ok(())
    }

    /// Upload a file as an artifact to the control plane.
    async fn upload_artifact(&self, session_id: Uuid, file_path: &std::path::Path) -> Result<()> {
        let bytes = tokio::fs::read(file_path)
            .await
            .with_context(|| format!("failed to read artifact file {}", file_path.display()))?;
        let name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("artifact")
            .to_owned();
        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_owned();
        let media_type = guess_media_type(file_path);
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
                file_path.display(),
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

fn guess_media_type(path: &std::path::Path) -> Option<String> {
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
/// Handles both `{"file_path": "..."}` and `{"path": "..."}` formats.
fn extract_file_path_from_tool_input(input: &str) -> Option<PathBuf> {
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    let path_value = parsed.get("file_path").or_else(|| parsed.get("path"))?;
    path_value.as_str().map(PathBuf::from)
}

async fn write_session_input(
    session_id: Uuid,
    mut child_stdin: ChildStdin,
    mut input_rx: mpsc::UnboundedReceiver<String>,
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

fn default_remote_code_bin() -> Result<PathBuf> {
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

async fn pull_runner_commands_from_control_plane(
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

async fn apply_pulled_runner_commands(
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

async fn run_control_plane_sync(
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
    const IDLE_POLL_SECS: u64 = 5;
    const ACTIVE_POLL_SECS: u64 = 1;
    const MAX_IDLE_POLL_SECS: u64 = 15;
    let mut poll_interval = Duration::from_secs(ACTIVE_POLL_SECS);
    let mut consecutive_empty_pulls: u32 = 0;

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
                let mut command_poll_interval = tokio::time::interval(poll_interval);

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
                        _ = command_poll_interval.tick() => {
                            match pull_runner_commands_from_control_plane(
                                &client,
                                &control_plane_url,
                                &registration.runner_id,
                                control_plane_auth_token.as_deref(),
                            ).await {
                                Ok(response) => {
                                    let command_count = response.commands.len();
                                    if let Err(error) = apply_pulled_runner_commands(&api, response).await {
                                        warn!("failed to apply pulled runner commands: {error:#}");
                                    }
                                    if command_count > 0 {
                                        consecutive_empty_pulls = 0;
                                        poll_interval = Duration::from_secs(ACTIVE_POLL_SECS);
                                    } else {
                                        consecutive_empty_pulls += 1;
                                        if consecutive_empty_pulls >= 3 {
                                            let backoff_secs = IDLE_POLL_SECS
                                                .saturating_mul(1 << (consecutive_empty_pulls - 3).min(2))
                                                .min(MAX_IDLE_POLL_SECS);
                                            poll_interval = Duration::from_secs(backoff_secs);
                                        }
                                    }
                                    command_poll_interval = tokio::time::interval(poll_interval);
                                }
                                Err(error) => warn!("failed to pull runner commands from control plane: {error}"),
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
async fn run_outbound_poll_loop(
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
    let auth = config.control_plane_auth_token.as_deref().unwrap_or("");
    let poll_timeout = Duration::from_secs(30);

    let mut retry_delay = Duration::from_secs(1);

    loop {
        if shutdown.has_changed().unwrap_or(true) {
            break;
        }

        let url = format!(
            "{cp_url}/v1/runners/{runner_id}/commands/pull?timeout={}",
            poll_timeout.as_secs(),
        );

        let result = client
            .post(&url)
            .header("authorization", format!("Bearer {auth}"))
            .timeout(poll_timeout + Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    retry_delay = Duration::from_secs(1);
                    if let Ok(body) = response.text().await {
                        if !body.is_empty() {
                            if let Ok(cmd_response) =
                                serde_json::from_str::<RunnerCommandPullResponse>(&body)
                            {
                                if let Err(e) =
                                    apply_pulled_runner_commands(&api, cmd_response).await
                                {
                                    tracing::warn!("outbound command processing failed: {e:#}");
                                }
                            }
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
                tokio::select! {
                    _ = tokio::time::sleep(retry_delay) => {},
                    _ = shutdown.changed() => break,
                }
                retry_delay = next_retry_delay(retry_delay);
                continue;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_runner", false)?;
    let cli = Cli::parse();
    let config = load_runner_config(
        cli.profile_dir,
        RunnerConfigOverrides {
            runner_id: cli.runner_id,
            control_plane_url: cli.control_plane_url,
            bind: cli.bind,
            public_base_url: cli.public_base_url,
            auth_token: cli.auth_token,
            control_plane_auth_token: cli.control_plane_auth_token,
            heartbeat_interval_secs: cli.heartbeat_interval_secs,
            max_parallel_sessions: cli.max_parallel_sessions,
            ..RunnerConfigOverrides::default()
        },
    )?;

    match cli.command.unwrap_or(Command::Doctor) {
        Command::Doctor => println!(
            "{}",
            serde_json::to_string_pretty(&describe_status(&config)?)?
        ),
        Command::PrintConfig => println!("{}", serde_json::to_string_pretty(&config)?),
        Command::Serve => {
            let blocking_issues = validate_runner_config(&config);
            if !blocking_issues.is_empty() {
                anyhow::bail!(blocking_issues.join("; "));
            }
            let bind = config.bind;
            let (event_tx, event_rx) = mpsc::channel(RUNNER_EVENT_CHANNEL_CAPACITY);
            let api = RunnerApi::new(
                config.clone(),
                "remote-code-runner",
                env!("CARGO_PKG_VERSION"),
            )
            .with_event_channel(event_tx);
            let remote_code_bin = cli
                .remote_code_bin
                .clone()
                .unwrap_or(default_remote_code_bin()?);
            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            if config.control_plane_url.is_some() {
                let cp_sync_task = tokio::spawn(run_control_plane_sync(
                    api.clone(),
                    config.clone(),
                    shutdown_rx,
                ));
                // Monitor the control-plane sync task for panics.
                let mut cp_monitor = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    tokio::select! {
                        result = cp_sync_task => {
                            if result.is_err() {
                                tracing::error!("control-plane sync task panicked");
                            }
                        }
                        _ = cp_monitor.changed() => {}
                    }
                });
                let hosted_manager = HostedSessionManager::new(
                    api.clone(),
                    config
                        .control_plane_url
                        .clone()
                        .ok_or_else(|| anyhow!("missing control plane URL"))?,
                    remote_code_bin,
                    config.profile_dir.profile_dir.clone(),
                    config.control_plane_auth_token.clone(),
                );
                let manager_task =
                    tokio::spawn(hosted_manager.run(event_rx, shutdown_tx.subscribe()));
                // Monitor the manager task for unexpected termination.
                let mut monitor_shutdown = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    tokio::select! {
                        result = manager_task => {
                            if result.is_err() {
                                tracing::error!("hosted session manager task panicked");
                            }
                        }
                        _ = monitor_shutdown.changed() => {}
                    }
                });
            }

            if cli.mode == "outbound" {
                // Outbound mode: long-poll control plane for commands instead of
                // listening for inbound connections. Works behind any firewall/NAT.
                if config.control_plane_url.is_none() {
                    anyhow::bail!("outbound mode requires --control-plane-url");
                }
                let outbound_shutdown = shutdown_tx.subscribe();
                tokio::spawn(run_outbound_poll_loop(
                    api.clone(),
                    config.clone(),
                    outbound_shutdown,
                ));
                // Block until shutdown.
                let mut wait_shutdown = shutdown_tx.subscribe();
                let _ = wait_shutdown.changed().await;
            } else {
                let app = api.router();
                let listener = tokio::net::TcpListener::bind(bind).await?;
                axum::serve(listener, app).await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        extract::{Path as AxumPath, State},
        http::StatusCode,
        response::IntoResponse,
        routing::post,
    };
    use chrono::Utc;
    use claude_runner::{
        ApprovalRequestRecord, RunnerHeartbeat, RunnerRegistrationLease, RunnerRegistrationRequest,
        RunnerSessionCreateRequest, RunnerSnapshot, RunnerState, RunnerWorkspace,
    };
    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    #[derive(Clone, Default)]
    struct FakeControlPlaneState {
        register_count: Arc<AtomicUsize>,
        heartbeat_count: Arc<AtomicUsize>,
        registration: Arc<tokio::sync::RwLock<Option<RunnerRegistrationRequest>>>,
    }

    #[derive(Clone)]
    struct HostedControlPlaneState {
        runner_base_url: String,
        client: reqwest::Client,
        next_sequence: Arc<AtomicUsize>,
        events: Arc<RwLock<Vec<(Uuid, RuntimeEventCreateRequest)>>>,
        state_updates: Arc<RwLock<Vec<(Uuid, SessionStateUpdateRequest)>>>,
        approval_creates: Arc<RwLock<Vec<(Uuid, ApprovalCreateRequest)>>>,
        approvals: Arc<RwLock<Vec<ApprovalRequestRecord>>>,
        approval_decisions: Arc<RwLock<Vec<(Uuid, ApprovalDecisionRequest)>>>,
    }

    impl HostedControlPlaneState {
        fn new(runner_base_url: String) -> Self {
            Self {
                runner_base_url,
                client: reqwest::Client::new(),
                next_sequence: Arc::new(AtomicUsize::new(0)),
                events: Arc::new(RwLock::new(Vec::new())),
                state_updates: Arc::new(RwLock::new(Vec::new())),
                approval_creates: Arc::new(RwLock::new(Vec::new())),
                approvals: Arc::new(RwLock::new(Vec::new())),
                approval_decisions: Arc::new(RwLock::new(Vec::new())),
            }
        }
    }

    #[test]
    fn effective_heartbeat_interval_uses_config_without_exceeding_lease_half() {
        assert_eq!(effective_heartbeat_interval(15, 6), Duration::from_secs(3));
        assert_eq!(effective_heartbeat_interval(1, 60), Duration::from_secs(1));
        assert_eq!(effective_heartbeat_interval(0, 1), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn apply_pulled_runner_commands_drives_local_runner_api() {
        let profile_dir = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile_dir.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-pull".to_owned()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile_dir.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let (event_tx, mut event_rx) = mpsc::channel(RUNNER_EVENT_CHANNEL_CAPACITY);
        let api =
            RunnerApi::new(config, "remote-code-runner", "0.1.0").with_event_channel(event_tx);

        let session = api
            .create_session_direct(RunnerSessionCreateRequest {
                session_id: Some(Uuid::nil()),
                workspace_id: "default".to_owned(),
                metadata: BTreeMap::new(),
            })
            .await
            .expect("session should be created");
        let _ = event_rx
            .recv()
            .await
            .expect("session created event should arrive");

        let approval = api
            .create_approval_direct(
                session.session_id,
                ApprovalCreateRequest {
                    approval_id: Some(Uuid::from_u128(7)),
                    title: "Approve command".to_owned(),
                    description: "Need confirmation".to_owned(),
                    metadata: BTreeMap::new(),
                },
            )
            .await
            .expect("approval should be created");

        apply_pulled_runner_commands(
            &api,
            RunnerCommandPullResponse {
                commands: vec![
                    claude_control_plane::RunnerQueuedCommand {
                        command_id: Uuid::new_v4(),
                        runner_id: "runner-pull".to_owned(),
                        created_at: Utc::now(),
                        body: RunnerQueuedCommandBody::SessionCommand {
                            session_id: session.session_id,
                            request: RunnerSessionCommandRequest::SendPrompt {
                                content: "queued prompt".to_owned(),
                            },
                        },
                    },
                    claude_control_plane::RunnerQueuedCommand {
                        command_id: Uuid::new_v4(),
                        runner_id: "runner-pull".to_owned(),
                        created_at: Utc::now(),
                        body: RunnerQueuedCommandBody::ApplyApprovalDecision {
                            approval_id: approval.approval_id,
                            request: ApprovalDecisionRequest {
                                decision: ApprovalDecision::Approved,
                                responder: Some("mobile-web".to_owned()),
                                note: Some("approved over pull".to_owned()),
                            },
                        },
                    },
                ],
            },
        )
        .await
        .expect("pulled commands should apply");

        match event_rx
            .recv()
            .await
            .expect("session command event should arrive")
        {
            RunnerApiEvent::SessionCommand {
                session_id,
                command,
            } => {
                assert_eq!(session_id, session.session_id);
                assert_eq!(
                    command,
                    RunnerSessionCommandRequest::SendPrompt {
                        content: "queued prompt".to_owned()
                    }
                );
            }
            other => panic!("unexpected event after pull command: {other:?}"),
        }

        match event_rx
            .recv()
            .await
            .expect("approval resolved event should arrive")
        {
            RunnerApiEvent::ApprovalResolved(record) => {
                assert_eq!(record.approval_id, approval.approval_id);
                assert_eq!(record.state, claude_runner::ApprovalState::Approved);
                assert_eq!(record.responder.as_deref(), Some("mobile-web"));
            }
            other => panic!("unexpected approval event after pull command: {other:?}"),
        }
    }

    #[test]
    fn retry_delay_caps_at_thirty_seconds() {
        assert_eq!(
            next_retry_delay(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(16)),
            Duration::from_secs(30)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }

    #[tokio::test]
    async fn control_plane_sync_re_registers_after_heartbeat_failure() {
        let state = FakeControlPlaneState::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let server_state = state.clone();
        let server = tokio::spawn(async move {
            let app = Router::new()
                .route("/v1/runners/register", post(fake_register_runner))
                .route("/v1/runners/{runner_id}/heartbeat", post(fake_heartbeat))
                .with_state(server_state);
            axum::serve(listener, app).await.expect("server should run");
        });

        let profile = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-loop".to_owned()),
                control_plane_url: Some(format!("http://{address}")),
                public_base_url: Some("http://127.0.0.1:9999".to_owned()),
                heartbeat_interval_secs: Some(1),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let api = RunnerApi::new(config.clone(), "remote-code-runner", "0.1.0");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let sync_task = tokio::spawn(run_control_plane_sync(api, config, shutdown_rx));

        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if state.register_count.load(Ordering::SeqCst) >= 2
                    && state.heartbeat_count.load(Ordering::SeqCst) >= 2
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("runner should re-register before timeout");

        shutdown_tx.send(true).expect("shutdown should send");
        tokio::time::timeout(Duration::from_secs(5), sync_task)
            .await
            .expect("sync task should stop")
            .expect("sync task should join");

        assert!(state.register_count.load(Ordering::SeqCst) >= 2);
        assert!(state.heartbeat_count.load(Ordering::SeqCst) >= 2);

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn hosted_session_manager_relays_protocol_events_commands_and_approvals() {
        let temp = tempdir().expect("tempdir should exist");
        let workspace_dir = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir should exist");
        let profile_dir = temp.path().join("profile");
        let input_log_path = temp.path().join("hosted-session-input.log");
        let artifact_id = Uuid::new_v4();
        let remote_code_bin =
            create_fake_remote_code_script(temp.path(), &input_log_path, artifact_id);

        let runner_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("runner listener should bind");
        let runner_addr = runner_listener
            .local_addr()
            .expect("runner listener addr should exist");
        let runner_base_url = format!("http://{runner_addr}");

        let (event_tx, event_rx) = mpsc::channel(RUNNER_EVENT_CHANNEL_CAPACITY);
        let runner_config = load_runner_config(
            Some(profile_dir.clone()),
            RunnerConfigOverrides {
                runner_id: Some("runner-hosted".to_owned()),
                public_base_url: Some(runner_base_url.clone()),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: workspace_dir.clone(),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("runner config should load");
        let api = RunnerApi::new(runner_config, "remote-code-runner", "0.1.0")
            .with_event_channel(event_tx);
        let runner_api = api.clone();
        let runner_server = tokio::spawn(async move {
            axum::serve(runner_listener, runner_api.router())
                .await
                .expect("runner server should run");
        });

        let control_plane_state = HostedControlPlaneState::new(runner_base_url.clone());
        let control_plane_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("control plane listener should bind");
        let control_plane_addr = control_plane_listener
            .local_addr()
            .expect("control plane listener addr should exist");
        let control_plane_url = format!("http://{control_plane_addr}");
        let control_plane_server_state = control_plane_state.clone();
        let control_plane_server = tokio::spawn(async move {
            let app = Router::new()
                .route(
                    "/v1/sessions/{session_id}/events",
                    post(hosted_runtime_event),
                )
                .route(
                    "/v1/sessions/{session_id}/state",
                    post(hosted_session_state_update),
                )
                .route(
                    "/v1/sessions/{session_id}/approvals",
                    post(hosted_create_approval),
                )
                .route(
                    "/v1/approvals/{approval_id}/decision",
                    post(hosted_approval_decision),
                )
                .with_state(control_plane_server_state);
            axum::serve(control_plane_listener, app)
                .await
                .expect("control plane server should run");
        });

        let (manager_shutdown_tx, manager_shutdown_rx) = watch::channel(false);
        let manager = HostedSessionManager::new(
            api.clone(),
            control_plane_url.clone(),
            remote_code_bin,
            profile_dir,
            None,
        );
        let manager_task = tokio::spawn(manager.run(event_rx, manager_shutdown_rx));

        let client = reqwest::Client::new();
        let session_id = Uuid::new_v4();
        let session = client
            .post(format!("{runner_base_url}/v1/sessions"))
            .json(&RunnerSessionCreateRequest {
                session_id: Some(session_id),
                workspace_id: "default".to_owned(),
                metadata: BTreeMap::new(),
            })
            .send()
            .await
            .expect("session create request should succeed")
            .error_for_status()
            .expect("session create response should succeed")
            .json::<RunnerSessionRecord>()
            .await
            .expect("session create response should decode");
        assert_eq!(session.session_id, session_id);

        let approval_wait = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if !control_plane_state.approvals.read().await.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        if approval_wait.is_err() {
            let event_count = control_plane_state.events.read().await.len();
            let state_update_count = control_plane_state.state_updates.read().await.len();
            let approval_create_count = control_plane_state.approval_creates.read().await.len();
            let input_log_exists = tokio::fs::try_exists(&input_log_path)
                .await
                .unwrap_or(false);
            panic!(
                "timed out waiting for approval propagation: events={event_count}, state_updates={state_update_count}, approval_creates={approval_create_count}, input_log_exists={input_log_exists}"
            );
        }

        let approval = control_plane_state
            .approvals
            .read()
            .await
            .first()
            .cloned()
            .expect("approval should exist");
        assert_eq!(
            approval
                .metadata
                .get("request_id")
                .expect("request id should be recorded"),
            "req-1"
        );

        client
            .post(format!(
                "{runner_base_url}/v1/sessions/{session_id}/commands"
            ))
            .json(&RunnerSessionCommandRequest::SendPrompt {
                content: "follow up".to_owned(),
            })
            .send()
            .await
            .expect("prompt command request should succeed")
            .error_for_status()
            .expect("prompt command should succeed");
        client
            .post(format!(
                "{runner_base_url}/v1/sessions/{session_id}/commands"
            ))
            .json(&RunnerSessionCommandRequest::Interrupt)
            .send()
            .await
            .expect("interrupt command request should succeed")
            .error_for_status()
            .expect("interrupt command should succeed");

        client
            .post(format!(
                "{control_plane_url}/v1/approvals/{}/decision",
                approval.approval_id
            ))
            .json(&ApprovalDecisionRequest {
                decision: ApprovalDecision::Approved,
                responder: Some("mobile-web".to_owned()),
                note: Some("approved remotely".to_owned()),
            })
            .send()
            .await
            .expect("approval decision request should succeed")
            .error_for_status()
            .expect("approval decision should succeed");

        let completion_wait = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let state_updates = control_plane_state.state_updates.read().await;
                if state_updates
                    .iter()
                    .any(|(_, update)| matches!(update.state, ControlPlaneSessionState::Completed))
                {
                    break;
                }
                drop(state_updates);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        if completion_wait.is_err() {
            let state_updates = control_plane_state.state_updates.read().await.clone();
            let input_log = tokio::fs::read_to_string(&input_log_path)
                .await
                .unwrap_or_else(|_| "<missing-input-log>".to_owned());
            panic!(
                "timed out waiting for hosted session completion: state_updates={state_updates:?}, input_log={input_log}"
            );
        }

        let input_log = tokio::fs::read_to_string(&input_log_path)
            .await
            .expect("input log should be readable");
        assert!(input_log.contains("\"type\":\"user\""));
        assert!(input_log.contains("\"subtype\":\"interrupt\""));
        assert!(input_log.contains("\"type\":\"control_response\""));
        assert!(input_log.contains("approved remotely"));

        let events = control_plane_state.events.read().await.clone();
        assert!(events.iter().any(|(_, event)| matches!(
            event.detail,
            RuntimeEventDetail::DaemonPresenceChanged {
                state: claude_control_plane::DaemonPresenceState::Online
            }
        )));
        assert!(
            events
                .iter()
                .any(|(_, event)| matches!(event.detail, RuntimeEventDetail::MessageDelta { .. }))
        );
        assert!(
            events.iter().any(|(_, event)| matches!(
                event.detail,
                RuntimeEventDetail::MessageCommitted { .. }
            ))
        );
        assert!(
            events
                .iter()
                .any(|(_, event)| matches!(event.detail, RuntimeEventDetail::ToolStarted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|(_, event)| matches!(event.detail, RuntimeEventDetail::ToolProgress { .. }))
        );
        assert!(
            events
                .iter()
                .any(|(_, event)| matches!(event.detail, RuntimeEventDetail::ToolFinished { .. }))
        );
        assert!(events.iter().any(|(_, event)| matches!(
            event.detail,
            RuntimeEventDetail::ArtifactManifest { ref artifact_ids }
                if artifact_ids == &vec![artifact_id]
        )));
        assert!(events.iter().any(|(_, event)| matches!(
            event.detail,
            RuntimeEventDetail::DaemonPresenceChanged {
                state: claude_control_plane::DaemonPresenceState::Offline
            }
        )));

        let state_updates = control_plane_state.state_updates.read().await.clone();
        assert!(
            state_updates
                .iter()
                .any(|(_, update)| { matches!(update.state, ControlPlaneSessionState::Running) })
        );
        assert!(
            state_updates
                .iter()
                .any(|(_, update)| { matches!(update.state, ControlPlaneSessionState::Completed) })
        );

        let approval_decisions = control_plane_state.approval_decisions.read().await.clone();
        assert_eq!(approval_decisions.len(), 1);
        assert!(matches!(
            approval_decisions[0].1.decision,
            ApprovalDecision::Approved
        ));

        manager_shutdown_tx
            .send(true)
            .expect("manager shutdown should send");
        tokio::time::timeout(Duration::from_secs(5), manager_task)
            .await
            .expect("manager task should stop")
            .expect("manager task should join");
        runner_server.abort();
        let _ = runner_server.await;
        control_plane_server.abort();
        let _ = control_plane_server.await;
    }

    async fn fake_register_runner(
        State(state): State<FakeControlPlaneState>,
        Json(request): Json<RunnerRegistrationRequest>,
    ) -> Json<RunnerRegistrationLease> {
        state.register_count.fetch_add(1, Ordering::SeqCst);
        *state.registration.write().await = Some(request.clone());
        let now = Utc::now();
        Json(RunnerRegistrationLease {
            runner_id: request.runner_id.clone(),
            registered_at: now,
            lease_ttl_secs: 2,
            snapshot: RunnerSnapshot {
                registration: request,
                state: RunnerState::Idle,
                active_sessions: 0,
                queued_sessions: 0,
                registered_at: now,
                last_seen_at: now,
            },
        })
    }

    async fn fake_heartbeat(
        State(state): State<FakeControlPlaneState>,
        AxumPath(runner_id): AxumPath<String>,
        Json(heartbeat): Json<RunnerHeartbeat>,
    ) -> impl IntoResponse {
        let heartbeat_count = state.heartbeat_count.fetch_add(1, Ordering::SeqCst) + 1;
        if heartbeat_count == 1 {
            return StatusCode::NOT_FOUND.into_response();
        }

        let registration = state
            .registration
            .read()
            .await
            .clone()
            .expect("runner should be registered");
        let snapshot = RunnerSnapshot {
            registration,
            state: heartbeat.state,
            active_sessions: heartbeat.active_sessions,
            queued_sessions: heartbeat.queued_sessions,
            registered_at: Utc::now(),
            last_seen_at: heartbeat.timestamp,
        };
        debug_assert_eq!(runner_id, snapshot.registration.runner_id);
        Json(snapshot).into_response()
    }

    async fn hosted_runtime_event(
        State(state): State<HostedControlPlaneState>,
        AxumPath(session_id): AxumPath<Uuid>,
        Json(request): Json<RuntimeEventCreateRequest>,
    ) -> (StatusCode, Json<claude_control_plane::TimelineEvent>) {
        state
            .events
            .write()
            .await
            .push((session_id, request.clone()));
        let sequence = state.next_sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        (
            StatusCode::CREATED,
            Json(claude_control_plane::TimelineEvent {
                sequence,
                recorded_at: Utc::now(),
                runner_id: Some("runner-hosted".to_owned()),
                session_id: Some(session_id),
                detail: request.detail.into(),
            }),
        )
    }

    async fn hosted_session_state_update(
        State(state): State<HostedControlPlaneState>,
        AxumPath(session_id): AxumPath<Uuid>,
        Json(request): Json<SessionStateUpdateRequest>,
    ) -> Json<serde_json::Value> {
        state
            .state_updates
            .write()
            .await
            .push((session_id, request));
        Json(serde_json::json!({ "ok": true }))
    }

    async fn hosted_create_approval(
        State(state): State<HostedControlPlaneState>,
        AxumPath(session_id): AxumPath<Uuid>,
        Json(request): Json<ApprovalCreateRequest>,
    ) -> impl IntoResponse {
        state
            .approval_creates
            .write()
            .await
            .push((session_id, request.clone()));
        let response = state
            .client
            .post(format!(
                "{}/v1/sessions/{session_id}/approvals",
                state.runner_base_url
            ))
            .json(&request)
            .send()
            .await
            .expect("approval proxy request should succeed");
        let status = response.status();
        let approval = response
            .json::<ApprovalRequestRecord>()
            .await
            .expect("approval proxy response should decode");
        state.approvals.write().await.push(approval.clone());
        (status, Json(approval)).into_response()
    }

    async fn hosted_approval_decision(
        State(state): State<HostedControlPlaneState>,
        AxumPath(approval_id): AxumPath<Uuid>,
        Json(request): Json<ApprovalDecisionRequest>,
    ) -> impl IntoResponse {
        state
            .approval_decisions
            .write()
            .await
            .push((approval_id, request.clone()));
        let response = state
            .client
            .post(format!(
                "{}/v1/approvals/{approval_id}/decision",
                state.runner_base_url
            ))
            .json(&request)
            .send()
            .await
            .expect("approval decision proxy request should succeed");
        let status = response.status();
        let approval = response
            .json::<ApprovalRequestRecord>()
            .await
            .expect("approval decision proxy response should decode");
        (status, Json(approval)).into_response()
    }

    fn create_fake_remote_code_script(
        directory: &Path,
        input_log_path: &Path,
        artifact_id: Uuid,
    ) -> PathBuf {
        if cfg!(windows) {
            let wrapper_path = directory.join("fake-remote-code.cmd");
            let powershell_path = directory.join("fake-remote-code.ps1");
            let powershell_script = format!(
                "$logFile = '{log_path}'\r\n\
Write-Output '{{\"type\":\"system\",\"subtype\":\"session_state_changed\",\"state\":\"running\"}}'\r\n\
Write-Output '{{\"type\":\"message_delta\",\"role\":\"assistant\",\"delta\":\"Hello from hosted session\",\"message_id\":\"msg-1\"}}'\r\n\
Write-Output '{{\"type\":\"message_committed\",\"role\":\"assistant\",\"text\":\"Hello from hosted session\",\"message_id\":\"msg-1\"}}'\r\n\
Write-Output '{{\"type\":\"tool_started\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\"}}'\r\n\
Write-Output '{{\"type\":\"tool_progress\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\",\"input_delta\":\"dir\"}}'\r\n\
Write-Output '{{\"type\":\"artifact_manifest\",\"artifact_ids\":[\"{artifact_id}\"]}}'\r\n\
Write-Output '{{\"type\":\"control_request\",\"request_id\":\"req-1\",\"request\":{{\"subtype\":\"can_use_tool\",\"tool_name\":\"shell\",\"tool_use_id\":\"tool-1\",\"title\":\"Approve shell\",\"description\":\"Need shell access\",\"blocked_path\":\"C:\\\\workspace\"}}}}'\r\n\
$lineCount = 0\r\n\
while (($line = [Console]::In.ReadLine()) -ne $null) {{\r\n\
  Add-Content -LiteralPath $logFile -Value $line\r\n\
  $lineCount += 1\r\n\
  if ($lineCount -ge 3) {{\r\n\
    break\r\n\
  }}\r\n\
}}\r\n\
Write-Output '{{\"type\":\"tool_finished\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\",\"is_error\":false,\"summary\":\"done\"}}'\r\n\
Write-Output '{{\"type\":\"system\",\"subtype\":\"session_state_changed\",\"state\":\"idle\"}}'\r\n",
                log_path = powershell_single_quote_escape(input_log_path),
            );
            std::fs::write(&powershell_path, powershell_script)
                .expect("fake remote-code powershell script should be written");
            let wrapper = format!(
                "@echo off\r\n\
powershell.exe -NoProfile -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                powershell_path.display()
            );
            std::fs::write(&wrapper_path, wrapper)
                .expect("fake remote-code wrapper should be written");
            return wrapper_path;
        }

        let script_path = directory.join("fake-remote-code");
        let script = format!(
            "#!/bin/sh\n\
log_file='{log_path}'\n\
printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"session_state_changed\",\"state\":\"running\"}}'\n\
printf '%s\\n' '{{\"type\":\"message_delta\",\"role\":\"assistant\",\"delta\":\"Hello from hosted session\",\"message_id\":\"msg-1\"}}'\n\
printf '%s\\n' '{{\"type\":\"message_committed\",\"role\":\"assistant\",\"text\":\"Hello from hosted session\",\"message_id\":\"msg-1\"}}'\n\
printf '%s\\n' '{{\"type\":\"tool_started\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\"}}'\n\
printf '%s\\n' '{{\"type\":\"tool_progress\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\",\"input_delta\":\"ls\"}}'\n\
printf '%s\\n' '{{\"type\":\"artifact_manifest\",\"artifact_ids\":[\"{artifact_id}\"]}}'\n\
printf '%s\\n' '{{\"type\":\"control_request\",\"request_id\":\"req-1\",\"request\":{{\"subtype\":\"can_use_tool\",\"tool_name\":\"shell\",\"tool_use_id\":\"tool-1\",\"title\":\"Approve shell\",\"description\":\"Need shell access\",\"blocked_path\":\"/workspace\"}}}}'\n\
line_count=0\n\
while IFS= read -r line; do\n\
  printf '%s\\n' \"$line\" >> \"$log_file\"\n\
  line_count=$((line_count + 1))\n\
  if [ \"$line_count\" -ge 3 ]; then\n\
    break\n\
  fi\n\
done\n\
printf '%s\\n' '{{\"type\":\"tool_finished\",\"tool_use_id\":\"tool-1\",\"tool_name\":\"shell\",\"is_error\":false,\"summary\":\"done\"}}'\n\
printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"session_state_changed\",\"state\":\"idle\"}}'\n",
            log_path = shell_escape_single_quoted(input_log_path),
        );
        std::fs::write(&script_path, script).expect("fake remote-code script should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)
                .expect("fake remote-code script should be executable");
        }
        script_path
    }

    fn shell_escape_single_quoted(path: &Path) -> String {
        path.display().to_string().replace('\'', "'\"'\"'")
    }

    fn powershell_single_quote_escape(path: &Path) -> String {
        path.display().to_string().replace('\'', "''")
    }
}
