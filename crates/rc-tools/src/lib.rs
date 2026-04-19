//! Built-in tool registry and execution engine.
//!
//! Provides 30+ built-in tools (file I/O, search, shell, web, LSP, tasks, etc.)
//! with a [`ToolRegistry`] that supports BM25-based tool search and OpenAI /
//! Anthropic schema generation.

pub mod agent;
pub mod command;
pub mod delegate;
pub mod discover_skills;
pub mod file_ops;
pub mod git;
pub mod hooks;
pub mod lsp;
pub mod mcp_catalog;
pub mod mcp_runtime;
pub mod mcp_tools;
pub mod memory_tools;
pub mod misc;
pub mod plan_mode;
pub mod review_artifact;
pub mod runtime_plan_mode;
pub mod sandbox;
pub mod search;
pub mod send_message;
pub mod send_user_file;
pub mod shell;
pub mod specs;
pub mod streaming_executor;
pub mod system;
pub mod task_output;
pub mod tasks;
pub mod team_runtime;
pub mod team_tools;
pub mod tool_hooks;
pub mod tool_orchestration;
pub mod tool_progress;
pub mod tool_prompts;
pub mod tool_result_summary;
pub mod web;
pub mod web_browser;
pub mod workflow;
pub mod worktree_tools;

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use rc_core::task_stack::TaskStack;
use rc_core::{
    ConversationEntry, ConversationRole, HookEvent, HookShell, SubAgentCompletion, ToolCall,
    ToolResult,
};
use rc_permissions::{
    PermissionBroker, PermissionClass, PermissionRequest, auto_allows, classify_tool,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Re-export specs::builtin_tool_specs at crate root for backward compatibility.
pub use specs::builtin_tool_specs;

// Re-export hooks::execute_command_hook at crate root for backward compatibility.
pub use hooks::execute_command_hook;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "dist",
    "coverage",
    ".next",
];

static TOOL_RUNTIME_POLICY: Lazy<Mutex<ToolRuntimePolicy>> =
    Lazy::new(|| Mutex::new(ToolRuntimePolicy::default()));

tokio::task_local! {
    static TOOL_RUNTIME_POLICY_OVERLAY: ToolRuntimePolicyOverlay;
}

/// Process-scoped runtime policy for tool exposure and task artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMcpServerPolicyEntry {
    pub origin_kind: String,
    pub origin_name: String,
    pub config_path: PathBuf,
    pub server: rc_mcp::McpServerConfig,
}

/// Process-scoped runtime policy for tool exposure and task artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolRuntimePolicy {
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub task_output_dir: Option<PathBuf>,
    #[serde(default)]
    pub mcp_servers: Vec<RuntimeMcpServerPolicyEntry>,
    #[serde(default)]
    pub shell_policy: shell::ShellExecutionPolicy,
}

/// Task-local overlay for a single query/agent run.
///
/// This mirrors Claude Code's per-run tool-pool filtering without mutating the
/// process-wide policy, so concurrent background agents keep independent tool
/// surfaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolRuntimePolicyOverlay {
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
}

/// Configure the current process-wide tool policy.
///
/// # Errors
/// Returns an error if task-output persistence cannot be configured.
pub fn configure_tool_runtime_policy(policy: ToolRuntimePolicy) -> Result<()> {
    tasks::configure_task_output_dir(policy.task_output_dir.clone())?;
    let mut current = TOOL_RUNTIME_POLICY
        .lock()
        .map_err(|_| anyhow!("tool runtime policy lock poisoned"))?;
    *current = normalize_tool_runtime_policy(policy);
    Ok(())
}

/// Return the active process-wide tool policy.
#[must_use]
pub fn current_tool_runtime_policy() -> ToolRuntimePolicy {
    let base = TOOL_RUNTIME_POLICY
        .lock()
        .map(|policy| policy.clone())
        .unwrap_or_default();
    TOOL_RUNTIME_POLICY_OVERLAY
        .try_with(|overlay| apply_tool_runtime_policy_overlay(base.clone(), overlay.clone()))
        .unwrap_or(base)
}

pub async fn with_tool_runtime_policy_overlay<F, T>(
    overlay: ToolRuntimePolicyOverlay,
    future: F,
) -> T
where
    F: Future<Output = T>,
{
    TOOL_RUNTIME_POLICY_OVERLAY
        .scope(normalize_tool_runtime_policy_overlay(overlay), future)
        .await
}

fn apply_tool_runtime_policy_overlay(
    mut base: ToolRuntimePolicy,
    overlay: ToolRuntimePolicyOverlay,
) -> ToolRuntimePolicy {
    let overlay = normalize_tool_runtime_policy_overlay(overlay);
    if let Some(overlay_allowed) = overlay.allowed_tools {
        base.allowed_tools = if base.allowed_tools.is_empty() {
            overlay_allowed
        } else {
            base.allowed_tools
                .into_iter()
                .filter(|tool| overlay_allowed.contains(tool))
                .collect()
        };
    }
    base.disallowed_tools.extend(overlay.disallowed_tools);
    normalize_tool_runtime_policy(base)
}

fn normalize_tool_runtime_policy_overlay(
    mut overlay: ToolRuntimePolicyOverlay,
) -> ToolRuntimePolicyOverlay {
    overlay.allowed_tools = overlay.allowed_tools.map(|tools| {
        let mut normalized = tools
            .into_iter()
            .map(|tool| tool.trim().to_ascii_lowercase())
            .filter(|tool| !tool.is_empty())
            .collect::<Vec<_>>();
        normalized.sort();
        normalized.dedup();
        normalized
    });
    overlay.disallowed_tools = overlay
        .disallowed_tools
        .into_iter()
        .map(|tool| tool.trim().to_ascii_lowercase())
        .filter(|tool| !tool.is_empty())
        .collect::<Vec<_>>();
    overlay.disallowed_tools.sort();
    overlay.disallowed_tools.dedup();
    overlay
}

/// Return the built-in tool specs visible under the active runtime policy.
#[must_use]
pub fn runtime_builtin_tool_specs() -> Vec<ToolSpec> {
    let policy = current_tool_runtime_policy();
    builtin_tool_specs()
        .into_iter()
        .filter(|spec| tool_allowed_by_policy(&spec.name, &policy))
        .collect()
}

fn sort_tool_specs_by_name(specs: &mut [ToolSpec]) {
    specs.sort_by(|left, right| left.name.cmp(&right.name));
}

fn merge_provider_tool_specs(
    mut builtin_specs: Vec<ToolSpec>,
    mut dynamic_specs: Vec<ToolSpec>,
) -> Vec<ToolSpec> {
    sort_tool_specs_by_name(&mut builtin_specs);
    sort_tool_specs_by_name(&mut dynamic_specs);

    let mut merged = Vec::with_capacity(builtin_specs.len() + dynamic_specs.len());
    let mut seen = std::collections::BTreeSet::new();

    for spec in builtin_specs.into_iter().chain(dynamic_specs) {
        if seen.insert(spec.name.clone()) {
            merged.push(spec);
        }
    }

    merged
}

#[must_use]
pub fn is_runtime_dynamic_mcp_tool_name(name: &str) -> bool {
    name.starts_with("mcp__") || matches!(name, "list_mcp_resources" | "read_mcp_resource")
}

fn runtime_policy_supports_mcp_resources() -> bool {
    current_tool_runtime_policy()
        .mcp_servers
        .iter()
        .any(|entry| entry.server.enabled && entry.server.capabilities.supports_resources)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolSourceKind {
    Builtin,
    Mcp,
    McpResource,
}

const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const TOOL_SEARCH_COMPAT_NAME: &str = "toolsearch";

fn builtin_tool_is_deferred(name: &str) -> bool {
    matches!(
        name,
        "ask_user"
            | "config_read"
            | "enter_plan_mode"
            | "exit_plan_mode"
            | "enter_worktree"
            | "exit_worktree"
            | "lsp"
            | "notebook_edit"
            | "todo_write"
            | "task_create"
            | "task_get"
            | "task_list"
            | "task_stop"
            | "task_update"
            | "task_output"
            | "send_message"
            | "team_create"
            | "team_delete"
            | "web_fetch"
            | "web_search"
            | "remote_trigger"
            | "schedule_cron"
            | "list_mcp_resources"
            | "read_mcp_resource"
    )
}

fn builtin_tool_search_hints(name: &str) -> &'static [&'static str] {
    match name {
        "agent" => &["delegate work to a subagent"],
        "ask_user" => &["prompt the user with a multiple-choice question"],
        "config_read" => &["get or set remote-code settings"],
        "enter_plan_mode" => &["switch to plan mode to design an approach before coding"],
        "exit_plan_mode" => &["present plan for approval and start coding"],
        "enter_worktree" => &["create an isolated git worktree and switch into it"],
        "exit_worktree" => &["exit a worktree session and return to the original directory"],
        "glob" => &["find files by name pattern or wildcard"],
        "grep" => &["search file contents with regex ripgrep"],
        "lsp" => &["code intelligence definitions references symbols hover"],
        "notebook_edit" => &["edit jupyter notebook cells ipynb"],
        "read_file" => &["read files images pdfs notebooks"],
        "remote_trigger" => &["manage scheduled remote agent triggers"],
        "send_message" => &["send messages to agent teammates swarm protocol"],
        "task_create" => &["create a task in the task list"],
        "task_get" => &["retrieve a task by id"],
        "task_list" => &["list all tasks"],
        "task_output" => &["read output logs from a background task"],
        "task_stop" => &["kill a running background task"],
        "task_update" => &["update a task"],
        "team_create" => &["create a multi-agent swarm team"],
        "team_delete" => &["disband a swarm team and clean up"],
        "todo_write" => &["manage the session task checklist"],
        "web_fetch" => &["fetch and extract content from a url"],
        "web_search" => &["search the web for current information"],
        "list_mcp_resources" => &["list resources from connected mcp servers"],
        "read_mcp_resource" => &["read a specific mcp resource by uri"],
        _ => &[],
    }
}

fn collect_tool_search_terms(raw: &str, terms: &mut std::collections::BTreeSet<String>) {
    if raw.trim().is_empty() {
        return;
    }
    terms.insert(raw.to_owned());
    for token in raw
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        terms.insert(token.to_ascii_lowercase());
    }
}

