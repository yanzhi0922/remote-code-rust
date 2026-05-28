//! # rc-claude-adapter
//!
//! In-process adapter for the Claude agent.
//!
//! [`ClaudeInProcessAdapter`] wraps the [`QueryEngine`](claude_query_engine::QueryEngine)
//! into a unified [`AgentAdapter`](rc_agent_protocol::adapter::AgentAdapter) interface,
//! consistent with [`CodexInProcessAdapter`](rc_codex_adapter::CodexInProcessAdapter)
//! and [`RooInProcessAdapter`](rc_roo_adapter::RooInProcessAdapter).
//!
//! # Architecture
//!
//! This adapter achieves 100% parity with the GUI direct path
//! ([`GuiToolRunner`] / [`GuiQueryObserver`]) by:
//! - Accepting [`RuntimeConfig`] + [`SessionStore`] for full configuration and persistence.
//! - Building a complete [`ToolExecutionContext`] with `sub_agent`, `progress_cb`,
//!   and `active_worktree_session`.
//! - Persisting tool results and lifecycle events via [`SessionStore`].
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │  ClaudeInProcessAdapter                                      │
//! │  ┌────────────────────────────────────────────────────────┐  │
//! │  │ AgentAdapter impl                                      │  │
//! │  │  start()  →  create ProviderClient, set Ready         │  │
//! │  │  send_message()  →  spawn QueryEngine with full config│  │
//! │  │  cancel()  →  CancellationToken                       │  │
//! │  │  resolve_permission()  →  oneshot tx                  │  │
//! │  └────────────────────────────────────────────────────────┘  │
//! │  ┌─────────────────┐  ┌──────────────────────────────────┐   │
//! │  │ AdapterPermission│  │ AdapterQueryObserver             │   │
//! │  │ Broker           │  │ maps observer events             │   │
//! │  │ decide() → event│  │ → UnifiedAgentEvent              │   │
//! │  └─────────────────┘  │ + SessionStore persistence       │   │
//! │                        └──────────────────────────────────┘   │
//! │  ┌────────────────────────────────────────────────────────┐  │
//! │  │ AdapterToolRunner                                      │  │
//! │  │ full ToolExecutionContext (sub_agent, progress_cb,     │  │
//! │  │ active_worktree_session) + SessionStore persistence    │  │
//! │  └────────────────────────────────────────────────────────┘  │
//! └──────────────────────────────────────────────────────────────┘
//! ```

mod event_mapper;

use std::collections::HashMap;
use std::sync::Arc;

use futures::FutureExt;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Mutex as StdMutex;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use claude_config::RuntimeConfig;
use claude_core::{ConversationEntry, Message, SessionId, SubAgentCompletion, ToolResult};
use claude_permissions::{
    PermissionBroker, PermissionDecision as PermissionsPermissionDecision,
    PermissionRequest as PermissionsPermissionRequest,
};
use claude_provider::ProviderCompatBackend;
use claude_provider::context::ContextWindowManager;
use claude_query_engine::{
    ProcessUserInputContext, ProviderInvocationMode, QueryEngine, QueryEngineConfig, QueryObserver,
    QueryObserverEvent, ToolRunResult, ToolRunner,
};
use claude_session::SessionStore;
use claude_session::conversation::ensure_conversation_initialized;
use claude_session::runtime_context::persist_runtime_config_session_context;
use claude_tools::{
    FileStateCache, agent::parse_delegate_progress_event, execute_tool_call,
    git::apply_worktree_tool_result_to_runtime, runtime_provider_tool_spec,
};
use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::error::AdapterError;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::permission::PermissionDecision as ProtocolPermissionDecision;
use rc_agent_protocol::types::{AgentConfig, AgentInfo, AgentStatus, AgentType};
use rc_engine_events::EventStream;

/// Maximum time (in seconds) to wait for a permission decision before denying.
// Kept short (30 s) because the Drop impl uses the sync drain to deny
// pending permissions. A shorter timeout ensures stale permissions are
// denied promptly on cleanup.
const PERMISSION_TIMEOUT_SECS: u64 = 30;

// ─── PendingPermission ─────────────────────────────────────────────────────

/// A pending permission request waiting for user resolution.
struct PendingPermission {
    response_tx: oneshot::Sender<PermissionsPermissionDecision>,
}

// ─── AdapterPermissionBroker ────────────────────────────────────────────────

