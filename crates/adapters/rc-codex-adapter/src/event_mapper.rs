//! Event mapping from Codex `AppServerEvent` → `UnifiedAgentEvent`.
//!
//! The Codex app-server protocol uses a rich notification system with many
//! specialized event types. This module translates the subset relevant to
//! the unified agent protocol into [`UnifiedAgentEvent`] variants.

use claude_agent_protocol::events::{AgentResult, ToolCallInfo, UnifiedAgentEvent, UsageInfo};

use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::{
    CommandExecutionStatus, DynamicToolCallStatus, McpToolCallStatus, PatchApplyStatus,
    ServerNotification, ServerRequest, ThreadItem,
};

fn progress_event(
    session_id: &str,
    tool_name: impl Into<String>,
    progress: impl Into<String>,
) -> Vec<UnifiedAgentEvent> {
    vec![UnifiedAgentEvent::ToolCallProgress {
        session_id: session_id.to_owned(),
        tool_name: tool_name.into(),
        progress: progress.into(),
    }]
}

fn json_progress_event(
    session_id: &str,
    tool_name: impl Into<String>,
    payload: impl serde::Serialize,
) -> Vec<UnifiedAgentEvent> {
    progress_event(
        session_id,
        tool_name,
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_owned()),
    )
}

fn official_event(
    session_id: &str,
    method: &'static str,
    payload: impl serde::Serialize,
) -> Vec<UnifiedAgentEvent> {
    vec![UnifiedAgentEvent::CodexAppServerNotification {
        session_id: session_id.to_owned(),
        method: method.to_owned(),
        params: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
    }]
}

fn raw_server_notification(
    session_id: &str,
    notification: &ServerNotification,
) -> UnifiedAgentEvent {
    UnifiedAgentEvent::CodexAppServerNotification {
        session_id: session_id.to_owned(),
        method: notification.to_string(),
        params: notification
            .clone()
            .to_params()
            .unwrap_or(serde_json::Value::Null),
    }
}

fn non_negative_i64_to_usize(value: i64) -> usize {
    usize::try_from(value).unwrap_or(0)
}

fn thread_item_tool_call(item: &ThreadItem) -> Option<ToolCallInfo> {
    match item {
        ThreadItem::CommandExecution {
            id,
            command,
            cwd,
            process_id,
            source,
            status,
            command_actions,
            aggregated_output,
            exit_code,
            duration_ms,
        } if *status != CommandExecutionStatus::InProgress => Some(ToolCallInfo {
            id: id.clone(),
            name: "command_execution".to_owned(),
            input: serde_json::json!({
                "command": command,
                "cwd": cwd,
                "processId": process_id,
                "source": source,
                "commandActions": command_actions,
            }),
            output: serde_json::json!({
                "status": status,
                "aggregatedOutput": aggregated_output,
                "exitCode": exit_code,
                "durationMs": duration_ms,
            }),
        }),
        ThreadItem::FileChange {
            id,
            changes,
            status,
        } if *status != PatchApplyStatus::InProgress => Some(ToolCallInfo {
            id: id.clone(),
            name: "file_change".to_owned(),
            input: serde_json::json!({
                "changes": changes,
            }),
            output: serde_json::json!({
                "status": status,
            }),
        }),
        ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            arguments,
            mcp_app_resource_uri,
            result,
            error,
            duration_ms,
        } if *status != McpToolCallStatus::InProgress => Some(ToolCallInfo {
            id: id.clone(),
            name: format!("mcp/{server}/{tool}"),
            input: serde_json::json!({
                "server": server,
                "tool": tool,
                "arguments": arguments,
                "mcpAppResourceUri": mcp_app_resource_uri,
            }),
            output: serde_json::json!({
                "status": status,
                "result": result,
                "error": error,
                "durationMs": duration_ms,
            }),
        }),
        ThreadItem::DynamicToolCall {
            id,
            namespace,
            tool,
            arguments,
            status,
            content_items,
            success,
            duration_ms,
        } if *status != DynamicToolCallStatus::InProgress => Some(ToolCallInfo {
            id: id.clone(),
            name: namespace
                .as_ref()
                .map(|namespace| format!("dynamic/{namespace}/{tool}"))
                .unwrap_or_else(|| format!("dynamic/{tool}")),
            input: serde_json::json!({
                "namespace": namespace,
                "tool": tool,
                "arguments": arguments,
            }),
            output: serde_json::json!({
                "status": status,
                "contentItems": content_items,
                "success": success,
                "durationMs": duration_ms,
            }),
        }),
        ThreadItem::CollabAgentToolCall {
            id,
            tool,
            status,
            sender_thread_id,
            receiver_thread_ids,
            prompt,
            model,
            reasoning_effort,
            agents_states,
        } => Some(ToolCallInfo {
            id: id.clone(),
            name: "collab_agent".to_owned(),
            input: serde_json::json!({
                "tool": tool,
                "senderThreadId": sender_thread_id,
                "receiverThreadIds": receiver_thread_ids,
                "prompt": prompt,
                "model": model,
                "reasoningEffort": reasoning_effort,
            }),
            output: serde_json::json!({
                "status": status,
                "agentsStates": agents_states,
            }),
        }),
        ThreadItem::WebSearch { id, query, action } => Some(ToolCallInfo {
            id: id.clone(),
            name: "web_search".to_owned(),
            input: serde_json::json!({ "query": query }),
            output: serde_json::json!({ "action": action }),
        }),
        ThreadItem::ImageView { id, path } => Some(ToolCallInfo {
            id: id.clone(),
            name: "image_view".to_owned(),
            input: serde_json::json!({ "path": path }),
            output: serde_json::json!({ "path": path }),
        }),
        ThreadItem::ImageGeneration {
            id,
            status,
            revised_prompt,
            result,
            saved_path,
        } => Some(ToolCallInfo {
            id: id.clone(),
            name: "image_generation".to_owned(),
            input: serde_json::json!({
                "revisedPrompt": revised_prompt,
            }),
            output: serde_json::json!({
                "status": status,
                "result": result,
                "savedPath": saved_path,
            }),
        }),
        _ => None,
    }
}

