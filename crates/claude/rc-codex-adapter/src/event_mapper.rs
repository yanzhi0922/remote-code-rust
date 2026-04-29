//! Event mapping from Codex `AppServerEvent` → `UnifiedAgentEvent`.
//!
//! The Codex app-server protocol uses a rich notification system with many
//! specialized event types. This module translates the subset relevant to
//! the unified agent protocol into [`UnifiedAgentEvent`] variants.

use rc_agent_protocol::events::{AgentResult, UnifiedAgentEvent, UsageInfo};

use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::{ServerNotification, ServerRequest, ThreadItem};

/// Map a Codex [`AppServerEvent`] into zero or more [`UnifiedAgentEvent`]s.
///
/// Most Codex events map 1:1 to a unified event, but some (like `Lagged`)
/// are silently consumed, and some may produce multiple unified events in
/// the future.
pub fn map_app_server_event(
    event: AppServerEvent,
    session_id: &str,
) -> Vec<UnifiedAgentEvent> {
    match event {
        AppServerEvent::Lagged { skipped: _ } => {
            // Backpressure signal — silently consumed.
            Vec::new()
        }

        AppServerEvent::ServerNotification(notification) => {
            map_server_notification(notification, session_id)
        }

        AppServerEvent::ServerRequest(request) => {
            map_server_request(request, session_id)
        }

        AppServerEvent::Disconnected { message } => {
            vec![UnifiedAgentEvent::Error {
                session_id: session_id.to_owned(),
                message: format!("Codex server disconnected: {message}"),
                recoverable: false,
            }]
        }
    }
}

/// Derive a human-readable tool name from a [`ThreadItem`] variant.
fn thread_item_kind(item: &ThreadItem) -> &'static str {
    match item {
        ThreadItem::UserMessage { .. } => "user_message",
        ThreadItem::HookPrompt { .. } => "hook_prompt",
        ThreadItem::AgentMessage { .. } => "agent_message",
        ThreadItem::Plan { .. } => "plan",
        ThreadItem::Reasoning { .. } => "reasoning",
        ThreadItem::CommandExecution { .. } => "command_execution",
        ThreadItem::FileChange { .. } => "file_change",
        ThreadItem::McpToolCall { .. } => "mcp_tool_call",
        ThreadItem::DynamicToolCall { .. } => "dynamic_tool_call",
        ThreadItem::CollabAgentToolCall { .. } => "collab_agent_tool_call",
        ThreadItem::WebSearch { .. } => "web_search",
        ThreadItem::ImageView { .. } => "image_view",
        ThreadItem::ImageGeneration { .. } => "image_generation",
        ThreadItem::EnteredReviewMode { .. } => "entered_review_mode",
        ThreadItem::ExitedReviewMode { .. } => "exited_review_mode",
        ThreadItem::ContextCompaction { .. } => "context_compaction",
    }
}