/// Permission broker that forwards requests through the event channel
/// and waits for resolution via [`ClaudeInProcessAdapter::resolve_permission`].
///
/// Uses `std::sync::Mutex` for `pending_permissions` so the broker can drain
/// pending requests synchronously from `Drop::drop` without async runtime
/// access. The lock hold time is negligible (only sends on oneshot channels).
struct AdapterPermissionBroker {
    event_tx: mpsc::Sender<UnifiedAgentEvent>,
    pending_permissions: Arc<StdMutex<HashMap<String, PendingPermission>>>,
    session_id: String,
}

impl AdapterPermissionBroker {
    fn new(
        event_tx: mpsc::Sender<UnifiedAgentEvent>,
        pending_permissions: Arc<StdMutex<HashMap<String, PendingPermission>>>,
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
    async fn decide(&self, request: PermissionsPermissionRequest) -> PermissionsPermissionDecision {
        let request_id = uuid::Uuid::new_v4().to_string();

        // Create oneshot channel for the response.
        let (response_tx, response_rx) = oneshot::channel();
        let pending = PendingPermission { response_tx };

        // Store the pending permission.
        {
            let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
                tracing::warn!("Permission mutex poisoned, recovering: {e}");
                e.into_inner()
            });
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
            // Clean up the pending entry.
            let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
                tracing::warn!("Permission mutex poisoned, recovering: {e}");
                e.into_inner()
            });
            map.remove(&request_id);
            return PermissionsPermissionDecision::deny("Failed to send permission request");
        }

        // Wait for the user's response with a timeout.
        match tokio::time::timeout(
            std::time::Duration::from_secs(PERMISSION_TIMEOUT_SECS),
            response_rx,
        )
        .await
        {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                warn!(%request_id, "permission response channel dropped");
                // Clean up the pending entry.
                let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
                    tracing::warn!("Permission mutex poisoned, recovering: {e}");
                    e.into_inner()
                });
                map.remove(&request_id);
                PermissionsPermissionDecision::deny("Permission request cancelled")
            }
            Err(_) => {
                warn!(%request_id, "permission request timed out after {PERMISSION_TIMEOUT_SECS}s");
                // Clean up the pending entry.
                let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
                    tracing::warn!("Permission mutex poisoned, recovering: {e}");
                    e.into_inner()
                });
                map.remove(&request_id);
                PermissionsPermissionDecision::deny("Permission request timed out")
            }
        }
    }
}

// ─── AdapterToolRunner ──────────────────────────────────────────────────────

/// Tool runner that wraps [`execute_tool_call`] with full [`ToolExecutionContext`]
/// parity to the GUI's [`GuiToolRunner`].
///
/// Key enhancements over the minimal version:
/// - `sub_agent` from `ProviderCompatBackend` enables the agent tool.
/// - `progress_cb` emits subtask events via the event channel.
/// - `active_worktree_session` from `RuntimeConfig` enables multi-workspace.
/// - Tool results are persisted to `SessionStore`.
/// - Worktree mutations are applied and persisted.
struct AdapterToolRunner {
    /// Runtime config (guarded by async mutex for worktree mutations).
    config: Arc<Mutex<RuntimeConfig>>,
    /// Session store for persisting tool results.
    store: Arc<SessionStore>,
    /// Permission broker for tool approval.
    broker: Arc<dyn PermissionBroker>,
    /// Sub-agent completion from the provider backend.
    sub_agent: Arc<dyn SubAgentCompletion>,
    /// Context window manager for output truncation.
    context_manager: ContextWindowManager,
    /// Event channel for emitting progress events.
    event_tx: mpsc::Sender<UnifiedAgentEvent>,
    /// Session ID for persistence.
    session_id: Uuid,
}

impl AdapterToolRunner {
    fn new(
        config: RuntimeConfig,
        store: Arc<SessionStore>,
        broker: Arc<dyn PermissionBroker>,
        sub_agent: Arc<dyn SubAgentCompletion>,
        context_manager: ContextWindowManager,
        event_tx: mpsc::Sender<UnifiedAgentEvent>,
        session_id: Uuid,
    ) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            store,
            broker,
            sub_agent,
            context_manager,
            event_tx,
            session_id,
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
        debug!(tool = %tool_call.name, "Executing tool");
        // 1. Validate tool spec.
        let _spec = runtime_provider_tool_spec(&tool_call.name)
            .await
            .ok_or_else(|| anyhow!("unknown tool {}", tool_call.name))?;

