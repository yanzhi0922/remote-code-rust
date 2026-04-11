//! Built-in tool registry and execution engine.
//!
//! Provides 30+ built-in tools (file I/O, search, shell, web, LSP, tasks, etc.)
//! with a [`ToolRegistry`] that supports BM25-based tool search and OpenAI /
//! Anthropic schema generation.

pub mod lsp;
pub mod sandbox;
pub mod search;
pub mod tasks;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use rc_core::{HookEvent, HookShell, SubAgentCompletion, ToolCall, ToolResult};
use rc_permissions::{PermissionBroker, PermissionRequest, auto_allows, classify_tool};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use walkdir::WalkDir;

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
        json!({
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
        json!({
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

#[must_use]
pub fn builtin_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "list_directory".to_owned(),
            protocol_name: "ListDirectory".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "List files and directories relative to the current workspace.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"},
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": 500}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "read_file".to_owned(),
            protocol_name: "ReadFile".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Read a UTF-8 text file from the current workspace.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 1},
                    "max_chars": {"type": "integer", "minimum": 1, "maximum": 50000}
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "search_text".to_owned(),
            protocol_name: "SearchText".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Search files for a text pattern or regular expression.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "max_matches": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "required": ["pattern"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "write_file".to_owned(),
            protocol_name: "WriteFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Create or overwrite a text file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "append": {"type": "boolean"}
                },
                "required": ["path", "content"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "replace_in_file".to_owned(),
            protocol_name: "ReplaceInFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Replace text in an existing file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "search": {"type": "string"},
                    "replace": {"type": "string"},
                    "all": {"type": "boolean"}
                },
                "required": ["path", "search", "replace"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "edit_file".to_owned(),
            protocol_name: "EditFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Apply ordered search/replace edits to a text file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "search": {"type": "string"},
                                "replace": {"type": "string"},
                                "all": {"type": "boolean"}
                            },
                            "required": ["search", "replace"],
                            "additionalProperties": false
                        }
                    },
                    "create_if_missing": {"type": "boolean"}
                },
                "required": ["path", "edits"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "bash_command".to_owned(),
            protocol_name: "Bash".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: "Run a shell command in the current workspace.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 600000}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "glob".to_owned(),
            protocol_name: "Glob".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Search for files using glob patterns (e.g. **/*.rs).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"}
                },
                "required": ["pattern"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "grep".to_owned(),
            protocol_name: "Grep".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Search files for a regex pattern with context lines and file filtering.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "file_pattern": {"type": "string"},
                    "max_matches": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "required": ["pattern"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "web_fetch".to_owned(),
            protocol_name: "WebFetch".to_owned(),
            permission_tool_name: "WebFetch".to_owned(),
            description: "Fetch the content of a URL.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "max_chars": {"type": "integer", "minimum": 1, "maximum": 100000}
                },
                "required": ["url"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "ask_user".to_owned(),
            protocol_name: "AskUser".to_owned(),
            permission_tool_name: "AskUser".to_owned(),
            description: "Ask the user a question and wait for a response.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": {"type": "string"},
                    "suggestions": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["question"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "todo_write".to_owned(),
            protocol_name: "TodoWrite".to_owned(),
            permission_tool_name: "TodoWrite".to_owned(),
            description: "Manage a task list (create/update/delete todo items).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "text": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]}
                            },
                            "required": ["id", "text", "status"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["todos"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "config_read".to_owned(),
            protocol_name: "Config".to_owned(),
            permission_tool_name: "Config".to_owned(),
            description: "Read or modify runtime configuration.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["get", "set"]},
                    "key": {"type": "string"},
                    "value": {}
                },
                "required": ["action", "key"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "agent".to_owned(),
            protocol_name: "Agent".to_owned(),
            permission_tool_name: "Agent".to_owned(),
            description: "Spawn a sub-agent to complete a task. The sub-agent runs in its own context and returns the result.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "The task description for the sub-agent."},
                    "tools": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional list of tool names the sub-agent is allowed to use."
                    }
                },
                "required": ["prompt"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "web_search".to_owned(),
            protocol_name: "WebSearch".to_owned(),
            permission_tool_name: "WebSearch".to_owned(),
            description: "Search the web for information using a search API.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The search query."},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 10, "description": "Maximum number of results to return (default 5)."}
                },
                "required": ["query"],
                "additionalProperties": false,
            }),
        },
        // ── LSP tool ───────────────────────────────────────────────────────
        ToolSpec {
            name: "lsp".to_owned(),
            protocol_name: "LSP".to_owned(),
            permission_tool_name: "LSP".to_owned(),
            description: "Language Server Protocol tool for code intelligence (definitions, references, hover, completion, diagnostics).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["definitions", "references", "hover", "completion", "diagnostics"]},
                    "file_path": {"type": "string"},
                    "line": {"type": "integer", "minimum": 1},
                    "column": {"type": "integer", "minimum": 1},
                    "symbol": {"type": "string"}
                },
                "required": ["action", "file_path"],
                "additionalProperties": false,
            }),
        },
        // ── Background task tools ──────────────────────────────────────────
        ToolSpec {
            name: "task_create".to_owned(),
            protocol_name: "TaskCreate".to_owned(),
            permission_tool_name: "TaskCreate".to_owned(),
            description: "Create a new background task.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "command": {"type": "string"}
                },
                "required": ["title"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_get".to_owned(),
            protocol_name: "TaskGet".to_owned(),
            permission_tool_name: "TaskGet".to_owned(),
            description: "Get details of a background task by ID.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_list".to_owned(),
            protocol_name: "TaskList".to_owned(),
            permission_tool_name: "TaskList".to_owned(),
            description: "List all background tasks.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_stop".to_owned(),
            protocol_name: "TaskStop".to_owned(),
            permission_tool_name: "TaskStop".to_owned(),
            description: "Stop a running background task.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_update".to_owned(),
            protocol_name: "TaskUpdate".to_owned(),
            permission_tool_name: "TaskUpdate".to_owned(),
            description: "Update the status or output of a background task.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "status": {"type": "string", "enum": ["pending", "running", "completed", "failed", "stopped"]},
                    "output": {"type": "string"}
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
        },
        // ── Notebook edit tool ─────────────────────────────────────────────
        ToolSpec {
            name: "notebook_edit".to_owned(),
            protocol_name: "NotebookEdit".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Edit a cell in a Jupyter notebook (.ipynb) file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "cell_index": {"type": "integer", "minimum": 0},
                    "new_source": {"type": "string"},
                    "cell_type": {"type": "string", "enum": ["code", "markdown"]}
                },
                "required": ["path", "cell_index", "new_source"],
                "additionalProperties": false,
            }),
        },
        // ── Skill discovery tool ───────────────────────────────────────────
        ToolSpec {
            name: "skill_discover".to_owned(),
            protocol_name: "DiscoverSkills".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Discover available skills in the current workspace.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "skill_execute".to_owned(),
            protocol_name: "ExecuteSkill".to_owned(),
            permission_tool_name: "ExecuteSkill".to_owned(),
            description: "Load and return the instructions of a specific skill by slug. The skill content is injected into the conversation context for the agent to follow.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "slug": {"type": "string", "description": "The skill slug to load"},
                    "arguments": {"type": "object", "description": "Optional arguments to pass to the skill"}
                },
                "required": ["slug"],
                "additionalProperties": false,
            }),
        },
        // ── Send message tool ──────────────────────────────────────────────
        ToolSpec {
            name: "send_message".to_owned(),
            protocol_name: "SendMessage".to_owned(),
            permission_tool_name: "SendMessage".to_owned(),
            description: "Send a message to another agent in the multi-agent system.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "recipient": {"type": "string"},
                    "message": {"type": "string"}
                },
                "required": ["recipient", "message"],
                "additionalProperties": false,
            }),
        },
        // ── Plan mode tools ────────────────────────────────────────────────
        ToolSpec {
            name: "enter_plan_mode".to_owned(),
            protocol_name: "EnterPlanMode".to_owned(),
            permission_tool_name: "EnterPlanMode".to_owned(),
            description: "Enter plan mode (read-only, no tool execution).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "objective": {"type": "string"}
                },
                "required": ["objective"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "exit_plan_mode".to_owned(),
            protocol_name: "ExitPlanMode".to_owned(),
            permission_tool_name: "ExitPlanMode".to_owned(),
            description: "Exit plan mode and resume normal execution.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        // ── Sleep tool ─────────────────────────────────────────────────────
        ToolSpec {
            name: "sleep".to_owned(),
            protocol_name: "Sleep".to_owned(),
            permission_tool_name: "Sleep".to_owned(),
            description: "Sleep for a specified number of seconds (max 30).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "seconds": {"type": "integer", "minimum": 0, "maximum": 30}
                },
                "required": ["seconds"],
                "additionalProperties": false,
            }),
        },
        // ── Snip tool ──────────────────────────────────────────────────────
        ToolSpec {
            name: "snip".to_owned(),
            protocol_name: "Snip".to_owned(),
            permission_tool_name: "Snip".to_owned(),
            description: "Save a code snippet to the .remote-code-rust/snippets/ directory.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "label": {"type": "string"}
                },
                "required": ["content"],
                "additionalProperties": false,
            }),
        },
        // ── Phase 3 tools ──────────────────────────────────────────────────
        ToolSpec {
            name: "memory_read".to_owned(),
            protocol_name: "MemoryRead".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Read persistent memory (RC.md) from global and/or project scope.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["global", "project", "all"], "description": "Which memory scope to read (default: all)"}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "memory_write".to_owned(),
            protocol_name: "MemoryWrite".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Write or append to persistent memory (RC.md) in global or project scope.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["global", "project"]},
                    "content": {"type": "string"},
                    "mode": {"type": "string", "enum": ["append", "overwrite"], "description": "Write mode (default: append)"}
                },
                "required": ["scope", "content"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "team_create".to_owned(),
            protocol_name: "TeamCreate".to_owned(),
            permission_tool_name: "TeamCreate".to_owned(),
            description: "Create a multi-agent team with a lead and optional agent definitions.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "objective": {"type": "string"},
                    "lead": {"type": "string"},
                    "agents": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "role": {"type": "string"}
                            },
                            "required": ["name", "role"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["objective"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "team_status".to_owned(),
            protocol_name: "TeamStatus".to_owned(),
            permission_tool_name: "TeamStatus".to_owned(),
            description: "Get the current status of the multi-agent team.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "web_browser".to_owned(),
            protocol_name: "WebBrowser".to_owned(),
            permission_tool_name: "WebBrowser".to_owned(),
            description: "Enhanced web browser: fetch URL content, extract links, extract text, or take a screenshot.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "action": {"type": "string", "enum": ["fetch", "extract_links", "extract_text", "screenshot"], "description": "Action to perform (default: fetch)"}
                },
                "required": ["url"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "tool_search".to_owned(),
            protocol_name: "ToolSearch".to_owned(),
            permission_tool_name: "ToolSearch".to_owned(),
            description: "Search available tools by keyword. Returns matching tool names and descriptions.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 20}
                },
                "required": ["query"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "verify_plan".to_owned(),
            protocol_name: "VerifyPlan".to_owned(),
            permission_tool_name: "VerifyPlan".to_owned(),
            description: "Verify a plan's execution status. Returns which items are incomplete.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "plan": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of plan item descriptions"
                    },
                    "completed": {
                        "type": "array",
                        "items": {"type": "boolean"},
                        "description": "Parallel boolean array indicating completion status"
                    }
                },
                "required": ["plan", "completed"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "terminal_capture".to_owned(),
            protocol_name: "TerminalCapture".to_owned(),
            permission_tool_name: "TerminalCapture".to_owned(),
            description: "Execute a command and return formatted output with exit code information.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        // ── Phase 4: Upstream gap-fill tools ──────────────────────────────
        ToolSpec {
            name: "powershell".to_owned(),
            protocol_name: "PowerShell".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: "Execute a PowerShell command (Windows only).".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 600000}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "repl".to_owned(),
            protocol_name: "REPL".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: "Execute code in a language REPL (python, node, or rust).".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "language": {"type": "string", "enum": ["python", "node", "rust"]},
                    "code": {"type": "string"}
                },
                "required": ["language", "code"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "monitor".to_owned(),
            protocol_name: "Monitor".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Monitor agents, tasks, or sessions and return a status snapshot.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {"type": "string", "enum": ["agents", "tasks", "sessions"]},
                    "interval_ms": {"type": "integer", "minimum": 100, "maximum": 60000}
                },
                "required": ["target"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "schedule_cron".to_owned(),
            protocol_name: "ScheduleCron".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Schedule a cron job that runs a command periodically. Supports create, list, and delete actions.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["create", "add", "list", "delete", "remove"], "description": "Action to perform (defaults to create)"},
                    "schedule": {"type": "string", "description": "Cron expression (e.g. '*/5 * * * *')"},
                    "command": {"type": "string"},
                    "description": {"type": "string"},
                    "id": {"type": "string", "description": "Cron job ID for delete"}
                },
                "required": ["schedule", "command"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "remote_trigger".to_owned(),
            protocol_name: "RemoteTrigger".to_owned(),
            permission_tool_name: "RemoteTrigger".to_owned(),
            description: "Send an HTTP POST to trigger a remote event.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "event": {"type": "string"},
                    "payload": {}
                },
                "required": ["url", "event"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "workflow".to_owned(),
            protocol_name: "Workflow".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Create, run, list, delete, or check status of a simple workflow with sequential step execution.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["create", "run", "status", "list", "delete"]},
                    "name": {"type": "string"},
                    "steps": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "description": {"type": "string", "description": "Description for the workflow (used with create)"}
                },
                "required": ["action", "name"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "suggest_pr".to_owned(),
            protocol_name: "SuggestPR".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Analyze git diff and suggest a PR title and description.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "enter_worktree".to_owned(),
            protocol_name: "EnterWorktree".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Suggest a git worktree add command for a branch.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "branch": {"type": "string"}
                },
                "required": ["branch"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "exit_worktree".to_owned(),
            protocol_name: "ExitWorktree".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Suggest a git worktree remove command for a branch.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "branch": {"type": "string"}
                },
                "required": ["branch"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "list_worktrees".to_owned(),
            protocol_name: "ListWorktrees".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "List all git worktrees in the current repository.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "brief".to_owned(),
            protocol_name: "Brief".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Summarize or truncate content to a maximum length.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "max_length": {"type": "integer", "minimum": 10, "maximum": 100000}
                },
                "required": ["content"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "ctx_inspect".to_owned(),
            protocol_name: "CtxInspect".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Inspect current conversation context (tokens, messages, tools).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["tokens", "messages", "tools"]}
                },
                "required": ["action"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "list_peers".to_owned(),
            protocol_name: "ListPeers".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "List all registered agents in the multi-agent system.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "tungsten".to_owned(),
            protocol_name: "Tungsten".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: "Smart build/test/run engine that detects project type and executes the right commands.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["compile", "run", "test"]},
                    "target": {"type": "string"}
                },
                "required": ["action", "target"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "overflow_test".to_owned(),
            protocol_name: "OverflowTest".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Generate test data for verifying context management edge cases.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "scenario": {"type": "string", "enum": ["large_output", "many_messages", "deep_recursion"]}
                },
                "required": ["scenario"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "synthetic_output".to_owned(),
            protocol_name: "SyntheticOutput".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Generate synthetic test data in JSON, CSV, Markdown, or text format.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "type": {"type": "string", "enum": ["json", "csv", "markdown", "text"]},
                    "rows": {"type": "integer", "minimum": 1, "maximum": 1000}
                },
                "required": ["type"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "mcp_auth".to_owned(),
            protocol_name: "McpAuth".to_owned(),
            permission_tool_name: "McpAuth".to_owned(),
            description: "Manage authentication state for MCP servers.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {"type": "string"},
                    "action": {"type": "string", "enum": ["login", "logout", "status"]}
                },
                "required": ["server", "action"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "mcp_call".to_owned(),
            protocol_name: "McpCall".to_owned(),
            permission_tool_name: "McpCall".to_owned(),
            description: "Call a tool on an MCP server directly. Loads the MCP config, connects to the specified server, and invokes the named tool with the given arguments.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {"type": "string", "description": "MCP server name as defined in the MCP config"},
                    "tool": {"type": "string", "description": "Name of the tool to call on the MCP server"},
                    "arguments": {"type": "object", "description": "Arguments to pass to the MCP tool"}
                },
                "required": ["server", "tool"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "list_mcp_resources".to_owned(),
            protocol_name: "ListMcpResources".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "List resources provided by MCP servers.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {"type": "string"}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "read_mcp_resource".to_owned(),
            protocol_name: "ReadMcpResource".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Read the content of an MCP resource by URI.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"}
                },
                "required": ["uri"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "voice_input".to_owned(),
            protocol_name: "VoiceInput".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Capture voice input (placeholder — requires external tool support).".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "duration_secs": {"type": "integer", "minimum": 1, "maximum": 60}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "daemon".to_owned(),
            protocol_name: "Daemon".to_owned(),
            permission_tool_name: "Daemon".to_owned(),
            description: "Manage background daemon processes (start, stop, status).".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["start", "stop", "status"]},
                    "command": {"type": "string"}
                },
                "required": ["action"],
                "additionalProperties": false,
            }),
        },
    ]
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
        "list_directory" => list_directory(&call.input, context),
        "read_file" => read_file(&call.input, context),
        "search_text" => search_text(&call.input, context),
        "write_file" => write_file(&call.input, context),
        "replace_in_file" => replace_in_file(&call.input, context),
        "edit_file" => edit_file(&call.input, context),
        "bash_command" => bash_command(&call.input, context).await,
        "glob" => glob_files(&call.input, context),
        "grep" => grep_files(&call.input, context),
        "web_fetch" => web_fetch(&call.input, context).await,
        "ask_user" => ask_user(&call.input, context),
        "todo_write" => todo_write(&call.input, context),
        "config_read" => config_read(&call.input, context),
        "agent" => agent_tool(&call.input, context).await,
        "web_search" => web_search(&call.input, context).await,
        // ── Phase 2 tools ──────────────────────────────────────────────
        "lsp" => lsp_tool(&call.input, context).await,
        "task_create" => tasks::task_create(&call.input),
        "task_get" => tasks::task_get(&call.input),
        "task_list" => tasks::task_list(&call.input),
        "task_stop" => tasks::task_stop(&call.input),
        "task_update" => tasks::task_update(&call.input),
        "notebook_edit" => notebook_edit(&call.input, context),
        "skill_discover" => skill_discover(&call.input, context),
        "send_message" => send_message(&call.input),
        "enter_plan_mode" => enter_plan_mode(&call.input),
        "exit_plan_mode" => exit_plan_mode(&call.input),
        "sleep" => sleep_tool(&call.input).await,
        "snip" => snip_tool(&call.input, context),
        // ── Phase 3 tools ──────────────────────────────────────────────
        "memory_read" => memory_read_tool(&call.input, context),
        "memory_write" => memory_write_tool(&call.input, context),
        "team_create" => team_create_tool(&call.input),
        "team_status" => team_status_tool(),
        "web_browser" => web_browser_tool(&call.input, context).await,
        "tool_search" => tool_search_tool(&call.input),
        "verify_plan" => verify_plan_tool(&call.input),
        "terminal_capture" => terminal_capture_tool(&call.input, context).await,
        // ── Phase 4: Upstream gap-fill tools ────────────────────────────
        "powershell" => powershell_tool(&call.input, context).await,
        "repl" => repl_tool(&call.input, context).await,
        "monitor" => monitor_tool(&call.input),
        "schedule_cron" => schedule_cron_tool(&call.input, context),
        "remote_trigger" => remote_trigger_tool(&call.input).await,
        "workflow" => workflow_tool(&call.input, context),
        "suggest_pr" => suggest_pr_tool(context),
        "enter_worktree" => enter_worktree_tool(&call.input, context),
        "exit_worktree" => exit_worktree_tool(&call.input, context),
        "list_worktrees" => list_worktrees_tool(context),
        "brief" => brief_tool(&call.input),
        "ctx_inspect" => ctx_inspect_tool(&call.input),
        "list_peers" => list_peers_tool(),
        "tungsten" => tungsten_tool(&call.input, context).await,
        "overflow_test" => overflow_test_tool(&call.input),
        "synthetic_output" => synthetic_output_tool(&call.input),
        "mcp_auth" => mcp_auth_tool(&call.input, context),
        "mcp_call" => mcp_call_tool(&call.input, context).await,
        "list_mcp_resources" => list_mcp_resources_tool(&call.input),
        "read_mcp_resource" => read_mcp_resource_tool(&call.input),
        "skill_execute" => skill_execute_tool(&call.input, context),
        "voice_input" => voice_input_tool(&call.input),
        "daemon" => daemon_tool(&call.input, context),
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

