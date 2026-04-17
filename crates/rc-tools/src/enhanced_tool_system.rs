//! Enhanced Tool System — Rich tool specs, execution context, and pipeline.
//!
//! Implements the P0 gaps identified in the Tool System:
//! - [`RichToolSpec`] — enriched tool specification with metadata flags
//! - [`RichToolUseContext`] — full execution context for tool invocations
//! - [`ToolExecutionPipeline`] — validate → pre-hooks → permission → execute → post-hooks → summarize
//! - [`ToolPermissionContext`] — permission evaluation context
//! - [`ValidationResult`] — structured validation outcome

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rc_core::permission_types::{PermissionBehavior, PermissionRule};
use rc_core::{PermissionMode, ToolCall, ToolResult};

// ---------------------------------------------------------------------------
// InterruptBehavior
// ---------------------------------------------------------------------------

/// How a tool reacts when a new user message arrives mid-execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptBehavior {
    /// Cancel the running tool immediately.
    #[default]
    Cancel,
    /// Block the new message until the tool finishes.
    Block,
}

// ---------------------------------------------------------------------------
// McpToolInfo
// ---------------------------------------------------------------------------

/// Metadata identifying an MCP-sourced tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// MCP server name that provides this tool.
    pub server_name: String,
    /// Tool name as registered on the MCP server.
    pub tool_name: String,
}

// ---------------------------------------------------------------------------
// RichToolSpec
// ---------------------------------------------------------------------------

/// Enriched tool specification with metadata flags for the execution pipeline.
///
/// Mirrors the fields from Claude Code's `Tool.ts` (lines 362–598).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichToolSpec {
    // ── Identity ──────────────────────────────────────────────────────────
    /// Internal tool name (e.g. `"read_file"`).
    pub name: String,
    /// Name used in provider protocol messages.
    pub protocol_name: String,
    /// Name used for permission classification.
    pub permission_tool_name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// Whether this tool requires user permission to execute.
    pub requires_permission: bool,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: Value,

    // ── Rich metadata ─────────────────────────────────────────────────────
    /// Alternative names for backwards compatibility.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// One-line capability phrase for ToolSearch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_hint: Option<String>,
    /// Whether tool can run in parallel with other tools.
    #[serde(default)]
    pub is_concurrency_safe: bool,
    /// Whether tool only reads (no side effects).
    #[serde(default)]
    pub is_read_only: bool,
    /// Whether tool performs irreversible operations.
    #[serde(default)]
    pub is_destructive: bool,
    /// Whether tool is deferred (ToolSearch required to load).
    #[serde(default)]
    pub should_defer: bool,
    /// Whether tool always appears in initial prompt.
    #[serde(default)]
    pub always_load: bool,
    /// Max size in characters before persisting result to disk.
    #[serde(default = "default_max_result_size_chars")]
    pub max_result_size_chars: usize,
    /// Whether strict mode is enabled for input validation.
    #[serde(default)]
    pub strict: bool,
    /// Whether this is an MCP tool.
    #[serde(default)]
    pub is_mcp: bool,
    /// Whether this is an LSP tool.
    #[serde(default)]
    pub is_lsp: bool,
    /// MCP server/tool names (only set when `is_mcp` is true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_info: Option<McpToolInfo>,
    /// How the tool reacts to an interrupt (new user message).
    #[serde(default)]
    pub interrupt_behavior: InterruptBehavior,
}

fn default_max_result_size_chars() -> usize {
    50_000
}

impl Default for RichToolSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            protocol_name: String::new(),
            permission_tool_name: String::new(),
            description: String::new(),
            requires_permission: false,
            input_schema: Value::Object(serde_json::Map::new()),
            aliases: Vec::new(),
            search_hint: None,
            is_concurrency_safe: false,
            is_read_only: false,
            is_destructive: false,
            should_defer: false,
            always_load: false,
            max_result_size_chars: default_max_result_size_chars(),
            strict: false,
            is_mcp: false,
            is_lsp: false,
            mcp_info: None,
            interrupt_behavior: InterruptBehavior::default(),
        }
    }
}

impl RichToolSpec {
    /// Convert to an OpenAI-compatible function-calling schema.
    #[must_use]
    pub fn to_openai_schema(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema,
            }
        })
    }

    /// Convert to an Anthropic-compatible tool-use schema.
    #[must_use]
    pub fn to_anthropic_schema(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
        })
    }

    /// Build a [`RichToolSpec`] from the legacy [`crate::ToolSpec`].
    pub fn from_legacy(spec: &crate::ToolSpec) -> Self {
        Self {
            name: spec.name.clone(),
            protocol_name: spec.protocol_name.clone(),
            permission_tool_name: spec.permission_tool_name.clone(),
            description: spec.description.clone(),
            requires_permission: spec.requires_permission,
            input_schema: spec.input_schema.clone(),
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// ThinkingConfigInfo
// ---------------------------------------------------------------------------

/// Configuration for extended thinking / reasoning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingConfigInfo {
    /// Whether extended thinking is enabled.
    pub enabled: bool,
    /// Maximum budget in tokens for thinking.
    #[serde(default)]
    pub budget_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// ToolDecision
// ---------------------------------------------------------------------------

/// Cached permission decision for a specific tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolDecision {
    /// Tool execution is allowed.
    Allowed,
    /// Tool execution is denied with a reason.
    Denied { reason: String },
    /// User was prompted and chose to allow for the session.
    AllowedForSession,
}

// ---------------------------------------------------------------------------
// FileReadingLimits
// ---------------------------------------------------------------------------

/// Limits applied to file-reading operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadingLimits {
    /// Maximum number of lines to read from a single file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<usize>,
    /// Maximum number of files that can be read in a single turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files_per_turn: Option<usize>,
}