        // 2. Build ToolExecutionContext with full parity to GuiToolRunner.
        let event_tx = self.event_tx.clone();
        let session_id_str = self.session_id.to_string();

        let tool_context = {
            let config = self.config.lock().await;
            claude_tools::ToolExecutionContext {
                cwd: config.cwd.clone(),
                original_cwd: config.original_cwd.clone(),
                active_worktree_session: config.active_worktree_session.clone(),
                timeout_ms: config.provider.timeout_ms,
                sub_agent: Some(self.sub_agent.clone()),
                progress_cb: Some(Arc::new(move |message: &str| {
                    emit_delegate_progress(&event_tx, &session_id_str, message);
                })),
                task_stack: Arc::new(parking_lot::Mutex::new(
                    claude_core::task_stack::TaskStack::default(),
                )),
                read_file_state: FileStateCache::new(),
                sub_agent_output_tokens: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            }
        };

        // 3. Execute tool.
        let tool_result =
            match execute_tool_call(tool_call, &tool_context, self.broker.as_ref()).await {
                Ok(result) => result,
                Err(error) => ToolResult {
                    content: format!("Tool execution error: {error}"),
                    is_error: true,
                    content_blocks: Vec::new(),
                    follow_up_user_blocks: Vec::new(),
                },
            };

        // 4. Handle worktree updates (mutates RuntimeConfig if worktree changed).
        {
            let mut config = self.config.lock().await;
            let mut temp_context = claude_tools::ToolExecutionContext {
                cwd: config.cwd.clone(),
                original_cwd: config.original_cwd.clone(),
                active_worktree_session: config.active_worktree_session.clone(),
                timeout_ms: config.provider.timeout_ms,
                sub_agent: Some(self.sub_agent.clone()),
                progress_cb: None,
                task_stack: Arc::new(parking_lot::Mutex::new(
                    claude_core::task_stack::TaskStack::default(),
                )),
                read_file_state: FileStateCache::new(),
                sub_agent_output_tokens: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            };

            if apply_worktree_tool_result_to_runtime(
                &tool_call.name,
                &tool_call.input,
                &tool_result,
                &mut config,
                &mut temp_context,
            )? {
                persist_runtime_config_session_context(self.store.as_ref(), &config)?;
            }
        }

        // 5. Handle follow-up user blocks as post_messages.
        let mut post_messages = Vec::new();
        if !tool_result.follow_up_user_blocks.is_empty() {
            let follow_up_entry = ConversationEntry::user_with_content_blocks(
                tool_result.follow_up_user_blocks.clone(),
            );
            post_messages.push(Message::from(follow_up_entry));
        }

        // 6. Persist tool result to session store.
        let output_for_context = self
            .context_manager
            .truncate_tool_output_default(&tool_result.content);
        let mut tool_entry = ConversationEntry::tool(
            tool_call.id.clone(),
            tool_call.name.clone(),
            output_for_context,
            tool_result.is_error,
        );
        tool_entry.content_blocks = tool_result.content_blocks.clone();
        self.store
            .append_conversation_entry(self.session_id, &tool_entry)?;

        self.store.append_named_event(
            self.session_id,
            "tool_result",
            json!({
                "tool_name": tool_call.name,
                "tool_use_id": tool_call.id,
                "is_error": tool_entry.is_error,
            }),
        )?;

        Ok(ToolRunResult {
            result: tool_result,
            pre_messages: Vec::new(),
            post_messages,
            permission_denial: None,
            output_tokens_consumed: None,
        })
    }
}