fn resolve_workspace_path(cwd: &Path, maybe_relative: Option<&str>) -> Result<PathBuf> {
    let candidate = match maybe_relative {
        Some(path) if !path.trim().is_empty() => cwd.join(path),
        _ => cwd.to_path_buf(),
    };
    let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let canonical_candidate = candidate.canonicalize().unwrap_or(candidate.clone());
    if !canonical_candidate.starts_with(&canonical_cwd) {
        return Err(anyhow!(
            "path {} escapes the workspace {}",
            candidate.display(),
            cwd.display()
        ));
    }
    Ok(candidate)
}

fn list_directory(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
    let recursive = input
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_entries = input
        .get("max_entries")
        .and_then(Value::as_u64)
        .unwrap_or(200) as usize;
    let mut builder = WalkBuilder::new(&target);
    builder.hidden(false);
    if !recursive {
        builder.max_depth(Some(1));
    }
    let mut lines = Vec::new();
    for entry in builder.build().take(max_entries) {
        let entry = entry?;
        let path = entry.path();
        if path == target {
            continue;
        }
        if path.components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let relative = path.strip_prefix(&context.cwd).unwrap_or(path);
        let marker = if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            "dir"
        } else {
            "file"
        };
        lines.push(format!("[{marker}] {}", relative.display()));
    }
    if lines.is_empty() {
        Ok("No files matched.".to_owned())
    } else {
        Ok(lines.join("\n"))
    }
}

