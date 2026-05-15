//! Core types for the auto-approval system.
//!
//! These types mirror the TypeScript `AutoApprovalState`, `CheckAutoApprovalResult`,
//! and related enums from `auto-approval/index.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AskType — mirrors the ClineAsk union used in checkAutoApproval
// ---------------------------------------------------------------------------

/// The kind of approval being requested.
///
/// Only a subset of the full `ClineAsk` enum is relevant for auto-approval
/// decisions. This enum captures those variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskType {
    Followup,
    Command,
    CommandOutput,
    CompletionResult,
    Tool,
    ApiReqFailed,
    ResumeTask,
    ResumeCompletedTask,
    MistakeLimitReached,
    UseMcpServer,
    AutoApprovalMaxReqReached,
}

impl AskType {
    /// Returns `true` for non-blocking asks — those that are not associated
    /// with an actual approval and are only used to update chat messages.
    ///
    /// Mirrors `isNonBlockingAsk` from `@roo-code/types`.
    pub fn is_non_blocking(&self) -> bool {
        matches!(self, AskType::CommandOutput)
    }
}

// ---------------------------------------------------------------------------
// CommandDecision — mirrors `commands.ts` CommandDecision
// ---------------------------------------------------------------------------

/// Decision returned by the command validation logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandDecision {
    AutoApprove,
    AutoDeny,
    AskUser,
}

// ---------------------------------------------------------------------------
// CheckAutoApprovalResult — mirrors index.ts CheckAutoApprovalResult
// ---------------------------------------------------------------------------

/// Result returned by [`check_auto_approval`](crate::check_auto_approval).
#[derive(Debug, Clone, PartialEq)]
pub enum CheckAutoApprovalResult {
    Approve,
    Deny,
    Ask,
    /// A follow-up question that will auto-approve after `timeout_ms`
    /// milliseconds using `auto_response` as the answer.
    Timeout {
        timeout_ms: u64,
        auto_response: String,
    },
}

// ---------------------------------------------------------------------------
// AutoApprovalLimitResult — mirrors AutoApprovalResult from AutoApprovalHandler.ts
// ---------------------------------------------------------------------------

/// Type of auto-approval limit that was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalLimitType {
    /// Maximum number of consecutive auto-approved requests.
    Requests,
    /// Maximum cost of consecutive auto-approved requests.
    Cost,
}

/// Result of checking auto-approval limits.
///
/// Mirrors `AutoApprovalResult` from `AutoApprovalHandler.ts`.
///
/// When `requires_approval` is `true`, the caller should ask the user for
/// approval. If the user approves, call
/// [`AutoApprovalHandler::approve_and_reset`] to reset tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoApprovalLimitResult {
    /// Whether the operation should proceed.
    pub should_proceed: bool,
    /// Whether user approval was required (limit was exceeded).
    pub requires_approval: bool,
    /// The type of limit that was exceeded, if any.
    pub approval_type: Option<ApprovalLimitType>,
    /// The count/value of the limit that was exceeded, if any.
    pub approval_count: Option<String>,
}

impl AutoApprovalLimitResult {
    /// Create a result indicating no limits were exceeded.
    pub fn proceed() -> Self {
        Self {
            should_proceed: true,
            requires_approval: false,
            approval_type: None,
            approval_count: None,
        }
    }