/// Emit delegate progress events as [`UnifiedAgentEvent`]s through the event channel.
///
/// Parses structured [`DelegateProgressEvent`] messages from the agent tool
/// and maps them to the appropriate subtask events.
fn emit_delegate_progress(
    event_tx: &mpsc::Sender<UnifiedAgentEvent>,
    session_id: &str,
    message: &str,
) {
    let Some(event) = parse_delegate_progress_event(message) else {
        // Unstructured progress — emit as tool progress.
        if let Err(e) = event_tx.try_send(UnifiedAgentEvent::ToolCallProgress {
            session_id: session_id.to_owned(),
            tool_name: "agent".to_owned(),
            progress: message.to_owned(),
        }) {
            tracing::debug!("event channel full, dropping event: {e}");
        }
        return;
    };

    match event {
        claude_tools::agent::DelegateProgressEvent::SubtaskStarted {
            task_id,
            description,
            ..
        } => {
            if let Err(e) = event_tx.try_send(UnifiedAgentEvent::SubtaskStarted {
                session_id: session_id.to_owned(),
                task_id,
                description,
            }) {
                tracing::debug!("event channel full, dropping event: {e}");
            }
        }
        claude_tools::agent::DelegateProgressEvent::SubtaskProgress {
            task_id, summary, ..
        } => {
            if let Err(e) = event_tx.try_send(UnifiedAgentEvent::SubtaskProgress {
                session_id: session_id.to_owned(),
                task_id,
                progress: summary,
            }) {
                tracing::debug!("event channel full, dropping event: {e}");
            }
        }
        claude_tools::agent::DelegateProgressEvent::SubtaskCompleted {
            task_id,
            success,
            output_preview,
            ..
        } => {
            if let Err(e) = event_tx.try_send(UnifiedAgentEvent::SubtaskCompleted {
                session_id: session_id.to_owned(),
                task_id,
                result: json!({
                    "success": success,
                    "output_preview": output_preview,
                }),
            }) {
                tracing::debug!("event channel full, dropping event: {e}");
            }
        }
        claude_tools::agent::DelegateProgressEvent::BatchProgress { .. } => {
            // BatchProgress doesn't have a direct UnifiedAgentEvent mapping.
        }
    }
}

// ─── AdapterQueryObserver ───────────────────────────────────────────────────

/// Query observer that maps [`QueryObserverEvent`] → [`UnifiedAgentEvent`]
/// and persists session state via [`SessionStore`].
///
/// Parity with the GUI's [`GuiQueryObserver`]:
/// - Persists assistant messages, context compaction, and lifecycle events.
/// - Maps all events to [`UnifiedAgentEvent`] for the adapter protocol.
struct AdapterQueryObserver {
    event_tx: mpsc::Sender<UnifiedAgentEvent>,
    session_id: String,
    store: Arc<SessionStore>,
    session_uuid: Uuid,
    /// Cached context window size from the last ContextBudgetEvaluated event.
    /// Used to populate the `total` field in StreamingUsageUpdated mappings.
    cached_max_tokens: std::sync::atomic::AtomicUsize,
}