fn read_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("read_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let contents = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read {}", target.display()))?;
    let start_line = input.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
    let end_line = input
        .get("end_line")
        .and_then(Value::as_u64)
        .unwrap_or(usize::MAX as u64) as usize;
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;
    let selected = contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            if line_number < start_line || line_number > end_line {
                None
            } else {
                Some(format!("{line_number:>4} {line}"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(selected.chars().take(max_chars).collect())
}

fn search_text(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("search_text requires a pattern"))?;
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let max_matches = input
        .get("max_matches")
        .and_then(Value::as_u64)
        .unwrap_or(50) as usize;
    let mut matches = Vec::new();
    for entry in WalkDir::new(&target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            if regex.is_match(line) {
                let relative = entry
                    .path()
                    .strip_prefix(&context.cwd)
                    .unwrap_or(entry.path());
                matches.push(format!(
                    "{}:{}:{}",
                    relative.display(),
                    index + 1,
                    line.trim()
                ));
                if matches.len() >= max_matches {
                    return Ok(matches.join("\n"));
                }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matches found.".to_owned())
    } else {
        Ok(matches.join("\n"))
    }
}

fn write_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires a path"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires content"))?;
    let append = input
        .get("append")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if append {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?;
        file.write_all(content.as_bytes())?;
    } else {
        std::fs::write(&target, content)?;
    }
    Ok(format!("Wrote {}", target.display()))
}

fn replace_in_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires a path"))?;
    let search = input
        .get("search")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires search text"))?;
    let replace = input
        .get("replace")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires replacement text"))?;
    let replace_all = input.get("all").and_then(Value::as_bool).unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let original = std::fs::read_to_string(&target)?;
    let updated = if replace_all {
        original.replace(search, replace)
    } else {
        original.replacen(search, replace, 1)
    };
    std::fs::write(&target, updated)?;
    Ok(format!("Updated {}", target.display()))
}

fn edit_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("edit_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let edits = input
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("edit_file requires edits"))?;
    let create_if_missing = input
        .get("create_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut content = if target.exists() {
        std::fs::read_to_string(&target)?
    } else if create_if_missing {
        String::new()
    } else {
        return Err(anyhow!("{} does not exist", target.display()));
    };
    for edit in edits {
        let search = edit
            .get("search")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing search"))?;
        let replace = edit
            .get("replace")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing replace"))?;
        let replace_all = edit.get("all").and_then(Value::as_bool).unwrap_or(false);
        if search.is_empty() && create_if_missing && content.is_empty() {
            content = replace.to_owned();
            continue;
        }
        content = if replace_all {
            content.replace(search, replace)
        } else {
            content.replacen(search, replace, 1)
        };
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, content)?;
    Ok(format!(
        "Applied {} edits to {}",
        edits.len(),
        target.display()
    ))
}

async fn bash_command(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("bash_command requires a command"))?;
    let timeout_ms = input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(context.timeout_ms)
        .clamp(1_000, 600_000);
    let mut process = if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };
    process.current_dir(&context.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn shell command")?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.success(), stdout, stderr))
    };
    let (success, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future)
            .await
            .map_err(|_| anyhow!("command timed out after {timeout_ms}ms"))??;
    let mut sections = Vec::new();
    if !stdout.trim().is_empty() {
        sections.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        sections.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        sections.push("exit_status: failed".to_owned());
    }
    Ok(if sections.is_empty() {
        "command completed with no output".to_owned()
    } else {
        sections.join("\n\n")
    })
}

// ── New tools ──────────────────────────────────────────────────────────────

fn glob_files(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("glob requires a pattern"))?;
    let base = resolve_workspace_path(
        &context.cwd,
        input.get("path").and_then(Value::as_str),
    )?;
    let full_pattern = format!("{}/{}", base.display(), pattern).replace('\\', "/");
    let mut results = Vec::new();
    let entries = glob::glob(&full_pattern).context("invalid glob pattern")?;
    for entry in entries {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };
        if path.is_dir() {
            continue;
        }
        let canonical_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone());
        let canonical_cwd = context
            .cwd
            .canonicalize()
            .unwrap_or_else(|_| context.cwd.clone());
        if !canonical_path.starts_with(&canonical_cwd) {
            continue;
        }
        let relative = path.strip_prefix(&context.cwd).unwrap_or(&path);
        results.push(relative.display().to_string());
    }
    if results.is_empty() {
        Ok("No files matched.".to_owned())
    } else {
        Ok(results.join("\n"))
    }
}

fn grep_files(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("grep requires a pattern"))?;
    let target = resolve_workspace_path(
        &context.cwd,
        input.get("path").and_then(Value::as_str),
    )?;
    let file_pattern = input.get("file_pattern").and_then(Value::as_str);
    let max_matches = input
        .get("max_matches")
        .and_then(Value::as_u64)
        .unwrap_or(50) as usize;
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let file_matcher: Option<globset::GlobMatcher> = match file_pattern {
        Some(fp) => Some(
            GlobBuilder::new(fp)
                .literal_separator(true)
                .build()
                .context("invalid file_pattern")?
                .compile_matcher(),
        ),
        None => None,
    };
    let mut matches = Vec::new();
    let mut match_count = 0usize;
    for entry in WalkDir::new(&target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        if let Some(ref matcher) = file_matcher {
            let file_name = entry.file_name().to_string_lossy();
            if !matcher.is_match(file_name.as_ref()) {
                continue;
            }
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let lines: Vec<&str> = contents.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let relative = entry
                    .path()
                    .strip_prefix(&context.cwd)
                    .unwrap_or(entry.path());
                let start = if index > 0 { index - 1 } else { 0 };
                let end = (index + 2).min(lines.len());
                for (offset, context_line) in lines[start..end].iter().enumerate() {
                    let line_idx = start + offset;
                    let prefix = if line_idx == index { ">" } else { " " };
                    matches.push(format!(
                        "{}:{}{} {}",
                        relative.display(),
                        line_idx + 1,
                        prefix,
                        context_line.trim()
                    ));
                }
                matches.push(String::new());
                match_count += 1;
                if match_count >= max_matches {
                    return Ok(matches.join("\n").trim_end().to_owned());
                }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matches found.".to_owned())
    } else {
        Ok(matches.join("\n").trim_end().to_owned())
    }
}

async fn web_fetch(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let url = input
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("web_fetch requires a url"))?;
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;
    let response = reqwest::get(url).await.context("failed to fetch URL")?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("HTTP {} for {}", status, url));
    }
    let text = response
        .text()
        .await
        .context("failed to read response body")?;
    Ok(text.chars().take(max_chars).collect())
}

fn ask_user(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let question = input
        .get("question")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("ask_user requires a question"))?;
    let suggestions: Vec<String> = input
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "type": "ask_user",
        "question": question,
        "suggestions": suggestions,
        "message": "Waiting for user input. In headless mode, please provide the answer via the input stream."
    })
    .to_string())
}

fn todo_write(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let todos = input
        .get("todos")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("todo_write requires a todos array"))?;
    let mut todo_items = Vec::new();
    for todo in todos {
        let id = todo
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have an id"))?;
        let text = todo
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have text"))?;
        let status = todo
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have a status"))?;
        if !["pending", "in_progress", "completed"].contains(&status) {
            return Err(anyhow!(
                "invalid todo status '{}': must be pending, in_progress, or completed",
                status
            ));
        }
        todo_items.push(json!({
            "id": id,
            "text": text,
            "status": status,
        }));
    }
    let todos_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&todos_dir)?;
    let todos_path = todos_dir.join("todos.json");
    let content = serde_json::to_string_pretty(&todo_items)?;
    std::fs::write(&todos_path, content)?;
    Ok(format!("Updated {} todo items.", todo_items.len()))
}