/// Map a Codex [`AppServerEvent`] into zero or more [`UnifiedAgentEvent`]s.
///
/// Most Codex events map 1:1 to a unified event, but some (like `Lagged`)
/// are silently consumed, and some may produce multiple unified events in
/// the future.
pub fn map_app_server_event(event: AppServerEvent, session_id: &str) -> Vec<UnifiedAgentEvent> {
    match event {
        AppServerEvent::Lagged { skipped: _ } => {
            // Backpressure signal — silently consumed.
            Vec::new()
        }

        AppServerEvent::ServerNotification(notification) => {
            let mut events = vec![raw_server_notification(session_id, &notification)];
            events.extend(map_server_notification(notification, session_id));
            events
        }

        AppServerEvent::ServerRequest(request) => map_server_request(request, session_id),

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

        ServerNotification::PlanDelta(delta) => progress_event(session_id, "plan", delta.delta),

        ServerNotification::ReasoningSummaryTextDelta(delta) => {
            progress_event(session_id, "reasoning_summary", delta.delta)
        }

        ServerNotification::ReasoningTextDelta(delta) => {
            progress_event(session_id, "reasoning", delta.delta)
        }

        // ── Tool / item lifecycle ──
        ServerNotification::ItemStarted(item) => {
            let tool_name = thread_item_kind(&item.item).to_owned();
            let tool_input = serde_json::to_value(&item.item).unwrap_or(serde_json::Value::Null);
            vec![UnifiedAgentEvent::ToolCallStarted {
                session_id: session_id.to_owned(),
                tool_name,
                tool_input,
            }]
        }

        ServerNotification::ItemCompleted(item) => {
            let tool_name = thread_item_kind(&item.item).to_owned();
            let result = serde_json::to_value(&item.item).unwrap_or(serde_json::Value::Null);
            vec![UnifiedAgentEvent::ToolCallCompleted {
                session_id: session_id.to_owned(),
                tool_name,
                result,
            }]
        }

        // ── Command output streaming ──
        ServerNotification::CommandExecutionOutputDelta(delta) => {
            progress_event(session_id, "command_execution", delta.delta)
        }

        ServerNotification::CommandExecOutputDelta(delta) => {
            json_progress_event(session_id, "command_exec", delta)
        }

        ServerNotification::TerminalInteraction(notification) => {
            json_progress_event(session_id, "terminal_interaction", notification)
        }

        // ── File change output ──
        ServerNotification::FileChangeOutputDelta(delta) => {
            progress_event(session_id, "file_change", delta.delta)
        }

        ServerNotification::FileChangePatchUpdated(notification) => {
            json_progress_event(session_id, "file_change_patch", notification)
        }

        ServerNotification::McpToolCallProgress(notification) => {
            progress_event(session_id, "mcp_tool_call", notification.message)
        }

        ServerNotification::ServerRequestResolved(notification) => {
            json_progress_event(session_id, "server_request_resolved", notification)
        }

        // ── Turn lifecycle ──
        ServerNotification::ThreadStarted(notification) => {
            json_progress_event(session_id, "thread_started", notification.thread)
        }

        ServerNotification::ThreadStatusChanged(notification) => {
            json_progress_event(session_id, "thread_status", notification)
        }

        ServerNotification::ThreadArchived(notification) => {
            json_progress_event(session_id, "thread_archived", notification)
        }

        ServerNotification::ThreadUnarchived(notification) => {
            json_progress_event(session_id, "thread_unarchived", notification)
        }

        ServerNotification::ThreadClosed(notification) => {
            json_progress_event(session_id, "thread_closed", notification)
        }

        ServerNotification::SkillsChanged(notification) => {
            official_event(session_id, "skills/changed", notification)
        }

        ServerNotification::ThreadNameUpdated(notification) => {
            json_progress_event(session_id, "thread_name_updated", notification)
        }

        ServerNotification::ThreadGoalUpdated(notification) => {
            official_event(session_id, "thread/goal/updated", notification)
        }

        ServerNotification::ThreadGoalCleared(notification) => {
            official_event(session_id, "thread/goal/cleared", notification)
        }

        ServerNotification::TurnStarted(notification) => {
            json_progress_event(session_id, "turn_started", notification)
        }

        ServerNotification::HookStarted(notification) => {
            official_event(session_id, "hook/started", notification)
        }

        ServerNotification::TurnPlanUpdated(notification) => {
            json_progress_event(session_id, "turn_plan", notification)
        }

        ServerNotification::TurnDiffUpdated(notification) => {
            progress_event(session_id, "turn_diff", notification.diff)
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
            let tool_calls = notification
                .turn
                .items
                .iter()
                .filter_map(thread_item_tool_call)
                .collect();

            vec![UnifiedAgentEvent::Completed {
                session_id: session_id.to_owned(),
                result: AgentResult {
                    response_text,
                    tool_calls,
                    usage: UsageInfo::default(),
                    cost: None,
                },
            }]
        }

        ServerNotification::HookCompleted(notification) => {
            official_event(session_id, "hook/completed", notification)
        }

        // ── Context management ──
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            let used = non_negative_i64_to_usize(notification.token_usage.total.total_tokens);
            let total = notification
                .token_usage
                .model_context_window
                .map(non_negative_i64_to_usize)
                .unwrap_or(0);
            let mut events = vec![UnifiedAgentEvent::ContextUsage {
                session_id: session_id.to_owned(),
                used,
                total,
            }];
            events.extend(json_progress_event(
                session_id,
                "codex_token_usage",
                notification,
            ));
            events
        }

        ServerNotification::ContextCompacted(_notification) => {
            vec![UnifiedAgentEvent::ContextCompacted {
                session_id: session_id.to_owned(),
                entries_removed: 0,
                usage_ratio: 0.0,
            }]
        }

        ServerNotification::McpServerStatusUpdated(notification) => {
            json_progress_event(session_id, "mcp_server_status", notification)
        }

        ServerNotification::McpServerOauthLoginCompleted(notification) => {
            json_progress_event(session_id, "mcp_oauth_login", notification)
        }

        ServerNotification::AccountUpdated(notification) => {
            official_event(session_id, "account/updated", notification)
        }

        ServerNotification::AccountRateLimitsUpdated(notification) => {
            official_event(session_id, "account/rateLimits/updated", notification)
        }

        ServerNotification::AppListUpdated(notification) => {
            official_event(session_id, "app/list/updated", notification)
        }

        ServerNotification::ExternalAgentConfigImportCompleted(notification) => official_event(
            session_id,
            "externalAgentConfig/import/completed",
            notification,
        ),

        ServerNotification::FsChanged(notification) => {
            official_event(session_id, "fs/changed", notification)
        }

        ServerNotification::ReasoningSummaryPartAdded(notification) => {
            official_event(session_id, "item/reasoning/summaryPartAdded", notification)
        }

        ServerNotification::RawResponseItemCompleted(notification) => {
            official_event(session_id, "rawResponseItem/completed", notification)
        }

        ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
            official_event(session_id, "item/autoApprovalReview/started", notification)
        }

        ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => official_event(
            session_id,
            "item/autoApprovalReview/completed",
            notification,
        ),

        ServerNotification::Warning(notification) => {
            progress_event(session_id, "warning", notification.message)
        }

        ServerNotification::GuardianWarning(notification) => {
            json_progress_event(session_id, "guardian_warning", notification)
        }

        ServerNotification::ConfigWarning(notification) => {
            json_progress_event(session_id, "config_warning", notification)
        }

        ServerNotification::ModelRerouted(notification) => {
            json_progress_event(session_id, "model_rerouted", notification)
        }

        ServerNotification::ModelVerification(notification) => {
            official_event(session_id, "model/verification", notification)
        }

        ServerNotification::DeprecationNotice(notification) => {
            official_event(session_id, "deprecationNotice", notification)
        }

        ServerNotification::FuzzyFileSearchSessionUpdated(notification) => {
            official_event(session_id, "fuzzyFileSearch/sessionUpdated", notification)
        }

        ServerNotification::FuzzyFileSearchSessionCompleted(notification) => {
            official_event(session_id, "fuzzyFileSearch/sessionCompleted", notification)
        }

        ServerNotification::ThreadRealtimeStarted(notification) => {
            official_event(session_id, "thread/realtime/started", notification)
        }

        ServerNotification::ThreadRealtimeItemAdded(notification) => {
            official_event(session_id, "thread/realtime/itemAdded", notification)
        }

        ServerNotification::ThreadRealtimeTranscriptDelta(notification) => {
            official_event(session_id, "thread/realtime/transcript/delta", notification)
        }

        ServerNotification::ThreadRealtimeTranscriptDone(notification) => {
            official_event(session_id, "thread/realtime/transcript/done", notification)
        }

        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => official_event(
            session_id,
            "thread/realtime/outputAudio/delta",
            notification,
        ),

        ServerNotification::ThreadRealtimeSdp(notification) => {
            official_event(session_id, "thread/realtime/sdp", notification)
        }

        ServerNotification::ThreadRealtimeError(notification) => {
            official_event(session_id, "thread/realtime/error", notification)
        }

        ServerNotification::ThreadRealtimeClosed(notification) => {
            official_event(session_id, "thread/realtime/closed", notification)
        }

        ServerNotification::WindowsWorldWritableWarning(notification) => {
            official_event(session_id, "windows/worldWritableWarning", notification)
        }

        ServerNotification::WindowsSandboxSetupCompleted(notification) => {
            official_event(session_id, "windowsSandbox/setupCompleted", notification)
        }

        ServerNotification::AccountLoginCompleted(notification) => {
            official_event(session_id, "account/login/completed", notification)
        }

        // ── Errors ──
        ServerNotification::Error(notification) => {
            vec![UnifiedAgentEvent::Error {
                session_id: session_id.to_owned(),
                message: notification.error.message,
                recoverable: notification.will_retry,
            }]
        }
    }
}