impl AdapterQueryObserver {
    fn new(
        event_tx: mpsc::Sender<UnifiedAgentEvent>,
        session_id: String,
        store: Arc<SessionStore>,
        session_uuid: Uuid,
    ) -> Self {
        Self {
            event_tx,
            session_id,
            store,
            session_uuid,
            cached_max_tokens: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl QueryObserver for AdapterQueryObserver {
    async fn on_event(&self, event: QueryObserverEvent) -> Result<()> {
        debug!("Observer event: {:?}", std::mem::discriminant(&event));
        // 1. Persist to SessionStore (matching GuiQueryObserver behavior).
        match &event {
            QueryObserverEvent::AssistantMessageCommitted {
                message,
                stop_reason,
                turn,
                usage,
                ..
            } => {
                if let Some(entry) = message.as_conversation_entry() {
                    self.store
                        .append_conversation_entry(self.session_uuid, &entry)?;
                }
                self.store.append_named_event(
                    self.session_uuid,
                    "assistant_turn",
                    json!({
                        "turn": turn,
                        "stop_reason": stop_reason,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                    }),
                )?;
            }

            QueryObserverEvent::ContextCompactionApplied {
                turn,
                before_messages,
                after_messages,
                usage_ratio_before,
                usage_ratio_after,
                estimated_tokens_before,
                estimated_tokens_after,
                ..
            } => {
                let removed = before_messages.saturating_sub(*after_messages);
                if removed > 0 {
                    self.store.append_named_event(
                        self.session_uuid,
                        "context_compacted",
                        json!({
                            "turn": turn,
                            "entries_removed": removed,
                            "usage_ratio_before": usage_ratio_before,
                            "usage_ratio_after": usage_ratio_after,
                            "estimated_tokens_before": estimated_tokens_before,
                            "estimated_tokens_after": estimated_tokens_after,
                        }),
                    )?;
                }
            }

            QueryObserverEvent::QueryFinished {
                stop_reason,
                turns,
                usage,
                ..
            } => {
                self.store.append_named_event(
                    self.session_uuid,
                    "result",
                    json!({
                        "is_error": false,
                        "stop_reason": stop_reason,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                        "num_turns": turns,
                    }),
                )?;
            }

            QueryObserverEvent::QueryFailed { error, turns, .. } => {
                self.store.append_named_event(
                    self.session_uuid,
                    "result",
                    json!({
                        "is_error": true,
                        "error": error,
                        "num_turns": turns,
                    }),
                )?;
            }

            _ => {}
        }

        // 2. Map to UnifiedAgentEvent and forward through the event channel.
        let mut max_tokens = self
            .cached_max_tokens
            .load(std::sync::atomic::Ordering::Relaxed);
        if let Some(unified) =
            event_mapper::map_observer_event(event, &self.session_id, &mut max_tokens)
        {
            self.cached_max_tokens
                .store(max_tokens, std::sync::atomic::Ordering::Relaxed);
            // Use a tolerant send — if the receiver is dropped (e.g. consumer
            // disconnected), we silently drop the event rather than erroring.
            let _ = self.event_tx.send(unified).await;
        }

        Ok(())
    }
}

// ─── ClaudeInProcessAdapter ─────────────────────────────────────────────────

/// In-process adapter that wraps the Claude [`QueryEngine`] into the
/// [`AgentAdapter`] trait with full parity to the GUI direct path.
///
/// Uses [`RuntimeConfig`] and [`SessionStore`] for complete configuration
/// and persistence, matching [`GuiToolRunner`] and [`GuiQueryObserver`].
pub struct ClaudeInProcessAdapter {
    info: AgentInfo,
    status: AgentStatus,

    // Full runtime configuration (from GUI).
    runtime_config: RuntimeConfig,
    session_store: Arc<SessionStore>,

    // Reusable provider client (created once in start()).
    provider_client: Option<Arc<claude_provider::ProviderClient>>,

    // Runtime state
    cancel_token: CancellationToken,
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    pending_permissions: Arc<StdMutex<HashMap<String, PendingPermission>>>,

    // Session ID (from RuntimeConfig, cached for convenience).
    session_id: Uuid,
}

impl ClaudeInProcessAdapter {
    /// Create a new adapter with full runtime configuration.
    ///
    /// The adapter uses [`RuntimeConfig`] for all settings (model, provider,
    /// working directory, permissions, etc.) and [`SessionStore`] for
    /// persisting conversation history and events.
    pub fn new(runtime_config: RuntimeConfig, session_store: Arc<SessionStore>) -> Self {
        let caps = rc_agent_protocol::util::standard_capabilities(&[]);

        let session_id = runtime_config.session_id;

        let info = AgentInfo {
            name: "Remote Claude".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities: caps,
            status: AgentStatus::Starting,
        };

        Self {
            info,
            status: AgentStatus::Starting,
            runtime_config,
            session_store,
            provider_client: None,
            cancel_token: CancellationToken::new(),
            worker_handle: None,
            pending_permissions: Arc::new(StdMutex::new(HashMap::new())),
            session_id,
        }
    }

    /// Abort the current worker (if any) and reset the cancel token.
    fn abort_worker(&mut self) {
        if let Some(handle) = self.worker_handle.take() {
            // Synchronously drain pending permissions before aborting the worker.
            self.drain_pending_permissions_sync();
            warn!("Aborting previous worker; pending permissions drained synchronously");
            handle.abort();
        }
        // Recreate the cancel token so future calls get a fresh token.
        self.cancel_token = CancellationToken::new();
    }

    /// Synchronously drain and deny all pending permissions.
    ///
    /// Uses `std::sync::Mutex` so this can be called from `Drop::drop` and
    /// other synchronous contexts without needing a tokio runtime handle.
    fn drain_pending_permissions_sync(&self) {
        let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
            tracing::warn!("Permission mutex poisoned, recovering: {e}");
            e.into_inner()
        });
        for (id, pending) in map.drain() {
            let _ = pending
                .response_tx
                .send(PermissionsPermissionDecision::deny("Adapter shutting down"));
            debug!(%id, "drained pending permission on shutdown");
        }
    }

    /// Clean up all pending permissions by denying them (async wrapper).
    async fn drain_pending_permissions(&self) {
        self.drain_pending_permissions_sync();
    }
}

#[async_trait]
impl AgentAdapter for ClaudeInProcessAdapter {
    async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
        info!("ClaudeInProcessAdapter starting");