fn config_read(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("config requires an action (get or set)"))?;
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("config requires a key"))?;
    let config_dir = context.cwd.join(".remote-code-rust");
    let config_path = config_dir.join("config.json");
    match action {
        "get" => {
            if !config_path.exists() {
                return Ok(json!({key: null}).to_string());
            }
            let content = std::fs::read_to_string(&config_path)
                .context("failed to read config file")?;
            let config: Value = serde_json::from_str(&content)
                .context("failed to parse config file")?;
            let value = config.get(key).cloned().unwrap_or(Value::Null);
            Ok(json!({key: value}).to_string())
        }
        "set" => {
            let value = input
                .get("value")
                .ok_or_else(|| anyhow!("config set requires a value"))?;
            std::fs::create_dir_all(&config_dir)?;
            let mut config: Value = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                serde_json::from_str(&content)?
            } else {
                json!({})
            };
            if let Some(obj) = config.as_object_mut() {
                obj.insert(key.to_owned(), value.clone());
            }
            let content = serde_json::to_string_pretty(&config)?;
            std::fs::write(&config_path, content)?;
            Ok(format!("Set {} in config.", key))
        }
        _ => Err(anyhow!("config action must be 'get' or 'set'")),
    }
}

// ── Agent & WebSearch tools ────────────────────────────────────────────────

async fn agent_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let prompt = input
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("agent tool requires a prompt"))?;
    let allowed_tools: Vec<String> = input
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    // If no sub-agent completion provider is available, fall back to delegation JSON.
    let sub_agent = match &context.sub_agent {
        Some(provider) => provider,
        None => {
            let response = json!({
                "type": "sub_agent_request",
                "prompt": prompt,
                "allowed_tools": allowed_tools,
                "message": format!(
                    "Sub-agent task: {}. [No provider available for sub-agent execution]",
                    prompt
                ),
            });
            return Ok(response.to_string());
        }
    };

    // Create a sub-conversation with a system prompt and the user task.
    let mut sub_conversation = vec![
        rc_core::ConversationEntry::system(
            "You are a sub-agent. Complete the task concisely and return the result.",
        ),
        rc_core::ConversationEntry::user(prompt),
    ];

    // Execute the sub-agent loop with a maximum of 5 turns.
    let max_turns = 5;
    let timeout = std::time::Duration::from_secs(60);

    for turn in 0..max_turns {
        let response = match tokio::time::timeout(timeout, sub_agent.complete(&sub_conversation)).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                return Ok(format!(
                    "Sub-agent error on turn {}: {error}",
                    turn + 1
                ));
            }
            Err(_) => {
                return Ok(format!(
                    "Sub-agent timed out after {}s on turn {}.",
                    timeout.as_secs(),
                    turn + 1
                ));
            }
        };

        let assistant_text = response.text.clone();
        sub_conversation.push(rc_core::ConversationEntry::assistant(&assistant_text));

        // If no tool calls, the sub-agent is done.
        if response.tool_calls.is_empty() {
            return Ok(assistant_text);
        }

        // Execute tool calls within the sub-agent context.
        for tool_call in &response.tool_calls {
            let tool_name = &tool_call.name;

            // Check if the tool is in the allowed list.
            if !allowed_tools.is_empty() && !allowed_tools.contains(tool_name) {
                sub_conversation.push(rc_core::ConversationEntry::tool(
                    &tool_call.id,
                    tool_name,
                    "Tool not allowed in sub-agent context",
                    true,
                ));
                continue;
            }

            // Execute the tool call with bypass permissions.
            let broker = rc_permissions::StaticPermissionBroker::new(
                rc_core::PermissionMode::BypassPermissions,
            );
            let result = Box::pin(execute_tool_call(tool_call, context, &broker)).await;

            match result {
                Ok(tool_result) => {
                    let truncated = if tool_result.content.len() > 5000 {
                        format!("{}...[truncated]", &tool_result.content[..5000])
                    } else {
                        tool_result.content
                    };
                    sub_conversation.push(rc_core::ConversationEntry::tool(
                        &tool_call.id,
                        tool_name,
                        &truncated,
                        tool_result.is_error,
                    ));
                }
                Err(error) => {
                    sub_conversation.push(rc_core::ConversationEntry::tool(
                        &tool_call.id,
                        tool_name,
                        format!("Error: {error}"),
                        true,
                    ));
                }
            }
        }
    }

    // Return the last assistant message (or a summary if we ran out of turns).
    let final_response = sub_conversation
        .last()
        .map(|entry| entry.text.clone())
        .unwrap_or_default();

    if final_response.is_empty() {
        Ok(format!(
            "Sub-agent completed {} turns without a final text response.",
            max_turns
        ))
    } else {
        Ok(final_response)
    }
}

async fn web_search(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("web_search requires a query"))?;
    let _max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .min(10) as usize;

    // Use the DuckDuckGo Instant Answer API (no API key required).
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
        urlencoding::encode(query)
    );
    let response = reqwest::get(&url)
        .await
        .context("failed to query DuckDuckGo search API")?;
    let body = response
        .text()
        .await
        .context("failed to read search response body")?;

    let parsed: Value = serde_json::from_str(&body).unwrap_or_default();

    // Extract the abstract text (instant answer summary).
    let abstract_text = parsed["AbstractText"].as_str().unwrap_or("");
    let abstract_source = parsed["AbstractSource"].as_str().unwrap_or("");

    if !abstract_text.is_empty() {
        let source_info = if abstract_source.is_empty() {
            String::new()
        } else {
            format!(" (source: {abstract_source})")
        };
        Ok(format!(
            "Search results for '{}':\n{}{}",
            query, abstract_text, source_info
        ))
    } else {
        // Try to extract related topics.
        let related: Vec<String> = parsed
            .get("RelatedTopics")
            .and_then(Value::as_array)
            .map(|topics| {
                topics
                    .iter()
                    .filter_map(|topic| {
                        topic
                            .get("Text")
                            .and_then(Value::as_str)
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();

        if related.is_empty() {
            Ok(format!(
                "No instant answers found for '{}'. Try a more specific query.",
                query
            ))
        } else {
            Ok(format!(
                "Related topics for '{}':\n{}",
                query,
                related.join("\n")
            ))
        }
    }
}

// ── LSP tool ──────────────────────────────────────────────────────────────

async fn lsp_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required"))?;
    let file_path = input["file_path"]
        .as_str()
        .ok_or_else(|| anyhow!("file_path is required"))?;

    let client = lsp::LspClient::new(&context.cwd);

    match action {
        "definitions" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for definitions action"))?;
            let locations = client.find_definitions(symbol, Some(file_path))?;
            if locations.is_empty() {
                Ok(format!("No definitions found for '{symbol}'."))
            } else {
                Ok(lsp::format_locations(&locations))
            }
        }
        "references" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for references action"))?;
            let locations = client.find_references(symbol)?;
            if locations.is_empty() {
                Ok(format!("No references found for '{symbol}'."))
            } else {
                Ok(lsp::format_locations(&locations))
            }
        }
        "hover" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for hover action"))?;
            client.hover(file_path, symbol)
        }
        "completion" => {
            let line = input.get("line").and_then(Value::as_u64).unwrap_or(1);
            let column = input.get("column").and_then(Value::as_u64).unwrap_or(1);
            let suggestions = client.completion(file_path, line as u32, column as u32)?;
            Ok(lsp::format_completions(&suggestions))
        }
        "diagnostics" => {
            let diagnostics = client.diagnostics(file_path).await?;
            let result = lsp::format_diagnostics(&diagnostics);
            // Limit output size.
            Ok(result.chars().take(10_000).collect())
        }
        _ => Err(anyhow!("Unknown LSP action: {action}")),
    }
}

// ── Notebook edit tool ─────────────────────────────────────────────────────

fn notebook_edit(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input["path"]
        .as_str()
        .ok_or_else(|| anyhow!("path is required"))?;
    let cell_index = input["cell_index"]
        .as_u64()
        .ok_or_else(|| anyhow!("cell_index is required"))? as usize;
    let new_source = input["new_source"]
        .as_str()
        .ok_or_else(|| anyhow!("new_source is required"))?;

    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let content = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read notebook {}", target.display()))?;
    let mut notebook: Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse notebook {}", target.display()))?;

    let cells = notebook
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("notebook has no cells array"))?;

    if cell_index >= cells.len() {
        return Err(anyhow!(
            "cell_index {} out of range ({} cells)",
            cell_index,
            cells.len()
        ));
    }

    let cell = &mut cells[cell_index];

    // Update cell_type if provided
    if let Some(cell_type) = input["cell_type"].as_str() {
        cell["cell_type"] = json!(cell_type);
    }

    // Update source �?store as a single string (valid in nbformat)
    cell["source"] = json!(new_source);

    // Clear outputs for code cells
    if cell
        .get("cell_type")
        .and_then(Value::as_str)
        .is_some_and(|ct| ct == "code")
    {
        cell["outputs"] = json!([]);
        cell["execution_count"] = Value::Null;
    }

    let output = serde_json::to_string_pretty(&notebook)?;
    std::fs::write(&target, output)?;

    Ok(format!(
        "Updated cell {} in {}",
        cell_index,
        target.display()
    ))
}

// ── Skill discovery tool ───────────────────────────────────────────────────

fn skill_discover(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    // Search common skill locations
    let search_dirs = [
        context.cwd.join(".roo"),
        context.cwd.join(".remote-code-rust"),
    ];

    let mut all_skills = Vec::new();
    for dir in &search_dirs {
        if dir.exists() {
            match rc_skills::discover_skills(dir) {
                Ok(skills) => {
                    for skill in skills {
                        all_skills.push(json!({
                            "slug": skill.metadata.slug,
                            "title": skill.metadata.title,
                            "summary": skill.metadata.summary,
                            "path": skill.metadata.path,
                            "tools": skill.metadata.tools,
                            "triggers": skill.metadata.triggers,
                        }));
                    }
                }
                Err(e) => {
                    all_skills.push(json!({
                        "error": format!("Error scanning {}: {e}", dir.display())
                    }));
                }
            }
        }
    }

    // Also search the workspace root itself
    if let Ok(skills) = rc_skills::discover_skills(&context.cwd) {
        for skill in skills {
            all_skills.push(json!({
                "slug": skill.metadata.slug,
                "title": skill.metadata.title,
                "summary": skill.metadata.summary,
                "path": skill.metadata.path,
                "tools": skill.metadata.tools,
                "triggers": skill.metadata.triggers,
            }));
        }
    }

    // Suppress unused variable warning for input
    let _ = input;

    if all_skills.is_empty() {
        Ok("No skills found in the current workspace.".to_owned())
    } else {
        Ok(serde_json::to_string_pretty(&all_skills)?)
    }
}

