//! Main auto-approval logic.
//!
//! Mirrors `checkAutoApproval` from `auto-approval/index.ts`.

use crate::commands::get_command_decision;
use crate::tools::{is_read_only_tool_action, is_write_tool_action};
use crate::types::{
    ApprovalLimitType, AskType, AutoApprovalLimitResult, AutoApprovalState,
    CheckAutoApprovalResult, CommandDecision, McpServer, McpServerUse, ToolAction,
};

// ---------------------------------------------------------------------------
// MCP tool approval
// ---------------------------------------------------------------------------

/// Check if an MCP tool is always allowed based on the server configuration.
///
/// Mirrors `isMcpToolAlwaysAllowed` from `mcp.ts`.
pub fn is_mcp_tool_always_allowed(
    mcp_server_use: &McpServerUse,
    mcp_servers: &[McpServer],
) -> bool {
    match mcp_server_use {
        McpServerUse::UseMcpTool {
            server_name,
            tool_name,
        } => {
            let server = mcp_servers.iter().find(|s| s.name == *server_name);
            let Some(server) = server else {
                return false;
            };
            let tool = server.tools.iter().find(|t| t.name == *tool_name);
            tool.map(|t| t.always_allow).unwrap_or(false)
        }
        McpServerUse::AccessMcpResource { .. } => false,
    }
}

// ---------------------------------------------------------------------------
// Follow-up data parsing
// ---------------------------------------------------------------------------

/// Minimal representation of the follow-up data JSON.
#[derive(Debug, Clone, serde::Deserialize)]
struct FollowUpData {
    #[serde(default)]
    suggest: Vec<FollowUpSuggestion>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FollowUpSuggestion {
    answer: String,
}

// ---------------------------------------------------------------------------
// Tool parsing helper
// ---------------------------------------------------------------------------

/// Attempt to parse the `text` field as a JSON [`ToolAction`].
fn parse_tool_action(text: Option<&str>) -> Option<ToolAction> {
    let text = text?;
    serde_json::from_str(text).ok()
}

// ---------------------------------------------------------------------------
// check_auto_approval
// ---------------------------------------------------------------------------

/// Parameters for [`check_auto_approval`].
pub struct CheckAutoApprovalParams<'a> {
    pub state: &'a AutoApprovalState,
    pub ask: &'a AskType,
    pub text: Option<&'a str>,
    pub is_protected: bool,
    /// MCP server configuration for tool allowlist checks.
    pub mcp_servers: &'a [McpServer],
}