        // Create the reusable provider client.
        let client = claude_provider::ProviderClient::new()
            .map_err(|e| anyhow!("failed to create provider client: {e}"))?;
        self.provider_client = Some(Arc::new(client));

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

        // Abort any previous worker to prevent zombie tasks.
        self.abort_worker();

        let (event_tx, event_rx) = mpsc::channel(64);

        // 1. Get or create provider client.
        let provider_client = self
            .provider_client
            .clone()
            .ok_or_else(|| AdapterError::NotStarted)?;

        // 2. Build provider config and create backend.
        let provider_config = self.runtime_config.provider.clone();
        let model = provider_config
            .model
            .clone()
            .unwrap_or_else(|| "unknown".to_owned());

        let backend: Arc<dyn claude_provider::ConversationBackend> = Arc::new(
            ProviderCompatBackend::new(provider_client, &provider_config),
        );

        // 3. Get sub_agent from backend (enables the agent/delegate tool).
        let sub_agent = backend.sub_agent_completion();

        // 4. Initialize session conversation from SessionStore.
        let conversation = ensure_conversation_initialized(
            &self.session_store,
            self.session_id,
            &self.runtime_config.cwd,
            &self.runtime_config.provider.name,
            self.runtime_config.provider.model.as_deref(),
            Some(message),
        )?;
        let existing_messages: Vec<Message> = conversation.into_iter().map(Message::from).collect();

        // Persist user entry.
        let user_entry = ConversationEntry::user(message);
        self.session_store
            .append_conversation_entry(self.session_id, &user_entry)?;

        // 5. Create permission broker.
        let broker = Arc::new(AdapterPermissionBroker::new(
            event_tx.clone(),
            self.pending_permissions.clone(),
            session_id.to_owned(),
        ));

        // 6. Create context manager for tool output truncation.
        let context_manager = ContextWindowManager::for_model(&model);

        // 7. Create tool runner with full config (sub_agent, progress_cb, worktree).
        let tool_runner = Arc::new(AdapterToolRunner::new(
            self.runtime_config.clone(),
            self.session_store.clone(),
            broker.clone(),
            sub_agent,
            context_manager.clone(),
            event_tx.clone(),
            self.session_id,
        ));

        // 8. Create query observer with SessionStore persistence.
        let observer = Arc::new(AdapterQueryObserver::new(
            event_tx.clone(),
            session_id.to_owned(),
            self.session_store.clone(),
            self.session_id,
        ));

        // 9. Build QueryEngineConfig.
        let claude_session_id = SessionId::from(session_id.to_owned());
        let event_stream = EventStream::new(64);
        let max_turns = self.runtime_config.max_turns.max(1) as u32;

        let mut query_config = QueryEngineConfig::new(
            claude_session_id.clone(),
            &model,
            backend,
            tool_runner,
            event_stream,
        )
        .with_observer(observer)
        .with_provider_invocation_mode(ProviderInvocationMode::Streaming);

        query_config.max_turns = max_turns;

        // 10. Create the query engine.
        let mut engine = QueryEngine::new(query_config, existing_messages);

        // 11. Build the process-user-input context with fields from RuntimeConfig.
        let mut context = ProcessUserInputContext::new(
            claude_session_id,
            self.runtime_config.permission_mode,
            &model,
        );

        // Populate optional fields from RuntimeConfig.
        context.system_prompt = self.runtime_config.system_prompt.clone();
        context.requested_effort = self.runtime_config.effort.clone();

        // 12. Create user message.
        let user_message = vec![Message::from(ConversationEntry::user(message))];

        // 13. Send Started event.
        let _ = event_tx
            .send(UnifiedAgentEvent::Started(self.info.clone()))
            .await;

        // 14. Spawn the query engine task.
        let cancel_token = self.cancel_token.clone();
        let event_tx_for_completion = event_tx.clone();
        let session_id_for_completion = session_id.to_owned();
        let session_id_for_panic = session_id.to_owned();

