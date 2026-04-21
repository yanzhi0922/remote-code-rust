//! Built-in tool specifications (schemas for all 40+ tools).

use serde_json::json;

use super::ToolSpec;
use super::tool_prompts;

#[must_use]
fn builtin_tool_specs_core() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "list_directory".to_owned(),
            protocol_name: "ListDirectory".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::LIST_DIRECTORY.to_owned(),
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
            description: tool_prompts::READ_FILE.to_owned(),
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
            description: tool_prompts::SEARCH_TEXT.to_owned(),
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
            description: tool_prompts::WRITE_FILE.to_owned(),
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
            description: tool_prompts::REPLACE_IN_FILE.to_owned(),
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
            description: tool_prompts::EDIT_FILE.to_owned(),
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
            description: tool_prompts::BASH_COMMAND.to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "cwd": {"type": "string", "description": "Optional working directory, relative to the current workspace. Use this instead of prefixing the command with cd or Set-Location."},
                    "description": {"type": "string", "description": "Optional short human description of what the command is doing."},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 600000},
                    "background": {"type": "boolean", "description": "Run the command in the background and return a task handle immediately."}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "glob".to_owned(),
            protocol_name: "Glob".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::GLOB.to_owned(),
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
            description: tool_prompts::GREP.to_owned(),
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
            description: tool_prompts::WEB_FETCH.to_owned(),
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
            protocol_name: "AskUserQuestion".to_owned(),
            permission_tool_name: "AskUserQuestion".to_owned(),
            description: tool_prompts::ASK_USER.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 4,
                        "description": "Questions to ask the user (1-4 questions)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": {
                                    "type": "string",
                                    "description": "The complete question to ask the user. Should be clear, specific, and end with a question mark."
                                },
                                "header": {
                                    "type": "string",
                                    "maxLength": 12,
                                    "description": "Very short label displayed as a chip/tag (max 12 chars)."
                                },
                                "options": {
                                    "type": "array",
                                    "minItems": 2,
                                    "maxItems": 4,
                                    "description": "The available choices for this question. Do not include an Other option; it is provided automatically.",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": {
                                                "type": "string",
                                                "description": "The display text for this option that the user will see and select. Should be concise (1-5 words)."
                                            },
                                            "description": {
                                                "type": "string",
                                                "description": "Explanation of what this option means or what will happen if chosen."
                                            },
                                            "preview": {
                                                "type": "string",
                                                "description": "Optional preview content rendered when this option is focused. Only use for single-select questions."
                                            }
                                        },
                                        "required": ["label", "description"],
                                        "additionalProperties": false
                                    }
                                },
                                "multiSelect": {
                                    "type": "boolean",
                                    "default": false,
                                    "description": "Set to true to allow the user to select multiple options instead of just one."
                                }
                            },
                            "required": ["question", "header", "options"],
                            "additionalProperties": false
                        }
                    },
                    "answers": {
                        "type": "object",
                        "additionalProperties": {"type": "string"},
                        "description": "User answers collected by the permission component"
                    },
                    "annotations": {
                        "type": "object",
                        "description": "Optional per-question annotations from the user, keyed by question text."
                    },
                    "metadata": {
                        "type": "object",
                        "properties": {
                            "source": {"type": "string"}
                        },
                        "additionalProperties": true
                    },
                    "question": {"type": "string", "description": "Deprecated compatibility alias; use questions[].question."},
                    "suggestions": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Deprecated compatibility alias; use questions[].options[]."
                    }
                },
                "required": ["questions"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "todo_write".to_owned(),
            protocol_name: "TodoWrite".to_owned(),
            permission_tool_name: "TodoWrite".to_owned(),
            description: tool_prompts::TODO_WRITE.to_owned(),
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
            description: tool_prompts::CONFIG_READ.to_owned(),
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
            description: tool_prompts::agent_tool_prompt(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "description": {"type": "string", "description": "A short (3-5 word) description of the task."},
                    "prompt": {"type": "string", "description": "The task for the agent to perform."},
                    "subagent_type": {"type": "string", "description": "The type of specialized agent to use for this task."},
                    "model": {"type": "string", "description": "Optional model override for this agent. Omit it or use inherit to reuse the parent model."},
                    "run_in_background": {"type": "boolean", "description": "Set to true to run this agent in the background."},
                    "name": {"type": "string", "description": "Name for the spawned agent. Makes it addressable via SendMessage({to: name}) while running."},
                    "team_name": {"type": "string", "description": "Team name for spawning. Uses the current team context if omitted."},
                    "mode": {"type": "string", "enum": ["default", "plan"], "description": "Permission mode for the spawned teammate."},
                    "isolation": {"type": "string", "enum": ["worktree"], "description": "Isolation mode. worktree creates a temporary git worktree so the agent works on an isolated copy of the repo."},
                    "cwd": {"type": "string", "description": "Absolute path to run the agent in. Overrides the working directory for all filesystem and shell operations within this agent."}
                },
                "required": ["description", "prompt"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "web_search".to_owned(),
            protocol_name: "WebSearch".to_owned(),
            permission_tool_name: "WebSearch".to_owned(),
            description: tool_prompts::WEB_SEARCH.to_owned(),
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
            description: tool_prompts::LSP.to_owned(),
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
        // ── Shared task-list tools ────────────────────────────────────────
        ToolSpec {
            name: "task_create".to_owned(),
            protocol_name: "TaskCreate".to_owned(),
            permission_tool_name: "TaskCreate".to_owned(),
            description: tool_prompts::TASK_CREATE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subject": {"type": "string", "description": "A brief title for the task"},
                    "description": {"type": "string", "description": "What needs to be done"},
                    "activeForm": {"type": "string", "description": "Present continuous form shown in spinner when in_progress (e.g., \"Running tests\")"},
                    "metadata": {"type": "object", "description": "Arbitrary metadata to attach to the task"}
                },
                "required": ["subject", "description"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_get".to_owned(),
            protocol_name: "TaskGet".to_owned(),
            permission_tool_name: "TaskGet".to_owned(),
            description: tool_prompts::TASK_GET.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "taskId": {"type": "string", "description": "The ID of the task to retrieve"}
                },
                "required": ["taskId"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_list".to_owned(),
            protocol_name: "TaskList".to_owned(),
            permission_tool_name: "TaskList".to_owned(),
            description: tool_prompts::TASK_LIST.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "task_update".to_owned(),
            protocol_name: "TaskUpdate".to_owned(),
            permission_tool_name: "TaskUpdate".to_owned(),
            description: tool_prompts::TASK_UPDATE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "taskId": {"type": "string", "description": "The ID of the task to update"},
                    "subject": {"type": "string", "description": "New subject for the task"},
                    "description": {"type": "string", "description": "New description for the task"},
                    "activeForm": {"type": "string", "description": "Present continuous form shown in spinner when in_progress (e.g., \"Running tests\")"},
                    "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "deleted"], "description": "New status for the task"},
                    "addBlocks": {"type": "array", "items": {"type": "string"}, "description": "Task IDs that this task blocks"},
                    "addBlockedBy": {"type": "array", "items": {"type": "string"}, "description": "Task IDs that block this task"},
                    "owner": {"type": "string", "description": "New owner for the task"},
                    "metadata": {"type": "object", "description": "Metadata keys to merge into the task. Set a key to null to delete it."}
                },
                "required": ["taskId"],
                "additionalProperties": false,
            }),
        },
        // ── Notebook edit tool ─────────────────────────────────────────────
        ToolSpec {
            name: "notebook_edit".to_owned(),
            protocol_name: "NotebookEdit".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: tool_prompts::NOTEBOOK_EDIT.to_owned(),
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
            description: tool_prompts::SKILL_DISCOVER.to_owned(),
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
            description: tool_prompts::SKILL_EXECUTE.to_owned(),
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
            description: tool_prompts::SEND_MESSAGE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Optional team name when more than one team exists."},
                    "recipient": {"type": "string", "description": "Target agent name within the selected team."},
                    "message": {"type": "string", "description": "Message content to deliver."},
                    "sender": {"type": "string", "description": "Optional sender agent name (default: coordinator)."},
                    "priority": {"type": "string", "enum": ["low", "normal", "high"], "description": "Optional message priority."},
                    "correlation_id": {"type": "string", "description": "Optional correlation identifier for request/response flows."}
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
            description: tool_prompts::ENTER_PLAN_MODE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "exit_plan_mode".to_owned(),
            protocol_name: "ExitPlanMode".to_owned(),
            permission_tool_name: "ExitPlanMode".to_owned(),
            description: tool_prompts::EXIT_PLAN_MODE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "allowedPrompts": {
                        "type": "array",
                        "description": "Prompt-based permissions needed to implement the plan. These describe categories of actions rather than specific commands.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tool": {"type": "string", "enum": ["Bash"], "description": "The tool this prompt applies to"},
                                "prompt": {"type": "string", "description": "Semantic description of the action, e.g. \"run tests\", \"install dependencies\""}
                            },
                            "required": ["tool", "prompt"],
                            "additionalProperties": false
                        }
                    },
                    "plan": {
                        "type": "string",
                        "description": "The plan content (injected by normalizeToolInput from disk)"
                    },
                    "planFilePath": {
                        "type": "string",
                        "description": "The plan file path (injected by normalizeToolInput)"
                    },
                    "plan_file_path": {
                        "type": "string",
                        "description": "The plan file path (runtime compatibility alias)"
                    }
                },
                "additionalProperties": false,
            }),
        },
        // ── Sleep tool ─────────────────────────────────────────────────────
        ToolSpec {
            name: "sleep".to_owned(),
            protocol_name: "Sleep".to_owned(),
            permission_tool_name: "Sleep".to_owned(),
            description: tool_prompts::SLEEP.to_owned(),
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
            description: tool_prompts::SNIP.to_owned(),
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
            description: tool_prompts::MEMORY_READ.to_owned(),
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
            description: tool_prompts::MEMORY_WRITE.to_owned(),
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
            description: tool_prompts::TEAM_CREATE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Optional persistent team name to create or update."},
                    "objective": {"type": "string"},
                    "lead": {"type": "string"},
                    "agents": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "role": {"type": "string"},
                                "cwd": {"type": "string"},
                                "model": {"type": "string"},
                                "color": {"type": "string"},
                                "worktree_path": {"type": "string"},
                                "session_id": {"type": "string"}
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
            description: tool_prompts::TEAM_STATUS.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Optional team name. If omitted, returns the active team or summaries for all teams."}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "web_browser".to_owned(),
            protocol_name: "WebBrowser".to_owned(),
            permission_tool_name: "WebBrowser".to_owned(),
            description: tool_prompts::WEB_BROWSER.to_owned(),
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
            description: tool_prompts::TOOL_SEARCH.to_owned(),
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
            description: tool_prompts::VERIFY_PLAN.to_owned(),
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
            description: tool_prompts::TERMINAL_CAPTURE.to_owned(),
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
            description: tool_prompts::POWERSHELL.to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "cwd": {"type": "string", "description": "Optional working directory, relative to the current workspace. Use this instead of prefixing the command with cd or Set-Location."},
                    "description": {"type": "string", "description": "Optional short human description of what the command is doing."},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 600000},
                    "background": {"type": "boolean", "description": "Run the command in the background and return a task handle immediately."}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "repl".to_owned(),
            protocol_name: "REPL".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: tool_prompts::REPL.to_owned(),
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
            description: tool_prompts::MONITOR.to_owned(),
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
            description: tool_prompts::SCHEDULE_CRON.to_owned(),
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
            description: tool_prompts::REMOTE_TRIGGER.to_owned(),
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
            description: tool_prompts::WORKFLOW.to_owned(),
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
            description: tool_prompts::SUGGEST_PR.to_owned(),
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
            description: tool_prompts::ENTER_WORKTREE.to_owned(),
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
            description: tool_prompts::EXIT_WORKTREE.to_owned(),
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
            description: tool_prompts::LIST_WORKTREES.to_owned(),
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
            description: tool_prompts::BRIEF.to_owned(),
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
            description: tool_prompts::CTX_INSPECT.to_owned(),
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
            description: tool_prompts::LIST_PEERS.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Optional team name to scope the peer listing."}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "tungsten".to_owned(),
            protocol_name: "Tungsten".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: tool_prompts::TUNGSTEN.to_owned(),
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
            description: tool_prompts::OVERFLOW_TEST.to_owned(),
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
            description: tool_prompts::SYNTHETIC_OUTPUT.to_owned(),
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
            description: tool_prompts::MCP_AUTH.to_owned(),
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
            description: tool_prompts::MCP_CALL.to_owned(),
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
            name: "voice_input".to_owned(),
            protocol_name: "VoiceInput".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::VOICE_INPUT.to_owned(),
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
            description: tool_prompts::DAEMON.to_owned(),
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

