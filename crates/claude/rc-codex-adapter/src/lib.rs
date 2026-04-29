//! Codex in-process adapter.
//!
//! [`CodexInProcessAdapter`] wraps the Codex `AppServerClient` (either in-process
//! or remote) and implements the [`AgentAdapter`] trait from `rc-agent-protocol`.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  CodexInProcessAdapter                       │
//! │  ┌──────────────┐  ┌───────────────────────┐ │
//! │  │ request_handle│  │ event_pump (bg task)  │ │
//! │  │ (Clone)       │  │ owns AppServerClient  │ │
//! │  │               │  │ loops next_event()    │ │
//! │  │ - request()   │  │ maps via event_mapper │ │
//! │  │ - resolve()   │  │ forwards to event_tx  │ │
//! │  │ - reject()    │  └───────────┬───────────┘ │
//! │  └──────┬───────┘              │             │
//! │         │          ┌───────────▼───────────┐ │
//! │         │          │ Arc<Mutex<Option<tx>>> │ │
//! │         │          │ (shared event router)  │ │
//! │         │          └───────────┬───────────┘ │
//! │  send_message() installs new rx│             │
//! │  cancel() sends TurnInterrupt  │             │
//! │  resolve_permission() resolves │             │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use rc_codex_adapter::CodexInProcessAdapter;
//! use codex_app_server_client::InProcessAppServerClient;
//!
//! // 1. Start the Codex in-process runtime (at the application level).
//! let client = InProcessAppServerClient::start(args).await?;
//!
//! // 2. Wrap it in the adapter.
//! let adapter = CodexInProcessAdapter::new(
//!     codex_app_server_client::AppServerClient::InProcess(client),
//! );
//!
//! // 3. Register with the agent router.
//! adapter.start(&config).await?;
//! router.register("session-id".into(), Box::new(adapter)).await;
//! ```

mod event_mapper;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::permission::PermissionDecision;
use rc_agent_protocol::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

use codex_app_server_client::{AppServerClient, AppServerRequestHandle};
use codex_app_server_protocol::{
    ClientRequest, JSONRPCErrorError, RequestId, ThreadStartParams, ThreadStartResponse,
    TurnStartParams, TurnStartResponse, UserInput as ProtocolUserInput,
};

// ---------------------------------------------------------------------------
// Shared event routing state
// ---------------------------------------------------------------------------

/// Shared state between the adapter and the background event pump.
///
/// The pump writes events to whichever sender is currently installed.
/// `send_message()` swaps in a new sender for each turn.
struct EventPumpState {
    /// The current event sender, swapped by `send_message()`.
    current_tx: Option<mpsc::Sender<UnifiedAgentEvent>>,
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// In-process Codex adapter that wraps [`AppServerClient`].
///
/// On [`start()`](AgentAdapter::start), the adapter extracts a cloneable
/// [`AppServerRequestHandle`] from the client for sending commands, then spawns
/// a background tokio task that continuously drains events from the client,
/// maps them through [`event_mapper`], and forwards them to the caller via
/// a shared `mpsc::Sender`.
pub struct CodexInProcessAdapter {
    /// Cloneable handle for sending commands (requests, resolve, reject).
    request_handle: Option<AppServerRequestHandle>,
    /// Shared event routing state between adapter and background pump.
    event_state: Arc<Mutex<EventPumpState>>,
    /// Handle to the background event pump task.
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    /// Static agent metadata.
    info: AgentInfo,
    /// Runtime status.
    status: AgentStatus,
    /// Current session ID (set during `start`).
    session_id: Option<String>,
    /// Current thread ID within the Codex runtime.
    thread_id: Option<String>,
    /// Monotonic request ID counter.
    request_counter: AtomicI64,
    /// Working directory for Codex operations.
    cwd: PathBuf,
    /// Model override.
    model: Option<String>,
    /// Placeholder to hold the client until `start()` consumes it for the event pump.
    _client_placeholder: Option<AppServerClient>,
}

impl CodexInProcessAdapter {
    /// Create a new adapter wrapping an already-started [`AppServerClient`].
    ///
    /// The caller is responsible for starting the Codex runtime
    /// (`InProcessAppServerClient::start` or `RemoteAppServerClient::connect`)
    /// before passing it here. The client will be consumed during [`start()`](AgentAdapter::start)
    /// when the background event pump is spawned.
    pub fn new(client: AppServerClient) -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Subtasks);
        caps.insert(AgentCapability::Permissions);