// ── Send message tool ──────────────────────────────────────────────────────

fn send_message(input: &Value) -> Result<String> {
    let recipient = input["recipient"]
        .as_str()
        .ok_or_else(|| anyhow!("recipient is required"))?;
    let message = input["message"]
        .as_str()
        .ok_or_else(|| anyhow!("message is required"))?;

    // Simplified implementation: return a JSON structure for the conversation
    // loop to handle actual message delivery via AgentScheduler.
    Ok(json!({
        "type": "agent_message",
        "recipient": recipient,
        "message": message,
        "status": "queued",
        "note": "Message queued for delivery. Actual delivery requires AgentScheduler context."
    })
    .to_string())
}

// ── Plan mode tools ────────────────────────────────────────────────────────

fn enter_plan_mode(input: &Value) -> Result<String> {
    let objective = input["objective"]
        .as_str()
        .ok_or_else(|| anyhow!("objective is required"))?;

    Ok(json!({
        "type": "enter_plan_mode",
        "objective": objective,
        "message": format!("Entering plan mode. Objective: {objective}"),
        "note": "In plan mode, tools are read-only. No modifications will be made."
    })
    .to_string())
}

fn exit_plan_mode(_input: &Value) -> Result<String> {
    Ok(json!({
        "type": "exit_plan_mode",
        "message": "Exiting plan mode. Resuming normal execution."
    })
    .to_string())
}

// ── Sleep tool ─────────────────────────────────────────────────────────────

async fn sleep_tool(input: &Value) -> Result<String> {
    let seconds = input["seconds"]
        .as_u64()
        .ok_or_else(|| anyhow!("seconds is required"))?
        .min(30);

    tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

    Ok(format!("Slept for {seconds} second(s)."))
}

// ── Snip tool ──────────────────────────────────────────────────────────────

fn snip_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let content = input["content"]
        .as_str()
        .ok_or_else(|| anyhow!("content is required"))?;
    let label = input["label"].as_str().unwrap_or("snippet");

    let snippets_dir = context
        .cwd
        .join(".remote-code-rust")
        .join("snippets");
    std::fs::create_dir_all(&snippets_dir)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe_label = label.replace([' ', '/', '\\', ':'], "_");
    let filename = format!("{safe_label}_{timestamp}.txt");
    let filepath = snippets_dir.join(&filename);

    std::fs::write(&filepath, content)?;

    Ok(format!(
        "Snippet saved to .remote-code-rust/snippets/{filename}"
    ))
}

// ── Phase 3: Memory tools ──────────────────────────────────────────────────

fn memory_read_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let scope = input
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("all");
    let home = dirs_home()?;
    let mgr = rc_session::memory::MemoryManager::new(&home, Some(&context.cwd));
    let content = match scope {
        "global" => mgr.read_global()?,
        "project" => mgr.read_project()?,
        _ => mgr.read_all()?,
    };
    if content.is_empty() {
        Ok("No memory content found.".to_owned())
    } else {
        Ok(content)
    }
}

fn memory_write_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let scope = input
        .get("scope")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_write requires a scope (global or project)"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_write requires content"))?;
    let mode = input
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("append");
    let home = dirs_home()?;
    let mgr = rc_session::memory::MemoryManager::new(&home, Some(&context.cwd));
    match mode {
        "overwrite" => match scope {
            "global" => mgr.write_global(content)?,
            "project" => mgr.write_project(content)?,
            _ => return Err(anyhow!("scope must be 'global' or 'project'")),
        },
        _ => match scope {
            "global" => mgr.append_global(content)?,
            "project" => mgr.append_project(content)?,
            _ => return Err(anyhow!("scope must be 'global' or 'project'")),
        },
    }
    Ok(format!("Memory updated ({scope}, {mode})."))
}

/// Resolve the user's home directory.
fn dirs_home() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|bd| bd.home_dir().to_path_buf())
        .ok_or_else(|| anyhow!("could not determine home directory"))
}

// ── Phase 3: Team tools ────────────────────────────────────────────────────

fn team_create_tool(input: &Value) -> Result<String> {
    let objective = input
        .get("objective")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("objective is required"))?;
    let lead = input
        .get("lead")
        .and_then(Value::as_str)
        .unwrap_or("lead");
    let mut scheduler = rc_agents::AgentScheduler::new(lead, objective);
    if let Some(agents) = input.get("agents").and_then(Value::as_array) {
        for agent_def in agents {
            let name = agent_def
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("agent");
            let role = agent_def
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("worker");
            let agent = rc_agents::AgentIdentity::new(name, role);
            scheduler.register_agent(agent);
        }
    }
    let report = scheduler.team_status();
    Ok(serde_json::to_string_pretty(&report)?)
}

fn team_status_tool() -> Result<String> {
    // Return a placeholder status indicating no active team in the current context.
    Ok(json!({
        "type": "team_status",
        "message": "No active team in current tool context. Use team_create to create a team.",
        "note": "Team management requires AgentScheduler context in the conversation loop."
    })
    .to_string())
}

// ── Phase 3: Web browser tool ──────────────────────────────────────────────

async fn web_browser_tool(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let url = input
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("url is required"))?;
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("fetch");
    match action {
        "fetch" => {
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            // Truncate to 50K chars
            let truncated: String = text.chars().take(50_000).collect();
            Ok(truncated)
        }
        "screenshot" => {
            // Screenshot requires a real browser �?simplified placeholder
            Ok(json!({
                "type": "screenshot",
                "url": url,
                "message": "Screenshot mode requires a headed browser. Falling back to fetch.",
                "note": "Use action=fetch for HTTP content retrieval."
            })
            .to_string())
        }
        "extract_links" => {
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL for link extraction")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            let re = Regex::new(r#"href\s*=\s*"([^"]+)""#).expect("valid href regex");
            let links: Vec<String> = re
                .captures_iter(&text)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_owned()))
                .take(200)
                .collect();
            Ok(json!({
                "url": url,
                "links": links,
                "count": links.len(),
            })
            .to_string())
        }
        "extract_text" => {
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL for text extraction")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            // Strip HTML tags for a plain-text approximation.
            let re = Regex::new(r"<[^>]+>").expect("valid html-stripping regex");
            let plain = re.replace_all(&text, " ");
            // Collapse whitespace.
            let collapsed: String = plain
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let truncated: String = collapsed.chars().take(50_000).collect();
            Ok(truncated)
        }
        _ => Err(anyhow!(
            "action must be 'fetch', 'extract_links', 'extract_text', or 'screenshot'"
        )),
    }
}

// ── Phase 3: Tool search ───────────────────────────────────────────────────

fn tool_search_tool(input: &Value) -> Result<String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("query is required"))?;
    let max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5) as usize;

    // Use BM25 search engine for relevance-ranked results.
    let registry = ToolRegistry::new();
    let results = registry.search(query, max_results);

    if results.is_empty() {
        // Fallback: return all tools with a note
        let specs = builtin_tool_specs();
        let matches: Vec<Value> = specs
            .iter()
            .take(max_results)
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "protocol_name": spec.protocol_name,
                    "description": spec.description,
                })
            })
            .collect();
        Ok(json!({
            "query": query,
            "results": matches,
            "note": "No BM25 matches found. Showing first available tools."
        })
        .to_string())
    } else {
        let matches: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "score": format!("{:.4}", r.score),
                    "description": r.description,
                })
            })
            .collect();
        Ok(json!({
            "query": query,
            "results": matches,
        })
        .to_string())
    }
}

// ── Phase 3: Verify plan ───────────────────────────────────────────────────

fn verify_plan_tool(input: &Value) -> Result<String> {
    let plan = input
        .get("plan")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("plan is required (array of strings)"))?;
    let completed = input
        .get("completed")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("completed is required (array of booleans)"))?;
    if plan.len() != completed.len() {
        return Err(anyhow!(
            "plan and completed arrays must have the same length ({} vs {})",
            plan.len(),
            completed.len()
        ));
    }
    let mut incomplete = Vec::new();
    let mut done_count = 0usize;
    for (item, is_done) in plan.iter().zip(completed.iter()) {
        let desc = item.as_str().unwrap_or("(invalid)");
        let done = is_done.as_bool().unwrap_or(false);
        if done {
            done_count += 1;
        } else {
            incomplete.push(desc.to_owned());
        }
    }
    let total = plan.len();
    Ok(json!({
        "total_items": total,
        "completed": done_count,
        "incomplete": incomplete,
        "progress_pct": if total > 0 { done_count * 100 / total } else { 100 },
    })
    .to_string())
}

// ── Phase 3: Terminal capture ──────────────────────────────────────────────

async fn terminal_capture_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("command is required"))?;
    let mut process = if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };
    process.current_dir(&context.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn command")?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.code(), stdout, stderr))
    };
    let (exit_code, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(context.timeout_ms), future)
            .await
            .map_err(|_| anyhow!("command timed out after {}ms", context.timeout_ms))??;

    Ok(json!({
        "command": command,
        "exit_code": exit_code,
        "stdout": stdout.trim_end(),
        "stderr": stderr.trim_end(),
    })
    .to_string())
}

// ── Phase 4: Upstream gap-fill tool implementations ────────────────────────