// ---------------------------------------------------------------------------
// GlobLimits
// ---------------------------------------------------------------------------

/// Limits applied to glob/search operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobLimits {
    /// Maximum number of results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
}

// ---------------------------------------------------------------------------
// ContentReplacementState
// ---------------------------------------------------------------------------

/// Tracks content replacements (e.g. image → placeholder) for the session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentReplacementState {
    /// Map from original content hash to replacement text.
    #[serde(default)]
    pub replacements: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// QueryChainTracking
// ---------------------------------------------------------------------------

/// Tracks tool-to-tool chaining within a single query turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryChainTracking {
    /// Ordered list of tool names invoked in this turn.
    #[serde(default)]
    pub chain: Vec<String>,
    /// Maximum allowed chain depth.
    #[serde(default)]
    pub max_depth: usize,
}

impl QueryChainTracking {
    /// Create a new tracker with the given maximum depth.
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self {
            chain: Vec::new(),
            max_depth,
        }
    }

    /// Push a tool name onto the chain, returning `false` if depth is exceeded.
    pub fn push(&mut self, tool_name: &str) -> bool {
        if self.chain.len() >= self.max_depth {
            return false;
        }
        self.chain.push(tool_name.to_owned());
        true
    }
}

// ---------------------------------------------------------------------------
// McpClientRef
// ---------------------------------------------------------------------------

/// A reference to an MCP client connection.
/// Uses `Arc<dyn Any + Send + Sync>` to avoid coupling to a specific MCP client type.
pub type McpClientRef = Arc<dyn std::any::Any + Send + Sync>;

// ---------------------------------------------------------------------------
// MessageRef
// ---------------------------------------------------------------------------

/// A reference-counted message in the conversation history.
pub type MessageRef = Arc<rc_core::Message>;

// ---------------------------------------------------------------------------
// RichToolUseContext
// ---------------------------------------------------------------------------

/// Full execution context for tool invocations.
///
/// Mirrors the fields from Claude Code's `Tool.ts` (lines 158–300).
#[derive(Clone)]
pub struct RichToolUseContext {
    /// Unique session identifier.
    pub session_id: String,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// Active permission mode.
    pub permission_mode: PermissionMode,
    /// Thinking / reasoning configuration.
    pub thinking_config: ThinkingConfigInfo,
    /// Active MCP client connections.
    pub mcp_clients: Vec<McpClientRef>,
    /// Whether the session is non-interactive (no TTY).
    pub is_non_interactive: bool,
    /// Handle to abort a running tool.
    pub abort_signal: Option<Arc<tokio::task::AbortHandle>>,
    /// Conversation messages visible to the tool.
    pub messages: Vec<MessageRef>,
    /// Cached permission decisions keyed by tool call ID.
    pub tool_decisions: HashMap<String, ToolDecision>,
    /// Limits for file-reading operations.
    pub file_reading_limits: Option<FileReadingLimits>,
    /// Limits for glob/search operations.
    pub glob_limits: Option<GlobLimits>,
    /// Content replacement state for the session.
    pub content_replacement_state: Option<ContentReplacementState>,
    /// Query-chain tracking for the current turn.
    pub query_chain_tracking: Option<QueryChainTracking>,
}

impl Default for RichToolUseContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            cwd: PathBuf::new(),
            model: String::new(),
            permission_mode: PermissionMode::default(),
            thinking_config: ThinkingConfigInfo::default(),
            mcp_clients: Vec::new(),
            is_non_interactive: false,
            abort_signal: None,
            messages: Vec::new(),
            tool_decisions: HashMap::new(),
            file_reading_limits: None,
            glob_limits: None,
            content_replacement_state: None,
            query_chain_tracking: None,
        }
    }
}

impl std::fmt::Debug for RichToolUseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RichToolUseContext")
            .field("session_id", &self.session_id)
            .field("cwd", &self.cwd)
            .field("model", &self.model)
            .field("permission_mode", &self.permission_mode)
            .field("thinking_config", &self.thinking_config)
            .field("mcp_clients_count", &self.mcp_clients.len())
            .field("is_non_interactive", &self.is_non_interactive)
            .field("abort_signal", &self.abort_signal.is_some())
            .field("messages_count", &self.messages.len())
            .field("tool_decisions_count", &self.tool_decisions.len())
            .field("file_reading_limits", &self.file_reading_limits)
            .field("glob_limits", &self.glob_limits)
            .field(
                "content_replacement_state",
                &self.content_replacement_state.is_some(),
            )
            .field("query_chain_tracking", &self.query_chain_tracking.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ValidationResult
// ---------------------------------------------------------------------------

/// Result of validating tool input against its schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationResult {
    /// Input is valid.
    Valid,
    /// Input is invalid with a message and error code.
    Invalid { message: String, error_code: u32 },
}

impl ValidationResult {
    /// Returns `true` if the validation passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Create an invalid result.
    #[must_use]
    pub fn invalid(message: impl Into<String>, error_code: u32) -> Self {
        Self::Invalid {
            message: message.into(),
            error_code,
        }
    }
}

// ---------------------------------------------------------------------------
// AdditionalWorkingDirectory
// ---------------------------------------------------------------------------

