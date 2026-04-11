//! Built-in tool specifications (schemas for all 40+ tools).

use serde_json::json;

use super::ToolSpec;

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
            description: "Capture voice input via microphone, record audio, and transcribe to text using whisper.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "duration_secs": {"type": "integer", "minimum": 1, "maximum": 60, "description": "Recording duration in seconds (default 5)"},
                    "language": {"type": "string", "description": "Language code for transcription (default 'en')"}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "daemon".to_owned(),
            protocol_name: "Daemon".to_owned(),
            permission_tool_name: "Daemon".to_owned(),
            description: "Manage background daemon processes: start (spawn background), stop (kill by id), status, list, restart, and logs.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["start", "stop", "status", "list", "restart", "logs"]},
                    "command": {"type": "string", "description": "Command to run (for start)"},
                    "id": {"type": "string", "description": "Daemon ID (for stop, restart, logs)"},
                    "lines": {"type": "integer", "description": "Number of log lines to read (for logs, default 50)", "minimum": 1, "maximum": 500}
                },
                "required": ["action"],
                "additionalProperties": false,
            }),
        },
    ]
}