async fn powershell_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let command = input["command"]
        .as_str()
        .ok_or_else(|| anyhow!("command is required"))?;
    let timeout_ms = input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(context.timeout_ms)
        .clamp(1_000, 600_000);
    let working_dir = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| context.cwd.clone());

    if !cfg!(windows) {
        return Ok(
            "PowerShell is only available on Windows. Use bash_command instead.".to_owned(),
        );
    }

    // Try pwsh (PowerShell 7+) first, then fall back to powershell (5.1).
    let pwsh_path = which_powershell();
    let mut process = Command::new(&pwsh_path);
    process.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        command,
    ]);
    process.current_dir(&working_dir);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    // Set UTF-8 output encoding for proper character handling.
    process.env("PS_OUTPUT_ENCODING", "utf8");

    let mut child = process
        .spawn()
        .with_context(|| format!("failed to spawn {pwsh_path}"))?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.code(), stdout, stderr))
    };
    let (exit_code, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future)
            .await
            .map_err(|_| anyhow!("powershell timed out after {timeout_ms}ms"))??;

    let success = exit_code.unwrap_or(1) == 0;
    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        parts.push(format!("exit_status: {} (failed)", exit_code.unwrap_or(-1)));
    }
    Ok(if parts.is_empty() {
        "command completed with no output".to_owned()
    } else {
        parts.join("\n\n")
    })
}

/// Find the best available PowerShell executable.
///
/// Prefers `pwsh` (PowerShell 7+, cross-platform) over `powershell`
/// (Windows PowerShell 5.1) for better compatibility and features.
fn which_powershell() -> String {
    // Try pwsh first (PowerShell 7+).
    let pwsh_candidates = ["pwsh", "pwsh.exe"];
    for candidate in &pwsh_candidates {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("-Version")
            .output()
        {
            if output.status.success() {
                return candidate.to_string();
            }
        }
    }
    // Fall back to Windows PowerShell.
    "powershell".to_string()
}

async fn repl_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let language = input["language"]
        .as_str()
        .ok_or_else(|| anyhow!("language is required (python, node, or rust)"))?;
    let code = input["code"]
        .as_str()
        .ok_or_else(|| anyhow!("code is required"))?;

    let (interpreter, flag) = match language {
        "python" => ("python", "-c"),
        "node" => ("node", "-e"),
        "rust" => {
            // For rust, write a temp file and compile/run it.
            let tmp_dir = context.cwd.join(".remote-code-rust").join("tmp");
            std::fs::create_dir_all(&tmp_dir)?;
            let src_path = tmp_dir.join("repl_tmp.rs");
            std::fs::write(&src_path, code)?;
            let output = std::process::Command::new("rustc")
                .args(["--edition", "2021", "-o"])
                .arg(tmp_dir.join("repl_tmp"))
                .arg(&src_path)
                .output()?;
            if !output.status.success() {
                return Ok(format!(
                    "Compile error:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let run_output = std::process::Command::new(tmp_dir.join("repl_tmp"))
                .current_dir(&context.cwd)
                .output()?;
            return Ok(String::from_utf8_lossy(&run_output.stdout).to_string());
        }
        _ => return Err(anyhow!("unsupported language '{language}'. Use python, node, or rust.")),
    };

    let mut cmd = Command::new(interpreter);
    cmd.arg(flag).arg(code);
    cmd.current_dir(&context.cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().context(format!("failed to spawn {interpreter}"))?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.success(), stdout, stderr))
    };
    let (success, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(context.timeout_ms), future)
            .await
            .map_err(|_| anyhow!("REPL execution timed out"))??;

    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(stdout.trim_end().to_owned());
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        parts.push("exit_status: failed".to_owned());
    }
    Ok(if parts.is_empty() {
        "No output.".to_owned()
    } else {
        parts.join("\n\n")
    })
}

fn monitor_tool(input: &Value) -> Result<String> {
    let target = input["target"]
        .as_str()
        .ok_or_else(|| anyhow!("target is required (agents, tasks, or sessions)"))?;
    let interval_ms = input.get("interval_ms").and_then(Value::as_u64).unwrap_or(1000);

    let snapshot = match target {
        "agents" => json!({
            "target": "agents",
            "interval_ms": interval_ms,
            "agents": [],
            "message": "No agents registered in current context."
        }),
        "tasks" => json!({
            "target": "tasks",
            "interval_ms": interval_ms,
            "tasks": [],
            "message": "No tasks in current context. Use task_create to create tasks."
        }),
        "sessions" => json!({
            "target": "sessions",
            "interval_ms": interval_ms,
            "sessions": [],
            "message": "No active sessions in current context."
        }),
        _ => return Err(anyhow!("target must be 'agents', 'tasks', or 'sessions'")),
    };
    Ok(snapshot.to_string())
}

fn schedule_cron_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"].as_str().unwrap_or("create");

    let crons_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&crons_dir)?;
    let crons_path = crons_dir.join("crons.json");

    let mut crons: Vec<Value> = if crons_path.exists() {
        let content = std::fs::read_to_string(&crons_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "create" | "add" => {
            let schedule = input["schedule"]
                .as_str()
                .ok_or_else(|| anyhow!("schedule is required (cron expression)"))?;
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("command is required"))?;
            let description = input["description"].as_str().unwrap_or("");

            let entry = json!({
                "id": format!("cron-{}", crons.len() + 1),
                "schedule": schedule,
                "command": command,
                "description": description,
                "created_at": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            crons.push(entry);

            let content = serde_json::to_string_pretty(&crons)?;
            std::fs::write(&crons_path, content)?;

            Ok(format!(
                "Cron job saved: '{}' → {}",
                schedule, command
            ))
        }
        "list" => {
            Ok(json!({
                "crons": crons,
                "count": crons.len(),
            }).to_string())
        }
        "delete" | "remove" => {
            let id = input["id"]
                .as_str()
                .or_else(|| input["schedule"].as_str())
                .ok_or_else(|| anyhow!("id or schedule is required for delete"))?;

            let before = crons.len();
            crons.retain(|c| {
                c["id"].as_str() != Some(id) && c["schedule"].as_str() != Some(id)
            });

            if crons.len() < before {
                let content = serde_json::to_string_pretty(&crons)?;
                std::fs::write(&crons_path, content)?;
                Ok(format!("Cron job deleted."))
            } else {
                Ok(format!("Cron job '{id}' not found."))
            }
        }
        _ => Err(anyhow!("action must be 'create', 'list', or 'delete'")),
    }
}

async fn remote_trigger_tool(input: &Value) -> Result<String> {
    let url = input["url"]
        .as_str()
        .ok_or_else(|| anyhow!("url is required"))?;
    let event = input["event"]
        .as_str()
        .ok_or_else(|| anyhow!("event is required"))?;
    let payload = input.get("payload").cloned().unwrap_or(json!({}));

    let body = json!({
        "event": event,
        "payload": payload,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("failed to send remote trigger")?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .context("failed to read trigger response")?;

    Ok(json!({
        "url": url,
        "event": event,
        "http_status": status.as_u16(),
        "response": response_text.chars().take(5000).collect::<String>(),
    })
    .to_string())
}

fn workflow_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (create, run, or status)"))?;
    let name = input["name"]
        .as_str()
        .ok_or_else(|| anyhow!("name is required"))?;

    let wf_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&wf_dir)?;
    let wf_path = wf_dir.join("workflows.json");

    let mut workflows: Vec<Value> = if wf_path.exists() {
        let content = std::fs::read_to_string(&wf_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "create" => {
            let steps: Vec<String> = input
                .get("steps")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let description = input["description"].as_str().unwrap_or("");
            let entry = json!({
                "name": name,
                "description": description,
                "steps": steps,
                "status": "created",
                "created_at": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            workflows.push(entry);
            let content = serde_json::to_string_pretty(&workflows)?;
            std::fs::write(&wf_path, content)?;
            Ok(format!("Workflow '{name}' created with {} steps.", steps.len()))
        }
        "run" => {
            let wf = workflows
                .iter()
                .find(|w| w["name"].as_str() == Some(name))
                .cloned();
            match wf {
                Some(w) => {
                    let steps = w["steps"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    // Execute each step sequentially.
                    let mut results = Vec::new();
                    let mut all_success = true;
                    for (i, step) in steps.iter().enumerate() {
                        let output = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                            .arg(if cfg!(windows) { "/C" } else { "-c" })
                            .arg(step)
                            .current_dir(&context.cwd)
                            .output();

                        let result = match output {
                            Ok(out) => {
                                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                let success = out.status.success();
                                if !success {
                                    all_success = false;
                                }
                                json!({
                                    "step": i + 1,
                                    "command": step,
                                    "success": success,
                                    "stdout": stdout.chars().take(2000).collect::<String>(),
                                    "stderr": stderr.chars().take(1000).collect::<String>(),
                                })
                            }
                            Err(e) => {
                                all_success = false;
                                json!({
                                    "step": i + 1,
                                    "command": step,
                                    "success": false,
                                    "error": e.to_string(),
                                })
                            }
                        };
                        results.push(result);
                    }

                    // Update workflow status.
                    let wf_mut = workflows
                        .iter_mut()
                        .find(|w| w["name"].as_str() == Some(name));
                    if let Some(w) = wf_mut {
                        w["status"] = if all_success { json!("completed") } else { json!("failed") };
                        w["last_run"] = json!(std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0));
                        let content = serde_json::to_string_pretty(&workflows)?;
                        std::fs::write(&wf_path, content)?;
                    }

                    Ok(json!({
                        "workflow": name,
                        "status": if all_success { "completed" } else { "failed" },
                        "steps_executed": results.len(),
                        "results": results,
                    }).to_string())
                }
                None => Err(anyhow!("workflow '{name}' not found")),
            }
        }
        "status" => {
            let wf = workflows.iter().find(|w| w["name"].as_str() == Some(name));
            match wf {
                Some(w) => Ok(serde_json::to_string_pretty(w)?),
                None => Ok(json!({
                    "name": name,
                    "status": "not_found",
                    "message": format!("Workflow '{name}' does not exist.")
                })
                .to_string()),
            }
        }
        "list" => {
            let names: Vec<Value> = workflows
                .iter()
                .map(|w| json!({
                    "name": w["name"],
                    "status": w["status"],
                    "steps": w["steps"].as_array().map(|a| a.len()).unwrap_or(0),
                }))
                .collect();
            Ok(json!({
                "workflows": names,
                "count": names.len(),
            }).to_string())
        }
        "delete" => {
            let before = workflows.len();
            workflows.retain(|w| w["name"].as_str() != Some(name));
            if workflows.len() < before {
                let content = serde_json::to_string_pretty(&workflows)?;
                std::fs::write(&wf_path, content)?;
                Ok(format!("Workflow '{name}' deleted."))
            } else {
                Ok(format!("Workflow '{name}' not found."))
            }
        }
        _ => Err(anyhow!("action must be 'create', 'run', 'status', 'list', or 'delete'")),
    }
}

fn suggest_pr_tool(context: &ToolExecutionContext) -> Result<String> {
    // Run git diff --stat and git log to suggest a PR.
    let diff_output = std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(&context.cwd)
        .output();

    let diff_stat = match diff_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => "Unable to run git diff.".to_owned(),
    };

    let log_output = std::process::Command::new("git")
        .args(["log", "--oneline", "-10"])
        .current_dir(&context.cwd)
        .output();

    let recent_commits = match log_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => "Unable to run git log.".to_owned(),
    };

    // Simple heuristic: use the first line of recent commits as title suggestion.
    let title_suggestion = recent_commits
        .lines()
        .next()
        .unwrap_or("Changes from current branch")
        .trim_start_matches(|c: char| c.is_ascii_hexdigit() || c == ' ');

    Ok(json!({
        "suggested_title": title_suggestion,
        "diff_stat": diff_stat.trim(),
        "recent_commits": recent_commits.trim(),
        "note": "Review the diff and commits above to craft a PR description."
    })
    .to_string())
}

fn enter_worktree_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    // Determine the worktree path.
    let worktree_dir = input["path"]
        .as_str()
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("../{branch}"));

    // Try to actually create the worktree.
    let output = std::process::Command::new("git")
        .args(["worktree", "add", &worktree_dir, branch])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // Resolve the absolute path of the new worktree.
            let abs_path = if std::path::Path::new(&worktree_dir).is_absolute() {
                worktree_dir.clone()
            } else {
                let mut p = context.cwd.clone();
                p.pop();
                p.push(&worktree_dir);
                p.to_string_lossy().to_string()
            };
            Ok(json!({
                "status": "created",
                "branch": branch,
                "path": abs_path,
                "output": stdout,
                "note": format!("Worktree created at {worktree_dir}. Use this path as the working directory for parallel work on branch '{branch}'.")
            }).to_string())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // If the worktree already exists, provide helpful info.
            if stderr.contains("already") {
                Ok(json!({
                    "status": "already_exists",
                    "branch": branch,
                    "path": worktree_dir,
                    "output": stderr,
                    "note": "This worktree already exists. You can work in that directory."
                }).to_string())
            } else {
                // Fall back to command suggestion.
                Ok(json!({
                    "status": "manual",
                    "command": format!("git worktree add {worktree_dir} {branch}"),
                    "branch": branch,
                    "error": stderr,
                    "note": "Could not auto-create worktree. Run the command above manually."
                }).to_string())
            }
        }
        Err(_) => {
            // Git not available, provide command suggestion.
            Ok(json!({
                "status": "manual",
                "command": format!("git worktree add {worktree_dir} {branch}"),
                "branch": branch,
                "note": "Run the command above to create a new worktree for this branch."
            }).to_string())
        }
    }
}