fn is_tool_search_tool_name(name: &str) -> bool {
    matches!(name, TOOL_SEARCH_TOOL_NAME | TOOL_SEARCH_COMPAT_NAME)
}

pub async fn runtime_provider_tool_specs() -> Vec<ToolSpec> {
    let builtin_specs = runtime_builtin_tool_specs();
    let catalog = mcp_catalog::runtime_mcp_catalog().await;
    let policy = current_tool_runtime_policy();
    let mut dynamic_specs = Vec::new();

    if catalog
        .clients
        .iter()
        .any(|client| client.supports_resources)
    {
        dynamic_specs.extend(
            specs::mcp_resource_tool_specs()
                .into_iter()
                .filter(|spec| tool_allowed_by_policy(&spec.name, &policy)),
        );
    }

    dynamic_specs.extend(catalog.tools.into_iter().map(|tool| tool.tool_spec));

    merge_provider_tool_specs(builtin_specs, dynamic_specs)
}

pub async fn runtime_tool_search_candidate_specs() -> Vec<ToolSpec> {
    runtime_provider_tool_specs()
        .await
        .into_iter()
        .filter(|spec| spec.is_deferred())
        .collect()
}

pub fn extract_discovered_tool_names_from_conversation(
    conversation: &[ConversationEntry],
) -> std::collections::BTreeSet<String> {
    extract_discovered_tool_names(conversation, &std::collections::BTreeSet::new())
}

#[must_use]
pub fn extract_discovered_tool_names(
    conversation: &[ConversationEntry],
    carried_discovered_tools: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut discovered = carried_discovered_tools.clone();

    for entry in conversation {
        if entry.role != ConversationRole::Tool {
            continue;
        }
        let Some(tool_name) = entry.name.as_deref() else {
            continue;
        };
        if !is_tool_search_tool_name(tool_name) {
            continue;
        }

        for block in &entry.content_blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_reference") {
                continue;
            }
            if let Some(name) = block.get("tool_name").and_then(Value::as_str) {
                discovered.insert(name.to_owned());
            }
        }

        let Ok(payload) = serde_json::from_str::<Value>(&entry.text) else {
            continue;
        };

        if let Some(matches) = payload
            .get("data")
            .and_then(|value| value.get("matches"))
            .and_then(Value::as_array)
        {
            for tool_name in matches.iter().filter_map(Value::as_str) {
                discovered.insert(tool_name.to_owned());
            }
        }

        for array_key in ["results", "found_tools"] {
            let Some(results) = payload.get(array_key).and_then(Value::as_array) else {
                continue;
            };
            for result in results {
                if let Some(name) = result.get("name").and_then(Value::as_str) {
                    discovered.insert(name.to_owned());
                }
            }
        }
    }

    discovered
}

pub async fn runtime_visible_provider_tool_specs(
    conversation: &[ConversationEntry],
) -> Vec<ToolSpec> {
    runtime_visible_provider_tool_specs_with_discovered_tools(
        conversation,
        &std::collections::BTreeSet::new(),
    )
    .await
}

pub async fn runtime_visible_provider_tool_specs_with_discovered_tools(
    conversation: &[ConversationEntry],
    carried_discovered_tools: &std::collections::BTreeSet<String>,
) -> Vec<ToolSpec> {
    let specs = runtime_provider_tool_specs().await;
    let has_tool_search = specs.iter().any(|spec| spec.is_tool_search());
    if !has_tool_search {
        return specs;
    }

    let has_deferred_tools = specs.iter().any(ToolSpec::is_deferred);
    if !has_deferred_tools {
        return specs
            .into_iter()
            .filter(|spec| !spec.is_tool_search())
            .collect();
    }

    let discovered = extract_discovered_tool_names(conversation, carried_discovered_tools);
    specs
        .into_iter()
        .filter(|spec| {
            spec.is_tool_search() || !spec.is_deferred() || discovered.contains(spec.name.as_str())
        })
        .collect()
}

pub async fn runtime_provider_tool_spec(name: &str) -> Option<ToolSpec> {
    if matches!(name, "list_mcp_resources" | "read_mcp_resource")
        && runtime_policy_supports_mcp_resources()
    {
        return specs::mcp_resource_tool_specs()
            .into_iter()
            .find(|spec| spec.name == name);
    }

    runtime_provider_tool_specs()
        .await
        .into_iter()
        .find(|spec| spec.name == name)
}

/// Specification for a single built-in tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
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
}

impl ToolSpec {
    #[must_use]
    pub fn source_kind(&self) -> ToolSourceKind {
        if matches!(
            self.name.as_str(),
            "list_mcp_resources" | "read_mcp_resource"
        ) {
            ToolSourceKind::McpResource
        } else if self.name.starts_with("mcp__") {
            ToolSourceKind::Mcp
        } else {
            ToolSourceKind::Builtin
        }
    }

    #[must_use]
    pub fn is_tool_search(&self) -> bool {
        is_tool_search_tool_name(&self.name)
    }

    #[must_use]
    pub fn is_always_loaded(&self) -> bool {
        self.is_tool_search()
    }

    #[must_use]
    pub fn is_deferred(&self) -> bool {
        if self.is_always_loaded() {
            return false;
        }

        match self.source_kind() {
            ToolSourceKind::Builtin => builtin_tool_is_deferred(&self.name),
            ToolSourceKind::Mcp | ToolSourceKind::McpResource => true,
        }
    }

    #[must_use]
    pub fn tool_search_terms(&self) -> Vec<String> {
        let mut terms = std::collections::BTreeSet::new();
        for raw in [
            self.name.as_str(),
            self.protocol_name.as_str(),
            self.permission_tool_name.as_str(),
        ] {
            collect_tool_search_terms(raw, &mut terms);
        }

        if let Some(stripped) = self.name.strip_prefix("mcp__") {
            for segment in stripped.split("__") {
                collect_tool_search_terms(segment, &mut terms);
            }
        }

        for hint in builtin_tool_search_hints(&self.name) {
            collect_tool_search_terms(hint, &mut terms);
        }

        terms.into_iter().collect()
    }

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
        self.to_anthropic_schema_with_options(false)
    }

    /// Convert to an Anthropic-compatible tool-use schema with per-request overlays.
    #[must_use]
    pub fn to_anthropic_schema_with_options(&self, defer_loading: bool) -> Value {
        let mut schema = serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
        });
        if defer_loading {
            schema["defer_loading"] = Value::Bool(true);
        }
        schema
    }
}

/// Callback type for subtask progress events.
///
/// Receives a human-readable status string that frontends can display.
pub type ProgressCallback = dyn Fn(&str) + Send + Sync;

/// Execution context passed to every tool implementation.
#[derive(Clone)]
pub struct ToolExecutionContext {
    /// Current working directory.
    pub cwd: PathBuf,
    /// Timeout in milliseconds for tool execution.
    pub timeout_ms: u64,
    /// Optional sub-agent completion provider for the agent tool.
    /// When `None`, the agent tool falls back to returning a delegation JSON.
    pub sub_agent: Option<Arc<dyn SubAgentCompletion>>,
    /// Optional progress callback for subtask delegation events.
    /// Frontends (TUI, GUI) provide this to render subtask progress.
    pub progress_cb: Option<Arc<ProgressCallback>>,
    /// Task stack for tracking nested subtask delegation depth.
    /// Shared across tool executions within the same conversation loop.
    pub task_stack: Arc<std::sync::Mutex<TaskStack>>,
}

impl Default for ToolExecutionContext {
    fn default() -> Self {
        Self {
            cwd: PathBuf::new(),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(TaskStack::default())),
        }
    }
}

/// Request to execute a command hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHookExecutionRequest {
    /// The hook event that triggered this execution.
    pub event: HookEvent,
    /// The shell command to execute.
    pub command: String,
    /// Working directory for the command.
    pub cwd: PathBuf,
    /// JSON input passed via stdin.
    pub input: Value,
    /// Shell interpreter to use.
    pub shell: Option<HookShell>,
    /// Timeout in seconds.
    pub timeout_secs: Option<u64>,
}

/// Result of a command hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHookExecutionResult {
    /// The hook event that was executed.
    pub event: HookEvent,
    pub command: String,
    pub shell: HookShell,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

// ---------------------------------------------------------------------------
// Tool registry with lazy loading
// ---------------------------------------------------------------------------

/// Names of tools that are always included in API requests (eager loading).
const EAGER_TOOL_NAMES: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "replace_in_file",
    "bash_command",
    "glob",
    "grep",
    "list_directory",
    "search_text",
    "ask_user",
    "todo_write",
];

/// Tool registry with eager/lazy loading support and BM25 search.
///
/// * **Eager tools** are always sent to the provider in every API request.
/// * **Lazy tools** are only included when explicitly needed (e.g. after a
///   `tool_search` call discovers them).
/// * The built-in BM25 search index allows semantic tool discovery.
pub struct ToolRegistry {
    /// Eagerly loaded tools (always included in API requests).
    eager_tools: Vec<ToolSpec>,
    /// Lazily loaded tools (included only when needed).
    lazy_tools: Vec<ToolSpec>,
    /// BM25 search engine for tool discovery.
    search_engine: search::ToolSearchEngine,
}

impl ToolRegistry {
    /// Create a new registry populated with all built-in tools.
    #[must_use]
    pub fn new() -> Self {
        let all = runtime_builtin_tool_specs();
        let mut eager = Vec::new();
        let mut lazy = Vec::new();
        let mut engine = search::ToolSearchEngine::new();

        for spec in &all {
            let tags: Vec<&str> = Vec::new();
            engine.add_tool(&spec.name, &spec.description, &tags);

            if EAGER_TOOL_NAMES.contains(&spec.name.as_str()) {
                eager.push(spec.clone());
            } else {
                lazy.push(spec.clone());
            }
        }

        Self {
            eager_tools: eager,
            lazy_tools: lazy,
            search_engine: engine,
        }
    }

    /// Returns the eagerly loaded tool specs (for API requests).
    #[must_use]
    pub fn eager_specs(&self) -> &[ToolSpec] {
        &self.eager_tools
    }

    /// Returns all tool specs (eager + lazy).
    pub fn all_specs(&self) -> Vec<ToolSpec> {
        let mut all = self.eager_tools.clone();
        all.extend(self.lazy_tools.clone());
        all
    }