/// Map a Codex [`ServerRequest`] (permission request) into a unified event.
fn map_server_request(request: ServerRequest, session_id: &str) -> Vec<UnifiedAgentEvent> {
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
        ServerRequest::ChatgptAuthTokensRefresh { params, .. } => {
            ("chatgpt_auth_refresh".to_owned(), serde_json::to_value(params).unwrap_or(serde_json::Value::Null))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    use codex_app_server_protocol::{
        AgentMessageDeltaNotification, ServerNotification, ServerRequest,
    };

    #[test]
    fn preserves_raw_codex_server_notification_before_derived_events() {
        let notification = ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            delta: "hello".to_owned(),
        });

        let events = map_app_server_event(
            AppServerEvent::ServerNotification(notification),
            "session-1",
        );

        assert_eq!(events.len(), 2);
        match &events[0] {
            UnifiedAgentEvent::CodexAppServerNotification {
                session_id,
                method,
                params,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(method, "item/agentMessage/delta");
                assert_eq!(params["threadId"], "thread-1");
                assert_eq!(params["turnId"], "turn-1");
                assert_eq!(params["itemId"], "item-1");
                assert_eq!(params["delta"], "hello");
            }
            other => panic!("expected raw Codex notification, got {other:?}"),
        }
    }

    #[test]
    fn derives_message_delta_from_agent_message_delta_notification() {
        let notification = ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            delta: "hello".to_owned(),
        });

        let events = map_app_server_event(
            AppServerEvent::ServerNotification(notification),
            "session-1",
        );

        match &events[1] {
            UnifiedAgentEvent::MessageDelta { session_id, delta } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(delta, "hello");
            }
            other => panic!("expected derived message delta, got {other:?}"),
        }
    }

    #[test]
    fn maps_chatgpt_auth_refresh_request_to_permission_event() {
        let request = ServerRequest::ChatgptAuthTokensRefresh {
            request_id: codex_app_server_protocol::RequestId::String("req-1".to_owned()),
            params: codex_app_server_protocol::ChatgptAuthTokensRefreshParams {
                reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                previous_account_id: None,
            },
        };

        let events = map_app_server_event(AppServerEvent::ServerRequest(request), "session-1");

        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedAgentEvent::PermissionRequest {
                session_id,
                request_id,
                tool_name,
                input,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(request_id, "req-1");
                assert_eq!(tool_name, "chatgpt_auth_refresh");
                assert_eq!(input["reason"], "unauthorized");
                assert!(input.get("previousAccountId").is_none() || input["previousAccountId"].is_null());
            }
            other => panic!("expected permission request, got {other:?}"),
        }
    }
}