        let handle = tokio::spawn(async move {
            // Check cancellation before starting.
            if cancel_token.is_cancelled() {
                debug!("Query cancelled before starting");
                let _ = event_tx_for_completion
                    .send(UnifiedAgentEvent::Stopped)
                    .await;
                return;
            }

            // Run the query engine to completion, catching panics.
            let fut = async {
                let result = engine.submit_message(user_message, context).await;
                match result {
                    Ok(query_result) => {
                        debug!(
                            stops = %query_result.stop_reason,
                            turns = query_result.turns,
                            "Query engine completed"
                        );
                    }
                    Err(e) => {
                        warn!("Query engine error: {e}");
                        let _ = event_tx_for_completion
                            .send(UnifiedAgentEvent::Error {
                                session_id: session_id_for_completion,
                                message: format!("{e:#}"),
                                recoverable: false,
                            })
                            .await;
                    }
                }
            };

            // Wrap with AssertUnwindSafe so we can catch panics inside the async block.
            let result = std::panic::AssertUnwindSafe(fut).catch_unwind().await;
            if let Err(panic_payload) = result {
                let event = rc_agent_protocol::util::panic_to_error_event(
                    &session_id_for_panic,
                    "Query engine task panicked",
                    panic_payload,
                );
                tracing::error!(
                    "{}",
                    match &event {
                        UnifiedAgentEvent::Error { message, .. } => message.clone(),
                        _ => unreachable!(),
                    }
                );
                let _ = event_tx_for_completion.send(event).await;
            }
        });

        self.worker_handle = Some(handle);
        self.status = AgentStatus::Busy;
        self.info.status = AgentStatus::Busy;

        Ok(event_rx)
    }

    async fn cancel(&mut self, _session_id: &str) -> Result<()> {
        info!("ClaudeInProcessAdapter: cancel");
        self.abort_worker();

        // Deny all pending permissions to unblock any waiting oneshot channels.
        self.drain_pending_permissions().await;

        // After cancel, the adapter can still accept new messages.
        self.status = AgentStatus::Ready;
        self.info.status = AgentStatus::Ready;
        Ok(())
    }

    async fn resolve_permission(
        &mut self,
        _session_id: &str,
        request_id: &str,
        decision: ProtocolPermissionDecision,
    ) -> Result<()> {
        debug!(%request_id, "Resolving permission request");

        let mut map = self.pending_permissions.lock().unwrap_or_else(|e| {
            tracing::warn!("Permission mutex poisoned, recovering: {e}");
            e.into_inner()
        });
        if let Some(pending) = map.remove(request_id) {
            let permissions_decision = match decision {
                ProtocolPermissionDecision::Allow => PermissionsPermissionDecision::allow(),
                ProtocolPermissionDecision::Deny => {
                    PermissionsPermissionDecision::deny("Permission denied by user")
                }
                ProtocolPermissionDecision::AllowAll => {
                    // AllowAll is treated as allow for the current request.
                    // TODO: Consider adding a session rule for future auto-approval
                    // via broker.add_session_rule().
                    PermissionsPermissionDecision::allow()
                }
                // #[non_exhaustive] on PermissionDecision requires a wildcard arm.
                // Any future variants are treated as deny-by-default.
                _ => PermissionsPermissionDecision::deny("Unsupported permission decision variant"),
            };

            let _ = pending.response_tx.send(permissions_decision);
            Ok(())
        } else {
            Err(anyhow!(
                "no pending permission request with id {request_id}"
            ))
        }
    }

    async fn stop(&mut self) -> Result<()> {
        info!("ClaudeInProcessAdapter: stop");
        self.abort_worker();

        // Deny all pending permissions to unblock any waiting oneshot channels.
        self.drain_pending_permissions().await;

        self.provider_client = None;
        self.status = AgentStatus::Stopped;
        self.info.status = AgentStatus::Stopped;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        // Consistent with rc-roo-adapter: alive unless explicitly stopped.
        !matches!(self.status, AgentStatus::Stopped)
    }

    fn info(&self) -> &AgentInfo {
        &self.info
    }

    fn agent_type(&self) -> AgentType {
        AgentType::RemoteClaude
    }
}

impl Drop for ClaudeInProcessAdapter {
    fn drop(&mut self) {
        // Abort any running worker task.
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }
        // Synchronously drain pending permissions so waiting oneshot channels
        // are unblocked immediately instead of relying on timeout expiry.
        self.drain_pending_permissions_sync();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    use tokio::sync::{mpsc, oneshot};

    use claude_permissions::PermissionDecision;
    use rc_agent_protocol::events::UnifiedAgentEvent;

    use super::{PendingPermission, emit_delegate_progress};