    /// Search tools using BM25 ranking.
    pub fn search(&self, query: &str, max: usize) -> Vec<search::SearchResult> {
        self.search_engine.search(query, max)
    }

    /// Convert eager tools to OpenAI-compatible `tools` parameter.
    #[must_use]
    pub fn to_api_tools(&self) -> Vec<Value> {
        self.eager_tools
            .iter()
            .map(|spec| spec.to_openai_schema())
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct EffectivePermission {
    class: PermissionClass,
    requires_permission: bool,
    blocked_path: Option<String>,
}

fn effective_permission_for_call(call: &ToolCall, spec: &ToolSpec) -> EffectivePermission {
    let default = EffectivePermission {
        class: classify_tool(&spec.permission_tool_name),
        requires_permission: spec.requires_permission,
        blocked_path: call
            .input
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };

    match spec.name.as_str() {
        "workflow" => match call.input.get("action").and_then(Value::as_str) {
            Some("run") => EffectivePermission {
                class: PermissionClass::Bash,
                requires_permission: true,
                blocked_path: None,
            },
            Some("create") | Some("delete") => EffectivePermission {
                class: PermissionClass::Edit,
                requires_permission: true,
                blocked_path: Some(".remote-code-rust/workflows.json".to_owned()),
            },
            Some("list") | Some("status") => EffectivePermission {
                class: PermissionClass::Read,
                requires_permission: false,
                blocked_path: Some(".remote-code-rust/workflows.json".to_owned()),
            },
            _ => default,
        },
        _ => default,
    }
}

/// Execute a tool call after checking permissions.
///
/// Looks up the tool by name, checks permissions via the broker, and dispatches
/// to the appropriate handler.
///
/// # Errors
/// Returns an error if the tool is unknown, permission is denied, or execution fails.
pub async fn execute_tool_call(
    call: &ToolCall,
    context: &ToolExecutionContext,
    broker: &dyn PermissionBroker,
) -> Result<ToolResult> {
    let spec = runtime_provider_tool_spec(&call.name)
        .await
        .unwrap_or_else(|| ToolSpec {
            name: call.name.clone(),
            protocol_name: call.name.clone(),
            permission_tool_name: call.name.clone(),
            description: String::new(),
            requires_permission: true,
            input_schema: Value::Null,
        });

    if spec.description.is_empty() {
        return Err(anyhow!("unknown tool {}", call.name));
    }

    if !tool_allowed_by_runtime(&spec.name) {
        return Ok(ToolResult {
            content: format!(
                "Tool {} is disallowed by the current runtime policy.",
                spec.name
            ),
            is_error: true,
            content_blocks: Vec::new(),
        });
    }

    if let Some(rejected) = plan_mode::plan_mode_guard(&spec, call, context, broker.mode()) {
        return Ok(rejected);
    }

    let permission = effective_permission_for_call(call, &spec);

    if permission.requires_permission {
        // Fast-path: if the broker exposes a mode and auto_allows covers this class, skip.
        let broker_mode = broker.mode();
        let skip_broker = broker_mode.is_some_and(|m| auto_allows(m, permission.class));
        if !skip_broker {
            let decision = broker
                .decide(PermissionRequest {
                    tool_name: spec.name.clone(),
                    permission_class: Some(permission.class),
                    tool_input: call.input.clone(),
                    working_directory: None,
                    tool_use_id: Some(call.id.clone()),
                    title: Some(format!("Allow {}", spec.protocol_name)),
                    description: Some(spec.description.clone()),
                    blocked_path: permission.blocked_path.clone(),
                })
                .await;
            if !decision.allowed {
                return Ok(ToolResult {
                    content: decision
                        .message
                        .unwrap_or_else(|| format!("Permission denied for {}.", spec.name)),
                    is_error: true,
                    content_blocks: Vec::new(),
                });
            }
        }
    }

    if spec.name == "tool_search" {
        return match system::tool_search_tool(&call.input).await {
            Ok(result) => Ok(result),
            Err(error) => Ok(ToolResult {
                content: error.to_string(),
                is_error: true,
                content_blocks: Vec::new(),
            }),
        };
    }

    let result = match spec.name.as_str() {
        "list_directory" => file_ops::list_directory(&call.input, context),
        "read_file" => file_ops::read_file(&call.input, context),
        "search_text" => file_ops::search_text(&call.input, context),
        "write_file" => file_ops::write_file(&call.input, context),
        "replace_in_file" => file_ops::replace_in_file(&call.input, context),
        "edit_file" => file_ops::edit_file(&call.input, context),
        "bash_command" => command::bash_command(&call.input, context).await,
        "glob" => file_ops::glob_files(&call.input, context),
        "grep" => file_ops::grep_files(&call.input, context),
        "web_fetch" => web::web_fetch(&call.input, context).await,
        "ask_user" => misc::ask_user(&call.input, context),
        "todo_write" => system::todo_write(&call.input, context),
        "config_read" => system::config_read(&call.input, context),
        "agent" => agent::agent_tool(&call.input, context).await,
        "web_search" => web::web_search(&call.input, context).await,
        // ── Phase 2 tools ──────────────────────────────────────────────
        "lsp" => misc::lsp_tool(&call.input, context).await,
        "task_create" => tasks::task_create(&call.input),
        "task_get" => tasks::task_get(&call.input),
        "task_list" => tasks::task_list(&call.input),
        "task_stop" => tasks::task_stop(&call.input),
        "task_update" => tasks::task_update(&call.input),
        "notebook_edit" => misc::notebook_edit(&call.input, context),
        "skill_discover" => misc::skill_discover(&call.input, context),
        "send_message" => send_message::send_message(&call.input, context).await,
        "enter_plan_mode" => plan_mode::enter_plan_mode(&call.input, context),
        "exit_plan_mode" => plan_mode::exit_plan_mode(&call.input, context),
        "sleep" => system::sleep_tool(&call.input).await,
        "snip" => system::snip_tool(&call.input, context),
        // ── Phase 3 tools ──────────────────────────────────────────────
        "memory_read" => memory_tools::memory_read_tool(&call.input, context),
        "memory_write" => memory_tools::memory_write_tool(&call.input, context),
        "team_create" => misc::team_create_tool(&call.input, context).await,
        "team_status" => misc::team_status_tool(&call.input).await,
        "web_browser" => web::web_browser_tool(&call.input, context).await,
        "verify_plan" => system::verify_plan_tool(&call.input),
        "terminal_capture" => system::terminal_capture_tool(&call.input, context).await,
        // ── Phase 4: Upstream gap-fill tools ────────────────────────────
        "powershell" => command::powershell_tool(&call.input, context).await,
        "repl" => command::repl_tool(&call.input, context).await,
        "monitor" => system::monitor_tool(&call.input),
        "schedule_cron" => workflow::schedule_cron_tool(&call.input, context),
        "remote_trigger" => misc::remote_trigger_tool(&call.input).await,
        "workflow" => workflow::workflow_tool(&call.input, context),
        "suggest_pr" => git::suggest_pr_tool(context),
        "enter_worktree" => git::enter_worktree_tool(&call.input, context),
        "exit_worktree" => git::exit_worktree_tool(&call.input, context),
        "list_worktrees" => git::list_worktrees_tool(context),
        "brief" => system::brief_tool(&call.input),
        "ctx_inspect" => system::ctx_inspect_tool(&call.input).await,
        "list_peers" => system::list_peers_tool(&call.input).await,
        "tungsten" => misc::tungsten_tool(&call.input, context).await,
        "overflow_test" => misc::overflow_test_tool(&call.input),
        "synthetic_output" => misc::synthetic_output_tool(&call.input),
        "mcp_auth" => mcp_tools::mcp_auth_tool(&call.input, context),
        "mcp_call" => mcp_tools::mcp_call_tool(&call.input, context).await,
        "list_mcp_resources" => mcp_tools::list_mcp_resources_tool(&call.input, context).await,
        "read_mcp_resource" => mcp_tools::read_mcp_resource_tool(&call.input, context).await,
        "skill_execute" => misc::skill_execute_tool(&call.input, context),
        "voice_input" => misc::voice_input_tool(&call.input),
        "daemon" => workflow::daemon_tool(&call.input, context),
        // ── Phase 9: New dedicated tool modules ────────────────────────────
        "discover_skills" => discover_skills::discover_skills(&call.input, context),
        "team_delete" => team_tools::team_delete(&call.input, context),
        "team_list" => team_tools::team_list(&call.input, context),
        "broadcast_message" => send_message::broadcast_message(&call.input, context).await,
        "review_artifact" => review_artifact::review_artifact(&call.input, context),
        "send_user_file" => send_user_file::send_user_file(&call.input, context),
        _ if call.name.starts_with("mcp__") => {
            mcp_catalog::execute_runtime_mcp_tool(&call.name, &call.input).await
        }
        _ => Err(anyhow!("unsupported tool {}", spec.name)),
    };

    match result {
        Ok(content) => Ok(ToolResult {
            content,
            is_error: false,
            content_blocks: Vec::new(),
        }),
        Err(error) => Ok(ToolResult {
            content: error.to_string(),
            is_error: true,
            content_blocks: Vec::new(),
        }),
    }
}

fn normalize_tool_runtime_policy(mut policy: ToolRuntimePolicy) -> ToolRuntimePolicy {
    policy.allowed_tools = policy
        .allowed_tools
        .into_iter()
        .map(|tool| tool.trim().to_ascii_lowercase())
        .filter(|tool| !tool.is_empty())
        .collect::<Vec<_>>();
    policy.allowed_tools.sort();
    policy.allowed_tools.dedup();

    policy.disallowed_tools = policy
        .disallowed_tools
        .into_iter()
        .map(|tool| tool.trim().to_ascii_lowercase())
        .filter(|tool| !tool.is_empty())
        .collect::<Vec<_>>();
    policy.disallowed_tools.sort();
    policy.disallowed_tools.dedup();

    policy.mcp_servers.sort_by(|left, right| {
        left.server
            .name
            .cmp(&right.server.name)
            .then_with(|| left.origin_kind.cmp(&right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
            .then_with(|| left.config_path.cmp(&right.config_path))
    });
    policy.mcp_servers.dedup_by(|left, right| {
        left.config_path == right.config_path && left.server == right.server
    });
    policy
}

fn tool_allowed_by_runtime(tool_name: &str) -> bool {
    let policy = current_tool_runtime_policy();
    tool_allowed_by_policy(tool_name, &policy)
}

pub(crate) fn tool_allowed_by_policy(tool_name: &str, policy: &ToolRuntimePolicy) -> bool {
    let normalized_name = tool_name.trim().to_ascii_lowercase();
    if !policy.allowed_tools.is_empty() && !policy.allowed_tools.contains(&normalized_name) {
        return false;
    }
    !policy.disallowed_tools.contains(&normalized_name)
}

#[cfg(test)]
mod tests {
    use super::{
        CommandHookExecutionRequest, HookShell, RuntimeMcpServerPolicyEntry, ToolExecutionContext,
        ToolRuntimePolicy, ToolRuntimePolicyOverlay, builtin_tool_specs,
        configure_tool_runtime_policy, execute_command_hook, execute_tool_call,
        extract_discovered_tool_names, extract_discovered_tool_names_from_conversation,
        runtime_provider_tool_specs, runtime_tool_search_candidate_specs,
        runtime_visible_provider_tool_specs,
        runtime_visible_provider_tool_specs_with_discovered_tools,
        with_tool_runtime_policy_overlay,
    };
    use once_cell::sync::Lazy;
    use rc_core::{
        HookEvent, PermissionMode, ProviderResponse, SubAgentCompletion, SubAgentExecutionRequest,
        ToolCall, UsageSummary,
    };
    use rc_mcp::{McpCapabilityMatrix, McpServerConfig, McpTransportConfig};
    use rc_permissions::StaticPermissionBroker;
    use rc_swarm::{TeamFile, TeamMember, mailbox, team_helpers};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use tokio::sync::Mutex as AsyncMutex;

    static RUNTIME_POLICY_TEST_MUTEX: Lazy<AsyncMutex<()>> = Lazy::new(|| AsyncMutex::new(()));

    struct TeamDirGuard;

    impl Drop for TeamDirGuard {
        fn drop(&mut self) {
            team_helpers::set_base_dir_override(None);
            crate::team_tools::set_base_dir_override(None);
        }
    }

    async fn seed_team(base: &std::path::Path, team_name: &str) -> TeamDirGuard {
        team_helpers::set_base_dir_override(Some(base.to_path_buf()));
        crate::team_tools::set_base_dir_override(Some(base.to_path_buf()));
        let mut team = TeamFile::new(team_name, "lead");
        team.description = Some("test objective".to_owned());
        team.members
            .push(TeamMember::new("worker-1", "agent-007", "pane-1", "."));
        team_helpers::create_team(&team)
            .await
            .expect("team should be created");
        TeamDirGuard
    }

    async fn create_team_via_tool(
        context: &ToolExecutionContext,
        broker: &StaticPermissionBroker,
        team_name: &str,
        agents: serde_json::Value,
    ) {
        let result = execute_tool_call(
            &ToolCall {
                id: format!("team-create-{team_name}"),
                name: "team_create".to_owned(),
                input: json!({
                    "team_name": team_name,
                    "objective": format!("Coordinate work for {team_name}"),
                    "lead": "lead",
                    "agents": agents,
                }),
            },
            context,
            broker,
        )
        .await
        .expect("team_create should work");
        assert!(!result.is_error, "team_create error: {}", result.content);
    }

    #[derive(Clone)]
    struct RecordingAgentRuntime {
        requests: Arc<Mutex<Vec<SubAgentExecutionRequest>>>,
        result: rc_core::SubAgentExecutionResult,
    }

    #[async_trait::async_trait]
    impl SubAgentCompletion for RecordingAgentRuntime {
        async fn complete(
            &self,
            _conversation: &[rc_core::ConversationEntry],
        ) -> anyhow::Result<ProviderResponse> {
            panic!("complete() should not be used when execute_agent is supported")
        }

        fn supports_agent_execution(&self) -> bool {
            true
        }

        async fn execute_agent(
            &self,
            request: SubAgentExecutionRequest,
        ) -> anyhow::Result<rc_core::SubAgentExecutionResult> {
            self.requests.lock().expect("requests lock").push(request);
            Ok(self.result.clone())
        }
    }

    #[tokio::test]
    async fn read_and_search_tools_work() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let file = tempdir.path().join("notes.txt");
        if let Err(error) = std::fs::write(&file, "hello\nremote code\n") {
            panic!("failed to seed file: {error}");
        }
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let read = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "read_file".to_owned(),
                input: json!({"path":"notes.txt"}),
            },
            &context,
            &broker,
        )
        .await;
        assert!(read.is_ok());
        let read = read.unwrap_or_else(|error| panic!("read failed: {error}"));
        assert!(read.content.contains("remote code"));

        let search = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "search_text".to_owned(),
                input: json!({"pattern":"remote","path":"."}),
            },
            &context,
            &broker,
        )
        .await;
        assert!(search.is_ok());
        let search = search.unwrap_or_else(|error| panic!("search failed: {error}"));
        assert!(search.content.contains("notes.txt:2"));