/// Main auto-approval decision function.
///
/// Mirrors `checkAutoApproval` from `auto-approval/index.ts`.
///
/// **Logic branches:**
/// 1. Non-blocking ask → `Approve`
/// 2. Auto-approval disabled → `Ask`
/// 3. `followup` → check `always_allow_followup_questions` + timeout
/// 4. `use_mcp_server` → check `always_allow_mcp` + MCP tool whitelist
/// 5. `command` → check `always_allow_execute` + `get_command_decision`
/// 6. `tool` → check specific tool type
/// 7. Default → `Ask`
pub fn check_auto_approval(params: CheckAutoApprovalParams<'_>) -> CheckAutoApprovalResult {
    // 1. Non-blocking asks are always approved
    if params.ask.is_non_blocking() {
        return CheckAutoApprovalResult::Approve;
    }

    // 2. If auto-approval is not enabled, ask
    if !params.state.auto_approval_enabled {
        return CheckAutoApprovalResult::Ask;
    }

    // 3. Followup handling
    if *params.ask == AskType::Followup {
        if params.state.always_allow_followup_questions {
            if let Some(text) = params.text {
                if let Ok(data) = serde_json::from_str::<FollowUpData>(text) {
                    if let Some(suggestion) = data.suggest.first() {
                        if let Some(timeout) = params.state.followup_auto_approve_timeout_ms {
                            if timeout > 0 {
                                return CheckAutoApprovalResult::Timeout {
                                    timeout_ms: timeout,
                                    auto_response: suggestion.answer.clone(),
                                };
                            }
                        }
                    }
                }
            }
            // If we can't parse or no suggestion, ask
            return CheckAutoApprovalResult::Ask;
        } else {
            return CheckAutoApprovalResult::Ask;
        }
    }

    // 4. MCP server handling
    if *params.ask == AskType::UseMcpServer {
        let Some(text) = params.text else {
            return CheckAutoApprovalResult::Ask;
        };

        if let Ok(mcp_use) = serde_json::from_str::<McpServerUse>(text) {
            match &mcp_use {
                McpServerUse::UseMcpTool { .. } => {
                    if params.state.always_allow_mcp
                        && is_mcp_tool_always_allowed(&mcp_use, params.mcp_servers)
                    {
                        return CheckAutoApprovalResult::Approve;
                    }
                }
                McpServerUse::AccessMcpResource { .. } => {
                    if params.state.always_allow_mcp {
                        return CheckAutoApprovalResult::Approve;
                    }
                }
            }
        }

        return CheckAutoApprovalResult::Ask;
    }

    // 5. Command handling
    if *params.ask == AskType::Command {
        let Some(text) = params.text else {
            return CheckAutoApprovalResult::Ask;
        };

        if params.state.always_allow_execute {
            let decision = get_command_decision(
                text,
                &params.state.allowed_commands,
                &params.state.denied_commands,
            );

            return match decision {
                CommandDecision::AutoApprove => CheckAutoApprovalResult::Approve,
                CommandDecision::AutoDeny => CheckAutoApprovalResult::Deny,
                CommandDecision::AskUser => CheckAutoApprovalResult::Ask,
            };
        }
    }

    // 6. Tool handling
    if *params.ask == AskType::Tool {
        let Some(tool) = parse_tool_action(params.text) else {
            return CheckAutoApprovalResult::Ask;
        };

        // updateTodoList is always approved
        if matches!(tool, ToolAction::UpdateTodoList) {
            return CheckAutoApprovalResult::Approve;
        }

        // skill is always approved
        if matches!(tool, ToolAction::Skill) {
            return CheckAutoApprovalResult::Approve;
        }

        // switchMode requires always_allow_mode_switch
        if matches!(tool, ToolAction::SwitchMode) {
            return if params.state.always_allow_mode_switch {
                CheckAutoApprovalResult::Approve
            } else {
                CheckAutoApprovalResult::Ask
            };
        }

        // newTask / finishTask require always_allow_subtasks
        if matches!(tool, ToolAction::NewTask | ToolAction::FinishTask) {
            return if params.state.always_allow_subtasks {
                CheckAutoApprovalResult::Approve
            } else {
                CheckAutoApprovalResult::Ask
            };
        }

        let is_outside = tool.is_outside_workspace();

        // Read-only tools
        if is_read_only_tool_action(&tool) {
            return if params.state.always_allow_read_only
                && (!is_outside || params.state.always_allow_read_only_outside_workspace)
            {
                CheckAutoApprovalResult::Approve
            } else {
                CheckAutoApprovalResult::Ask
            };
        }

        // Write tools
        if is_write_tool_action(&tool) {
            return if params.state.always_allow_write
                && (!is_outside || params.state.always_allow_write_outside_workspace)
                && (!params.is_protected || params.state.always_allow_write_protected)
            {
                CheckAutoApprovalResult::Approve
            } else {
                CheckAutoApprovalResult::Ask
            };
        }
    }

    // 7. Default
    CheckAutoApprovalResult::Ask
}

// ===========================================================================
// AutoApprovalHandler — mirrors AutoApprovalHandler.ts
// ===========================================================================