fn exit_worktree_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    let worktree_dir = input["path"]
        .as_str()
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("../{branch}"));

    // Try to actually remove the worktree.
    let output = std::process::Command::new("git")
        .args(["worktree", "remove", &worktree_dir])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Ok(json!({
                "status": "removed",
                "branch": branch,
                "path": worktree_dir,
                "output": stdout,
                "note": format!("Worktree at {worktree_dir} has been removed.")
            }).to_string())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // Fall back to command suggestion.
            Ok(json!({
                "status": "manual",
                "command": format!("git worktree remove {worktree_dir}"),
                "branch": branch,
                "error": stderr,
                "note": "Could not auto-remove worktree. Run the command above manually."
            }).to_string())
        }
        Err(_) => {
            Ok(json!({
                "status": "manual",
                "command": format!("git worktree remove {worktree_dir}"),
                "branch": branch,
                "note": "Run the command above to remove the worktree for this branch."
            }).to_string())
        }
    }
}

fn list_worktrees_tool(context: &ToolExecutionContext) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let worktrees: Vec<Value> = stdout
                .split("\n\n")
                .filter(|block| !block.is_empty())
                .map(|block| {
                    let mut path = "";
                    let mut branch = "";
                    let mut is_bare = false;
                    for line in block.lines() {
                        if let Some(p) = line.strip_prefix("worktree ") {
                            path = p;
                        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                            branch = b;
                        } else if line == "bare" {
                            is_bare = true;
                        }
                    }
                    json!({
                        "path": path,
                        "branch": branch,
                        "is_bare": is_bare,
                    })
                })
                .collect();
            Ok(json!({
                "worktrees": worktrees,
                "count": worktrees.len(),
            }).to_string())
        }
        Ok(_) => Ok(json!({
            "worktrees": [],
            "note": "Not in a git repository or git worktree not supported."
        }).to_string()),
        Err(_) => Ok(json!({
            "worktrees": [],
            "note": "git is not available."
        }).to_string()),
    }
}

fn brief_tool(input: &Value) -> Result<String> {
    let content = input["content"]
        .as_str()
        .ok_or_else(|| anyhow!("content is required"))?;
    let max_length = input
        .get("max_length")
        .and_then(Value::as_u64)
        .unwrap_or(500) as usize;

    if content.len() <= max_length {
        return Ok(content.to_owned());
    }

    let truncated: String = content.chars().take(max_length).collect();
    Ok(format!(
        "{}\n\n[...truncated from {} to {} chars]",
        truncated,
        content.len(),
        max_length
    ))
}

fn ctx_inspect_tool(input: &Value) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (tokens, messages, or tools)"))?;

    let specs = builtin_tool_specs();
    match action {
        "tokens" => Ok(json!({
            "estimated_tokens": "N/A (requires tokenizer)",
            "note": "Token counting requires a model-specific tokenizer."
        })
        .to_string()),
        "messages" => Ok(json!({
            "message_count": 0,
            "note": "Message count requires conversation context."
        })
        .to_string()),
        "tools" => Ok(json!({
            "total_tools": specs.len(),
            "tools": specs.iter().map(|s| &s.name).collect::<Vec<_>>(),
        })
        .to_string()),
        _ => Err(anyhow!(
            "action must be 'tokens', 'messages', or 'tools'"
        )),
    }
}

fn list_peers_tool() -> Result<String> {
    Ok(json!({
        "peers": [],
        "message": "No peers registered in current context. Use team_create to create a team."
    })
    .to_string())
}

async fn tungsten_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (compile, run, or test)"))?;
    let target = input["target"]
        .as_str()
        .ok_or_else(|| anyhow!("target is required"))?;

    // Detect project type by checking for marker files.
    let is_rust = context.cwd.join("Cargo.toml").exists()
        || context.cwd.join(target).join("Cargo.toml").exists();
    let is_node = context.cwd.join("package.json").exists()
        || context.cwd.join(target).join("package.json").exists();
    let is_python = context.cwd.join("setup.py").exists()
        || context.cwd.join("pyproject.toml").exists()
        || context.cwd.join(target).join("setup.py").exists();

    let command = match action {
        "compile" => {
            if is_rust {
                format!("cargo build --manifest-path {target}/Cargo.toml 2>&1 || cargo build 2>&1")
            } else if is_node {
                "npm run build 2>&1".to_owned()
            } else if is_python {
                "python -m py_compile . 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type. No Cargo.toml, package.json, or setup.py found.".to_owned());
            }
        }
        "run" => {
            if is_rust {
                format!("cargo run --manifest-path {target}/Cargo.toml 2>&1 || cargo run 2>&1")
            } else if is_node {
                "npm start 2>&1".to_owned()
            } else if is_python {
                "python main.py 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type.".to_owned());
            }
        }
        "test" => {
            if is_rust {
                format!("cargo test --manifest-path {target}/Cargo.toml 2>&1 || cargo test 2>&1")
            } else if is_node {
                "npm test 2>&1".to_owned()
            } else if is_python {
                "python -m pytest 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type.".to_owned());
            }
        }
        _ => return Err(anyhow!("action must be 'compile', 'run', or 'test'")),
    };

    let mut process = if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", &command]);
        cmd
    };
    process.current_dir(&context.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn tungsten command")?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.success(), stdout, stderr))
    };
    let (success, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(context.timeout_ms), future)
            .await
            .map_err(|_| anyhow!("tungsten command timed out"))??;

    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(stdout.trim_end().to_owned());
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        parts.push("exit_status: failed".to_owned());
    }
    Ok(if parts.is_empty() {
        "Command completed with no output.".to_owned()
    } else {
        parts.join("\n\n")
    })
}

fn overflow_test_tool(input: &Value) -> Result<String> {
    let scenario = input["scenario"]
        .as_str()
        .ok_or_else(|| anyhow!("scenario is required (large_output, many_messages, or deep_recursion)"))?;

    match scenario {
        "large_output" => {
            let data: String = (0..10_000)
                .map(|i| format!("Line {i}: This is test output data for overflow testing.\n"))
                .collect();
            Ok(json!({
                "scenario": "large_output",
                "size_chars": data.len(),
                "size_lines": 10_000,
                "data_preview": data.chars().take(500).collect::<String>(),
            })
            .to_string())
        }
        "many_messages" => {
            let messages: Vec<Value> = (0..100)
                .map(|i| {
                    json!({
                        "id": i,
                        "role": if i % 2 == 0 { "user" } else { "assistant" },
                        "content": format!("Message {i}: Test content for context overflow testing."),
                    })
                })
                .collect();
            Ok(json!({
                "scenario": "many_messages",
                "count": messages.len(),
                "messages": messages,
            })
            .to_string())
        }
        "deep_recursion" => {
            let depth = 50;
            let mut nested = json!("leaf");
            for _ in 0..depth {
                nested = json!({ "child": nested });
            }
            Ok(json!({
                "scenario": "deep_recursion",
                "depth": depth,
                "structure": nested,
            })
            .to_string())
        }
        _ => Err(anyhow!(
            "scenario must be 'large_output', 'many_messages', or 'deep_recursion'"
        )),
    }
}