/// Map a Codex [`ServerNotification`] into unified events.
fn map_server_notification(
    notification: ServerNotification,
    session_id: &str,
) -> Vec<UnifiedAgentEvent> {
    match notification {
        // ── Streaming text ──
        ServerNotification::AgentMessageDelta(delta) => {
            vec![UnifiedAgentEvent::MessageDelta {
                session_id: session_id.to_owned(),
                delta: delta.delta,
            }]
        }

        // ── Tool / item lifecycle ──
        ServerNotification::ItemStarted(item) => {
            let tool_name = thread_item_kind(&item.item).to_owned();
            let tool_input =
                serde_json::to_value(&item.item).unwrap_or(serde_json::Value::Null);
            vec![UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_owned(),
                tool_name,
                tool_input,
            }]
        }

        ServerNotification::ItemCompleted(item) => {
            let tool_name = thread_item_kind(&item.item).to_owned();
            let result =
                serde_json::to_value(&item.item).unwrap_or(serde_json::Value::Null);
            vec![UnifiedAgentEvent::ToolCallCompleted {
                session_id: session_id.to_owned(),
                tool_name,
                result,
            }]
        }

        // ── Command output streaming ──
        ServerNotification::CommandExecutionOutputDelta(delta) => {
            vec![UnifiedAgentEvent::ToolCallProgress {
                session_id: session_id.to_owned(),
                tool_name: "command_execution".to_owned(),
                progress: delta.delta,
            }]
        }

        ServerNotification::CommandExecOutputDelta(delta) => {
            vec![UnifiedAgentEvent::ToolCallProgress {
                session_id: session_id.to_owned(),
                tool_name: "command_exec".to_owned(),
                progress: delta.delta_base64,
            }]
        }

        // ── File change output ──
        ServerNotification::FileChangeOutputDelta(delta) => {
            vec![UnifiedAgentEvent::ToolCallProgress {
                session_id: session_id.to_owned(),
                tool_name: "file_change".to_owned(),
                progress: delta.delta,
            }]
        }

        // ── Turn lifecycle ──
        ServerNotification::TurnStarted(_notification) => {
            // Internal lifecycle event — not surfaced to the unified protocol.
            Vec::new()
        }

        ServerNotification::TurnCompleted(notification) => {
            let response_text = notification
                .turn
                .items
                .iter()
                .filter_map(|item| match item {
                    ThreadItem::AgentMessage { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");

            vec![UnifiedAgentEvent::Completed {
                session_id: session_id.to_owned(),
                result: AgentResult {
                    response_text,
                    tool_calls: Vec::new(),
                    usage: UsageInfo::default(),
                    cost: None,
                },
            }]
        }

        // ── Context management ──
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            let used = notification.token_usage.total.input_tokens as usize;
            let total = notification
                .token_usage
                .model_context_window
                .map(|t| t as usize)
                .unwrap_or(0);
            vec![UnifiedAgentEvent::ContextUsage {
                session_id: session_id.to_owned(),
                used,
                total,
            }]
        }

        ServerNotification::ContextCompacted(_notification) => {
            vec![UnifiedAgentEvent::ContextCompacted {
                session_id: session_id.to_owned(),
                entries_removed: 0,
                usage_ratio: 0.0,
            }]
        }

        // ── Errors ──
        ServerNotification::Error(notification) => {
            vec![UnifiedAgentEvent::Error {
                session_id: session_id.to_owned(),
                message: notification.error.message,
                recoverable: notification.will_retry,
            }]
        }

        // ── All other notifications: silently consumed ──
        other => {
            tracing::debug!(
                notification = ?other,
                "Codex notification not mapped to unified event"
            );
            Vec::new()
        }
    }
}

/// Map a Codex [`ServerRequest`] (permission request) into a unified event.
fn map_server_request(
    request: ServerRequest,
    session_id: &str,
) -> Vec<UnifiedAgentEvent> {
    let (tool_name, input) = match &request {
        ServerRequest::CommandExecutionRequestApproval { params, .. } => (
            "command_execution".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::FileChangeRequestApproval { params, .. } => (
            "file_change".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::ApplyPatchApproval { params, .. } => (
            "apply_patch".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::ExecCommandApproval { params, .. } => (
            "exec_command".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::PermissionsRequestApproval { params, .. } => (
            "permissions".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::ToolRequestUserInput { params, .. } => (
            "tool_user_input".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::McpServerElicitationRequest { params, .. } => (
            "mcp_elicitation".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::DynamicToolCall { params, .. } => (
            "dynamic_tool".to_owned(),
            serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        ),
        ServerRequest::ChatgptAuthTokensRefresh { .. } => (
            "chatgpt_auth_refresh".to_owned(),
            serde_json::Value::Null,
        ),
    };

    vec![UnifiedAgentEvent::PermissionRequest {
        session_id: session_id.to_owned(),
        request_id: request_id_to_string(request.id()),
        tool_name,
        input,
    }]
}

/// Convert a [`RequestId`] to a string for the unified protocol.
fn request_id_to_string(id: &codex_app_server_protocol::RequestId) -> String {
    match id {
        codex_app_server_protocol::RequestId::String(s) => s.clone(),
        codex_app_server_protocol::RequestId::Integer(n) => n.to_string(),
    }
}