/// Stateful handler for tracking auto-approval limits across requests.
///
/// In the TypeScript version this class owns the `askForApproval` async
/// callback and blocks until the user responds.  The Rust port keeps the
/// crate synchronous and pure-logic: the caller is responsible for
/// presenting the approval dialog and calling
/// [`approve_and_reset`](Self::approve_and_reset) when the user clicks
/// "yes".
///
/// # Usage
///
/// ```ignore
/// let mut handler = AutoApprovalHandler::new();
///
/// // The caller slices messages starting at last_reset_message_index().
/// let idx = handler.last_reset_message_index();
/// let api_req_count = messages[idx..].iter().filter(|m| m.is_api_req_started()).count();
/// let total_cost  = compute_cost(&messages[idx..]);
///
/// let result = handler.check_auto_approval_limits(
///     max_requests, max_cost,
///     messages.len(),
///     api_req_count,
///     total_cost,
/// );
///
/// if result.requires_approval {
///     // Show dialog to user …
///     if user_approved {
///         handler.approve_and_reset(messages.len());
///         // proceed
///     } else {
///         // abort
///     }
/// }
/// ```
///
/// Mirrors `AutoApprovalHandler` from `AutoApprovalHandler.ts`.
pub struct AutoApprovalHandler {
    /// Index into the message array after which counting starts.
    /// Reset to `messages.len()` when the user approves a limit dialog.
    last_reset_message_index: usize,
    /// Cached count of consecutive auto-approved requests.
    consecutive_auto_approved_requests_count: usize,
    /// Cached accumulated cost of consecutive auto-approved requests.
    consecutive_auto_approved_cost: f64,
}

impl AutoApprovalHandler {
    /// Create a new handler with default (zeroed) state.
    pub fn new() -> Self {
        Self {
            last_reset_message_index: 0,
            consecutive_auto_approved_requests_count: 0,
            consecutive_auto_approved_cost: 0.0,
        }
    }

    /// The message index after which counting starts.
    ///
    /// The caller should slice `messages[last_reset_message_index()..]`
    /// before counting `api_req_started` messages or computing cost.
    pub fn last_reset_message_index(&self) -> usize {
        self.last_reset_message_index
    }

    /// Check if auto-approval limits have been reached.
    ///
    /// This is the main entry point.  It checks the request-count limit
    /// first, then the cost limit — exactly like the TS version.
    ///
    /// # Parameters
    ///
    /// * `max_requests` – `Some(N)` to cap consecutive requests, `None` for
    ///   unlimited (maps to `state?.allowedMaxRequests || Infinity` in TS).
    /// * `max_cost` – `Some(C)` to cap cumulative cost, `None` for unlimited.
    /// * `total_message_count` – `messages.len()` (used only for the
    ///   reset-point on approval).
    /// * `api_req_started_count` – Number of `api_req_started` messages in
    ///   `messages[last_reset_message_index()..]`.
    /// * `total_cost_after_reset` – Total cost computed from
    ///   `messages[last_reset_message_index()..]`.
    ///
    /// # Returns
    ///
    /// An [`AutoApprovalLimitResult`].  When `requires_approval` is `true`
    /// the caller should present a dialog.  If the user approves, call
    /// [`approve_and_reset`](Self::approve_and_reset).
    pub fn check_auto_approval_limits(
        &mut self,
        max_requests: Option<usize>,
        max_cost: Option<f64>,
        total_message_count: usize,
        api_req_started_count: usize,
        total_cost_after_reset: f64,
    ) -> AutoApprovalLimitResult {
        // Check request count limit first (matches TS order).
        let request_result =
            self.check_request_limit(max_requests, total_message_count, api_req_started_count);
        if !request_result.should_proceed || request_result.requires_approval {
            return request_result;
        }

        // Then check cost limit.
        self.check_cost_limit(max_cost, total_message_count, total_cost_after_reset)
    }

