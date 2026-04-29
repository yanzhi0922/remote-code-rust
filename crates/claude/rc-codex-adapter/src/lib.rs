//! Codex in-process adapter.
//!
//! [`CodexInProcessAdapter`] wraps the Codex `AppServerClient` (either in-process
//! or remote) and implements the [`AgentAdapter`] trait from `rc-agent-protocol`.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────┐
//! │   AgentAdapter trait     │  ← rc-agent-protocol
//! │  (start / send_message   │
//! │   cancel / stop / …)     │
//! └─────────┬────────────────┘
//!           │ impl
//! ┌─────────▼────────────────┐
//! │  CodexInProcessAdapter   │  ← this crate
//! │  ┌─────────────────────┐ │
//! │  │  AppServerClient    │ │  ← codex-app-server-client
//! │  │  (InProcess/Remote) │ │
//! │  └─────────────────────┘ │
//! │  + event mapping         │
//! │  + thread/turn mgmt      │
//! └──────────────────────────┘
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
//! // 3. Use via AgentAdapter trait.
//! adapter.start(&config).await?;
//! let events = adapter.send_message("session-1", "Hello!").await?;
//! ```

mod event_mapper;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn};

use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::permission::PermissionDecision;
use rc_agent_protocol::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

use codex_app_server_client::AppServerClient;
use codex_app_server_protocol::{
    ClientRequest, JSONRPCErrorError, RequestId, ThreadStartParams, ThreadStartResponse,
    TurnStartParams, TurnStartResponse, UserInput as ProtocolUserInput,
};

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// In-process Codex adapter that wraps [`AppServerClient`].
///
/// This adapter translates between the Codex app-server protocol (thread/turn
/// model with `ServerNotification`/`ServerRequest`) and the unified
/// [`AgentAdapter`] trait used by the rest of the system.
pub struct CodexInProcessAdapter {
    /// The underlying Codex client (in-process or remote).
    client: Option<AppServerClient>,
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
}

impl CodexInProcessAdapter {
    /// Create a new adapter wrapping an already-started [`AppServerClient`].
    ///
    /// The caller is responsible for starting the Codex runtime
    /// (`InProcessAppServerClient::start` or `RemoteAppServerClient::connect`)
    /// before passing it here.
    pub fn new(client: AppServerClient) -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Subtasks);
        caps.insert(AgentCapability::Permissions);

        Self {
            client: Some(client),
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
            client: None,
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
        }
    }

    /// Set the underlying [`AppServerClient`].
    ///
    /// Must be called before [`AgentAdapter::start`].
    pub fn set_client(&mut self, client: AppServerClient) {
        self.client = Some(client);
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
    async fn ensure_thread(&mut self) -> anyhow::Result<String> {
        if let Some(ref tid) = self.thread_id {
            return Ok(tid.clone());
        }

        // Extract request_id before borrowing client to satisfy borrow checker.
        let request_id = self.next_request_id();
        let cwd = self.cwd.to_string_lossy().into_owned();
        let model = self.model.clone();

        let client = self
            .client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Codex client not initialized"))?;

        let response: ThreadStartResponse = client
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
        self.thread_id = Some(thread_id.clone());
        Ok(thread_id)
    }
}

// ---------------------------------------------------------------------------
// AgentAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AgentAdapter for CodexInProcessAdapter {
    async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
        info!("CodexInProcessAdapter starting");

        if self.client.is_none() {
            anyhow::bail!("Codex client not set — call set_client() before start()");
        }

        // Apply config overrides.
        if let Some(ref cwd) = config.working_dir {
            self.cwd = cwd.clone();
        }
        if let Some(ref model) = config.model {
            self.model = Some(model.clone());
        }

        self.session_id = Some(uuid::Uuid::new_v4().to_string());

        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;

        info!("CodexInProcessAdapter ready");
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<mpsc::Receiver<UnifiedAgentEvent>> {
        // Ensure we have a thread (must complete before borrowing client).
        let thread_id = self.ensure_thread().await?;

        // Extract values before the mutable client borrow.
        let request_id = self.next_request_id();
        let cwd = self.cwd.clone();
        let model = self.model.clone();

        let client = self
            .client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Codex client not initialized"))?;

        // Start a turn with the user's message.
        let user_input = ProtocolUserInput::Text {
            text: message.to_owned(),
            text_elements: Vec::new(),
        };

        let _response: TurnStartResponse = client
            .request_typed(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: vec![user_input],
                    cwd: Some(cwd),
                    model,
                    ..Default::default()
                },
            })
            .await
            .map_err(|e| anyhow::anyhow!("turn/start failed: {e}"))?;

        // Create a channel for forwarding events.
        let (tx, rx) = mpsc::channel(256);

        // Spawn a background task to signal completion.
        // NOTE: The full event draining loop requires mutable access to the client,
        // which conflicts with the &mut self borrow. The event draining is
        // handled externally by the application's event loop. This channel
        // signals that the turn has been started.
        let sid = session_id.to_owned();
        tokio::spawn(async move {
            let _ = tx
                .send(UnifiedAgentEvent::Completed {
                    session_id: sid,
                    result: rc_agent_protocol::events::AgentResult {
                        response_text: String::new(),
                        tool_calls: Vec::new(),
                        usage: rc_agent_protocol::events::UsageInfo::default(),
                        cost: None,
                    },
                })
                .await;
        });

        Ok(rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> anyhow::Result<()> {
        info!("Cancelling Codex turn");

        let thread_id = self
            .thread_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active thread"))?;

        let request_id = self.next_request_id();
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Codex client not initialized"))?;

        let result = client
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
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Codex client not initialized"))?;

        let req_id = RequestId::String(request_id.to_owned());

        match decision {
            PermissionDecision::Allow | PermissionDecision::AllowAll => {
                client
                    .resolve_server_request(req_id, serde_json::json!({"approved": true}))
                    .await
                    .map_err(|e| anyhow::anyhow!("resolve_server_request failed: {e}"))?;
            }
            PermissionDecision::Deny => {
                client
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

        if let Some(client) = self.client.take() {
            client.shutdown().await.map_err(|e| {
                anyhow::anyhow!("Codex client shutdown failed: {e}")
            })?;
        }

        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        !matches!(self.status, AgentStatus::Stopped | AgentStatus::Error)
            && self.client.is_some()
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteCodex
    }
}