        // Extract the request handle immediately — it's cloneable and doesn't
        // need the full client.
        let request_handle = Some(client.request_handle());

        Self {
            request_handle,
            event_state: Arc::new(Mutex::new(EventPumpState { current_tx: None })),
            worker_handle: None,
            info: AgentInfo {
                name: "Codex In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            session_id: None,
            thread_id: None,
            request_counter: AtomicI64::new(1),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: None,
            // Hold the client until start() consumes it for the event pump.
            _client_placeholder: Some(client),
        }
    }

    /// Create a new adapter in the **Starting** state without a client.
    ///
    /// The client must be set later via [`Self::set_client`] before calling
    /// [`AgentAdapter::start`].
    pub fn empty() -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Subtasks);
        caps.insert(AgentCapability::Permissions);

        Self {
            request_handle: None,
            event_state: Arc::new(Mutex::new(EventPumpState { current_tx: None })),
            worker_handle: None,
            info: AgentInfo {
                name: "Codex In-Process".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                capabilities: caps,
                status: AgentStatus::Starting,
            },
            status: AgentStatus::Starting,
            session_id: None,
            thread_id: None,
            request_counter: AtomicI64::new(1),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model: None,
            _client_placeholder: None,
        }
    }

    /// Set the underlying [`AppServerClient`].
    ///
    /// Must be called before [`AgentAdapter::start`].
    pub fn set_client(&mut self, client: AppServerClient) {
        self.request_handle = Some(client.request_handle());
        self._client_placeholder = Some(client);
    }

    /// Set the working directory for Codex operations.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    /// Set the model override.
    pub fn set_model(&mut self, model: String) {
        self.model = Some(model);
    }

    /// Generate the next unique request ID.
    fn next_request_id(&self) -> RequestId {
        let n = self.request_counter.fetch_add(1, Ordering::Relaxed);
        RequestId::Integer(n)
    }

    /// Ensure a thread exists (create one if needed) and return its ID.
    async fn ensure_thread(&self) -> anyhow::Result<String> {
        if let Some(ref tid) = self.thread_id {
            return Ok(tid.clone());
        }

        let request_id = self.next_request_id();
        let cwd = self.cwd.to_string_lossy().into_owned();
        let model = self.model.clone();

        let handle = self
            .request_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex request handle not initialized"))?;

        let response: ThreadStartResponse = handle
            .request_typed(ClientRequest::ThreadStart {
                request_id,
                params: ThreadStartParams {
                    cwd: Some(cwd),
                    model,
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| anyhow::anyhow!("thread/start failed: {e}"))?;

        let thread_id = response.thread.id.clone();
        info!(thread_id = %thread_id, "Codex thread started");
        // Note: we can't set self.thread_id here because this takes &self.
        // The caller (send_message) will set it.
        Ok(thread_id)
    }

    /// Background task that continuously drains events from the Codex client,
    /// maps them through the event mapper, and forwards them to the current
    /// event sender.
    async fn event_pump(
        mut client: AppServerClient,
        event_state: Arc<Mutex<EventPumpState>>,
        session_id: String,
    ) {
        info!("Codex event pump started");
        loop {
            match client.next_event().await {
                Some(event) => {
                    let mapped = event_mapper::map_app_server_event(event, &session_id);
                    let mut state = event_state.lock().await;
                    if let Some(ref tx) = state.current_tx {
                        for evt in mapped {
                            if tx.send(evt).await.is_err() {
                                // Receiver dropped — clear the sender.
                                state.current_tx = None;
                                break;
                            }
                        }
                    }
                    // Don't hold the lock while awaiting next event.
                }
                None => {
                    info!("Codex event pump: client disconnected");
                    break;
                }
            }
        }
        info!("Codex event pump stopped");
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for CodexInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!("CodexInProcessAdapter starting");

        // Apply config overrides.
        if let Some(ref cwd) = config.working_dir {
            self.cwd = cwd.clone();
        }
        if let Some(ref model) = config.model {
            self.model = Some(model.clone());
        }

        self.session_id = Some(uuid::Uuid::new_v4().to_string());
        let session_id = self.session_id.as_ref().unwrap().clone();

        // Take the client and spawn the background event pump.
        let client = self
            ._client_placeholder
            .take()
            .ok_or_else(|| anyhow::anyhow!("Codex client not set — call set_client() before start()"))?;

        // Request handle was already extracted in new()/set_client().
        let handle = self.event_state.clone();
        let worker = tokio::spawn(Self::event_pump(client, handle, session_id));
        self.worker_handle = Some(worker);

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!("CodexInProcessAdapter ready");
        Ok(())
    }

    async fn send_message(
        &mut self,
        _session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        // Ensure we have a thread. Use the request handle (&self, no borrow conflict).
        let thread_id = self.ensure_thread().await?;
        self.thread_id = Some(thread_id.clone());

        // Create a new channel for this turn's events.
        let (tx, rx) = mpsc::channel(256);

        // Install the sender in the shared state so the event pump can forward
        // events to it. Do this BEFORE starting the turn so we don't miss any
        // events.
        {
            let mut state = self.event_state.lock().await;
            state.current_tx = Some(tx);
        }

        // Send TurnStart via the request handle.
        let request_id = self.next_request_id();
        let cwd = self.cwd.clone();
        let model = self.model.clone();

        let handle = self
            .request_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex request handle not initialized"))?;

        let user_input = ProtocolUserInput::Text {
            text: message.to_owned(),
            text_elements: Vec::new(),
        };

        let _response: TurnStartResponse = handle
            .request_typed(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: vec![user_input],
                    cwd: Some(cwd.clone()),
                    model,
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| anyhow::anyhow!("turn/start failed: {e}"))?;

        Ok(rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        info!("Cancelling Codex turn");

        let thread_id = self
            .thread_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active thread"))?;

        let request_id = self.next_request_id();
        let handle = self
            .request_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex request handle not initialized"))?;

        let result = handle
            .request_typed::<codex_app_server_protocol::TurnInterruptResponse>(
                ClientRequest::TurnInterrupt {
                    request_id,
                    params: codex_app_server_protocol::TurnInterruptParams {
                        thread_id,
                        turn_id: String::new(), // empty = current turn
                    },
                },
            )
            .await;

        if let Err(err) = result {
            warn!(error = %err, "turn/interrupt failed (may be no active turn)");
        }

        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: PermissionDecision,
    ) -> anyhow::Result<()> {
        let handle = self
            .request_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex request handle not initialized"))?;

        let req_id = RequestId::String(request_id.to_owned());

        match decision {
            PermissionDecision::Allow | PermissionDecision::AllowAll => {
                handle
                    .resolve_server_request(req_id, serde_json::json!({"approved": true}))
                    .await
                    .map_err(|e| anyhow::anyhow!("resolve_server_request failed: {e}"))?;
            }
            PermissionDecision::Deny => {
                handle
                    .reject_server_request(
                        req_id,
                        JSONRPCErrorError {
                            code: -32000,
                            message: "Permission denied by user".to_string(),
                            data: None,
                        },
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("reject_server_request failed: {e}"))?;
            }
        }

        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("CodexInProcessAdapter stopping");

        // Clear the event sender so the pump stops forwarding.
        {
            let mut state = self.event_state.lock().await;
            state.current_tx = None;
        }

        // Drop the request handle.
        self.request_handle = None;

        // Abort the background event pump.
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        !matches!(self.status, AgentStatus::Stopped | AgentStatus::Error)
            && self.request_handle.is_some()
            && self
                .worker_handle
                .as_ref()
                .map_or(false, |h| !h.is_finished())
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteCodex
    }
}