    /// Check the request-count limit.
    ///
    /// Mirrors `checkRequestLimit` from `AutoApprovalHandler.ts`.
    fn check_request_limit(
        &mut self,
        max_requests: Option<usize>,
        _total_message_count: usize,
        api_req_started_count: usize,
    ) -> AutoApprovalLimitResult {
        let max_requests = match max_requests {
            Some(max) => max,
            None => return AutoApprovalLimitResult::proceed(),
        };

        // +1 for the current request being checked (matches TS).
        self.consecutive_auto_approved_requests_count = api_req_started_count + 1;

        if self.consecutive_auto_approved_requests_count > max_requests {
            return AutoApprovalLimitResult::limit_exceeded(
                ApprovalLimitType::Requests,
                max_requests,
            );
        }

        AutoApprovalLimitResult::proceed()
    }

    /// Check the cost limit.
    ///
    /// Mirrors `checkCostLimit` from `AutoApprovalHandler.ts`.
    fn check_cost_limit(
        &mut self,
        max_cost: Option<f64>,
        _total_message_count: usize,
        total_cost_after_reset: f64,
    ) -> AutoApprovalLimitResult {
        let max_cost = match max_cost {
            Some(max) => max,
            None => return AutoApprovalLimitResult::proceed(),
        };

        self.consecutive_auto_approved_cost = total_cost_after_reset;

        // Use epsilon for floating-point comparison to avoid precision
        // issues (matches TS).
        const EPSILON: f64 = 0.0001;
        if self.consecutive_auto_approved_cost > max_cost + EPSILON {
            return AutoApprovalLimitResult::limit_exceeded(
                ApprovalLimitType::Cost,
                format!("{:.2}", max_cost),
            );
        }

        AutoApprovalLimitResult::proceed()
    }

    /// Record that the user approved a limit-exceeded dialog.
    ///
    /// Resets the tracking window so that only messages *after*
    /// `current_message_count` are considered in future checks.
    ///
    /// Mirrors the `this.lastResetMessageIndex = messages.length` line
    /// inside `checkRequestLimit` / `checkCostLimit` when
    /// `response === "yesButtonClicked"`.
    pub fn approve_and_reset(&mut self, current_message_count: usize) {
        self.last_reset_message_index = current_message_count;
    }

    /// Reset all tracking state (typically called when starting a new task).
    ///
    /// Mirrors `resetRequestCount` from `AutoApprovalHandler.ts`.
    pub fn reset_request_count(&mut self) {
        self.last_reset_message_index = 0;
        self.consecutive_auto_approved_requests_count = 0;
        self.consecutive_auto_approved_cost = 0.0;
    }

    /// Get the current approval state for debugging / testing.
    ///
    /// Returns `(request_count, current_cost)`.
    ///
    /// Mirrors `getApprovalState` from `AutoApprovalHandler.ts`.
    pub fn get_approval_state(&self) -> (usize, f64) {
        (
            self.consecutive_auto_approved_requests_count,
            self.consecutive_auto_approved_cost,
        )
    }
}

