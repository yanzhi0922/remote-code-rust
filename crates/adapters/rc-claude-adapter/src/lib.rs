//! # rc-claude-adapter
//!
//! In-process adapter for the Claude agent.
//!
//! [`ClaudeInProcessAdapter`] wraps the [`QueryEngine`](claude_query_engine::QueryEngine)
//! into a unified [`AgentAdapter`](claude_agent_protocol::adapter::AgentAdapter) interface,
//! consistent with [`CodexInProcessAdapter`](rc_codex_adapter::CodexInProcessAdapter)
//! and [`RooInProcessAdapter`](rc_roo_adapter::RooInProcessAdapter).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  ClaudeInProcessAdapter                          │
//! │  ┌────────────────────────────────────────────┐  │
//! │  │ AgentAdapter impl                          │  │
//! │  │  start()  →  save config, set Ready       │  │
//! │  │  send_message()  →  spawn QueryEngine     │  │
//! │  │  cancel()  →  CancellationToken            │  │
//! │  │  resolve_permission()  →  oneshot tx       │  │
//! │  └────────────────────────────────────────────┘  │
//! │  ┌─────────────────┐  ┌──────────────────────┐   │
//! │  │ AdapterPermission│  │ AdapterQueryObserver │   │
//! │  │ Broker           │  │ maps observer events │   │
//! │  │ decide() → event│  │ → UnifiedAgentEvent  │   │
//! │  └─────────────────┘  └──────────────────────┘   │
//! │  ┌────────────────────────────────────────────┐  │
//! │  │ AdapterToolRunner                          │  │
//! │  │ wraps execute_tool_call()                  │  │
//! │  └────────────────────────────────────────────┘  │
//! └──────────────────────────────────────────────────┘
//! ```

mod event_mapper;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use claude_agent_protocol::adapter::AgentAdapter;
use claude_agent_protocol::events::UnifiedAgentEvent;
use claude_agent_protocol::permission::PermissionDecision as ProtocolPermissionDecision;
use claude_agent_protocol::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};
use claude_config::ProviderConfig;
use claude_core::{ConversationEntry, Message, PermissionMode, ProviderProtocol, SessionId};
use claude_engine_events::EventStream;
use claude_permissions::{
    PermissionBroker, PermissionDecision as PermissionsPermissionDecision,
    PermissionRequest as PermissionsPermissionRequest,
};
use claude_provider::ProviderCompatBackend;
use claude_query_engine::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngine, QueryEngineConfig, QueryObserver,
    QueryObserverEvent, ToolRunResult, ToolRunner,
};
use claude_tools::{FileStateCache, execute_tool_call};

// ─── PendingPermission ─────────────────────────────────────────────────────

/// A pending permission request waiting for user resolution.
struct PendingPermission {
    response_tx: oneshot::Sender<PermissionsPermissionDecision>,
}

// ─── AdapterPermissionBroker ────────────────────────────────────────────────

/// Permission broker that forwards requests through the event channel
/// and waits for resolution via [`ClaudeInProcessAdapter::resolve_permission`].
struct AdapterPermissionBroker {
    event_tx: mpsc::Sender<UnifiedAgentEvent>,
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
    session_id: String,
}

impl AdapterPermissionBroker {
    fn new(
        event_tx: mpsc::Sender<UnifiedAgentEvent>,
        pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
        session_id: String,
    ) -> Self {
        Self {
            event_tx,
            pending_permissions,
            session_id,
        }
    }
}

#[async_trait]
impl PermissionBroker for AdapterPermissionBroker {
    async fn decide(
        &self,
        request: PermissionsPermissionRequest,
    ) -> PermissionsPermissionDecision {
        let request_id = uuid::Uuid::new_v4().to_string();

        // Create oneshot channel for the response.
        let (response_tx, response_rx) = oneshot::channel();
        let pending = PendingPermission { response_tx };

        // Store the pending permission.
        {
            let mut map = self.pending_permissions.lock().await;
            map.insert(request_id.clone(), pending);
        }

        // Send the permission request event.
        let event = UnifiedAgentEvent::PermissionRequest {
            session_id: self.session_id.clone(),
            request_id: request_id.clone(),
            tool_name: request.tool_name.clone(),
            input: request.tool_input.clone(),
        };

        if let Err(e) = self.event_tx.send(event).await {
            warn!(%request_id, "failed to send permission request event: {e}");
            return PermissionsPermissionDecision::deny("Failed to send permission request");
        }

        // Wait for the user's response.
        match response_rx.await {
            Ok(decision) => decision,
            Err(_) => {
                warn!(%request_id, "permission response channel dropped");
                PermissionsPermissionDecision::deny("Permission request cancelled")
            }
        }
    }
}

// ─── AdapterToolRunner ──────────────────────────────────────────────────────

/// Tool runner that wraps [`execute_tool_call`] and emits tool events.
struct AdapterToolRunner {
    broker: Arc<dyn PermissionBroker>,
    cwd: PathBuf,
    timeout_ms: u64,
}

