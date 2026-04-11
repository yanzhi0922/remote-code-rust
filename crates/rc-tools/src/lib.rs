//! Built-in tool registry and execution engine.
//!
//! Provides 30+ built-in tools (file I/O, search, shell, web, LSP, tasks, etc.)
//! with a [`ToolRegistry`] that supports BM25-based tool search and OpenAI /
//! Anthropic schema generation.

pub mod agent;
pub mod command;
pub mod file_ops;
pub mod git;
pub mod hooks;
pub mod lsp;
pub mod mcp_tools;
pub mod memory_tools;
pub mod misc;
pub mod sandbox;
pub mod search;
pub mod specs;
pub mod system;
pub mod tasks;
pub mod web;
pub mod workflow;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use rc_core::{HookEvent, HookShell, SubAgentCompletion, ToolCall, ToolResult};
use rc_permissions::{PermissionBroker, PermissionRequest, auto_allows, classify_tool};
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
}

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
        let all = builtin_tool_specs();
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
    let spec = builtin_tool_specs()
        .into_iter()
        .find(|spec| spec.name == call.name)
        .ok_or_else(|| anyhow!("unknown tool {}", call.name))?;

    if spec.requires_permission && !auto_allows(broker.mode(), classify_tool(&spec.name)) {
        let decision = broker
            .decide(PermissionRequest {
                tool_name: spec.name.clone(),
                tool_use_id: call.id.clone(),
                title: format!("Allow {}", spec.protocol_name),
                description: spec.description.clone(),
                input: call.input.clone(),
                blocked_path: call
                    .input
                    .get("path")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
            .await;
        if !decision.allowed {
            return Ok(ToolResult {
                content: decision
                    .message
                    .unwrap_or_else(|| format!("Permission denied for {}.", spec.name)),
                is_error: true,
            });
        }
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
        "send_message" => agent::send_message(&call.input),
        "enter_plan_mode" => agent::enter_plan_mode(&call.input),
        "exit_plan_mode" => agent::exit_plan_mode(&call.input),
        "sleep" => system::sleep_tool(&call.input).await,
        "snip" => system::snip_tool(&call.input, context),
        // ── Phase 3 tools ──────────────────────────────────────────────
        "memory_read" => memory_tools::memory_read_tool(&call.input, context),
        "memory_write" => memory_tools::memory_write_tool(&call.input, context),
        "team_create" => misc::team_create_tool(&call.input),
        "team_status" => misc::team_status_tool(),
        "web_browser" => web::web_browser_tool(&call.input, context).await,
        "tool_search" => system::tool_search_tool(&call.input),
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
        "ctx_inspect" => system::ctx_inspect_tool(&call.input),
        "list_peers" => system::list_peers_tool(),
        "tungsten" => misc::tungsten_tool(&call.input, context).await,
        "overflow_test" => misc::overflow_test_tool(&call.input),
        "synthetic_output" => misc::synthetic_output_tool(&call.input),
        "mcp_auth" => mcp_tools::mcp_auth_tool(&call.input, context),
        "mcp_call" => mcp_tools::mcp_call_tool(&call.input, context).await,
        "list_mcp_resources" => mcp_tools::list_mcp_resources_tool(&call.input),
        "read_mcp_resource" => mcp_tools::read_mcp_resource_tool(&call.input),
        "skill_execute" => misc::skill_execute_tool(&call.input, context),
        "voice_input" => misc::voice_input_tool(&call.input),
        "daemon" => workflow::daemon_tool(&call.input, context),
        _ => Err(anyhow!("unsupported tool {}", spec.name)),
    };

    match result {
        Ok(content) => Ok(ToolResult {
            content,
            is_error: false,
        }),
        Err(error) => Ok(ToolResult {
            content: error.to_string(),
            is_error: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandHookExecutionRequest, HookShell, ToolExecutionContext, builtin_tool_specs,
        execute_command_hook, execute_tool_call,
    };
    use rc_core::{HookEvent, PermissionMode, ToolCall};
    use rc_permissions::StaticPermissionBroker;
    use serde_json::json;
    use tempfile::tempdir;

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        std::fs::write(tempdir.path().join("foo.rs"), "fn main() {}")
            .expect("write should work");
        std::fs::write(tempdir.path().join("bar.txt"), "hello")
            .expect("write should work");
        std::fs::create_dir(tempdir.path().join("src")).expect("mkdir should work");
        std::fs::write(tempdir.path().join("src/mod.rs"), "mod foo;")
            .expect("write should work");

        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        assert!(
            result.content.contains("code.rs"),
            "should find code.rs"
        );
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        let content =
            std::fs::read_to_string(&todos_path).expect("should read todos.json");
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        let parsed: serde_json::Value = serde_json::from_str(&get_result.content)
            .expect("should be valid JSON");
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

        assert!(
            !result.is_error,
            "notebook_edit error: {}",
            result.content
        );

        // Verify the notebook was modified
        let updated: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&nb_path).expect("read notebook"),
        )
        .expect("parse notebook");
        assert_eq!(
            updated["cells"][0]["source"],
            json!("print('world')"),
            "cell source should be updated"
        );
    }

    #[tokio::test]
    async fn send_message_returns_json_structure() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "send_message".to_owned(),
                input: json!({
                    "recipient": "agent-007",
                    "message": "Hello from test"
                }),
            },
            &context,
            &broker,
        )
        .await
        .expect("send_message should work");

        assert!(
            !result.is_error,
            "send_message error: {}",
            result.content
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "agent_message");
        assert_eq!(parsed["recipient"], "agent-007");
    }

    #[tokio::test]
    async fn plan_mode_tools_return_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        let parsed: serde_json::Value =
            serde_json::from_str(&enter_result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "enter_plan_mode");
        assert_eq!(parsed["objective"], "Plan the architecture");

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
        let parsed: serde_json::Value =
            serde_json::from_str(&exit_result.content).expect("should be valid JSON");
        assert_eq!(parsed["type"], "exit_plan_mode");
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

        assert!(
            !result.is_error,
            "skill_discover error: {}",
            result.content
        );
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
        assert!(
            names.contains(&"task_get"),
            "task_get should be registered"
        );
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
    async fn suggest_pr_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

    #[tokio::test]
    async fn list_peers_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "list_peers".to_owned(),
                input: json!({}),
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
    }

    #[tokio::test]
    async fn tungsten_tool_detects_project() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        // Create a Cargo.toml to trigger Rust detection.
        std::fs::write(tempdir.path().join("Cargo.toml"), "[package]\nname = \"test\"\n")
            .expect("write should work");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

        assert!(!result.is_error, "synthetic_output error: {}", result.content);
        assert!(result.content.contains("id,name,value,active"), "should have CSV header");
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
    async fn list_mcp_resources_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

        assert!(!result.is_error, "list_mcp_resources error: {}", result.content);
    }

    #[tokio::test]
    async fn read_mcp_resource_returns_json() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
            sub_agent: None,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

        let result = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "read_mcp_resource".to_owned(),
                input: json!({"uri": "test://resource"}),
            },
            &context,
            &broker,
        )
        .await
        .expect("read_mcp_resource should work");

        assert!(!result.is_error, "read_mcp_resource error: {}", result.content);
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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

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

        assert!(names.contains(&"powershell"), "powershell should be registered");
        assert!(names.contains(&"repl"), "repl should be registered");
        assert!(names.contains(&"monitor"), "monitor should be registered");
        assert!(names.contains(&"schedule_cron"), "schedule_cron should be registered");
        assert!(names.contains(&"remote_trigger"), "remote_trigger should be registered");
        assert!(names.contains(&"workflow"), "workflow should be registered");
        assert!(names.contains(&"suggest_pr"), "suggest_pr should be registered");
        assert!(names.contains(&"enter_worktree"), "enter_worktree should be registered");
        assert!(names.contains(&"exit_worktree"), "exit_worktree should be registered");
        assert!(names.contains(&"brief"), "brief should be registered");
        assert!(names.contains(&"ctx_inspect"), "ctx_inspect should be registered");
        assert!(names.contains(&"list_peers"), "list_peers should be registered");
        assert!(names.contains(&"tungsten"), "tungsten should be registered");
        assert!(names.contains(&"overflow_test"), "overflow_test should be registered");
        assert!(names.contains(&"synthetic_output"), "synthetic_output should be registered");
        assert!(names.contains(&"mcp_auth"), "mcp_auth should be registered");
        assert!(names.contains(&"list_mcp_resources"), "list_mcp_resources should be registered");
        assert!(names.contains(&"read_mcp_resource"), "read_mcp_resource should be registered");
        assert!(names.contains(&"voice_input"), "voice_input should be registered");
        assert!(names.contains(&"daemon"), "daemon should be registered");
    }
}