/// MCP resource tools are injected only when at least one connected MCP server
/// advertises resources support. Keeping them out of the unconditional built-in
/// prefix matches Claude Code's MCP resource surface and avoids exposing dead
/// schemas to the model.
#[must_use]
pub fn mcp_resource_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "list_mcp_resources".to_owned(),
            protocol_name: "ListMcpResources".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::LIST_MCP_RESOURCES.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {"type": "string"}
                },
                "required": ["server"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "read_mcp_resource".to_owned(),
            protocol_name: "ReadMcpResource".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::READ_MCP_RESOURCE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {"type": "string"},
                    "uri": {"type": "string"}
                },
                "required": ["server", "uri"],
                "additionalProperties": false,
            }),
        },
    ]
}

#[must_use]
pub fn builtin_tool_specs() -> Vec<ToolSpec> {
    let mut specs = builtin_tool_specs_core();
    specs.extend(phase9_tool_specs());
    specs
}

/// Phase 9: Additional tool specs for new dedicated modules.
#[must_use]
pub fn phase9_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "discover_skills".to_owned(),
            protocol_name: "DiscoverSkills".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::DISCOVER_SKILLS.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Task description to search for matching skills"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 20, "description": "Maximum number of results (default 10)"}
                },
                "required": ["query"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "team_delete".to_owned(),
            protocol_name: "TeamDelete".to_owned(),
            permission_tool_name: "TeamDelete".to_owned(),
            description: tool_prompts::TEAM_DELETE.to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Name of the team to delete"}
                },
                "required": ["team_name"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "team_list".to_owned(),
            protocol_name: "TeamList".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::TEAM_LIST.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "broadcast_message".to_owned(),
            protocol_name: "BroadcastMessage".to_owned(),
            permission_tool_name: "SendMessage".to_owned(),
            description: tool_prompts::BROADCAST_MESSAGE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": {"type": "string", "description": "Optional team name when more than one team exists."},
                    "message": {"type": "string", "description": "Message content to broadcast"},
                    "sender": {"type": "string", "description": "Sender agent name (default: coordinator)"},
                    "priority": {"type": "string", "enum": ["low", "normal", "high"], "description": "Message priority (default: normal)"},
                    "recipients": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional list of specific recipient agent names"
                    }
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "review_artifact".to_owned(),
            protocol_name: "ReviewArtifact".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::REVIEW_ARTIFACT.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["view_diff", "add_comment", "update_status", "get_comments", "summary"], "description": "Review action to perform"},
                    "artifact_id": {"type": "string", "description": "Artifact identifier"},
                    "comment": {"type": "string", "description": "Comment text (for add_comment)"},
                    "status": {"type": "string", "enum": ["pending", "in_progress", "approved", "changes_requested", "rejected"], "description": "Review status (for update_status)"},
                    "author": {"type": "string", "description": "Comment author (default: reviewer)"},
                    "severity": {"type": "string", "enum": ["info", "suggestion", "warning", "critical"], "description": "Comment severity (default: info)"},
                    "file_path": {"type": "string", "description": "File path for inline comment"},
                    "line": {"type": "integer", "description": "Line number for inline comment"},
                    "from_version": {"type": "string", "description": "Git ref for diff start (default: HEAD~1)"},
                    "to_version": {"type": "string", "description": "Git ref for diff end (default: HEAD)"},
                    "reviewer": {"type": "string", "description": "Reviewer name (for update_status)"}
                },
                "required": ["action", "artifact_id"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "send_user_file".to_owned(),
            protocol_name: "SendUserFile".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: tool_prompts::SEND_USER_FILE.to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "Path to the file (relative to workspace)"},
                    "description": {"type": "string", "description": "Optional description of the file"},
                    "max_size_bytes": {"type": "integer", "minimum": 1, "maximum": 104857600, "description": "Maximum file size in bytes (default 10MB)"},
                    "max_text_chars": {"type": "integer", "minimum": 1000, "maximum": 500000, "description": "Maximum text content characters (default 50000)"}
                },
                "required": ["file_path"],
                "additionalProperties": false,
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{builtin_tool_specs, mcp_resource_tool_specs};

    #[test]
    fn shell_tool_schemas_expose_cwd_controls() {
        let specs = builtin_tool_specs();
        for tool_name in ["bash_command", "powershell"] {
            let spec = specs
                .iter()
                .find(|spec| spec.name == tool_name)
                .unwrap_or_else(|| panic!("missing tool spec for {tool_name}"));
            let properties = spec
                .input_schema
                .get("properties")
                .and_then(|value| value.as_object())
                .unwrap_or_else(|| panic!("missing properties object for {tool_name}"));

            assert!(
                properties.contains_key("cwd"),
                "{tool_name} should expose cwd"
            );
            assert!(
                properties.contains_key("description"),
                "{tool_name} should expose description"
            );
            assert!(
                properties.contains_key("background"),
                "{tool_name} should expose background"
            );
        }
    }

    #[test]
    fn enter_plan_mode_schema_matches_research_empty_input_contract() {
        let specs = builtin_tool_specs();
        let spec = specs
            .iter()
            .find(|spec| spec.name == "enter_plan_mode")
            .expect("enter_plan_mode spec");

        assert_eq!(spec.protocol_name, "EnterPlanMode");
        assert_eq!(
            spec.input_schema,
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            })
        );
    }

    #[test]
    fn exit_plan_mode_schema_allows_runtime_injected_plan_fields() {
        let specs = builtin_tool_specs();
        let spec = specs
            .iter()
            .find(|spec| spec.name == "exit_plan_mode")
            .expect("exit_plan_mode spec");
        let properties = spec
            .input_schema
            .get("properties")
            .and_then(|value| value.as_object())
            .expect("properties");

        assert!(properties.contains_key("allowedPrompts"));
        assert!(properties.contains_key("plan"));
        assert!(properties.contains_key("planFilePath"));
        assert!(properties.contains_key("plan_file_path"));
        assert_eq!(
            spec.input_schema["additionalProperties"],
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn agent_schema_matches_research_surface() {
        let specs = builtin_tool_specs();
        let agent = specs
            .iter()
            .find(|spec| spec.name == "agent")
            .expect("missing agent spec");
        let properties = agent
            .input_schema
            .get("properties")
            .and_then(|value| value.as_object())
            .expect("agent properties");

        for field in [
            "description",
            "prompt",
            "subagent_type",
            "model",
            "run_in_background",
            "name",
            "team_name",
            "mode",
            "isolation",
            "cwd",
        ] {
            assert!(
                properties.contains_key(field),
                "agent schema should expose {field}"
            );
        }

        assert!(
            !properties.contains_key("tools"),
            "agent schema should hide legacy tools overrides from the model"
        );
        assert!(
            !properties.contains_key("tasks"),
            "agent schema should hide legacy batch delegation fields from the model"
        );

        let required = agent
            .input_schema
            .get("required")
            .and_then(|value| value.as_array())
            .expect("agent required list");
        let required_fields = required
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(required_fields.contains("description"));
        assert!(required_fields.contains("prompt"));
    }

    #[test]
    fn team_and_message_schemas_match_runtime_contract() {
        let specs = builtin_tool_specs();
        let spec_by_name = |name: &str| {
            specs
                .iter()
                .find(|spec| spec.name == name)
                .unwrap_or_else(|| panic!("missing tool spec for {name}"))
        };
        let properties_for = |name: &str| {
            spec_by_name(name)
                .input_schema
                .get("properties")
                .and_then(|value| value.as_object())
                .unwrap_or_else(|| panic!("missing properties object for {name}"))
        };

        let send_message = properties_for("send_message");
        for field in [
            "team_name",
            "recipient",
            "message",
            "sender",
            "priority",
            "correlation_id",
        ] {
            assert!(
                send_message.contains_key(field),
                "send_message should expose {field}"
            );
        }

        let team_create = properties_for("team_create");
        for field in ["team_name", "objective", "lead", "agents"] {
            assert!(
                team_create.contains_key(field),
                "team_create should expose {field}"
            );
        }
        let agent_properties = team_create
            .get("agents")
            .and_then(|value| value.get("items"))
            .and_then(|value| value.get("properties"))
            .and_then(|value| value.as_object())
            .expect("team_create agents items should expose properties");
        for field in [
            "name",
            "role",
            "cwd",
            "model",
            "color",
            "worktree_path",
            "session_id",
        ] {
            assert!(
                agent_properties.contains_key(field),
                "team_create agents should expose {field}"
            );
        }

        let team_status = properties_for("team_status");
        assert!(
            team_status.contains_key("team_name"),
            "team_status should expose team_name"
        );

        let list_peers = properties_for("list_peers");
        assert!(
            list_peers.contains_key("team_name"),
            "list_peers should expose team_name"
        );

        let broadcast = properties_for("broadcast_message");
        for field in ["team_name", "message", "sender", "priority", "recipients"] {
            assert!(
                broadcast.contains_key(field),
                "broadcast_message should expose {field}"
            );
        }
    }

    #[test]
    fn mcp_resource_schemas_match_runtime_contract() {
        let specs = mcp_resource_tool_specs();
        let read = specs
            .iter()
            .find(|spec| spec.name == "read_mcp_resource")
            .expect("read_mcp_resource spec");
        let required = read
            .input_schema
            .get("required")
            .and_then(|value| value.as_array())
            .expect("required array")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(required, vec!["server", "uri"]);

        let list = specs
            .iter()
            .find(|spec| spec.name == "list_mcp_resources")
            .expect("list_mcp_resources spec");
        let required = list
            .input_schema
            .get("required")
            .and_then(|value| value.as_array())
            .expect("required array")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(required, vec!["server"]);
    }
}