impl AdapterToolRunner {
    fn new(broker: Arc<dyn PermissionBroker>, cwd: PathBuf, timeout_ms: u64) -> Self {
        Self {
            broker,
            cwd,
            timeout_ms,
        }
    }
}

#[async_trait]
impl ToolRunner for AdapterToolRunner {
    async fn run_tool(
        &self,
        tool_call: &claude_core::ToolCall,
        _context: &ProcessUserInputContext,
    ) -> Result<ToolRunResult> {
        let tool_context = claude_tools::ToolExecutionContext {
            cwd: self.cwd.clone(),
            original_cwd: self.cwd.clone(),
            active_worktree_session: None,
            timeout_ms: self.timeout_ms,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                claude_core::task_stack::TaskStack::default(),
            )),
            read_file_state: FileStateCache::new(),
        };

        let tool_result = match execute_tool_call(tool_call, &tool_context, self.broker.as_ref()).await {
            Ok(result) => result,
            Err(error) => claude_core::ToolResult {
                content: format!("Tool execution error: {error}"),
                is_error: true,
                content_blocks: Vec::new(),
                follow_up_user_blocks: Vec::new(),
            },
        };

        // Handle follow-up user blocks as post_messages.
        let mut post_messages = Vec::new();
        if !tool_result.follow_up_user_blocks.is_empty() {
            let follow_up_entry = ConversationEntry::user_with_content_blocks(
                tool_result.follow_up_user_blocks.clone(),
            );
            post_messages.push(Message::from(follow_up_entry));
        }

        Ok(ToolRunResult {
            result: tool_result,
            pre_messages: Vec::new(),
            post_messages,
            permission_denial: None,
        })
    }
}

// ─── AdapterQueryObserver ───────────────────────────────────────────────────

/// Query observer that maps [`QueryObserverEvent`] → [`UnifiedAgentEvent`]
/// and forwards them through the event channel.
struct AdapterQueryObserver {
    event_tx: mpsc::Sender<UnifiedAgentEvent>,
    session_id: String,
}

impl AdapterQueryObserver {
    fn new(event_tx: mpsc::Sender<UnifiedAgentEvent>, session_id: String) -> Self {
        Self {
            event_tx,
            session_id,
        }
    }
}

#[async_trait]
impl QueryObserver for AdapterQueryObserver {
    async fn on_event(&self, event: QueryObserverEvent) -> Result<()> {
        if let Some(unified) = event_mapper::map_observer_event(event, &self.session_id) {
            // Use a tolerant send — if the receiver is dropped (e.g. consumer
            // disconnected), we silently drop the event rather than erroring.
            let _ = self.event_tx.send(unified).await;
        }
        Ok(())
    }
}

// ─── ClaudeInProcessAdapter ─────────────────────────────────────────────────

/// In-process adapter that wraps the Claude [`QueryEngine`] into the
/// [`AgentAdapter`] trait.
pub struct ClaudeInProcessAdapter {
    info: AgentInfo,
    status: AgentStatus,

    // Configuration
    model: String,
    provider_name: String,
    api_key: Option<String>,
    base_url: Option<String>,
    working_dir: PathBuf,
    permission_mode: PermissionMode,
    max_turns: usize,

    // Runtime state
    cancel_token: CancellationToken,
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,

    // Session state
    session_messages: Arc<Mutex<Vec<Message>>>,
}