        assert!(
            builtin_tool_specs()
                .iter()
                .any(|spec| spec.protocol_name == "Bash")
        );
    }

    #[tokio::test]
    async fn command_hook_receives_json_input_over_stdin() {
        let tempdir = tempdir().expect("tempdir should work");
        let (shell, command) = if cfg!(windows) {
            (
                HookShell::PowerShell,
                "$inputJson = [Console]::In.ReadToEnd(); Write-Output $inputJson".to_owned(),
            )
        } else {
            (
                HookShell::Bash,
                "payload=$(cat); printf '%s' \"$payload\"".to_owned(),
            )
        };

        let result = execute_command_hook(&CommandHookExecutionRequest {
            event: HookEvent::SessionStart,
            command,
            cwd: tempdir.path().to_path_buf(),
            input: json!({"hello":"world"}),
            shell: Some(shell),
            timeout_secs: Some(5),
        })
        .await
        .expect("hook execution should work");

        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("\"hello\":\"world\""));
    }

    // ── New tool tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn glob_finds_matching_files() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        std::fs::write(tempdir.path().join("foo.rs"), "fn main() {}").expect("write should work");
        std::fs::write(tempdir.path().join("bar.txt"), "hello").expect("write should work");
        std::fs::create_dir(tempdir.path().join("src")).expect("mkdir should work");
        std::fs::write(tempdir.path().join("src/mod.rs"), "mod foo;").expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "glob".to_owned(),
                input: json!({"pattern": "**/*.rs"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("glob should work");

        assert!(!result.is_error, "glob error: {}", result.content);
        assert!(result.content.contains("foo.rs"), "should find foo.rs");
        assert!(result.content.contains("mod.rs"), "should find src/mod.rs");
        assert!(
            !result.content.contains("bar.txt"),
            "should not find bar.txt"
        );
    }

    #[tokio::test]
    async fn grep_finds_pattern_in_files() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        std::fs::write(
            tempdir.path().join("code.rs"),
            "fn hello() {}\nfn world() {}\n",
        )
        .expect("write should work");
        std::fs::write(tempdir.path().join("notes.txt"), "hello world\n")
            .expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        // Search with file_pattern filter
        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "grep".to_owned(),
                input: json!({
                    "pattern": "fn hello",
                    "file_pattern": "*.rs"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("grep should work");

        assert!(!result.is_error, "grep error: {}", result.content);
        assert!(result.content.contains("code.rs"), "should find code.rs");
        assert!(
            result.content.contains("fn hello"),
            "should show matching line"
        );
        assert!(
            !result.content.contains("notes.txt"),
            "should not match notes.txt due to file_pattern"
        );
    }

    #[tokio::test]
    async fn web_fetch_handles_invalid_url() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "web_fetch".to_owned(),
                input: json!({"url": "not-a-valid-url"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("web_fetch should not panic");

        assert!(result.is_error, "invalid URL should produce an error");
    }

    #[tokio::test]
    async fn ask_user_returns_json_structure() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "ask_user".to_owned(),
                input: json!({
                    "question": "What is your name?",
                    "suggestions": ["Alice", "Bob"]
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("ask_user should work");

        assert!(!result.is_error, "ask_user error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "ask_user");
        assert_eq!(parsed["question"], "What is your name?");
        assert!(parsed["suggestions"].is_array());
        let suggestions = parsed["suggestions"].as_array().expect("should be array");
        assert_eq!(suggestions.len(), 2);
    }

    #[tokio::test]
    async fn todo_write_persists_and_reads() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "todo_write".to_owned(),
                input: json!({
                    "todos": [
                        {"id": "1", "text": "Task 1", "status": "completed"},
                        {"id": "2", "text": "Task 2", "status": "in_progress"},
                        {"id": "3", "text": "Task 3", "status": "pending"}
                    ]
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("todo_write should work");

        assert!(!result.is_error, "todo_write error: {}", result.content);
        assert!(result.content.contains("3 todo items"));

        // Verify the file was persisted
        let todos_path = tempdir.path().join(".remote-code-rust").join("todos.json");
        assert!(todos_path.exists(), "todos.json should exist");
        let content = std::fs::read_to_string(&todos_path).expect("should read todos.json");
        let todos: Vec<serde_json::Value> =
            serde_json::from_str(&content).expect("should parse JSON");
        assert_eq!(todos.len(), 3);
        assert_eq!(todos[0]["id"], "1");
        assert_eq!(todos[0]["status"], "completed");
        assert_eq!(todos[1]["status"], "in_progress");
        assert_eq!(todos[2]["status"], "pending");
    }

    #[tokio::test]
    async fn config_read_returns_current_settings() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        // Set a config value
        let set_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "config_read".to_owned(),
                input: json!({
                    "action": "set",
                    "key": "test_key",
                    "value": "test_value"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("config set should work");

        assert!(
            !set_result.is_error,
            "config set error: {}",
            set_result.content
        );

        // Get the config value
        let get_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "config_read".to_owned(),
                input: json!({
                    "action": "get",
                    "key": "test_key"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("config get should work");

        assert!(
            !get_result.is_error,
            "config get error: {}",
            get_result.content
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&get_result.content).expect("should be valid JSON");
        assert_eq!(parsed["test_key"], "test_value");
    }

    // ── Phase 2 tool tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn lsp_definitions_finds_function() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        std::fs::write(
            tempdir.path().join("lib.rs"),
            "fn my_function() -> i32 { 42 }\nstruct MyStruct { x: i32 }\n",
        )
        .expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "lsp".to_owned(),
                input: json!({
                    "action": "definitions",
                    "file_path": "lib.rs",
                    "symbol": "my_function"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("lsp definitions should work");

        assert!(!result.is_error, "lsp error: {}", result.content);
        assert!(
            result.content.contains("my_function"),
            "should find my_function definition"
        );
    }

    #[tokio::test]
    async fn lsp_references_finds_usages() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        std::fs::write(
            tempdir.path().join("main.rs"),
            "fn helper() {}\nfn main() { helper(); }\n",
        )
        .expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "lsp".to_owned(),
                input: json!({
                    "action": "references",
                    "file_path": "main.rs",
                    "symbol": "helper"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("lsp references should work");

        assert!(!result.is_error, "lsp error: {}", result.content);
        assert!(
            result.content.contains("helper"),
            "should find helper references"
        );
    }

    #[tokio::test]
    async fn lsp_hover_returns_context() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        std::fs::write(
            tempdir.path().join("code.rs"),
            "/// A great function.\nfn awesome_thing() -> bool { true }\n",
        )
        .expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "lsp".to_owned(),
                input: json!({
                    "action": "hover",
                    "file_path": "code.rs",
                    "symbol": "awesome_thing"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("lsp hover should work");

        assert!(!result.is_error, "lsp error: {}", result.content);
        assert!(
            result.content.contains("awesome_thing"),
            "should contain hover info for awesome_thing"
        );
    }

    #[tokio::test]
    async fn lsp_completion_returns_stub() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "lsp".to_owned(),
                input: json!({
                    "action": "completion",
                    "file_path": "main.rs",
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("lsp completion should work");

        assert!(!result.is_error, "lsp error: {}", result.content);
        assert!(
            result.content.contains("No completions available"),
            "completion should indicate no completions, got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn task_tools_crud_workflow() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        // Create a task
        let create_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "task_create".to_owned(),
                input: json!({"title": "Build feature"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("task_create should work");

        assert!(
            !create_result.is_error,
            "task_create error: {}",
            create_result.content
        );
        let create_json: serde_json::Value =
            serde_json::from_str(&create_result.content).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        // Get the task
        let get_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "task_get".to_owned(),
                input: json!({"id": task_id}),
            },
            &context,
            &broker,
        )
        .await
        .expect("task_get should work");

        assert!(
            !get_result.is_error,
            "task_get error: {}",
            get_result.content
        );
        assert!(
            get_result.content.contains("Build feature"),
            "should contain task title"
        );

        // Update the task
        let update_result = execute_tool_call(
            &ToolCall {
                id: "3".to_owned(),
                name: "task_update".to_owned(),
                input: json!({"id": task_id, "status": "running"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("task_update should work");

        assert!(
            !update_result.is_error,
            "task_update error: {}",
            update_result.content
        );

        // List tasks
        let list_result = execute_tool_call(
            &ToolCall {
                id: "4".to_owned(),
                name: "task_list".to_owned(),
                input: json!({}),
            },
            &context,
            &broker,
        )
        .await
        .expect("task_list should work");

        assert!(
            !list_result.is_error,
            "task_list error: {}",
            list_result.content
        );
        assert!(
            !list_result.content.contains("No tasks found"),
            "should have at least one task"
        );

        // Stop the task
        let stop_result = execute_tool_call(
            &ToolCall {
                id: "5".to_owned(),
                name: "task_stop".to_owned(),
                input: json!({"id": task_id}),
            },
            &context,
            &broker,
        )
        .await
        .expect("task_stop should work");

        assert!(
            !stop_result.is_error,
            "task_stop error: {}",
            stop_result.content
        );
    }

    #[tokio::test]
    async fn notebook_edit_modifies_cell() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let notebook = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('hello')"],
                    "outputs": [],
                    "execution_count": 1
                },
                {
                    "cell_type": "markdown",
                    "source": ["# Title"]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        let nb_path = tempdir.path().join("test.ipynb");
        std::fs::write(
            &nb_path,
            serde_json::to_string_pretty(&notebook).expect("serialize notebook"),
        )
        .expect("write notebook");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "notebook_edit".to_owned(),
                input: json!({
                    "path": "test.ipynb",
                    "cell_index": 0,
                    "new_source": "print('world')",
                    "cell_type": "code"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("notebook_edit should work");

        assert!(!result.is_error, "notebook_edit error: {}", result.content);

        // Verify the notebook was modified
        let updated: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&nb_path).expect("read notebook"))
                .expect("parse notebook");
        assert_eq!(
            updated["cells"][0]["source"],
            json!("print('world')"),
            "cell source should be updated"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_returns_json_structure() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let _guard = seed_team(tempdir.path(), "message-team").await;
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "send_message".to_owned(),
                input: json!({
                    "team_name": "message-team",
                    "recipient": "agent-007",
                    "message": "Hello from test",
                    "priority": "high",
                    "correlation_id": "corr-1"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("send_message should work");

        assert!(!result.is_error, "send_message error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "agent_message");
        assert_eq!(parsed["to"], "agent-007");
        assert_eq!(parsed["team_name"], "message-team");
        assert_eq!(parsed["priority"], "high");
        assert_eq!(parsed["correlation_id"], "corr-1");
        let stored = mailbox::read_messages("message-team", "agent-007")
            .await
            .expect("read mailbox");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].priority.as_deref(), Some("high"));
        assert_eq!(stored[0].correlation_id.as_deref(), Some("corr-1"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn team_create_status_and_broadcast_round_trip() {
        let tempdir = tempdir().expect("tempdir should work");
        let _guard = seed_team(tempdir.path(), "seed-team").await;
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        create_team_via_tool(
            &context,
            &broker,
            "review-team",
            json!([
                {"name": "agent-1", "role": "worker", "color": "blue"},
                {"name": "agent-2", "role": "reviewer", "cwd": tempdir.path().to_string_lossy()}
            ]),
        )
        .await;

        let created_team = team_helpers::read_team("review-team")
            .await
            .expect("read created team");
        assert_eq!(created_team.team_allowed_paths.len(), 1);
        assert_eq!(
            created_team.team_allowed_paths[0].path,
            tempdir.path().to_string_lossy()
        );

        let status_result = execute_tool_call(
            &ToolCall {
                id: "team-status".to_owned(),
                name: "team_status".to_owned(),
                input: json!({"team_name": "review-team"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("team_status should work");
        assert!(
            !status_result.is_error,
            "team_status error: {}",
            status_result.content
        );
        let status: serde_json::Value =
            serde_json::from_str(&status_result.content).expect("valid team_status json");
        assert_eq!(status["count"], 1);
        assert_eq!(
            status["teams"][0]["members"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(status["teams"][0]["lead"]["unread_messages"], 0);

        let peers_result = execute_tool_call(
            &ToolCall {
                id: "list-peers".to_owned(),
                name: "list_peers".to_owned(),
                input: json!({"team_name": "review-team"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("list_peers should work");
        assert!(
            !peers_result.is_error,
            "list_peers error: {}",
            peers_result.content
        );
        let peers: serde_json::Value =
            serde_json::from_str(&peers_result.content).expect("valid peers json");
        assert_eq!(peers["count"], 3);

        let broadcast_result = execute_tool_call(
            &ToolCall {
                id: "broadcast".to_owned(),
                name: "broadcast_message".to_owned(),
                input: json!({
                    "team_name": "review-team",
                    "sender": "lead",
                    "message": "All hands check-in",
                    "priority": "high"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("broadcast_message should work");
        assert!(
            !broadcast_result.is_error,
            "broadcast_message error: {}",
            broadcast_result.content
        );
        let broadcast: serde_json::Value =
            serde_json::from_str(&broadcast_result.content).expect("valid broadcast json");
        assert_eq!(broadcast["recipients"].as_array().map(Vec::len), Some(2));
        assert_eq!(broadcast["message_ids"].as_array().map(Vec::len), Some(2));

        let agent_1 = mailbox::read_messages("review-team", "agent-1")
            .await
            .expect("agent-1 mailbox");
        let agent_2 = mailbox::read_messages("review-team", "agent-2")
            .await
            .expect("agent-2 mailbox");
        assert_eq!(agent_1.len(), 1);
        assert_eq!(agent_2.len(), 1);
        assert_eq!(agent_1[0].priority.as_deref(), Some("high"));

        let follow_up = execute_tool_call(
            &ToolCall {
                id: "status-after-broadcast".to_owned(),
                name: "team_status".to_owned(),
                input: json!({"team_name": "review-team"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("team_status should work");
        let follow_up_status: serde_json::Value =
            serde_json::from_str(&follow_up.content).expect("valid team_status json");
        let members = follow_up_status["teams"][0]["members"]
            .as_array()
            .expect("members array");
        assert!(members.iter().all(|member| member["unread_messages"] == 1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn team_list_and_delete_cover_cleanup_and_permissions() {
        let tempdir = tempdir().expect("tempdir should work");
        let _guard = seed_team(tempdir.path(), "seed-team").await;
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let allow_broker = StaticPermissionBroker::new(true);
        create_team_via_tool(
            &context,
            &allow_broker,
            "cleanup-team",
            json!([{"name": "agent-1", "role": "worker"}]),
        )
        .await;

        let list_result = execute_tool_call(
            &ToolCall {
                id: "team-list".to_owned(),
                name: "team_list".to_owned(),
                input: json!({}),
            },
            &context,
            &allow_broker,
        )
        .await
        .expect("team_list should work");
        assert!(
            !list_result.is_error,
            "team_list error: {}",
            list_result.content
        );
        let list: serde_json::Value =
            serde_json::from_str(&list_result.content).expect("valid team_list json");
        assert!(
            list["teams"]
                .as_array()
                .expect("teams array")
                .iter()
                .any(|team| team["name"] == "cleanup-team")
        );

        let deny_broker = StaticPermissionBroker::new(false);
        let denied = execute_tool_call(
            &ToolCall {
                id: "team-delete-denied".to_owned(),
                name: "team_delete".to_owned(),
                input: json!({"team_name": "cleanup-team"}),
            },
            &context,
            &deny_broker,
        )
        .await
        .expect("team_delete should return tool result");
        assert!(denied.is_error);
        assert!(team_helpers::read_team("cleanup-team").await.is_ok());

        let deleted = execute_tool_call(
            &ToolCall {
                id: "team-delete".to_owned(),
                name: "team_delete".to_owned(),
                input: json!({"team_name": "cleanup-team"}),
            },
            &context,
            &allow_broker,
        )
        .await
        .expect("team_delete should work");
        assert!(!deleted.is_error, "team_delete error: {}", deleted.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&deleted.content).expect("valid delete json");
        assert_eq!(parsed["status"], "deleted");
        assert_eq!(parsed["cleanup"]["team_dir"], "removed");
        assert!(team_helpers::read_team("cleanup-team").await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_tool_call_routes_agent_tool_to_host_runtime() {
        let tempdir = tempdir().expect("tempdir should work");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "verified".to_owned(),
                success: true,
                turns: 3,
                usage: UsageSummary::default(),
            },
        });
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: Some(runtime),
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "agent".to_owned(),
                input: json!({
                    "prompt": "Review the Rust changes and report the wiring path.",
                    "description": "Inspect agent runtime wiring"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("agent tool should execute");

        assert!(!result.is_error, "agent tool error: {}", result.content);
        assert_eq!(result.content, "verified");

        let requests = requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.agent_type, "general-purpose");
        assert_eq!(request.max_turns, 200);
        assert!(request.allowed_tools.contains(&"read_file".to_owned()));
        assert!(request.allowed_tools.contains(&"write_file".to_owned()));
        assert!(request.allowed_tools.contains(&"edit_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"agent".to_owned()));
    }

    #[tokio::test]
    async fn execute_tool_call_returns_structured_agent_request_without_provider() {
        let tempdir = tempdir().expect("tempdir should work");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "agent-no-provider".to_owned(),
                name: "agent".to_owned(),
                input: json!({
                    "prompt": "Inspect the project and report one refactor target.",
                    "description": "Plan inspection",
                    "subagent_type": "Plan",
                    "model": "minimax-m2.7",
                    "tools": ["Read", "Grep"]
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("agent tool should succeed");

        assert!(!result.is_error, "agent tool error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("valid sub_agent_request json");
        assert_eq!(parsed["type"], "sub_agent_request");
        assert_eq!(parsed["description"], "Plan inspection");
        assert_eq!(parsed["subagent_type"], "Plan");
        assert_eq!(parsed["model"], "minimax-m2.7");
        assert_eq!(parsed["allowed_tools"].as_array().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn execute_tool_call_routes_plan_agent_and_emits_progress() {
        let tempdir = tempdir().expect("tempdir should work");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "plan ready".to_owned(),
                success: true,
                turns: 3,
                usage: UsageSummary::default(),
            },
        });
        let progress = Arc::new(Mutex::new(Vec::<String>::new()));
        let progress_sink = Arc::clone(&progress);
        let progress_cb: Arc<super::ProgressCallback> = Arc::new(move |message| {
            progress_sink
                .lock()
                .expect("progress lock")
                .push(message.to_owned());
        });
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: Some(runtime),
            progress_cb: Some(progress_cb),
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "agent-plan".to_owned(),
                name: "agent".to_owned(),
                input: json!({
                    "prompt": "Inspect the current Rust project and identify one concrete refactor target.",
                    "description": "Plan refactor target",
                    "subagent_type": "Plan",
                    "model": "minimax-m2.7",
                    "tools": ["Read", "Grep"]
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("agent tool should succeed");

        assert!(!result.is_error, "agent tool error: {}", result.content);
        assert_eq!(result.content, "plan ready");

        let requests = requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.agent_type, "Plan");
        assert_eq!(request.description.as_deref(), Some("Plan refactor target"));
        assert_eq!(request.model.as_deref(), Some("minimax-m2.7"));
        assert_eq!(request.working_dir, PathBuf::from(tempdir.path()));
        assert!(request.allowed_tools.contains(&"read_file".to_owned()));
        assert!(request.allowed_tools.contains(&"grep".to_owned()));
        assert!(!request.allowed_tools.contains(&"write_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"edit_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"agent".to_owned()));
        drop(requests);

        let progress = progress.lock().expect("progress lock");
        assert_eq!(progress.len(), 2);
        let started = crate::agent::parse_delegate_progress_event(&progress[0])
            .expect("start progress event");
        let completed = crate::agent::parse_delegate_progress_event(&progress[1])
            .expect("completed progress event");
        match started {
            crate::agent::DelegateProgressEvent::SubtaskStarted { description, .. } => {
                assert_eq!(description, "Plan refactor target");
            }
            other => panic!("expected started event, got {other:?}"),
        }
        match completed {
            crate::agent::DelegateProgressEvent::SubtaskCompleted {
                success,
                turns_used,
                ..
            } => {
                assert!(success);
                assert_eq!(turns_used, 3);
            }
            other => panic!("expected completed event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_tool_call_rejects_unknown_subagent_type() {
        let tempdir = tempdir().expect("tempdir should work");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: String::new(),
                success: true,
                turns: 1,
                usage: UsageSummary::default(),
            },
        });
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: Some(runtime),
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "agent-unknown".to_owned(),
                name: "agent".to_owned(),
                input: json!({
                    "prompt": "Do work.",
                    "description": "Unknown agent",
                    "subagent_type": "does-not-exist"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("agent tool should return tool result");

        assert!(result.is_error);
        assert!(result.content.contains("unknown subagent_type"));
        assert!(requests.lock().expect("requests lock").is_empty());
    }

    #[tokio::test]
    async fn execute_tool_call_blocks_agent_runtime_when_permission_denied() {
        let tempdir = tempdir().expect("tempdir should work");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "should not run".to_owned(),
                success: true,
                turns: 1,
                usage: UsageSummary::default(),
            },
        });
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: Some(runtime),
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(false);

        let result = execute_tool_call(
            &ToolCall {
                id: "agent-denied".to_owned(),
                name: "agent".to_owned(),
                input: json!({"prompt": "Inspect the project"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("agent tool should return tool result");

        assert!(result.is_error);
        assert!(result.content.contains("Permission denied"));
        assert!(requests.lock().expect("requests lock").is_empty());
    }

    #[tokio::test]
    async fn plan_mode_tools_return_human_readable_messages() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let enter_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "enter_plan_mode".to_owned(),
                input: json!({"objective": "Plan the architecture"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("enter_plan_mode should work");

        assert!(
            !enter_result.is_error,
            "enter_plan_mode error: {}",
            enter_result.content
        );
        assert!(enter_result.content.contains("Entered plan mode"));
        assert!(enter_result.content.contains("Plan the architecture"));

        let exit_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "exit_plan_mode".to_owned(),
                input: json!({}),
            },
            &context,
            &broker,
        )
        .await
        .expect("exit_plan_mode should work");

        assert!(
            !exit_result.is_error,
            "exit_plan_mode error: {}",
            exit_result.content
        );
        assert!(exit_result.content.contains("Exited plan mode"));
    }

    #[tokio::test]
    async fn sleep_tool_completes() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "sleep".to_owned(),
                input: json!({"seconds": 0}),
            },
            &context,
            &broker,
        )
        .await
        .expect("sleep should work");

        assert!(!result.is_error, "sleep error: {}", result.content);
        assert!(
            result.content.contains("Slept for 0"),
            "should report sleeping"
        );
    }

    #[tokio::test]
    async fn snip_tool_saves_file() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "snip".to_owned(),
                input: json!({
                    "content": "fn main() { println!(\"hello\"); }",
                    "label": "hello world"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("snip should work");

        assert!(!result.is_error, "snip error: {}", result.content);
        assert!(
            result.content.contains("snippets"),
            "should mention snippets dir"
        );

        // Verify the file was created
        let snippets_dir = tempdir.path().join(".remote-code-rust").join("snippets");
        assert!(snippets_dir.exists(), "snippets dir should exist");
        let entries: Vec<_> = std::fs::read_dir(&snippets_dir)
            .expect("read snippets dir")
            .filter_map(Result::ok)
            .collect();
        assert!(!entries.is_empty(), "should have at least one snippet file");

        // Verify content
        let file_content = std::fs::read_to_string(entries[0].path()).expect("read snippet");
        assert!(
            file_content.contains("fn main()"),
            "snippet should contain the saved content"
        );
    }

    #[tokio::test]
    async fn skill_discover_returns_result() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "skill_discover".to_owned(),
                input: json!({}),
            },
            &context,
            &broker,
        )
        .await
        .expect("skill_discover should work");

        assert!(!result.is_error, "skill_discover error: {}", result.content);
        // Empty workspace should return "No skills found"
        assert!(
            result.content.contains("No skills found"),
            "empty workspace should report no skills"
        );
    }

    #[tokio::test]
    async fn all_phase2_tools_are_registered() {
        let specs = builtin_tool_specs();
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();

        // Phase 2 tools
        assert!(names.contains(&"lsp"), "lsp should be registered");
        assert!(
            names.contains(&"task_create"),
            "task_create should be registered"
        );
        assert!(names.contains(&"task_get"), "task_get should be registered");
        assert!(
            names.contains(&"task_list"),
            "task_list should be registered"
        );
        assert!(
            names.contains(&"task_stop"),
            "task_stop should be registered"
        );
        assert!(
            names.contains(&"task_update"),
            "task_update should be registered"
        );
        assert!(
            names.contains(&"notebook_edit"),
            "notebook_edit should be registered"
        );
        assert!(
            names.contains(&"skill_discover"),
            "skill_discover should be registered"
        );
        assert!(
            names.contains(&"send_message"),
            "send_message should be registered"
        );
        assert!(
            names.contains(&"enter_plan_mode"),
            "enter_plan_mode should be registered"
        );
        assert!(
            names.contains(&"exit_plan_mode"),
            "exit_plan_mode should be registered"
        );
        assert!(names.contains(&"sleep"), "sleep should be registered");
        assert!(names.contains(&"snip"), "snip should be registered");
    }

    // ── Phase 4: Upstream gap-fill tool tests ────────────────────────────

    #[tokio::test]
    async fn powershell_tool_returns_result() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "powershell".to_owned(),
                input: json!({"command": "Write-Output hello"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("powershell should work");

        if cfg!(windows) {
            assert!(!result.is_error, "powershell error: {}", result.content);
            assert!(
                result.content.contains("hello"),
                "should contain hello, got: {}",
                result.content
            );
        } else {
            assert!(
                result.content.contains("only available on Windows"),
                "non-windows should return hint"
            );
        }
    }

    #[tokio::test]
    async fn repl_tool_requires_language_and_code() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "repl".to_owned(),
                input: json!({"language": "python", "code": "print(42)"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("repl should not panic");

        // May succeed or fail depending on whether python is installed,
        // but should not panic.
        let _ = result;
    }

    #[tokio::test]
    async fn monitor_tool_returns_snapshot() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "monitor".to_owned(),
                input: json!({"target": "tasks"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("monitor should work");

        assert!(!result.is_error, "monitor error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["target"], "tasks");
    }

    #[tokio::test]
    async fn schedule_cron_saves_to_file() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "schedule_cron".to_owned(),
                input: json!({
                    "schedule": "*/5 * * * *",
                    "command": "echo hello",
                    "description": "test cron"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("schedule_cron should work");

        assert!(!result.is_error, "schedule_cron error: {}", result.content);
        let crons_path = tempdir.path().join(".remote-code-rust").join("crons.json");
        assert!(crons_path.exists(), "crons.json should exist");
    }

    #[tokio::test]
    async fn remote_trigger_sends_post() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "remote_trigger".to_owned(),
                input: json!({
                    "url": "http://127.0.0.1:1/does-not-exist",
                    "event": "test"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("remote_trigger should not panic");

        // Will likely fail to connect, but should not panic.
        let _ = result;
    }

    #[tokio::test]
    async fn workflow_create_and_status() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let create_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "workflow".to_owned(),
                input: json!({
                    "action": "create",
                    "name": "test-wf",
                    "steps": ["step1", "step2"]
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("workflow create should work");

        assert!(
            !create_result.is_error,
            "workflow create error: {}",
            create_result.content
        );

        let status_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "workflow".to_owned(),
                input: json!({
                    "action": "status",
                    "name": "test-wf"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("workflow status should work");

        assert!(
            !status_result.is_error,
            "workflow status error: {}",
            status_result.content
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&status_result.content).expect("should be valid JSON");
        assert_eq!(parsed["name"], "test-wf");
        assert_eq!(parsed["status"], "created");
    }

    #[tokio::test]
    async fn workflow_run_requires_bash_permission_even_in_accept_edits_mode() {
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };

        let allow_broker = StaticPermissionBroker::new(true);
        let create_result = execute_tool_call(
            &ToolCall {
                id: "wf-create".to_owned(),
                name: "workflow".to_owned(),
                input: json!({
                    "action": "create",
                    "name": "test-wf",
                    "steps": ["echo hello"]
                }),
            },
            &context,
            &allow_broker,
        )
        .await
        .expect("workflow create should work");
        assert!(
            !create_result.is_error,
            "workflow create error: {}",
            create_result.content
        );

        let accept_edits_broker = StaticPermissionBroker::from_mode(PermissionMode::AcceptEdits);
        let run_result = execute_tool_call(
            &ToolCall {
                id: "wf-run".to_owned(),
                name: "workflow".to_owned(),
                input: json!({
                    "action": "run",
                    "name": "test-wf"
                }),
            },
            &context,
            &accept_edits_broker,
        )
        .await
        .expect("workflow run should return a tool result");
        assert!(run_result.is_error, "workflow run unexpectedly succeeded");
        assert!(
            run_result.content.contains("Permission denied"),
            "unexpected workflow denial message: {}",
            run_result.content
        );

        let status_result = execute_tool_call(
            &ToolCall {
                id: "wf-status".to_owned(),
                name: "workflow".to_owned(),
                input: json!({
                    "action": "status",
                    "name": "test-wf"
                }),
            },
            &context,
            &accept_edits_broker,
        )
        .await
        .expect("workflow status should still work");
        assert!(
            !status_result.is_error,
            "workflow status error: {}",
            status_result.content
        );
    }

    #[tokio::test]
    async fn suggest_pr_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "suggest_pr".to_owned(),
                input: json!({}),
            },
            &context,
            &broker,
        )
        .await
        .expect("suggest_pr should work");

        assert!(!result.is_error, "suggest_pr error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert!(parsed["suggested_title"].is_string());
    }

    #[tokio::test]
    async fn enter_and_exit_worktree_return_commands() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let enter_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "enter_worktree".to_owned(),
                input: json!({"branch": "feature-x"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("enter_worktree should work");

        assert!(
            !enter_result.is_error,
            "enter_worktree error: {}",
            enter_result.content
        );
        assert!(
            enter_result.content.contains("git worktree add"),
            "should contain git worktree add"
        );

        let exit_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "exit_worktree".to_owned(),
                input: json!({"branch": "feature-x"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("exit_worktree should work");

        assert!(
            !exit_result.is_error,
            "exit_worktree error: {}",
            exit_result.content
        );
        assert!(
            exit_result.content.contains("git worktree remove"),
            "should contain git worktree remove"
        );
    }

    #[tokio::test]
    async fn brief_tool_truncates_content() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let long_content: String = "x".repeat(1000);
        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "brief".to_owned(),
                input: json!({
                    "content": long_content,
                    "max_length": 50
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("brief should work");

        assert!(!result.is_error, "brief error: {}", result.content);
        assert!(
            result.content.contains("truncated"),
            "should mention truncation"
        );
    }

    #[tokio::test]
    async fn ctx_inspect_lists_tools() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "ctx_inspect".to_owned(),
                input: json!({"action": "tools"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("ctx_inspect should work");

        assert!(!result.is_error, "ctx_inspect error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert!(parsed["total_tools"].as_u64().unwrap_or(0) > 40);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_peers_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let _guard = seed_team(tempdir.path(), "peers-team").await;
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "list_peers".to_owned(),
                input: json!({"team_name": "peers-team"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("list_peers should work");

        assert!(!result.is_error, "list_peers error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert!(parsed["peers"].is_array());
        assert!(
            parsed["peers"]
                .as_array()
                .expect("peer array")
                .iter()
                .any(|peer| peer["name"] == "agent-007")
        );
    }

    #[tokio::test]
    async fn tungsten_tool_detects_project() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        // Create a Cargo.toml to trigger Rust detection.
        std::fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .expect("write should work");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "tungsten".to_owned(),
                input: json!({"action": "compile", "target": "."}),
            },
            &context,
            &broker,
        )
        .await
        .expect("tungsten should work");

        // May succeed or fail depending on cargo availability, but should not panic.
        let _ = result;
    }

    #[tokio::test]
    async fn overflow_test_generates_data() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "overflow_test".to_owned(),
                input: json!({"scenario": "large_output"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("overflow_test should work");

        assert!(!result.is_error, "overflow_test error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["scenario"], "large_output");
    }

    #[tokio::test]
    async fn synthetic_output_generates_csv() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "synthetic_output".to_owned(),
                input: json!({"type": "csv", "rows": 3}),
            },
            &context,
            &broker,
        )
        .await
        .expect("synthetic_output should work");

        assert!(
            !result.is_error,
            "synthetic_output error: {}",
            result.content
        );
        assert!(
            result.content.contains("id,name,value,active"),
            "should have CSV header"
        );
        assert!(result.content.contains("item_0"), "should have data rows");
    }

    #[tokio::test]
    async fn mcp_auth_login_and_status() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let login_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "mcp_auth".to_owned(),
                input: json!({"server": "test-server", "action": "login"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("mcp_auth login should work");

        assert!(
            !login_result.is_error,
            "mcp_auth login error: {}",
            login_result.content
        );

        let status_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "mcp_auth".to_owned(),
                input: json!({"server": "test-server", "action": "status"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("mcp_auth status should work");

        assert!(
            !status_result.is_error,
            "mcp_auth status error: {}",
            status_result.content
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&status_result.content).expect("should be valid JSON");
        assert_eq!(parsed["status"], "authenticated");
    }

    #[tokio::test]
    async fn mcp_call_uses_runtime_inventory_from_policy() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let original_policy = super::current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            mcp_servers: vec![RuntimeMcpServerPolicyEntry {
                origin_kind: "profile".to_owned(),
                origin_name: "profile".to_owned(),
                config_path: tempdir.path().join("mcp.toml"),
                server: McpServerConfig {
                    name: "demo".to_owned(),
                    enabled: false,
                    transport: McpTransportConfig::Stdio {
                        command: "python".to_owned(),
                        args: Vec::new(),
                        cwd: None,
                        env: Default::default(),
                    },
                    capabilities: McpCapabilityMatrix::default(),
                    startup_timeout_secs: None,
                    request_timeout_secs: None,
                    metadata: Default::default(),
                },
            }],
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "mcp_call".to_owned(),
                input: json!({"server": "demo", "tool": "search", "arguments": {"q": "rust"}}),
            },
            &context,
            &broker,
        )
        .await
        .expect("mcp_call should return a tool result");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(
            result.is_error,
            "mcp_call should fail for disabled inventory"
        );
        assert!(
            result
                .content
                .contains("disabled by the current runtime inventory"),
            "unexpected error: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn mcp_call_requires_runtime_inventory_in_policy() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let original_policy = super::current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            mcp_servers: Vec::new(),
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "mcp_call".to_owned(),
                input: json!({"server": "demo", "tool": "search", "arguments": {"q": "rust"}}),
            },
            &context,
            &broker,
        )
        .await
        .expect("mcp_call should return a tool result");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(
            result.is_error,
            "mcp_call should fail without runtime inventory"
        );
        assert!(
            result
                .content
                .contains("MCP runtime inventory is not configured"),
            "unexpected error: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn mcp_call_rejects_ambiguous_runtime_inventory_entries() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let original_policy = super::current_tool_runtime_policy();
        let demo_server =
            |origin_kind: &str, origin_name: &str, config_name: &str| RuntimeMcpServerPolicyEntry {
                origin_kind: origin_kind.to_owned(),
                origin_name: origin_name.to_owned(),
                config_path: tempdir.path().join(config_name),
                server: McpServerConfig {
                    name: "demo".to_owned(),
                    enabled: true,
                    transport: McpTransportConfig::Stdio {
                        command: "python".to_owned(),
                        args: Vec::new(),
                        cwd: None,
                        env: Default::default(),
                    },
                    capabilities: McpCapabilityMatrix::default(),
                    startup_timeout_secs: None,
                    request_timeout_secs: None,
                    metadata: Default::default(),
                },
            };
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            mcp_servers: vec![
                demo_server("cwd", "workspace", "cwd-mcp.toml"),
                demo_server("profile", "profile", "profile-mcp.toml"),
            ],
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "mcp_call".to_owned(),
                input: json!({"server": "demo", "tool": "search", "arguments": {"q": "rust"}}),
            },
            &context,
            &broker,
        )
        .await
        .expect("mcp_call should return a tool result");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(
            result.is_error,
            "mcp_call should fail for ambiguous runtime inventory"
        );
        assert!(
            result.content.contains("ambiguous across"),
            "unexpected error: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn list_mcp_resources_returns_json() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);
        let original_policy = super::current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            mcp_servers: vec![RuntimeMcpServerPolicyEntry {
                origin_kind: "profile".to_owned(),
                origin_name: "profile".to_owned(),
                config_path: tempdir.path().join("mcp.toml"),
                server: McpServerConfig {
                    name: "test".to_owned(),
                    enabled: true,
                    transport: McpTransportConfig::Stdio {
                        command: "python".to_owned(),
                        args: Vec::new(),
                        cwd: None,
                        env: Default::default(),
                    },
                    capabilities: McpCapabilityMatrix {
                        supports_resources: true,
                        ..Default::default()
                    },
                    startup_timeout_secs: None,
                    request_timeout_secs: None,
                    metadata: Default::default(),
                },
            }],
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "list_mcp_resources".to_owned(),
                input: json!({"server": "test"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("list_mcp_resources should work");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(
            !result.is_error,
            "list_mcp_resources error: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn read_mcp_resource_returns_json() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);
        let original_policy = super::current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            mcp_servers: vec![RuntimeMcpServerPolicyEntry {
                origin_kind: "profile".to_owned(),
                origin_name: "profile".to_owned(),
                config_path: tempdir.path().join("mcp.toml"),
                server: McpServerConfig {
                    name: "test".to_owned(),
                    enabled: true,
                    transport: McpTransportConfig::Stdio {
                        command: "python".to_owned(),
                        args: Vec::new(),
                        cwd: None,
                        env: Default::default(),
                    },
                    capabilities: McpCapabilityMatrix {
                        supports_resources: true,
                        ..Default::default()
                    },
                    startup_timeout_secs: None,
                    request_timeout_secs: None,
                    metadata: Default::default(),
                },
            }],
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "read_mcp_resource".to_owned(),
                input: json!({"server": "test", "uri": "test://resource"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("read_mcp_resource should work");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(
            !result.is_error,
            "read_mcp_resource error: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn voice_input_returns_placeholder() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "voice_input".to_owned(),
                input: json!({"duration_secs": 3}),
            },
            &context,
            &broker,
        )
        .await
        .expect("voice_input should work");

        assert!(!result.is_error, "voice_input error: {}", result.content);
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "voice_input");
        assert_eq!(parsed["duration_secs"], 3);
    }

    #[tokio::test]
    async fn daemon_start_and_stop() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let broker = StaticPermissionBroker::new(true);

        let start_result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "daemon".to_owned(),
                input: json!({"action": "start", "command": "sleep 999"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("daemon start should work");

        assert!(
            !start_result.is_error,
            "daemon start error: {}",
            start_result.content
        );

        let status_result = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "daemon".to_owned(),
                input: json!({"action": "status"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("daemon status should work");

        assert!(
            !status_result.is_error,
            "daemon status error: {}",
            status_result.content
        );

        let stop_result = execute_tool_call(
            &ToolCall {
                id: "3".to_owned(),
                name: "daemon".to_owned(),
                input: json!({"action": "stop"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("daemon stop should work");

        assert!(
            !stop_result.is_error,
            "daemon stop error: {}",
            stop_result.content
        );
        assert!(
            stop_result.content.contains("Stopped 1 daemon"),
            "should stop 1 daemon"
        );
    }

    #[tokio::test]
    async fn all_phase4_tools_are_registered() {
        let specs = builtin_tool_specs();
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"powershell"),
            "powershell should be registered"
        );
        assert!(names.contains(&"repl"), "repl should be registered");
        assert!(names.contains(&"monitor"), "monitor should be registered");
        assert!(
            names.contains(&"schedule_cron"),
            "schedule_cron should be registered"
        );
        assert!(
            names.contains(&"remote_trigger"),
            "remote_trigger should be registered"
        );
        assert!(names.contains(&"workflow"), "workflow should be registered");
        assert!(
            names.contains(&"suggest_pr"),
            "suggest_pr should be registered"
        );
        assert!(
            names.contains(&"enter_worktree"),
            "enter_worktree should be registered"
        );
        assert!(
            names.contains(&"exit_worktree"),
            "exit_worktree should be registered"
        );
        assert!(names.contains(&"brief"), "brief should be registered");
        assert!(
            names.contains(&"ctx_inspect"),
            "ctx_inspect should be registered"
        );
        assert!(
            names.contains(&"list_peers"),
            "list_peers should be registered"
        );
        assert!(names.contains(&"tungsten"), "tungsten should be registered");
        assert!(
            names.contains(&"overflow_test"),
            "overflow_test should be registered"
        );
        assert!(
            names.contains(&"synthetic_output"),
            "synthetic_output should be registered"
        );
        assert!(names.contains(&"mcp_auth"), "mcp_auth should be registered");
        assert!(
            !names.contains(&"list_mcp_resources"),
            "list_mcp_resources should be injected only for resource-capable MCP servers"
        );
        assert!(
            !names.contains(&"read_mcp_resource"),
            "read_mcp_resource should be injected only for resource-capable MCP servers"
        );
        assert!(
            names.contains(&"voice_input"),
            "voice_input should be registered"
        );
        assert!(names.contains(&"daemon"), "daemon should be registered");
    }

    #[test]
    fn tool_search_result_history_discovers_tools() {
        let discovered = extract_discovered_tool_names_from_conversation(&[
            rc_core::ConversationEntry::tool(
                "tool-1",
                "tool_search",
                r#"{"query":"web","results":[{"name":"web_fetch"},{"name":"web_search"}]}"#,
                false,
            ),
            rc_core::ConversationEntry::tool(
                "tool-2",
                "toolsearch",
                r#"{"query":"tasks","found_tools":[{"name":"task_create"}]}"#,
                false,
            ),
        ]);

        assert!(discovered.contains("web_fetch"));
        assert!(discovered.contains("web_search"));
        assert!(discovered.contains("task_create"));
    }

    #[test]
    fn carried_discovered_tools_are_merged_with_conversation_history() {
        let carried = std::collections::BTreeSet::from(["web_fetch".to_owned()]);
        let discovered = extract_discovered_tool_names(
            &[rc_core::ConversationEntry::tool(
                "tool-2",
                "toolsearch",
                r#"{"query":"tasks","found_tools":[{"name":"task_create"}]}"#,
                false,
            )],
            &carried,
        );

        assert!(discovered.contains("web_fetch"));
        assert!(discovered.contains("task_create"));
    }

    #[tokio::test]
    async fn tool_search_candidates_only_include_deferred_tools() {
        let specs = runtime_tool_search_candidate_specs().await;
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"web_fetch"));
        assert!(!names.contains(&"tool_search"));
        assert!(!names.contains(&"read_file"));
        assert!(!names.contains(&"bash_command"));
    }

    #[tokio::test]
    async fn visible_provider_pool_hides_deferred_tools_until_discovered() {
        let specs = runtime_visible_provider_tool_specs(&[]).await;
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"tool_search"));
        assert!(names.contains(&"read_file"));
        assert!(!names.contains(&"web_fetch"));
        assert!(!names.contains(&"todo_write"));

        let discovered_specs =
            runtime_visible_provider_tool_specs(&[rc_core::ConversationEntry::tool(
                "tool-1",
                "tool_search",
                r#"{"query":"web","results":[{"name":"web_fetch"}]}"#,
                false,
            )])
            .await;
        let discovered_names = discovered_specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();

        assert!(discovered_names.contains(&"web_fetch"));
        assert!(!discovered_names.contains(&"todo_write"));
    }

    #[tokio::test]
    async fn task_local_runtime_policy_overlay_filters_provider_surface() {
        let outside = runtime_provider_tool_specs()
            .await
            .into_iter()
            .map(|spec| spec.name)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(outside.contains("read_file"));
        assert!(outside.contains("write_file"));

        let inside = with_tool_runtime_policy_overlay(
            ToolRuntimePolicyOverlay {
                allowed_tools: Some(vec!["read_file".to_owned()]),
                disallowed_tools: Vec::new(),
            },
            async {
                runtime_provider_tool_specs()
                    .await
                    .into_iter()
                    .map(|spec| spec.name)
                    .collect::<std::collections::BTreeSet<_>>()
            },
        )
        .await;
        assert!(inside.contains("read_file"));
        assert!(!inside.contains("write_file"));

        let after = runtime_provider_tool_specs()
            .await
            .into_iter()
            .map(|spec| spec.name)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(after.contains("write_file"));
    }

    #[tokio::test]
    async fn visible_provider_pool_keeps_full_tools_when_tool_search_is_unavailable() {
        let _runtime_policy_guard = RUNTIME_POLICY_TEST_MUTEX.lock().await;
        let original_policy = super::current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: vec!["tool_search".to_owned()],
            task_output_dir: None,
            mcp_servers: Vec::new(),
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let specs = runtime_visible_provider_tool_specs(&[]).await;
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");

        assert!(names.contains(&"web_fetch"));
        assert!(names.contains(&"todo_write"));
        assert!(!names.contains(&"tool_search"));
    }

    #[tokio::test]
    async fn visible_provider_pool_keeps_carried_deferred_tools_after_compaction() {
        let carried = std::collections::BTreeSet::from(["web_fetch".to_owned()]);
        let specs = runtime_visible_provider_tool_specs_with_discovered_tools(&[], &carried).await;
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"tool_search"));
        assert!(names.contains(&"web_fetch"));
        assert!(!names.contains(&"todo_write"));
    }
}