    /// Create a result indicating a limit was exceeded.
    pub fn limit_exceeded(approval_type: ApprovalLimitType, count: impl std::fmt::Display) -> Self {
        Self {
            should_proceed: false,
            requires_approval: true,
            approval_type: Some(approval_type),
            approval_count: Some(count.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// AutoApprovalState — mirrors ExtensionState subset used in checkAutoApproval
// ---------------------------------------------------------------------------

/// Holds all auto-approval configuration flags and settings.
///
/// This is the Rust equivalent of
/// `Pick<ExtensionState, AutoApprovalState | AutoApprovalStateOptions>`.
///
/// Includes limit fields from `GlobalState` (`allowedMaxRequests`, `allowedMaxCost`)
/// and runtime tracking counters for request count and cost.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoApprovalState {
    pub auto_approval_enabled: bool,
    pub always_allow_read_only: bool,
    pub always_allow_read_only_outside_workspace: bool,
    pub always_allow_write: bool,
    pub always_allow_write_outside_workspace: bool,
    pub always_allow_write_protected: bool,
    pub always_allow_mcp: bool,
    pub always_allow_mode_switch: bool,
    pub always_allow_subtasks: bool,
    pub always_allow_execute: bool,
    pub always_allow_followup_questions: bool,
    pub followup_auto_approve_timeout_ms: Option<u64>,
    pub allowed_commands: Vec<String>,
    pub denied_commands: Vec<String>,

    // --- Limit fields (from GlobalState in TS) ---
    /// Maximum number of consecutive auto-approved API requests before
    /// requiring manual approval. Maps to `state?.allowedMaxRequests`.
    ///
    /// Source: `AutoApprovalHandler.ts` — `checkRequestLimit`
    pub allowed_max_requests: Option<u32>,

    /// Maximum cumulative cost of consecutive auto-approved requests before
    /// requiring manual approval. Maps to `state?.allowedMaxCost`.
    ///
    /// Source: `AutoApprovalHandler.ts` — `checkCostLimit`
    pub allowed_max_cost: Option<f64>,

    // --- Runtime tracking counters ---
    /// Current count of consecutive auto-approved requests.
    /// Updated by `check_auto_approval_limits()`.
    pub current_request_count: u32,

    /// Current accumulated cost of consecutive auto-approved requests.
    /// Updated by `check_auto_approval_limits()`.
    pub current_cost: f64,
}

impl AutoApprovalState {
    /// Check if auto-approval limits have been reached.
    ///
    /// This is a convenience method that checks both request count and cost
    /// limits using the state's own limit fields. It updates the runtime
    /// tracking counters (`current_request_count`, `current_cost`) and
    /// returns an [`AutoApprovalLimitResult`] indicating whether the
    /// operation should proceed.
    ///
    /// # Parameters
    ///
    /// * `api_req_started_count` – Number of `api_req_started` messages in
    ///   the current tracking window.
    /// * `total_cost` – Total cost accumulated in the current tracking window.
    ///
    /// # Returns
    ///
    /// An [`AutoApprovalLimitResult`]. When `requires_approval` is `true`,
    /// the caller should present a dialog. If the user approves, call
    /// [`Self::reset_limits`] to reset the counters.
    ///
    /// Source: `AutoApprovalHandler.ts` — `checkAutoApprovalLimits`
    pub fn check_auto_approval_limits(
        &mut self,
        api_req_started_count: usize,
        total_cost: f64,
    ) -> AutoApprovalLimitResult {
        // Check request count limit first (matches TS order).
        let request_result = self.check_request_limit(api_req_started_count);
        if !request_result.should_proceed || request_result.requires_approval {
            return request_result;
        }

        // Then check cost limit.
        self.check_cost_limit(total_cost)
    }

    /// Check the request-count limit.
    ///
    /// Source: `AutoApprovalHandler.ts` — `checkRequestLimit`
    fn check_request_limit(&mut self, api_req_started_count: usize) -> AutoApprovalLimitResult {
        let max_requests = match self.allowed_max_requests {
            Some(max) => max,
            None => return AutoApprovalLimitResult::proceed(),
        };

        // +1 for the current request being checked (matches TS).
        self.current_request_count = api_req_started_count as u32 + 1;

        if self.current_request_count > max_requests {
            return AutoApprovalLimitResult::limit_exceeded(
                ApprovalLimitType::Requests,
                max_requests,
            );
        }

        AutoApprovalLimitResult::proceed()
    }

    /// Check the cost limit.
    ///
    /// Source: `AutoApprovalHandler.ts` — `checkCostLimit`
    fn check_cost_limit(&mut self, total_cost: f64) -> AutoApprovalLimitResult {
        let max_cost = match self.allowed_max_cost {
            Some(max) => max,
            None => return AutoApprovalLimitResult::proceed(),
        };

        self.current_cost = total_cost;

        // Use epsilon for floating-point comparison to avoid precision
        // issues (matches TS).
        const EPSILON: f64 = 0.0001;
        if self.current_cost > max_cost + EPSILON {
            return AutoApprovalLimitResult::limit_exceeded(
                ApprovalLimitType::Cost,
                format!("{:.2}", max_cost),
            );
        }

        AutoApprovalLimitResult::proceed()
    }

    /// Reset the runtime tracking counters.
    ///
    /// Call this when the user approves a limit-exceeded dialog, or when
    /// starting a new task.
    ///
    /// Source: `AutoApprovalHandler.ts` — `resetRequestCount`
    pub fn reset_limits(&mut self) {
        self.current_request_count = 0;
        self.current_cost = 0.0;
    }
}

// ---------------------------------------------------------------------------
// MCP types — minimal types for MCP tool approval
// ---------------------------------------------------------------------------

/// Describes an MCP tool with its `always_allow` flag.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub always_allow: bool,
}

/// Describes an MCP server with its list of tools.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpServer {
    pub name: String,
    pub tools: Vec<McpTool>,
}

/// The kind of MCP usage being requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServerUse {
    UseMcpTool {
        server_name: String,
        tool_name: String,
    },
    AccessMcpResource {
        server_name: String,
        uri: String,
    },
}