/// Metadata for an additional working directory beyond the primary cwd.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdditionalWorkingDirectory {
    /// Absolute path of the directory.
    pub path: PathBuf,
    /// Whether the directory is read-only.
    #[serde(default)]
    pub read_only: bool,
    /// Optional label for display purposes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

// ---------------------------------------------------------------------------
// RichToolPermissionContext
// ---------------------------------------------------------------------------

/// Permission evaluation context for the enhanced tool pipeline.
///
/// This is a richer version of [`rc_core::state::ToolPermissionContext`] that
/// includes rule-based evaluation, bypass availability, and auto-mode state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RichToolPermissionContext {
    /// Active permission mode.
    #[serde(default)]
    pub mode: PermissionMode,
    /// Additional working directories beyond the primary cwd.
    #[serde(default)]
    pub additional_working_directories: HashMap<String, AdditionalWorkingDirectory>,
    /// Rules that always allow (skip prompting).
    #[serde(default)]
    pub always_allow_rules: Vec<PermissionRule>,
    /// Rules that always deny.
    #[serde(default)]
    pub always_deny_rules: Vec<PermissionRule>,
    /// Rules that always ask the user.
    #[serde(default)]
    pub always_ask_rules: Vec<PermissionRule>,
    /// Whether bypass mode is available.
    #[serde(default)]
    pub is_bypass_available: bool,
    /// Whether auto mode is available.
    #[serde(default)]
    pub is_auto_mode_available: bool,
    /// Whether permission prompts should be suppressed.
    #[serde(default)]
    pub should_avoid_permission_prompts: bool,
}

impl RichToolPermissionContext {
    /// Create a new permission context with the given mode.
    #[must_use]
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Check whether a tool is allowed by the always-allow rules.
    pub fn is_always_allowed(&self, tool_name: &str) -> bool {
        self.always_allow_rules
            .iter()
            .any(|rule| rule.tool_name == tool_name)
    }

    /// Check whether a tool is denied by the always-deny rules.
    pub fn is_always_denied(&self, tool_name: &str) -> bool {
        self.always_deny_rules
            .iter()
            .any(|rule| rule.tool_name == tool_name)
    }

    /// Evaluate the permission for a tool name, returning the effective behavior.
    pub fn evaluate(&self, tool_name: &str) -> PermissionBehavior {
        if self.is_always_denied(tool_name) {
            return PermissionBehavior::Deny;
        }
        if self.is_always_allowed(tool_name) {
            return PermissionBehavior::Allow;
        }
        // Check always-ask rules
        if self
            .always_ask_rules
            .iter()
            .any(|rule| rule.tool_name == tool_name)
        {
            return PermissionBehavior::Ask;
        }
        // Default behavior based on mode
        match self.mode {
            PermissionMode::BypassPermissions => PermissionBehavior::Allow,
            PermissionMode::DontAsk => PermissionBehavior::Allow,
            PermissionMode::Plan => PermissionBehavior::Deny,
            _ => PermissionBehavior::Ask,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolExecutionRequest
// ---------------------------------------------------------------------------

/// Request to execute a tool through the pipeline.
#[derive(Debug, Clone)]
pub struct ToolExecutionRequest {
    /// The tool call from the provider.
    pub tool_call: ToolCall,
    /// The rich spec for the tool being invoked.
    pub spec: RichToolSpec,
    /// Execution context.
    pub context: RichToolUseContext,
    /// Permission context.
    pub permission_context: RichToolPermissionContext,
}

// ---------------------------------------------------------------------------
// ToolExecutionResult
// ---------------------------------------------------------------------------

/// Result of executing a tool through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// The tool call ID.
    pub tool_call_id: String,
    /// The tool name.
    pub tool_name: String,
    /// Output content.
    pub content: String,
    /// Whether the execution resulted in an error.
    pub is_error: bool,
    /// Duration of execution in milliseconds.
    #[serde(default)]
    pub duration_ms: u64,
    /// Whether the result was persisted to disk due to size.
    #[serde(default)]
    pub was_persisted: bool,
    /// Path the result was persisted to (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persisted_path: Option<PathBuf>,
}

impl ToolExecutionResult {
    /// Create a successful result.
    #[must_use]
    pub fn success(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            content: content.into(),
            is_error: false,
            duration_ms: 0,
            was_persisted: false,
            persisted_path: None,
        }
    }

    /// Create an error result.
    #[must_use]
    pub fn error(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            content: message.into(),
            is_error: true,
            duration_ms: 0,
            was_persisted: false,
            persisted_path: None,
        }
    }

    /// Convert to a legacy [`ToolResult`].
    #[must_use]
    pub fn to_tool_result(&self) -> ToolResult {
        ToolResult {
            content: self.content.clone(),
            is_error: self.is_error,
        }
    }
}

// ---------------------------------------------------------------------------
// PreToolHook / PostToolHook
// ---------------------------------------------------------------------------

/// Outcome of a pre-tool hook.
#[derive(Debug, Clone)]
pub enum PreToolHookOutcome {
    /// Allow the tool to proceed.
    Proceed,
    /// Block the tool with a message.
    Block(String),
}

/// Outcome of a post-tool hook.
#[derive(Debug, Clone)]
pub enum PostToolHookOutcome {
    /// Accept the result as-is.
    Accept,
    /// Replace the result content.
    Replace(String),
}

// ---------------------------------------------------------------------------
// ToolExecutor trait
// ---------------------------------------------------------------------------