impl ClaudeInProcessAdapter {
    /// Create a new adapter with the given configuration.
    pub fn new(
        model: String,
        provider_name: String,
        api_key: Option<String>,
        base_url: Option<String>,
        working_dir: PathBuf,
        permission_mode: PermissionMode,
        max_turns: usize,
    ) -> Self {
        let mut caps = HashSet::new();
        caps.insert(AgentCapability::Streaming);
        caps.insert(AgentCapability::ToolUse);
        caps.insert(AgentCapability::Permissions);

        let info = AgentInfo {
            name: "Remote Claude".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities: caps,
            status: AgentStatus::Starting,
        };

        Self {
            info,
            status: AgentStatus::Starting,
            model,
            provider_name,
            api_key,
            base_url,
            working_dir,
            permission_mode,
            max_turns,
            cancel_token: CancellationToken::new(),
            worker_handle: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            session_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Determine the provider protocol from the provider name.
    fn infer_protocol(&self) -> ProviderProtocol {
        match self.provider_name.to_ascii_lowercase().as_str() {
            "anthropic" => ProviderProtocol::Anthropic,
            _ => ProviderProtocol::OpenAi,
        }
    }

    /// Build a [`ProviderConfig`] from the adapter's configuration.
    fn build_provider_config(&self) -> ProviderConfig {
        ProviderConfig {
            name: self.provider_name.clone(),
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: Some(self.model.clone()),
            protocol: self.infer_protocol(),
            timeout_ms: 600_000,
            max_output_tokens: 16_384,
            max_retries: 3,
            retry_initial_backoff_ms: 1_000,
            retry_max_backoff_ms: 30_000,
            respect_retry_after: true,
            request_header_overrides: Default::default(),
            request_metadata: Default::default(),
            thinking_budget: None,
        }
    }
}

#[async_trait]
impl AgentAdapter for ClaudeInProcessAdapter {
    async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
        info!("ClaudeInProcessAdapter starting");
        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;
        Ok(())
    }

    async fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> Result<mpsc::Receiver<UnifiedAgentEvent>> {
        info!(%session_id, "ClaudeInProcessAdapter: send_message");

        let (event_tx, event_rx) = mpsc::channel(64);

        // 1. Build provider config and client.
        let provider_config = self.build_provider_config();
        let provider_client = Arc::new(
            claude_provider::ProviderClient::new()
                .map_err(|e| anyhow!("failed to create provider client: {e}"))?,
        );

        // 2. Create the conversation backend.
        let backend: Arc<dyn claude_provider::ConversationBackend> = Arc::new(
            ProviderCompatBackend::new(provider_client, &provider_config),
        );

        // 3. Create permission broker.
        let broker = Arc::new(AdapterPermissionBroker::new(
            event_tx.clone(),
            self.pending_permissions.clone(),
            session_id.to_owned(),
        ));

        // 4. Create tool runner.
        let tool_runner = Arc::new(AdapterToolRunner::new(
            broker.clone(),
            self.working_dir.clone(),
            provider_config.timeout_ms,
        ));

        // 5. Create query observer.
        let observer = Arc::new(AdapterQueryObserver::new(
            event_tx.clone(),
            session_id.to_owned(),
        ));

        // 6. Build QueryEngineConfig.
        let claude_session_id = SessionId::from(session_id.to_owned());
        let event_stream = EventStream::new(64);

        let mut query_config = QueryEngineConfig::new(
            claude_session_id.clone(),
            &self.model,
            backend,
            tool_runner,
            event_stream,
        )
        .with_observer(observer)
        .with_provider_invocation_mode(ProviderInvocationMode::Streaming);

        query_config.max_turns = self.max_turns.max(1) as u32;

        // 7. Load existing messages and create user message.
        let existing_messages = {
            let guard = self.session_messages.lock().await;
            guard.clone()
        };

        let user_message = vec![Message::from(ConversationEntry::user(message))];

        // 8. Create the query engine.
        let mut engine = QueryEngine::new(query_config, existing_messages);

        // 9. Build the process-user-input context.
        let context = ProcessUserInputContext::new(
            claude_session_id,
            self.permission_mode,
            &self.model,
        );

        // 10. Send Started event.
        let _ = event_tx
            .send(UnifiedAgentEvent::Started(self.info.clone()))
            .await;

        // 11. Spawn the query engine task.
        let cancel_token = self.cancel_token.clone();
        let session_messages = self.session_messages.clone();

        let handle = tokio::spawn(async move {
            // Check cancellation before starting.
            if cancel_token.is_cancelled() {
                debug!("Query cancelled before starting");
                return;
            }

            // Run the query engine to completion.
            match engine.submit_message(user_message, context).await {
                Ok(result) => {
                    debug!(
                        stops = %result.stop_reason,
                        turns = result.turns,
                        "Query engine completed"
                    );
                    // Persist the updated messages.
                    let mut guard = session_messages.lock().await;
                    *guard = result.state.messages;
                }
                Err(e) => {
                    warn!("Query engine error: {e}");
                }
            }
        });

        self.worker_handle = Some(handle);
        self.status = AgentStatus::Busy;
        self.info.status = AgentStatus::Busy;

        Ok(event_rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> Result<()> {
        info!("ClaudeInProcessAdapter: cancel");
        self.cancel_token.cancel();

        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.status = AgentStatus::Idle;
        self.info.status = AgentStatus::Idle;
        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: ProtocolPermissionDecision,
    ) -> Result<()> {
        debug!(%request_id, "Resolving permission request");

        let mut map = self.pending_permissions.lock().await;
        if let Some(pending) = map.remove(request_id) {
            let permissions_decision = match decision {
                ProtocolPermissionDecision::Allow => PermissionsPermissionDecision::allow(),
                ProtocolPermissionDecision::Deny => {
                    PermissionsPermissionDecision::deny("Permission denied by user")
                }
                ProtocolPermissionDecision::AllowAll => {
                    // AllowAll is treated as allow for the current request.
                    // TODO: Consider adding a session rule for future auto-approval.
                    PermissionsPermissionDecision::allow()
                }
            };

            let _ = pending.response_tx.send(permissions_decision);
            Ok(())
        } else {
            Err(anyhow!("no pending permission request with id {request_id}"))
        }
    }

    async fn stop(&mut self) -> Result<()> {
        info!("ClaudeInProcessAdapter: stop");
        self.cancel_token.cancel();

        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }

        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.worker_handle
            .as_ref()
            .map_or(false, |h| !h.is_finished())
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteClaude
    }
}