// ---------------------------------------------------------------------------
// Tool info — minimal representation for tool approval
// ---------------------------------------------------------------------------

/// The kind of tool action being performed, used by the approval logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "camelCase")]
pub enum ToolAction {
    ReadFile {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    ListFiles {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    ListFilesTopLevel {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    ListFilesRecursive {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    SearchFiles {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    CodebaseSearch {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    RunSlashCommand {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    EditedExistingFile {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    AppliedDiff {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    NewFileCreated {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    GenerateImage {
        #[serde(default, rename = "isOutsideWorkspace")]
        is_outside_workspace: bool,
    },
    UpdateTodoList,
    Skill,
    SwitchMode,
    NewTask,
    FinishTask,
}

impl ToolAction {
    /// Returns the tool name as a string slice.
    pub fn tool_name(&self) -> &'static str {
        match self {
            ToolAction::ReadFile { .. } => "readFile",
            ToolAction::ListFiles { .. } => "listFiles",
            ToolAction::ListFilesTopLevel { .. } => "listFilesTopLevel",
            ToolAction::ListFilesRecursive { .. } => "listFilesRecursive",
            ToolAction::SearchFiles { .. } => "searchFiles",
            ToolAction::CodebaseSearch { .. } => "codebaseSearch",
            ToolAction::RunSlashCommand { .. } => "runSlashCommand",
            ToolAction::EditedExistingFile { .. } => "editedExistingFile",
            ToolAction::AppliedDiff { .. } => "appliedDiff",
            ToolAction::NewFileCreated { .. } => "newFileCreated",
            ToolAction::GenerateImage { .. } => "generateImage",
            ToolAction::UpdateTodoList => "updateTodoList",
            ToolAction::Skill => "skill",
            ToolAction::SwitchMode => "switchMode",
            ToolAction::NewTask => "newTask",
            ToolAction::FinishTask => "finishTask",
        }
    }

    /// Returns `true` if the tool is outside the workspace.
    pub fn is_outside_workspace(&self) -> bool {
        match self {
            ToolAction::ReadFile {
                is_outside_workspace,
            }
            | ToolAction::ListFiles {
                is_outside_workspace,
            }
            | ToolAction::ListFilesTopLevel {
                is_outside_workspace,
            }
            | ToolAction::ListFilesRecursive {
                is_outside_workspace,
            }
            | ToolAction::SearchFiles {
                is_outside_workspace,
            }
            | ToolAction::CodebaseSearch {
                is_outside_workspace,
            }
            | ToolAction::RunSlashCommand {
                is_outside_workspace,
            }
            | ToolAction::EditedExistingFile {
                is_outside_workspace,
            }
            | ToolAction::AppliedDiff {
                is_outside_workspace,
            }
            | ToolAction::NewFileCreated {
                is_outside_workspace,
            }
            | ToolAction::GenerateImage {
                is_outside_workspace,
            } => *is_outside_workspace,
            _ => false,
        }
    }
}