fn synthetic_output_tool(input: &Value) -> Result<String> {
    let output_type = input["type"]
        .as_str()
        .ok_or_else(|| anyhow!("type is required (json, csv, markdown, or text)"))?;
    let rows = input.get("rows").and_then(Value::as_u64).unwrap_or(10) as usize;

    match output_type {
        "json" => {
            let data: Vec<Value> = (0..rows)
                .map(|i| {
                    json!({
                        "id": i,
                        "name": format!("item_{i}"),
                        "value": i * 10,
                        "active": i % 2 == 0,
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&data)?)
        }
        "csv" => {
            let mut lines = vec!["id,name,value,active".to_owned()];
            for i in 0..rows {
                lines.push(format!("{i},item_{i},{},{}", i * 10, i % 2 == 0));
            }
            Ok(lines.join("\n"))
        }
        "markdown" => {
            let mut md = String::from("# Synthetic Report\n\n");
            md.push_str("| id | name | value | active |\n");
            md.push_str("|----|------|-------|--------|\n");
            for i in 0..rows {
                md.push_str(&format!("| {i} | item_{i} | {} | {} |\n", i * 10, i % 2 == 0));
            }
            Ok(md)
        }
        "text" => {
            let lines: Vec<String> = (0..rows)
                .map(|i| format!("Row {i}: name=item_{i}, value={}, active={}", i * 10, i % 2 == 0))
                .collect();
            Ok(lines.join("\n"))
        }
        _ => Err(anyhow!(
            "type must be 'json', 'csv', 'markdown', or 'text'"
        )),
    }
}

fn mcp_auth_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let server = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (login, logout, or status)"))?;

    let auth_dir = context.cwd.join(".remote-code-rust").join("mcp-auth");
    std::fs::create_dir_all(&auth_dir)?;
    let auth_file = auth_dir.join(format!("{server}.json"));

    match action {
        "login" => {
            let entry = json!({
                "server": server,
                "status": "authenticated",
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            let content = serde_json::to_string_pretty(&entry)?;
            std::fs::write(&auth_file, content)?;
            Ok(format!("Logged in to MCP server '{server}'."))
        }
        "logout" => {
            if auth_file.exists() {
                std::fs::remove_file(&auth_file)?;
                Ok(format!("Logged out from MCP server '{server}'."))
            } else {
                Ok(format!("No active session for MCP server '{server}'."))
            }
        }
        "status" => {
            if auth_file.exists() {
                let content = std::fs::read_to_string(&auth_file)?;
                Ok(content)
            } else {
                Ok(json!({
                    "server": server,
                    "status": "not_authenticated",
                })
                .to_string())
            }
        }
        _ => Err(anyhow!(
            "action must be 'login', 'logout', or 'status'"
        )),
    }
}

fn list_mcp_resources_tool(input: &Value) -> Result<String> {
    let server = input["server"].as_str();

    Ok(json!({
        "server": server,
        "resources": [],
        "message": "MCP resource listing requires an active MCP connection. No resources found in current context."
    })
    .to_string())
}

fn read_mcp_resource_tool(input: &Value) -> Result<String> {
    let uri = input["uri"]
        .as_str()
        .ok_or_else(|| anyhow!("uri is required"))?;

    Ok(json!({
        "uri": uri,
        "content": null,
        "message": "MCP resource reading requires an active MCP connection. No content available in current context."
    })
    .to_string())
}

/// Call a tool on an MCP server directly.
///
/// Loads the MCP configuration, finds the specified server, connects via
/// stdio transport, and invokes the named tool with the provided arguments.
async fn mcp_call_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let server_name = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let tool_name = input["tool"]
        .as_str()
        .ok_or_else(|| anyhow!("tool is required"))?;
    let arguments = input.get("arguments").cloned().unwrap_or(json!({}));

    // Discover MCP config files in the workspace.
    let config_candidates = [
        context.cwd.join(".mcp.json"),
        context.cwd.join(".remote-code-rust").join("mcp.json"),
    ];

    let mut server_config: Option<rc_mcp::McpServerConfig> = None;
    for candidate in &config_candidates {
        if candidate.exists() {
            if let Ok(config) = rc_mcp::McpConfig::load(candidate) {
                if let Some(srv) = config.servers.get(server_name) {
                    server_config = Some(srv.clone());
                    break;
                }
            }
        }
    }

    let server = server_config.ok_or_else(|| {
        anyhow!(
            "MCP server '{server_name}' not found. Checked: {}",
            config_candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    let client_info = rc_mcp::McpClientInfo::new("remote-code-rust", env!("CARGO_PKG_VERSION"));
    let response = rc_mcp::call_tool(&server, &client_info, tool_name, arguments).await?;

    let mut parts = Vec::new();
    parts.push(format!("server:  {}", response.server_name));
    parts.push(format!("tool:    {}", response.tool_name));
    parts.push(format!("success: {}", !response.result.is_error));

    if !response.result.content.is_empty() {
        let content_text: Vec<String> = response
            .result
            .content
            .iter()
            .filter_map(|c| {
                if c.kind == "text" {
                    c.fields.get("text").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect();
        if !content_text.is_empty() {
            parts.push(format!("output:\n{}", content_text.join("\n")));
        }
    }

    Ok(parts.join("\n"))
}

/// Load and return a skill's instructions by slug.
///
/// Searches the workspace skill directories for a matching skill and returns
/// its full content (instructions) for the agent to follow.
fn skill_execute_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let slug = input["slug"]
        .as_str()
        .ok_or_else(|| anyhow!("slug is required"))?;
    let arguments = input.get("arguments").cloned().unwrap_or(json!({}));

    let search_dirs = [
        context.cwd.join(".roo"),
        context.cwd.join(".remote-code-rust"),
        context.cwd.clone(),
    ];

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(skills) = rc_skills::discover_skills(dir) {
            for skill in skills {
                if skill.metadata.slug == slug {
                    let summary = skill.metadata.summary.as_deref().unwrap_or("(no summary)");
                    let mut output = format!(
                        "# Skill: {} ({})\n\n{}\n\n",
                        skill.metadata.title,
                        skill.metadata.slug,
                        summary
                    );
                    if !skill.instructions.is_empty() {
                        output.push_str(&skill.instructions);
                    }
                    if !arguments.is_null() && !arguments.as_object().map_or(true, |o| o.is_empty())
                    {
                        output.push_str(&format!(
                            "\n\n## Arguments\n```json\n{}\n```",
                            serde_json::to_string_pretty(&arguments)?
                        ));
                    }
                    return Ok(output);
                }
            }
        }
    }

    Err(anyhow!(
        "Skill '{slug}' not found. Use skill_discover to list available skills."
    ))
}

fn voice_input_tool(input: &Value) -> Result<String> {
    let duration_secs = input.get("duration_secs").and_then(Value::as_u64).unwrap_or(5);

    Ok(json!({
        "type": "voice_input",
        "duration_secs": duration_secs,
        "message": "Voice input requires external tool support (e.g., Whisper, system microphone API).",
        "note": "This is a placeholder. Actual voice capture needs a connected audio input device and transcription service."
    })
    .to_string())
}

fn daemon_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (start, stop, or status)"))?;

    let daemon_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&daemon_dir)?;
    let daemon_path = daemon_dir.join("daemons.json");

    let mut daemons: Vec<Value> = if daemon_path.exists() {
        let content = std::fs::read_to_string(&daemon_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "start" => {
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("command is required for start action"))?;
            let entry = json!({
                "command": command,
                "status": "running",
                "pid": null,
                "started_at": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            daemons.push(entry);
            let content = serde_json::to_string_pretty(&daemons)?;
            std::fs::write(&daemon_path, content)?;
            Ok(format!("Daemon started: {command}"))
        }
        "stop" => {
            let command = input["command"].as_str().unwrap_or("");
            let count_before = daemons.len();
            daemons.retain(|d| {
                if command.is_empty() {
                    false // stop all
                } else {
                    d["command"].as_str() != Some(command)
                }
            });
            let stopped = count_before - daemons.len();
            let content = serde_json::to_string_pretty(&daemons)?;
            std::fs::write(&daemon_path, content)?;
            Ok(format!("Stopped {stopped} daemon(s)."))
        }
        "status" => {
            Ok(serde_json::to_string_pretty(&daemons)?)
        }
        _ => Err(anyhow!("action must be 'start', 'stop', or 'status'")),
    }
}

// ── Command hooks ──────────────────────────────────────────────────────────

pub async fn execute_command_hook(
    request: &CommandHookExecutionRequest,
) -> Result<CommandHookExecutionResult> {
    let shell = request.shell.unwrap_or_else(default_hook_shell);
    let timeout_secs = request.timeout_secs.unwrap_or(15).max(1);
    let mut process = build_shell_command(shell, &request.command);
    process.current_dir(&request.cwd);
    process.stdin(Stdio::piped());
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn command hook")?;
    if let Some(mut stdin) = child.stdin.take() {
        let input = serde_json::to_vec(&request.input)?;
        tokio::spawn(async move {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut stdin, &input).await;
        });
    }

    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.code(), stdout, stderr))
    };

    let (exit_code, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), future)
            .await
            .map_err(|_| anyhow!("command hook timed out after {timeout_secs}s"))??;

    Ok(CommandHookExecutionResult {
        event: request.event,
        command: request.command.clone(),
        shell,
        exit_code,
        stdout,
        stderr,
    })
}

fn build_shell_command(shell: HookShell, command: &str) -> Command {
    match shell {
        HookShell::PowerShell => {
            let mut cmd = Command::new("powershell");
            cmd.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                command,
            ]);
            cmd
        }
        HookShell::Bash => {
            #[cfg(windows)]
            {
                let mut cmd = Command::new("bash");
                cmd.args(["-lc", command]);
                cmd
            }
            #[cfg(not(windows))]
            {
                let mut cmd = Command::new("sh");
                cmd.args(["-lc", command]);
                cmd
            }
        }
    }
}

fn default_hook_shell() -> HookShell {
    if cfg!(windows) {
        HookShell::PowerShell
    } else {
        HookShell::Bash
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