    fn collect_events(
        _tx: &mpsc::Sender<UnifiedAgentEvent>,
        rx: &mut mpsc::Receiver<UnifiedAgentEvent>,
    ) -> Vec<UnifiedAgentEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[test]
    fn emit_delegate_progress_unstructured_message_emits_tool_progress() {
        let (tx, mut rx) = mpsc::channel(64);
        emit_delegate_progress(&tx, "session-1", "doing some work");

        let events = collect_events(&tx, &mut rx);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            UnifiedAgentEvent::ToolCallProgress { session_id, tool_name, progress }
                if session_id == "session-1"
                    && tool_name == "agent"
                    && progress == "doing some work"
        ));
    }

    #[test]
    fn emit_delegate_progress_subtask_started_event() {
        let (tx, mut rx) = mpsc::channel(64);
        let message =
            r#"{"kind":"subtask_started","task_id":"t-1","description":"refactor auth","depth":0}"#;
        emit_delegate_progress(&tx, "session-1", message);

        let events = collect_events(&tx, &mut rx);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            UnifiedAgentEvent::SubtaskStarted { session_id, task_id, description }
                if session_id == "session-1"
                    && task_id == "t-1"
                    && description == "refactor auth"
        ));
    }

    #[test]
    fn emit_delegate_progress_subtask_progress_event() {
        let (tx, mut rx) = mpsc::channel(64);
        let message = r#"{"kind":"subtask_progress","task_id":"t-2","turn":1,"max_turns":10,"summary":"50% done"}"#;
        emit_delegate_progress(&tx, "session-1", message);

        let events = collect_events(&tx, &mut rx);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            UnifiedAgentEvent::SubtaskProgress { session_id, task_id, progress }
                if session_id == "session-1"
                    && task_id == "t-2"
                    && progress == "50% done"
        ));
    }

    #[test]
    fn emit_delegate_progress_subtask_completed_event() {
        let (tx, mut rx) = mpsc::channel(64);
        let message = r#"{"kind":"subtask_completed","task_id":"t-3","success":true,"output_preview":"ok","turns_used":1}"#;
        emit_delegate_progress(&tx, "session-1", message);

        let events = collect_events(&tx, &mut rx);
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedAgentEvent::SubtaskCompleted {
                session_id,
                task_id,
                result,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(task_id, "t-3");
                assert_eq!(result["success"], true);
                assert_eq!(result["output_preview"], "ok");
            }
            other => panic!("expected SubtaskCompleted, got {other:?}"),
        }
    }

    #[test]
    fn emit_delegate_progress_batch_progress_is_silently_ignored() {
        let (tx, mut rx) = mpsc::channel(64);
        let message = r#"{"kind":"batch_progress","total":5,"completed":2,"running":1}"#;
        emit_delegate_progress(&tx, "session-1", message);

        let events = collect_events(&tx, &mut rx);
        assert!(
            events.is_empty(),
            "batch_progress should not emit a UnifiedAgentEvent"
        );
    }

    #[test]
    fn drain_pending_permissions_sends_deny_to_all_waiters() {
        let pending_permissions: Arc<StdMutex<HashMap<String, PendingPermission>>> =
            Arc::new(StdMutex::new(HashMap::new()));

        let (response_tx1, response_rx1) = oneshot::channel();
        let (response_tx2, response_rx2) = oneshot::channel();

        {
            let mut map = pending_permissions.lock().unwrap();
            map.insert(
                "req-1".into(),
                PendingPermission {
                    response_tx: response_tx1,
                },
            );
            map.insert(
                "req-2".into(),
                PendingPermission {
                    response_tx: response_tx2,
                },
            );
        }

        let mut map = pending_permissions.lock().unwrap();
        for (_id, pending) in map.drain() {
            let _ = pending
                .response_tx
                .send(PermissionDecision::deny("Adapter shutting down"));
        }

        let decision1 = response_rx1
            .blocking_recv()
            .expect("should receive decision");
        assert!(!decision1.allowed);

        let decision2 = response_rx2
            .blocking_recv()
            .expect("should receive decision");
        assert!(!decision2.allowed);
    }

    #[test]
    fn drain_on_empty_map_is_noop() {
        let pending_permissions: Arc<StdMutex<HashMap<String, PendingPermission>>> =
            Arc::new(StdMutex::new(HashMap::new()));

        let mut map = pending_permissions.lock().unwrap();
        let original_len = map.len();
        for (_id, pending) in map.drain() {
            let _ = pending
                .response_tx
                .send(PermissionDecision::deny("shutdown"));
        }
        assert_eq!(original_len, 0);
        assert!(map.is_empty());
    }
}