impl Default for AutoApprovalHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::McpTool;

    fn default_state() -> AutoApprovalState {
        AutoApprovalState {
            auto_approval_enabled: true,
            ..Default::default()
        }
    }

    fn full_access_state() -> AutoApprovalState {
        AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_read_only: true,
            always_allow_read_only_outside_workspace: true,
            always_allow_write: true,
            always_allow_write_outside_workspace: true,
            always_allow_write_protected: true,
            always_allow_mcp: true,
            always_allow_mode_switch: true,
            always_allow_subtasks: true,
            always_allow_execute: true,
            always_allow_followup_questions: true,
            followup_auto_approve_timeout_ms: Some(5000),
            allowed_commands: vec!["*".to_string()],
            denied_commands: vec![],
            ..Default::default()
        }
    }

    // ---- Basic ----

    #[test]
    fn test_auto_approval_disabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: false,
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_non_blocking_ask_approved() {
        let state = default_state();
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::CommandOutput,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    // ---- Followup ----

    #[test]
    fn test_followup_with_suggestion_timeout() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_followup_questions: true,
            followup_auto_approve_timeout_ms: Some(3000),
            ..Default::default()
        };
        let text = r#"{"suggest":[{"answer":"yes, do it"}]}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Followup,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(
            result,
            CheckAutoApprovalResult::Timeout {
                timeout_ms: 3000,
                auto_response: "yes, do it".to_string(),
            }
        );
    }

    #[test]
    fn test_followup_without_suggestion() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_followup_questions: true,
            followup_auto_approve_timeout_ms: Some(3000),
            ..Default::default()
        };
        let text = r#"{"suggest":[]}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Followup,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_followup_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_followup_questions: false,
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Followup,
            text: Some(r#"{"suggest":[{"answer":"yes"}]}"#),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_followup_invalid_json() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_followup_questions: true,
            followup_auto_approve_timeout_ms: Some(3000),
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Followup,
            text: Some("not json"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_followup_no_timeout_configured() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_followup_questions: true,
            followup_auto_approve_timeout_ms: None,
            ..Default::default()
        };
        let text = r#"{"suggest":[{"answer":"yes"}]}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Followup,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- MCP ----

    #[test]
    fn test_mcp_tool_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mcp: true,
            ..Default::default()
        };
        let mcp_servers = vec![McpServer {
            name: "test-server".to_string(),
            tools: vec![McpTool {
                name: "read_file".to_string(),
                always_allow: true,
            }],
        }];
        let text = r#"{"type":"use_mcp_tool","server_name":"test-server","tool_name":"read_file"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::UseMcpServer,
            text: Some(text),
            is_protected: false,
            mcp_servers: &mcp_servers,
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_mcp_tool_not_always_allowed() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mcp: true,
            ..Default::default()
        };
        let mcp_servers = vec![McpServer {
            name: "test-server".to_string(),
            tools: vec![McpTool {
                name: "dangerous_tool".to_string(),
                always_allow: false,
            }],
        }];
        let text = r#"{"type":"use_mcp_tool","server_name":"test-server","tool_name":"dangerous_tool"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::UseMcpServer,
            text: Some(text),
            is_protected: false,
            mcp_servers: &mcp_servers,
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_mcp_resource_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mcp: true,
            ..Default::default()
        };
        let text = r#"{"type":"access_mcp_resource","server_name":"test-server","uri":"test://resource"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::UseMcpServer,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_mcp_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mcp: false,
            ..Default::default()
        };
        let text = r#"{"type":"access_mcp_resource","server_name":"test-server","uri":"test://resource"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::UseMcpServer,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_mcp_no_text() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mcp: true,
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::UseMcpServer,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- Command ----

    #[test]
    fn test_command_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_execute: true,
            allowed_commands: vec!["git".to_string()],
            denied_commands: vec![],
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Command,
            text: Some("git status"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_command_denied() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_execute: true,
            allowed_commands: vec!["git".to_string()],
            denied_commands: vec!["rm".to_string()],
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Command,
            text: Some("rm -rf /"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Deny);
    }

    #[test]
    fn test_command_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_execute: false,
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Command,
            text: Some("git status"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_command_no_text() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_execute: true,
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Command,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_command_wildcard_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_execute: true,
            allowed_commands: vec!["*".to_string()],
            denied_commands: vec![],
            ..Default::default()
        };
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Command,
            text: Some("anything here"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    // ---- Tool: read-only ----

    #[test]
    fn test_read_only_tool_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_read_only: true,
            ..Default::default()
        };
        let text = r#"{"tool":"readFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_read_only_outside_workspace_blocked() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_read_only: true,
            always_allow_read_only_outside_workspace: false,
            ..Default::default()
        };
        let text = r#"{"tool":"readFile","isOutsideWorkspace":true}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_read_only_outside_workspace_allowed() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_read_only: true,
            always_allow_read_only_outside_workspace: true,
            ..Default::default()
        };
        let text = r#"{"tool":"readFile","isOutsideWorkspace":true}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_read_only_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_read_only: false,
            ..Default::default()
        };
        let text = r#"{"tool":"readFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- Tool: write ----

    #[test]
    fn test_write_tool_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: true,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_write_outside_workspace_blocked() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: true,
            always_allow_write_outside_workspace: false,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":true}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_write_outside_workspace_allowed() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: true,
            always_allow_write_outside_workspace: true,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":true}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_write_protected_blocked() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: true,
            always_allow_write_protected: false,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: true,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_write_protected_allowed() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: true,
            always_allow_write_protected: true,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: true,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_write_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_write: false,
            ..Default::default()
        };
        let text = r#"{"tool":"editedExistingFile","isOutsideWorkspace":false}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- Tool: special tools ----

    #[test]
    fn test_todo_list_always_approved() {
        let state = default_state();
        let text = r#"{"tool":"updateTodoList"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_skill_always_approved() {
        let state = default_state();
        let text = r#"{"tool":"skill"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_switch_mode_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mode_switch: true,
            ..Default::default()
        };
        let text = r#"{"tool":"switchMode"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_switch_mode_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_mode_switch: false,
            ..Default::default()
        };
        let text = r#"{"tool":"switchMode"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_subtasks_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_subtasks: true,
            ..Default::default()
        };
        let text = r#"{"tool":"newTask"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_finish_task_approved() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_subtasks: true,
            ..Default::default()
        };
        let text = r#"{"tool":"finishTask"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Approve);
    }

    #[test]
    fn test_subtasks_not_enabled() {
        let state = AutoApprovalState {
            auto_approval_enabled: true,
            always_allow_subtasks: false,
            ..Default::default()
        };
        let text = r#"{"tool":"newTask"}"#;
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some(text),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- Tool: invalid JSON ----

    #[test]
    fn test_tool_invalid_json() {
        let state = full_access_state();
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: Some("not json"),
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    #[test]
    fn test_tool_no_text() {
        let state = full_access_state();
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::Tool,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- Default ----

    #[test]
    fn test_unknown_ask_returns_ask() {
        let state = full_access_state();
        let result = check_auto_approval(CheckAutoApprovalParams {
            state: &state,
            ask: &AskType::CompletionResult,
            text: None,
            is_protected: false,
            mcp_servers: &[],
        });
        assert_eq!(result, CheckAutoApprovalResult::Ask);
    }

    // ---- MCP tool always allowed helper ----

    #[test]
    fn test_mcp_tool_always_allowed_match() {
        let mcp_servers = vec![McpServer {
            name: "server1".to_string(),
            tools: vec![McpTool {
                name: "tool1".to_string(),
                always_allow: true,
            }],
        }];
        let mcp_use = McpServerUse::UseMcpTool {
            server_name: "server1".to_string(),
            tool_name: "tool1".to_string(),
        };
        assert!(is_mcp_tool_always_allowed(&mcp_use, &mcp_servers));
    }

    #[test]
    fn test_mcp_tool_always_allowed_no_match() {
        let mcp_servers = vec![McpServer {
            name: "server1".to_string(),
            tools: vec![McpTool {
                name: "tool1".to_string(),
                always_allow: false,
            }],
        }];
        let mcp_use = McpServerUse::UseMcpTool {
            server_name: "server1".to_string(),
            tool_name: "tool1".to_string(),
        };
        assert!(!is_mcp_tool_always_allowed(&mcp_use, &mcp_servers));
    }

    #[test]
    fn test_mcp_tool_always_allowed_resource() {
        let mcp_use = McpServerUse::AccessMcpResource {
            server_name: "server1".to_string(),
            uri: "test://resource".to_string(),
        };
        assert!(!is_mcp_tool_always_allowed(&mcp_use, &[]));
    }
}

// ===========================================================================
// AutoApprovalHandler tests — mirror AutoApprovalHandler.spec.ts
// ===========================================================================

#[cfg(test)]
mod handler_tests {
    use super::*;

    // ---- Basic ----

    #[test]
    fn test_no_limits_set() {
        let mut handler = AutoApprovalHandler::new();
        let result = handler.check_auto_approval_limits(
            None, // max_requests (unlimited)
            None, // max_cost (unlimited)
            0,    // total_message_count
            0,    // api_req_started_count
            0.0,  // total_cost_after_reset
        );
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    // ---- Request limit ----

    #[test]
    fn test_request_limit_not_exceeded() {
        let mut handler = AutoApprovalHandler::new();
        // max_requests = 3, api_req_started_count = 2, count = 2 + 1 = 3 <= 3
        let result = handler.check_auto_approval_limits(Some(3), None, 2, 2, 0.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_request_limit_exceeded() {
        let mut handler = AutoApprovalHandler::new();
        // max_requests = 3, api_req_started_count = 3, count = 3 + 1 = 4 > 3
        let result = handler.check_auto_approval_limits(Some(3), None, 3, 3, 0.0);
        assert!(!result.should_proceed);
        assert!(result.requires_approval);
        assert_eq!(result.approval_type, Some(ApprovalLimitType::Requests));
        assert_eq!(result.approval_count, Some("3".to_string()));
    }

    #[test]
    fn test_request_limit_at_boundary() {
        let mut handler = AutoApprovalHandler::new();
        // max_requests = 1, api_req_started_count = 0, count = 0 + 1 = 1 <= 1
        let result = handler.check_auto_approval_limits(Some(1), None, 0, 0, 0.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_request_limit_calculates_count() {
        let mut handler = AutoApprovalHandler::new();

        // First check — no messages, count = 0 + 1 = 1
        handler.check_auto_approval_limits(Some(5), None, 0, 0, 0.0);
        let (count, _) = handler.get_approval_state();
        assert_eq!(count, 1);

        // Second check — 1 api_req_started message, count = 1 + 1 = 2
        handler.check_auto_approval_limits(Some(5), None, 1, 1, 0.0);
        let (count, _) = handler.get_approval_state();
        assert_eq!(count, 2);

        // Third check — 2 api_req_started messages, count = 2 + 1 = 3
        handler.check_auto_approval_limits(Some(5), None, 2, 2, 0.0);
        let (count, _) = handler.get_approval_state();
        assert_eq!(count, 3);
    }

    // ---- Cost limit ----

    #[test]
    fn test_cost_limit_not_exceeded() {
        let mut handler = AutoApprovalHandler::new();
        let result = handler.check_auto_approval_limits(None, Some(5.0), 0, 0, 3.5);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_cost_limit_exceeded() {
        let mut handler = AutoApprovalHandler::new();
        let result = handler.check_auto_approval_limits(None, Some(5.0), 0, 0, 5.5);
        assert!(!result.should_proceed);
        assert!(result.requires_approval);
        assert_eq!(result.approval_type, Some(ApprovalLimitType::Cost));
        assert_eq!(result.approval_count, Some("5.00".to_string()));
    }

    #[test]
    fn test_cost_floating_point_precision() {
        let mut handler = AutoApprovalHandler::new();

        // Exactly at limit — should not trigger
        let result = handler.check_auto_approval_limits(None, Some(5.0), 0, 0, 5.0);
        assert!(!result.requires_approval);

        // Slight floating-point error — should not trigger (within epsilon)
        let result = handler.check_auto_approval_limits(None, Some(5.0), 0, 0, 5.00009);
        assert!(!result.requires_approval);

        // Actually exceeded — should trigger
        let result = handler.check_auto_approval_limits(None, Some(5.0), 0, 0, 5.001);
        assert!(result.requires_approval);
    }

    // ---- Approve and reset ----

    #[test]
    fn test_approve_and_reset_requests() {
        let mut handler = AutoApprovalHandler::new();

        // Exceed request limit (3 messages + current = 4 > 3)
        let result = handler.check_auto_approval_limits(Some(3), None, 3, 3, 0.0);
        assert!(result.requires_approval);

        // User approves — reset
        handler.approve_and_reset(3);

        // After reset, only 1 message after reset point, count = 1 + 1 = 2 <= 3
        let result = handler.check_auto_approval_limits(Some(3), None, 4, 1, 0.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_approve_and_reset_cost() {
        let mut handler = AutoApprovalHandler::new();

        // Cost exceeds limit (6.0 > 5.0)
        let result = handler.check_auto_approval_limits(None, Some(5.0), 2, 0, 6.0);
        assert!(result.requires_approval);

        // User approves — reset at message index 2
        handler.approve_and_reset(2);

        // After reset, cost from messages after index 2 is 3.0 < 5.0
        let result = handler.check_auto_approval_limits(None, Some(5.0), 4, 0, 3.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    #[test]
    fn test_multiple_resets() {
        let mut handler = AutoApprovalHandler::new();

        // First cost limit hit (6.0 > 5.0)
        handler.check_auto_approval_limits(None, Some(5.0), 1, 0, 6.0);
        handler.approve_and_reset(1);

        // Second cost limit hit (6.0 > 5.0, counting from index 1)
        handler.check_auto_approval_limits(None, Some(5.0), 3, 0, 6.0);
        handler.approve_and_reset(3);

        // Third check — cost from index 3 is 2.0 < 5.0
        let result = handler.check_auto_approval_limits(None, Some(5.0), 4, 0, 2.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    // ---- Reset ----

    #[test]
    fn test_reset_request_count() {
        let mut handler = AutoApprovalHandler::new();

        // Build up some state
        handler.check_auto_approval_limits(Some(5), Some(10.0), 3, 3, 5.0);
        let (count, cost) = handler.get_approval_state();
        assert_eq!(count, 4); // 3 + 1
        assert!((cost - 5.0).abs() < f64::EPSILON);

        // Reset
        handler.reset_request_count();

        let (count, cost) = handler.get_approval_state();
        assert_eq!(count, 0);
        assert!((cost - 0.0).abs() < f64::EPSILON);

        // After reset, all messages are counted again
        handler.check_auto_approval_limits(Some(5), Some(10.0), 3, 3, 8.0);
        let (count, cost) = handler.get_approval_state();
        assert_eq!(count, 4); // 3 + 1
        assert!((cost - 8.0).abs() < f64::EPSILON);
    }

    // ---- Combined limits ----

    #[test]
    fn test_request_limit_checked_before_cost() {
        let mut handler = AutoApprovalHandler::new();
        // Both limits would be exceeded, but request is checked first
        let result = handler.check_auto_approval_limits(Some(1), Some(0.01), 1, 1, 5.0);
        assert!(result.requires_approval);
        assert_eq!(result.approval_type, Some(ApprovalLimitType::Requests));
    }

    #[test]
    fn test_combined_limits_request_then_cost() {
        let mut handler = AutoApprovalHandler::new();

        // First check — under both limits
        let result = handler.check_auto_approval_limits(Some(2), Some(10.0), 0, 0, 3.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);

        // Second check — request limit exceeded (count = 2 + 1 = 3 > 2)
        let result = handler.check_auto_approval_limits(Some(2), Some(10.0), 2, 2, 3.0);
        assert!(result.requires_approval);
        assert_eq!(result.approval_type, Some(ApprovalLimitType::Requests));

        // Approve and reset
        handler.approve_and_reset(2);

        // Third check — under limits after reset
        let result = handler.check_auto_approval_limits(Some(2), Some(10.0), 3, 0, 3.0);
        assert!(result.should_proceed);
        assert!(!result.requires_approval);
    }

    // ---- Default trait ----

    #[test]
    fn test_default() {
        let handler = AutoApprovalHandler::default();
        assert_eq!(handler.last_reset_message_index(), 0);
        let (count, cost) = handler.get_approval_state();
        assert_eq!(count, 0);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }
}