/// Trait for executing a tool given its input.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute the tool with the given input and context.
    async fn execute(&self, input: &Value, context: &RichToolUseContext) -> Result<String>;
}

// ---------------------------------------------------------------------------
// ToolExecutionPipeline
// ---------------------------------------------------------------------------

/// Full execution pipeline: validate → pre-hooks → permission check → execute → post-hooks → summarize.
pub struct ToolExecutionPipeline {
    /// Tool executor implementation.
    executor: Arc<dyn ToolExecutor>,
    /// Pre-tool hooks.
    pre_hooks: Vec<Arc<dyn PreToolHook + Send + Sync>>,
    /// Post-tool hooks.
    post_hooks: Vec<Arc<dyn PostToolHook + Send + Sync>>,
}

/// Pre-tool hook trait.
pub trait PreToolHook {
    /// Run the pre-tool hook.
    fn run(&self, request: &ToolExecutionRequest) -> PreToolHookOutcome;
}

/// Post-tool hook trait.
pub trait PostToolHook {
    /// Run the post-tool hook.
    fn run(&self, result: &ToolExecutionResult) -> PostToolHookOutcome;
}

impl ToolExecutionPipeline {
    /// Create a new pipeline with the given executor.
    #[must_use]
    pub fn new(executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            executor,
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
        }
    }

    /// Add a pre-tool hook.
    pub fn add_pre_hook(&mut self, hook: Arc<dyn PreToolHook + Send + Sync>) {
        self.pre_hooks.push(hook);
    }

    /// Add a post-tool hook.
    pub fn add_post_hook(&mut self, hook: Arc<dyn PostToolHook + Send + Sync>) {
        self.post_hooks.push(hook);
    }

    /// Full pipeline: validate → pre-hooks → permission check → execute → post-hooks → summarize.
    ///
    /// # Errors
    /// Returns an error if validation fails, permission is denied, or execution fails.
    pub async fn execute_tool(&self, request: ToolExecutionRequest) -> ToolExecutionResult {
        let tool_call_id = request.tool_call.id.clone();
        let tool_name = request.tool_call.name.clone();
        let max_size = request.spec.max_result_size_chars;

        // 1. Validate input
        let validation = self.validate_input(&request.spec, &request.tool_call.input);
        if let ValidationResult::Invalid {
            message,
            error_code,
        } = validation
        {
            return ToolExecutionResult {
                tool_call_id,
                tool_name,
                content: format!("Validation error (code {error_code}): {message}"),
                is_error: true,
                duration_ms: 0,
                was_persisted: false,
                persisted_path: None,
            };
        }

        // 2. Run pre-tool hooks
        for hook in &self.pre_hooks {
            match hook.run(&request) {
                PreToolHookOutcome::Proceed => {}
                PreToolHookOutcome::Block(msg) => {
                    return ToolExecutionResult {
                        tool_call_id,
                        tool_name,
                        content: msg,
                        is_error: true,
                        duration_ms: 0,
                        was_persisted: false,
                        persisted_path: None,
                    };
                }
            }
        }

        // 3. Check permissions
        let perm_decision = request.permission_context.evaluate(&tool_name);
        if perm_decision == PermissionBehavior::Deny {
            return ToolExecutionResult {
                tool_call_id,
                tool_name,
                content: "Permission denied for this tool.".to_owned(),
                is_error: true,
                duration_ms: 0,
                was_persisted: false,
                persisted_path: None,
            };
        }

        // 4. Execute the tool
        let start = std::time::Instant::now();
        let execution_result = self
            .executor
            .execute(&request.tool_call.input, &request.context)
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let content = match execution_result {
            Ok(c) => c,
            Err(e) => {
                return ToolExecutionResult {
                    tool_call_id,
                    tool_name,
                    content: e.to_string(),
                    is_error: true,
                    duration_ms,
                    was_persisted: false,
                    persisted_path: None,
                };
            }
        };

        let mut result = ToolExecutionResult {
            tool_call_id,
            tool_name,
            content,
            is_error: false,
            duration_ms,
            was_persisted: false,
            persisted_path: None,
        };

        // 5. Run post-tool hooks
        for hook in &self.post_hooks {
            match hook.run(&result) {
                PostToolHookOutcome::Accept => {}
                PostToolHookOutcome::Replace(new_content) => {
                    result.content = new_content;
                }
            }
        }

        // 6. Summarize result (truncate if needed)
        result.content = self.summarize_result(&result.content, max_size);

        // 7. Persist large results to disk if needed
        if result.content.len() > max_size {
            if let Ok(path) = self.persist_result(&result.content, &result.tool_call_id) {
                result.was_persisted = true;
                result.persisted_path = Some(path);
                result.content = format!(
                    "Result too large ({} chars), persisted to disk. See persisted_path.",
                    result.content.len()
                );
            }
        }

        result
    }

    /// Validate tool input against the spec's schema.
    pub fn validate_input(&self, spec: &RichToolSpec, input: &Value) -> ValidationResult {
        // Basic structural validation: if the schema says required fields, check them.
        if spec.strict {
            if let Some(properties) = spec.input_schema.get("properties") {
                if let Some(required) = spec.input_schema.get("required") {
                    if let Some(required_arr) = required.as_array() {
                        for field in required_arr {
                            if let Some(field_name) = field.as_str() {
                                // Check if the property exists in the input
                                if !properties
                                    .get(field_name)
                                    .map_or(false, |_| input.get(field_name).is_some())
                                {
                                    // Property is defined in schema but missing from input
                                    if input.get(field_name).is_none() {
                                        return ValidationResult::Invalid {
                                            message: format!(
                                                "Missing required field: {field_name}"
                                            ),
                                            error_code: 4001,
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check that input is an object (tools expect JSON objects)
        if !input.is_object() && !input.is_null() {
            return ValidationResult::Invalid {
                message: "Tool input must be a JSON object.".to_owned(),
                error_code: 4002,
            };
        }

        ValidationResult::Valid
    }

    /// Summarize/truncate result content if it exceeds the limit.
    fn summarize_result(&self, content: &str, max_chars: usize) -> String {
        if content.len() <= max_chars {
            return content.to_owned();
        }

        let truncation_notice = "\n\n[... truncated ...]";
        let budget = max_chars.saturating_sub(truncation_notice.len());
        let mut truncated = String::with_capacity(budget + truncation_notice.len());
        truncated.push_str(&content[..budget]);
        truncated.push_str(truncation_notice);
        truncated
    }

    /// Persist a large result to a temporary file.
    fn persist_result(&self, content: &str, tool_call_id: &str) -> Result<PathBuf> {
        let dir = std::env::temp_dir().join("remote-code-tool-results");
        std::fs::create_dir_all(&dir)?;
        let filename = format!("tool-result-{}.txt", tool_call_id);
        let path = dir.join(filename);
        std::fs::write(&path, content)?;
        Ok(path)
    }
}

// ---------------------------------------------------------------------------
// Tool matching functions
// ---------------------------------------------------------------------------

/// Check whether a tool spec matches a given name (primary name or alias).
pub fn tool_matches_name(tool: &RichToolSpec, name: &str) -> bool {
    if tool.name == name || tool.protocol_name == name {
        return true;
    }
    tool.aliases.iter().any(|alias| alias == name)
}

/// Find a tool spec by name in a list, checking primary name, protocol name, and aliases.
pub fn find_tool_by_name<'a>(tools: &'a [RichToolSpec], name: &str) -> Option<&'a RichToolSpec> {
    // First pass: exact match on primary name
    if let Some(tool) = tools.iter().find(|t| t.name == name) {
        return Some(tool);
    }
    // Second pass: protocol name
    if let Some(tool) = tools.iter().find(|t| t.protocol_name == name) {
        return Some(tool);
    }
    // Third pass: aliases
    tools.iter().find(|t| t.aliases.iter().any(|a| a == name))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::PermissionMode;
    use rc_core::permission_types::{PermissionBehavior, PermissionRuleSource};
    use serde_json::json;

    // ── Helper builders ───────────────────────────────────────────────────

    fn make_spec(name: &str) -> RichToolSpec {
        RichToolSpec {
            name: name.to_owned(),
            protocol_name: name.to_owned(),
            permission_tool_name: name.to_owned(),
            description: format!("Tool: {name}"),
            requires_permission: false,
            input_schema: json!({"type": "object", "properties": {}}),
            ..RichToolSpec::default()
        }
    }

    fn make_spec_with_aliases(name: &str, aliases: &[&str]) -> RichToolSpec {
        RichToolSpec {
            name: name.to_owned(),
            aliases: aliases.iter().map(|s| (*s).to_owned()).collect(),
            ..make_spec(name)
        }
    }

    fn make_read_only_spec(name: &str) -> RichToolSpec {
        RichToolSpec {
            is_read_only: true,
            ..make_spec(name)
        }
    }

    fn make_destructive_spec(name: &str) -> RichToolSpec {
        RichToolSpec {
            is_destructive: true,
            requires_permission: true,
            ..make_spec(name)
        }
    }

    fn make_mcp_spec(name: &str, server: &str, tool: &str) -> RichToolSpec {
        RichToolSpec {
            is_mcp: true,
            mcp_info: Some(McpToolInfo {
                server_name: server.to_owned(),
                tool_name: tool.to_owned(),
            }),
            ..make_spec(name)
        }
    }

    fn make_strict_spec(name: &str) -> RichToolSpec {
        RichToolSpec {
            strict: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
            ..make_spec(name)
        }
    }

    fn make_context() -> RichToolUseContext {
        RichToolUseContext::default()
    }

    fn make_permission_context(mode: PermissionMode) -> RichToolPermissionContext {
        RichToolPermissionContext::new(mode)
    }

    // ── 1. RichToolSpec construction ──────────────────────────────────────

    #[test]
    fn rich_tool_spec_default_values() {
        let spec = RichToolSpec::default();
        assert!(spec.name.is_empty());
        assert!(spec.aliases.is_empty());
        assert!(!spec.is_concurrency_safe);
        assert!(!spec.is_read_only);
        assert!(!spec.is_destructive);
        assert!(!spec.should_defer);
        assert!(!spec.always_load);
        assert!(!spec.strict);
        assert!(!spec.is_mcp);
        assert!(!spec.is_lsp);
        assert!(spec.mcp_info.is_none());
        assert_eq!(spec.interrupt_behavior, InterruptBehavior::Cancel);
        assert_eq!(spec.max_result_size_chars, 50_000);
    }

    #[test]
    fn rich_tool_spec_serialization_roundtrip() {
        let spec = make_mcp_spec("mcp_search", "my_server", "search");
        let json = serde_json::to_string(&spec).expect("serialize");
        let deserialized: RichToolSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.name, "mcp_search");
        assert!(deserialized.is_mcp);
        assert_eq!(
            deserialized.mcp_info.as_ref().unwrap().server_name,
            "my_server"
        );
    }

    #[test]
    fn rich_tool_spec_from_legacy() {
        let legacy = crate::ToolSpec {
            name: "read_file".to_owned(),
            protocol_name: "Read".to_owned(),
            permission_tool_name: "read_file".to_owned(),
            description: "Read a file".to_owned(),
            requires_permission: false,
            input_schema: json!({"type": "object"}),
        };
        let rich = RichToolSpec::from_legacy(&legacy);
        assert_eq!(rich.name, "read_file");
        assert_eq!(rich.protocol_name, "Read");
        assert!(!rich.is_mcp);
        assert!(rich.aliases.is_empty());
    }

    #[test]
    fn rich_tool_spec_openai_schema() {
        let spec = make_spec("bash_command");
        let schema = spec.to_openai_schema();
        assert_eq!(schema["type"], "function");
        assert_eq!(schema["function"]["name"], "bash_command");
    }

    #[test]
    fn rich_tool_spec_anthropic_schema() {
        let spec = make_spec("glob");
        let schema = spec.to_anthropic_schema();
        assert_eq!(schema["name"], "glob");
        assert!(schema["input_schema"].is_object());
    }

    // ── 2. InterruptBehavior ──────────────────────────────────────────────

    #[test]
    fn interrupt_behavior_default_is_cancel() {
        assert_eq!(InterruptBehavior::default(), InterruptBehavior::Cancel);
    }

    #[test]
    fn interrupt_behavior_serialization() {
        let cancel = serde_json::to_string(&InterruptBehavior::Cancel).unwrap();
        assert_eq!(cancel, "\"cancel\"");
        let block = serde_json::to_string(&InterruptBehavior::Block).unwrap();
        assert_eq!(block, "\"block\"");
    }

    // ── 3. McpToolInfo ────────────────────────────────────────────────────

    #[test]
    fn mcp_tool_info_serialization() {
        let info = McpToolInfo {
            server_name: "github".to_owned(),
            tool_name: "create_issue".to_owned(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.server_name, "github");
        assert_eq!(parsed.tool_name, "create_issue");
    }

    // ── 4. ValidationResult ──────────────────────────────────────────────

    #[test]
    fn validation_result_valid() {
        let v = ValidationResult::Valid;
        assert!(v.is_valid());
    }

    #[test]
    fn validation_result_invalid() {
        let v = ValidationResult::invalid("bad input", 4001);
        assert!(!v.is_valid());
        if let ValidationResult::Invalid {
            message,
            error_code,
        } = v
        {
            assert_eq!(message, "bad input");
            assert_eq!(error_code, 4001);
        } else {
            panic!("expected Invalid");
        }
    }

    #[test]
    fn validation_result_serialization() {
        let v = ValidationResult::invalid("missing field", 4001);
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("missing field"));
        let parsed: ValidationResult = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_valid());
    }

    // ── 5. Tool matching ──────────────────────────────────────────────────

    #[test]
    fn tool_matches_name_primary() {
        let spec = make_spec("read_file");
        assert!(tool_matches_name(&spec, "read_file"));
    }

    #[test]
    fn tool_matches_name_protocol() {
        let spec = RichToolSpec {
            protocol_name: "Read".to_owned(),
            ..make_spec("read_file")
        };
        assert!(tool_matches_name(&spec, "Read"));
    }

    #[test]
    fn tool_matches_name_alias() {
        let spec = make_spec_with_aliases("bash_command", &["bash", "shell"]);
        assert!(tool_matches_name(&spec, "bash"));
        assert!(tool_matches_name(&spec, "shell"));
        assert!(!tool_matches_name(&spec, "unknown"));
    }

    #[test]
    fn find_tool_by_name_returns_primary() {
        let tools = vec![make_spec("read_file"), make_spec("write_file")];
        let found = find_tool_by_name(&tools, "read_file");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "read_file");
    }

    #[test]
    fn find_tool_by_name_returns_none_for_unknown() {
        let tools = vec![make_spec("read_file")];
        assert!(find_tool_by_name(&tools, "nonexistent").is_none());
    }

    #[test]
    fn find_tool_by_name_finds_alias() {
        let tools = vec![make_spec_with_aliases("bash_command", &["bash"])];
        let found = find_tool_by_name(&tools, "bash");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "bash_command");
    }

    // ── 6. RichToolPermissionContext ──────────────────────────────────────

    #[test]
    fn permission_context_default_is_ask() {
        let ctx = RichToolPermissionContext::default();
        assert_eq!(ctx.evaluate("anything"), PermissionBehavior::Ask);
    }

    #[test]
    fn permission_context_bypass_allows() {
        let ctx = make_permission_context(PermissionMode::BypassPermissions);
        assert_eq!(ctx.evaluate("dangerous_tool"), PermissionBehavior::Allow);
    }

    #[test]
    fn permission_context_dont_ask_allows() {
        let ctx = make_permission_context(PermissionMode::DontAsk);
        assert_eq!(ctx.evaluate("some_tool"), PermissionBehavior::Allow);
    }

    #[test]
    fn permission_context_plan_denies() {
        let ctx = make_permission_context(PermissionMode::Plan);
        assert_eq!(ctx.evaluate("some_tool"), PermissionBehavior::Deny);
    }

    #[test]
    fn permission_context_always_allow_overrides() {
        let mut ctx = RichToolPermissionContext::default();
        ctx.always_allow_rules.push(PermissionRule {
            source: PermissionRuleSource::UserSettings,
            behavior: PermissionBehavior::Allow,
            tool_name: "read_file".to_owned(),
            rule_content: None,
        });
        assert_eq!(ctx.evaluate("read_file"), PermissionBehavior::Allow);
    }

    #[test]
    fn permission_context_always_deny_overrides() {
        let mut ctx = make_permission_context(PermissionMode::BypassPermissions);
        ctx.always_deny_rules.push(PermissionRule {
            source: PermissionRuleSource::PolicySettings,
            behavior: PermissionBehavior::Deny,
            tool_name: "rm_rf".to_owned(),
            rule_content: None,
        });
        // Deny takes precedence even in bypass mode
        assert_eq!(ctx.evaluate("rm_rf"), PermissionBehavior::Deny);
    }

    // ── 7. QueryChainTracking ─────────────────────────────────────────────

    #[test]
    fn query_chain_tracking_push_and_limit() {
        let mut tracker = QueryChainTracking::new(3);
        assert!(tracker.push("tool_a"));
        assert!(tracker.push("tool_b"));
        assert!(tracker.push("tool_c"));
        assert!(!tracker.push("tool_d")); // exceeds max_depth
        assert_eq!(tracker.chain, vec!["tool_a", "tool_b", "tool_c"]);
    }

    // ── 8. ToolExecutionResult ────────────────────────────────────────────

    #[test]
    fn tool_execution_result_success() {
        let result = ToolExecutionResult::success("id1", "read_file", "file contents");
        assert!(!result.is_error);
        assert_eq!(result.tool_call_id, "id1");
        assert_eq!(result.tool_name, "read_file");
        assert_eq!(result.content, "file contents");
    }

    #[test]
    fn tool_execution_result_error() {
        let result = ToolExecutionResult::error("id2", "bash_command", "command failed");
        assert!(result.is_error);
        assert_eq!(result.content, "command failed");
    }

    #[test]
    fn tool_execution_result_to_tool_result() {
        let result = ToolExecutionResult::success("id3", "glob", "*.rs");
        let tool_result = result.to_tool_result();
        assert_eq!(tool_result.content, "*.rs");
        assert!(!tool_result.is_error);
    }

    // ── 9. ThinkingConfigInfo ─────────────────────────────────────────────

    #[test]
    fn thinking_config_default() {
        let config = ThinkingConfigInfo::default();
        assert!(!config.enabled);
        assert!(config.budget_tokens.is_none());
    }

    // ── 10. FileReadingLimits / GlobLimits ────────────────────────────────

    #[test]
    fn file_reading_limits_default() {
        let limits = FileReadingLimits::default();
        assert!(limits.max_lines.is_none());
        assert!(limits.max_files_per_turn.is_none());
    }

    #[test]
    fn glob_limits_default() {
        let limits = GlobLimits::default();
        assert!(limits.max_results.is_none());
    }

    // ── 11. ContentReplacementState ───────────────────────────────────────

    #[test]
    fn content_replacement_state_default() {
        let state = ContentReplacementState::default();
        assert!(state.replacements.is_empty());
    }

    // ── 12. Pipeline validation ───────────────────────────────────────────

    /// A simple executor that echoes back the input as a string.
    struct EchoExecutor;

    #[async_trait::async_trait]
    impl ToolExecutor for EchoExecutor {
        async fn execute(&self, input: &Value, _context: &RichToolUseContext) -> Result<String> {
            Ok(format!("{input}"))
        }
    }

    #[tokio::test]
    async fn pipeline_validates_strict_schema() {
        let spec = make_strict_spec("strict_tool");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));

        // Missing required field "path"
        let result = pipeline
            .execute_tool(ToolExecutionRequest {
                tool_call: ToolCall {
                    id: "1".to_owned(),
                    name: "strict_tool".to_owned(),
                    input: json!({}),
                },
                spec,
                context: make_context(),
                permission_context: make_permission_context(PermissionMode::BypassPermissions),
            })
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("Missing required field: path"));
    }

    #[tokio::test]
    async fn pipeline_executes_successfully() {
        let spec = make_spec("echo_tool");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));

        let result = pipeline
            .execute_tool(ToolExecutionRequest {
                tool_call: ToolCall {
                    id: "2".to_owned(),
                    name: "echo_tool".to_owned(),
                    input: json!({"msg": "hello"}),
                },
                spec,
                context: make_context(),
                permission_context: make_permission_context(PermissionMode::BypassPermissions),
            })
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn pipeline_blocks_on_permission_deny() {
        let spec = make_destructive_spec("rm_rf");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));

        let result = pipeline
            .execute_tool(ToolExecutionRequest {
                tool_call: ToolCall {
                    id: "3".to_owned(),
                    name: "rm_rf".to_owned(),
                    input: json!({}),
                },
                spec,
                context: make_context(),
                permission_context: make_permission_context(PermissionMode::Plan),
            })
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("Permission denied"));
    }

    // ── 13. Pre/Post hooks ────────────────────────────────────────────────

    struct BlockingPreHook;
    impl PreToolHook for BlockingPreHook {
        fn run(&self, _request: &ToolExecutionRequest) -> PreToolHookOutcome {
            PreToolHookOutcome::Block("blocked by pre-hook".to_owned())
        }
    }

    struct ReplacingPostHook;
    impl PostToolHook for ReplacingPostHook {
        fn run(&self, _result: &ToolExecutionResult) -> PostToolHookOutcome {
            PostToolHookOutcome::Replace("replaced content".to_owned())
        }
    }

    #[tokio::test]
    async fn pipeline_pre_hook_blocks_execution() {
        let spec = make_spec("hooked_tool");
        let mut pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        pipeline.add_pre_hook(Arc::new(BlockingPreHook));

        let result = pipeline
            .execute_tool(ToolExecutionRequest {
                tool_call: ToolCall {
                    id: "4".to_owned(),
                    name: "hooked_tool".to_owned(),
                    input: json!({}),
                },
                spec,
                context: make_context(),
                permission_context: make_permission_context(PermissionMode::BypassPermissions),
            })
            .await;

        assert!(result.is_error);
        assert_eq!(result.content, "blocked by pre-hook");
    }

    #[tokio::test]
    async fn pipeline_post_hook_replaces_content() {
        let spec = make_spec("post_hook_tool");
        let mut pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        pipeline.add_post_hook(Arc::new(ReplacingPostHook));

        let result = pipeline
            .execute_tool(ToolExecutionRequest {
                tool_call: ToolCall {
                    id: "5".to_owned(),
                    name: "post_hook_tool".to_owned(),
                    input: json!({"data": "original"}),
                },
                spec,
                context: make_context(),
                permission_context: make_permission_context(PermissionMode::BypassPermissions),
            })
            .await;

        assert!(!result.is_error);
        assert_eq!(result.content, "replaced content");
    }

    // ── 14. AdditionalWorkingDirectory ────────────────────────────────────

    #[test]
    fn additional_working_directory_serialization() {
        let awd = AdditionalWorkingDirectory {
            path: PathBuf::from("/tmp/workspace"),
            read_only: true,
            label: Some("temp workspace".to_owned()),
        };
        let json = serde_json::to_string(&awd).unwrap();
        let parsed: AdditionalWorkingDirectory = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, PathBuf::from("/tmp/workspace"));
        assert!(parsed.read_only);
        assert_eq!(parsed.label.as_deref(), Some("temp workspace"));
    }

    // ── 15. RichToolUseContext debug ──────────────────────────────────────

    #[test]
    fn rich_tool_use_context_debug_format() {
        let ctx = make_context();
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("RichToolUseContext"));
        assert!(debug_str.contains("session_id"));
    }

    // ── 16. ToolDecision ──────────────────────────────────────────────────

    #[test]
    fn tool_decision_serialization() {
        let allowed = ToolDecision::Allowed;
        let denied = ToolDecision::Denied {
            reason: "unsafe".to_owned(),
        };
        let session = ToolDecision::AllowedForSession;

        let a = serde_json::to_string(&allowed).unwrap();
        let d = serde_json::to_string(&denied).unwrap();
        let s = serde_json::to_string(&session).unwrap();

        assert!(a.contains("Allowed"));
        assert!(d.contains("unsafe"));
        assert!(s.contains("AllowedForSession"));
    }

    // ── 17. Pipeline summarize_result ─────────────────────────────────────

    #[test]
    fn summarize_result_truncates_long_content() {
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        let long_content = "x".repeat(100);
        let summarized = pipeline.summarize_result(&long_content, 50);
        assert!(summarized.len() <= 100); // well within bounds
        assert!(summarized.contains("truncated"));
    }

    #[test]
    fn summarize_result_keeps_short_content() {
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        let content = "short".to_owned();
        let summarized = pipeline.summarize_result(&content, 100);
        assert_eq!(summarized, "short");
    }

    // ── 18. Pipeline validate_input ───────────────────────────────────────

    #[test]
    fn validate_input_rejects_non_object() {
        let spec = make_spec("test");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        let result = pipeline.validate_input(&spec, &json!("not an object"));
        assert!(!result.is_valid());
    }

    #[test]
    fn validate_input_accepts_object() {
        let spec = make_spec("test");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        let result = pipeline.validate_input(&spec, &json!({"key": "value"}));
        assert!(result.is_valid());
    }

    #[test]
    fn validate_input_accepts_null() {
        let spec = make_spec("test");
        let pipeline = ToolExecutionPipeline::new(Arc::new(EchoExecutor));
        let result = pipeline.validate_input(&spec, &Value::Null);
        assert!(result.is_valid());
    }

    // ── 19. RichToolSpec with all flags ───────────────────────────────────

    #[test]
    fn rich_tool_spec_all_flags_set() {
        let spec = RichToolSpec {
            name: "super_tool".to_owned(),
            protocol_name: "SuperTool".to_owned(),
            permission_tool_name: "super_tool".to_owned(),
            description: "Does everything".to_owned(),
            requires_permission: true,
            input_schema: json!({"type": "object"}),
            aliases: vec!["st".to_owned(), "supertool".to_owned()],
            search_hint: Some("A tool that does everything".to_owned()),
            is_concurrency_safe: true,
            is_read_only: false,
            is_destructive: true,
            should_defer: false,
            always_load: true,
            max_result_size_chars: 100_000,
            strict: true,
            is_mcp: false,
            is_lsp: false,
            mcp_info: None,
            interrupt_behavior: InterruptBehavior::Block,
        };

        assert_eq!(spec.aliases.len(), 2);
        assert!(spec.is_concurrency_safe);
        assert!(spec.is_destructive);
        assert!(spec.always_load);
        assert!(spec.strict);
        assert_eq!(spec.interrupt_behavior, InterruptBehavior::Block);
        assert_eq!(spec.max_result_size_chars, 100_000);
    }
}
