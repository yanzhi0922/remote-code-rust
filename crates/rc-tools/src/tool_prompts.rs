//! Detailed tool prompt descriptions for all built-in tools.
//!
//! Each prompt is a static string constant providing the LLM with rich context
//! about when and how to use the tool. Prompts are modelled after Claude Code's
//! prompt.ts files but adapted for the Rust codebase.

// ── Core tools (P0) ──────────────────────────────────────────────────────────

/// Prompt for `list_directory`.
pub const LIST_DIRECTORY: &str = "\
List files and directories within a specified path relative to the current workspace.

Usage:
- Returns file names, types (file/directory), and metadata for each entry.
- Set `recursive` to true to traverse nested directories (use with caution on large trees).
- Use `max_entries` to cap results and avoid overwhelming output (max 500).
- Prefer this tool over running `ls` via Bash — it is faster and respects workspace boundaries.
- This tool can only list directories, not read file contents. Use `read_file` for that.

Notes:
- The path is relative to the current workspace directory.
- Returns an error if the path does not exist or is not a directory.
- For large monorepos, start with a non-recursive listing then drill into subdirectories.";

/// Prompt for `read_file`.
pub const READ_FILE: &str = "\
Reads a UTF-8 text file from the local filesystem. You can access any file directly by using this tool.

Usage:
- The `path` parameter must be a path relative to the current workspace.
- By default, it reads up to 2000 lines starting from the beginning of the file.
- Use `start_line` and `end_line` to read specific ranges, especially for large files.
- Results are returned with line numbers, starting at 1.
- This tool can read images (PNG, JPG, etc.) — contents are presented visually.
- This tool can read Jupyter notebooks (.ipynb) and returns all cells with outputs.
- This tool can only read files, not directories. Use `list_directory` for directories.

Notes:
- If you read a file that exists but has empty contents, a warning will be returned.
- It is okay to read a file that does not exist; an error will be returned.
- Always read a file before editing it — the edit tool requires a prior read.
- For very large files, read in chunks using start_line/end_line to avoid truncation.";

/// Prompt for `search_text`.
pub const SEARCH_TEXT: &str = "\
Search files for a text pattern or regular expression within the workspace.

Usage:
- The `pattern` parameter supports full regex syntax (e.g., 'fn\\s+\\w+', 'TODO.*fix').
- Optionally specify a `path` to narrow the search scope to a subdirectory.
- Use `max_matches` to limit results (default 50, max 200).
- Returns matching file paths, line numbers, and surrounding context.
- Prefer this tool over running `grep` via Bash — it is optimized for workspace access.

Notes:
- Searches are case-sensitive by default. Use (?i) prefix for case-insensitive mode.
- Binary files are automatically skipped.
- Hidden files (starting with .) and common ignored directories (node_modules, .git) are excluded.
- For open-ended searches requiring multiple rounds, consider using the `agent` tool.";

/// Prompt for `write_file`.
pub const WRITE_FILE: &str = "\
Writes a file to the local filesystem. Creates the file if it does not exist; overwrites if it does.

Usage:
- The `path` parameter is relative to the current workspace directory.
- The `content` parameter must contain the COMPLETE file content — partial writes are not supported.
- Set `append` to true to append content to an existing file instead of overwriting.
- If this is an existing file, you MUST read it first to understand its current contents.
- Prefer the `edit_file` or `replace_in_file` tool for modifying existing files — it only sends the diff.

Notes:
- NEVER create documentation files (*.md) or README files unless explicitly requested.
- This tool automatically creates any intermediate directories needed.
- Do NOT use this tool for small edits to existing files — use `edit_file` instead.
- ALWAYS provide the COMPLETE intended content. Partial updates or placeholders are forbidden.";

/// Prompt for `replace_in_file`.
pub const REPLACE_IN_FILE: &str = "\
Performs a simple string replacement in an existing file.

Usage:
- You must have read the file at least once before editing. The tool will error otherwise.
- The `search` string must match exactly, including whitespace and indentation.
- The `replace` string replaces the first occurrence of `search` in the file.
- Set `all` to true to replace every occurrence of `search` in the file.
- The edit will FAIL if `search` is not found in the file.

Notes:
- Always preserve exact indentation (tabs/spaces) when writing the search string.
- For multiple edits in one operation, use `edit_file` instead.
- Use `all` for renaming variables or updating repeated patterns across a file.
- If the search string is not unique and `all` is false, only the first match is replaced.";

/// Prompt for `edit_file`.
pub const EDIT_FILE: &str = "\
Performs ordered search/replace edits on a text file. Applies multiple edits in sequence.

Usage:
- You must read the file at least once before editing. The tool will error otherwise.
- Each edit in the `edits` array has `search` and `replace` fields. Edits are applied in order.
- The `search` string must match exactly, including whitespace and indentation.
- Set `all` on an individual edit to replace every occurrence of that search string.
- The edit will FAIL if any `search` string is not found in the file at its point of application.
- Set `create_if_missing` to true to create the file if it does not exist.

Notes:
- ALWAYS prefer editing existing files over creating new files.
- Use the smallest search string that is clearly unique — usually 2-4 adjacent lines is sufficient.
- When editing text from read_file output, ensure you preserve the exact indentation.
- Never include line number prefixes in the search or replace strings.
- For a single replacement, `replace_in_file` is simpler.";

/// Prompt for `bash_command`.
pub const BASH_COMMAND: &str = "\
Executes a shell command and returns its output.

The working directory persists between commands, but shell state does not. The shell \
environment is initialized from the user's profile (bash or zsh).

IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, \
`awk`, or `echo` commands unless explicitly instructed. Instead, use the appropriate \
dedicated tool for a better experience:
- File search: Use `glob` (NOT find or ls)
- Content search: Use `grep` or `search_text` (NOT grep or rg)
- Read files: Use `read_file` (NOT cat/head/tail)
- Edit files: Use `edit_file` (NOT sed/awk)
- Write files: Use `write_file` (NOT echo/cat)

Instructions:
- If creating new directories/files, first run `ls` to verify the parent directory exists.
- Always quote file paths containing spaces with double quotes.
- Try to maintain your current working directory by using absolute paths and avoiding `cd`.
- Use `cwd` parameter to run in a subdirectory instead of prefixing with cd.
- Specify an optional timeout in milliseconds (up to 600000ms / 10 minutes).
- Use `background` to run long-running commands without blocking.

For multiple commands:
- Independent commands: make multiple bash_command calls in parallel.
- Dependent commands: chain with && in a single call.
- Use ; when you don't care if earlier commands fail.

Git safety:
- NEVER run destructive git commands (push --force, reset --hard) unless explicitly requested.
- NEVER skip hooks (--no-verify) unless the user explicitly requests it.
- Prefer creating new commits over amending existing ones.";

/// Prompt for `glob`.
pub const GLOB: &str = "\
Fast file pattern matching tool that works with any codebase size.

Usage:
- Supports glob patterns like '**/*.rs', 'src/**/*.ts', or '*.toml'.
- Returns matching file paths sorted by modification time (newest first).
- Optionally specify a `path` to search within a subdirectory.
- Use this tool when you need to find files by name patterns.

Notes:
- For content-based searches, use `grep` or `search_text` instead.
- When doing an open-ended search that may require multiple rounds of globbing \
  and grepping, use the `agent` tool instead.
- Patterns are case-sensitive on most systems. Use appropriate casing.";

/// Prompt for `grep`.
pub const GREP: &str = "\
A powerful search tool for finding patterns in files.

Usage:
- ALWAYS use this tool for search tasks. NEVER invoke `grep` or `rg` as a bash command.
- Supports full regex syntax (e.g., 'log.*Error', 'function\\s+\\w+').
- Filter files with `file_pattern` parameter (e.g., '*.rs', '**/*.tsx').
- Use `max_matches` to limit the number of results (max 200).
- Returns matching file paths, line numbers, and content.

Notes:
- Pattern syntax uses Rust regex — literal braces need escaping.
- For open-ended searches requiring multiple rounds, use the `agent` tool.
- Binary files and common ignored directories are automatically excluded.
- Searches are case-sensitive by default.";

/// Prompt for `web_fetch`.
pub const WEB_FETCH: &str = "\
Fetch the content of a URL and return it as text.

Usage:
- The `url` parameter must be a valid HTTP or HTTPS URL.
- Use `max_chars` to limit the response size (max 100000 characters).
- Returns the page content as plain text or markdown.
- Useful for reading documentation, API responses, or web pages.

Notes:
- Requires network access. May fail behind restrictive firewalls.
- Some websites block automated requests — consider using `web_browser` as a fallback.
- For search queries, prefer `web_search` over fetching search engine URLs directly.
- Large pages may be truncated — use `max_chars` to control the output size.";

/// Prompt for `agent`.
pub const AGENT: &str = "\
Spawn a sub-agent to complete a task. The sub-agent runs in its own context and returns the result.

Usage:
- Write a detailed `prompt` describing what the sub-agent should accomplish and why.
- Use `subagent_type` to choose a specialized built-in agent. Current built-ins are `general-purpose`, `Explore`, `Plan`, and `verification`.
- If you omit `subagent_type`, the default `general-purpose` agent is used.
- Optionally provide a short `description` to summarize the assignment.
- Optionally provide `name` together with `team_name` to register a live teammate identity for this run.
- Optionally set `mode` to control the child runtime. Use `default` for normal execution or `plan` when the teammate must enter plan mode before implementation.
- Optionally override the sub-agent model with `model`. Omit it or use `inherit` to reuse the parent model.
- Optionally restrict available tools via the `tools` array.
- The sub-agent starts with zero context — brief it like a smart colleague who just walked in.
- Explain what you're trying to accomplish, what you've already learned, and what the agent should do.
- If you need a short response, say so ('report in under 200 words').

Writing the prompt:
- Explain what you're trying to accomplish and why.
- Describe what you've already learned or ruled out.
- Give enough context for the agent to make judgment calls.
- Lookups: hand over the exact command. Investigations: hand over the question.
- Terse command-style prompts produce shallow, generic work.

Notes:
- Never delegate understanding. Don't write 'based on your findings, fix the bug'.
- Include file paths, line numbers, and what specifically to change.
- To continue a previously spawned agent or teammate, use `send_message` with the agent ID or name.
- The sub-agent cannot see this conversation — provide all necessary context in the prompt.
- Teammates cannot spawn other teammates. Omit `name`, `team_name`, and `mode` when you only need a normal sub-agent.";

// ── System tools (P1) ────────────────────────────────────────────────────────

/// Prompt for `todo_write`.
pub const TODO_WRITE: &str = "\
Manage a task list by creating, updating, or deleting todo items.

Usage:
- Pass a `todos` array with objects containing `id`, `text`, and `status` fields.
- Status values: 'pending', 'in_progress', 'completed'.
- Each call replaces the entire todo list — include ALL items, not just changes.
- Use this to track multi-step tasks and show progress to the user.

Notes:
- Keep todo items concise and actionable.
- Update status to 'in_progress' when starting a step, 'completed' when done.
- This tool is useful for complex, multi-step tasks that benefit from progress tracking.
- Do not use for simple single-step tasks.";

/// Prompt for `config_read`.
pub const CONFIG_READ: &str = "\
Read or modify runtime configuration settings.

Usage:
- Use action 'get' with a `key` to read a configuration value.
- Use action 'set' with `key` and `value` to update a configuration value.
- Configuration keys are dot-separated paths (e.g., 'shell.default', 'tools.timeout_ms').

Notes:
- Some configuration changes take effect immediately; others require a restart.
- Be cautious when modifying configuration — incorrect values may break functionality.
- Prefer reading configuration before modifying it to understand current state.";

/// Prompt for `sleep`.
pub const SLEEP: &str = "\
Sleep for a specified number of seconds (max 30).

Usage:
- Use sparingly — most tasks do not require delays.
- Useful for waiting for external processes or rate limiting.
- Maximum sleep duration is 30 seconds.

Notes:
- Do not sleep between commands that can run immediately — just run them.
- Do not retry failing commands in a sleep loop — diagnose the root cause.
- If waiting for a background task, you will be notified when it completes — do not poll.
- If you must poll an external process, use a check command rather than sleeping first.";

/// Prompt for `snip`.
pub const SNIP: &str = "\
Save a code snippet to the .remote-code/snippets/ directory for later reference.

Usage:
- The `content` parameter contains the snippet text to save.
- Optionally provide a `label` to name the snippet file.
- Snippets are saved as individual files in the snippets directory.

Notes:
- Useful for saving intermediate results, code fragments, or reference material.
- Snippets persist across sessions and can be retrieved later.
- Do not use for large file contents — use `write_file` instead.";

/// Prompt for `tool_search`.
pub const TOOL_SEARCH: &str = "\
Search available tools by keyword. Returns matching tool names and descriptions.

Usage:
- Pass a `query` string to search tool names and descriptions.
- Use `max_results` to limit the number of matches (max 20).
- Returns tool names and their descriptions for review.

Notes:
- Useful when you are unsure which tool to use for a task.
- Searches are fuzzy — partial matches and synonyms are supported.
- After finding the right tool, read its description carefully before using it.";

/// Prompt for `verify_plan`.
pub const VERIFY_PLAN: &str = "\
Verify a plan's execution status. Returns which items are incomplete.

Usage:
- Pass a `plan` array of plan item descriptions.
- Pass a `completed` array of booleans indicating completion status (parallel to plan).
- Returns a summary of which items are done and which remain.

Notes:
- Use this after executing a multi-step plan to confirm all items are addressed.
- The plan and completed arrays must have the same length.
- Useful for self-checking before reporting completion to the user.";

/// Prompt for `terminal_capture`.
pub const TERMINAL_CAPTURE: &str = "\
Execute a command and return formatted output with exit code information.

Usage:
- Runs the given `command` and captures stdout, stderr, and exit code.
- Returns structured output with clear separation of streams.
- Useful for capturing command output in a structured format.

Notes:
- Requires permission as it executes arbitrary commands.
- For interactive or long-running commands, prefer `bash_command` with `background`.
- The command runs in the workspace directory.";

/// Prompt for `monitor`.
pub const MONITOR: &str = "\
Monitor agents, tasks, or sessions and return a status snapshot.

Usage:
- Set `target` to 'agents', 'tasks', or 'sessions' to choose what to monitor.
- Optionally set `interval_ms` for periodic monitoring (min 100ms, max 60000ms).
- Returns a snapshot of current status for the selected target.

Notes:
- Use this to check on background tasks or agent progress.
- For one-shot status checks, omit `interval_ms`.
- For streaming events from a background process, prefer `bash_command` with background.";

/// Prompt for `brief`.
pub const BRIEF: &str = "\
Summarize or truncate content to a maximum length.

Usage:
- Pass `content` to summarize or truncate.
- Use `max_length` to set the maximum output length (max 100000 characters).
- Useful for condensing large outputs before including them in context.

Notes:
- When content exceeds max_length, it is truncated with a marker indicating the cut point.
- Prefer using this tool when you need to reduce large outputs for context management.
- Does not perform AI summarization — only truncation.";

/// Prompt for `ctx_inspect`.
pub const CTX_INSPECT: &str = "\
Inspect current conversation context (tokens, messages, tools).

Usage:
- Set `action` to 'tokens' to see token usage statistics.
- Set `action` to 'messages' to see message count and types.
- Set `action` to 'tools' to see available tools and their usage.

Notes:
- Useful for debugging context window issues.
- Helps understand how much context budget remains.
- Use this before launching long operations to ensure context is available.";

// ── Communication tools (P1) ─────────────────────────────────────────────────

/// Prompt for `ask_user`.
pub const ASK_USER: &str = "\
Ask the user a question and wait for a response.

Usage:
- The `question` parameter contains the question to ask the user.
- Optionally provide `suggestions` — a list of 2-4 suggested answers the user can pick.
- Suggestions should be specific, actionable, and directly related to the task.
- The tool blocks until the user responds.

Notes:
- Use only when you need additional information to proceed effectively.
- Do not ask for more information than necessary.
- If you can infer the answer from context, do so instead of asking.
- Keep questions clear and concise.";

/// Prompt for `send_message`.
pub const SEND_MESSAGE: &str = "\
Send a message to another agent in the multi-agent system.

Usage:
- `team_name` optionally selects the target team when multiple teams exist.
- The `recipient` parameter specifies the target agent name.
- The `message` parameter contains the message content.
- `sender` optionally identifies the sender (default: coordinator).
- `priority` optionally sets message priority: 'low', 'normal', or 'high'.
- `correlation_id` optionally links request/response exchanges.
- Messages are delivered asynchronously to the recipient's mailbox.

Notes:
- Use this for inter-agent communication in multi-agent workflows.
- The recipient must be a registered agent in the system.
- For broadcasting to all agents, use `broadcast_message` instead.";

/// Prompt for `send_user_file`.
pub const SEND_USER_FILE: &str = "\
Send a file to the user (logs, screenshots, exported data). Supports base64 encoding and file type detection.

Usage:
- The `file_path` parameter specifies the file to send (relative to workspace).
- Optionally provide a `description` of the file.
- Use `max_size_bytes` to limit file size (default 10MB, max 100MB).
- Use `max_text_chars` to limit text content characters (default 50000).

Notes:
- Automatically detects file type (text, image, binary) and encodes appropriately.
- For very large files, consider truncating or summarizing instead.
- The file must exist at the specified path.";

// ── Development tools (P1) ───────────────────────────────────────────────────

/// Prompt for `lsp`.
pub const LSP: &str = "\
Language Server Protocol tool for code intelligence.

Usage:
- `action` determines the operation: 'definitions', 'references', 'hover', 'completion', 'diagnostics'.
- `file_path` is required for all actions (relative to workspace).
- `line` and `column` are required for definitions, references, hover, and completion.
- `symbol` can be used instead of line/column for some actions.

Actions:
- definitions: Go to the definition of a symbol at the given position.
- references: Find all references to a symbol at the given position.
- hover: Get type information and documentation for a symbol.
- completion: Get auto-completion suggestions at a position.
- diagnostics: Get linting/error diagnostics for a file.

Notes:
- Requires a running language server for the file's language.
- Falls back to text-based search if no language server is available.
- Line and column numbers are 1-based.";

/// Prompt for `notebook_edit`.
pub const NOTEBOOK_EDIT: &str = "\
Edit a cell in a Jupyter notebook (.ipynb) file.

Usage:
- `path` specifies the notebook file (relative to workspace).
- `cell_index` is the 0-based index of the cell to edit.
- `new_source` is the new cell content.
- Optionally set `cell_type` to 'code' or 'markdown' to change the cell type.

Notes:
- The notebook must be a valid .ipynb file.
- Cell index is 0-based — the first cell has index 0.
- Requires read permission on the notebook file.";

/// Prompt for `skill_discover`.
pub const SKILL_DISCOVER: &str = "\
Discover available skills in the current workspace.

Usage:
- Returns a list of all registered skills with their names, descriptions, and slugs.
- Use this to find skills that match the current task.
- After discovering a skill, use `skill_execute` to load its instructions.

Notes:
- Skills are loaded from the workspace's .remote-code/skills/ directory.
- Skills provide specialized instructions for common tasks.
- Always discover skills before attempting to execute them.";

/// Prompt for `skill_execute`.
pub const SKILL_EXECUTE: &str = "\
Load and return the instructions of a specific skill by slug. The skill content is injected into the conversation context.

Usage:
- The `slug` parameter identifies the skill to load (e.g., 'react-native-dev').
- Optionally pass `arguments` to provide context to the skill.
- The skill's instructions are returned as text for the agent to follow.

Notes:
- Use `skill_discover` first to find available skills and their slugs.
- Skill content replaces the current context — use judiciously.
- Follow the skill's instructions precisely after loading.";

/// Prompt for `enter_plan_mode`.
pub const ENTER_PLAN_MODE: &str = "\
Enter plan mode (read-only, no tool execution).

Usage:
- Pass an `objective` describing what you want to plan.
- In plan mode, you can read files and search but cannot modify anything.
- Use this to analyze a problem and create a plan before executing.

Notes:
- Exit plan mode with `exit_plan_mode` when ready to execute.
- Plan mode is useful for complex tasks that benefit from upfront analysis.
- All write operations are blocked in plan mode.";

/// Prompt for `exit_plan_mode`.
pub const EXIT_PLAN_MODE: &str = "\
Exit plan mode and resume normal execution.

Usage:
- Call this when you have finished planning and are ready to execute.
- No parameters required.
- After exiting, all tools are available again.

Notes:
- Only call this after entering plan mode via `enter_plan_mode`.
- Review your plan before exiting to ensure it is complete.";

// ── MCP tools (P1) ───────────────────────────────────────────────────────────

/// Prompt for `mcp_call`.
pub const MCP_CALL: &str = "\
Call a tool on an MCP (Model Context Protocol) server directly.

Usage:
- `server` is the MCP server name as defined in the MCP configuration.
- `tool` is the name of the tool to call on that server.
- `arguments` is an optional object of arguments to pass to the MCP tool.
- The server must be configured and connected before calling.

Notes:
- MCP servers extend available tools with external capabilities.
- Check `list_mcp_resources` to discover what a server provides.
- Connection errors may occur if the server is not running or not configured.
- Arguments must match the tool's expected schema.";

/// Prompt for `mcp_auth`.
pub const MCP_AUTH: &str = "\
Manage authentication state for MCP servers.

Usage:
- `server` identifies the MCP server.
- `action` can be 'login', 'logout', or 'status'.
- Use 'status' to check if authenticated, 'login' to authenticate, 'logout' to clear credentials.

Notes:
- Some MCP servers require authentication before their tools can be used.
- Authentication may open a browser window for OAuth flows.
- Credentials are stored securely and persist across sessions.";

/// Prompt for `list_mcp_resources`.
pub const LIST_MCP_RESOURCES: &str = "\
List resources provided by MCP servers.

Usage:
- Optionally specify a `server` to list resources from a specific server.
- Without a server parameter, lists resources from all connected servers.
- Returns resource names, URIs, and descriptions.

Notes:
- Resources represent data sources that can be read using `read_mcp_resource`.
- MCP servers must be connected to list their resources.
- Use this to discover what data is available from MCP integrations.";

/// Prompt for `read_mcp_resource`.
pub const READ_MCP_RESOURCE: &str = "\
Read the content of an MCP resource by URI.

Usage:
- The `uri` parameter identifies the resource to read (e.g., 'file:///path/to/data.json').
- Returns the resource content as text.
- Use `list_mcp_resources` to discover available resource URIs.

Notes:
- The URI must be a valid resource URI from a connected MCP server.
- Some resources may be large — consider the context budget before reading.
- Resource content format depends on the MCP server implementation.";

// ── Task/Team tools (P2) ─────────────────────────────────────────────────────

/// Prompt for `task_create`.
pub const TASK_CREATE: &str = "\
Create a new background task.

Usage:
- `title` is a human-readable name for the task.
- `command` is the shell command to execute in the background.
- Returns a task ID that can be used to track progress.

Notes:
- Background tasks run independently — use `task_get` to check status.
- Use `task_list` to see all running tasks.
- Tasks can be stopped with `task_stop`.";

/// Prompt for `task_get`.
pub const TASK_GET: &str = "\
Get details of a background task by ID.

Usage:
- Pass the task `id` to retrieve its current status, output, and metadata.
- Returns status (pending, running, completed, failed, stopped) and any output.

Notes:
- Use this to check on background tasks created with `task_create`.
- For a list of all tasks, use `task_list` instead.";

/// Prompt for `task_list`.
pub const TASK_LIST: &str = "\
List all background tasks.

Usage:
- Returns a list of all tasks with their IDs, titles, statuses, and outputs.
- No parameters required.

Notes:
- Useful for getting an overview of all running and completed tasks.
- Use `task_get` for detailed information about a specific task.";

/// Prompt for `task_stop`.
pub const TASK_STOP: &str = "\
Stop a running background task.

Usage:
- Pass the task `id` to stop it.
- The task's status will change to 'stopped'.
- Any partial output is preserved.

Notes:
- Only running tasks can be stopped.
- Stopped tasks cannot be restarted — create a new task instead.";

/// Prompt for `task_update`.
pub const TASK_UPDATE: &str = "\
Update the status or output of a background task.

Usage:
- `id` identifies the task to update.
- Optionally set `status` to a new value (pending, running, completed, failed, stopped).
- Optionally set `output` to update the task's output text.

Notes:
- Use this to record task results or change status programmatically.
- At least one of `status` or `output` should be provided.";

/// Prompt for `team_create`.
pub const TEAM_CREATE: &str = "\
Create a multi-agent team with a lead and optional agent definitions.

Usage:
- `team_name` optionally requests a specific persistent team name.
- `objective` describes the team's overall goal.
- `lead` optionally names the lead agent.
- `agents` is an array of agent definitions with `name`, `role`, and optional routing fields such as `cwd`, `model`, `color`, `worktree_path`, and `session_id`.

Notes:
- Teams coordinate multiple agents to work on complex tasks.
- Each agent gets its own context and tool access.
- Use `team_status` to monitor team progress.";

/// Prompt for `team_delete`.
pub const TEAM_DELETE: &str = "\
Delete a multi-agent team and clean up associated resources (team file, worktree, mailbox).

Usage:
- `team_name` identifies the team to delete.
- All associated resources are cleaned up on deletion.

Notes:
- This action is irreversible — deleted teams cannot be recovered.
- Ensure all team tasks are complete before deleting.";

/// Prompt for `team_status`.
pub const TEAM_STATUS: &str = "\
Get the current status of the multi-agent team.

Usage:
- `team_name` optionally selects one specific team.
- Returns the team's objective, member statuses, unread mailbox counts, and overall progress.

Notes:
- Use this to monitor team progress and identify blocked agents.
- For a list of all teams, use `team_list`.";

/// Prompt for `team_list`.
pub const TEAM_LIST: &str = "\
List all multi-agent teams with their metadata.

Usage:
- Returns a list of all teams with their names, objectives, and member counts.
- No parameters required.

Notes:
- Use this to discover existing teams before creating new ones.
- For detailed status of a specific team, use `team_status`.";

/// Prompt for `review_artifact`.
pub const REVIEW_ARTIFACT: &str = "\
Review an artifact: view diff, add comments, update status, or get review summary.

Usage:
- `action` determines the operation: 'view_diff', 'add_comment', 'update_status', 'get_comments', 'summary'.
- `artifact_id` identifies the artifact to review.
- Additional parameters depend on the action (comment, status, file_path, line, etc.).

Notes:
- Use 'view_diff' to see changes between versions.
- Use 'add_comment' for inline or general review comments with severity levels.
- Use 'update_status' to approve, request changes, or reject.
- Use 'summary' for an overview of all review feedback.";

// ── Workflow tools (P2) ──────────────────────────────────────────────────────

/// Prompt for `schedule_cron`.
pub const SCHEDULE_CRON: &str = "\
Schedule a cron job that runs a command periodically.

Usage:
- `schedule` is a cron expression (e.g., '*/5 * * * *' for every 5 minutes).
- `command` is the shell command to run on each invocation.
- Optionally provide a `description` for the cron job.
- Use action 'list' to see all scheduled jobs, 'delete' to remove one by ID.

Notes:
- Cron expressions follow standard 5-field format (minute hour day month weekday).
- Scheduled jobs persist across sessions.
- Be cautious with frequency — avoid scheduling resource-intensive commands too often.";

/// Prompt for `workflow`.
pub const WORKFLOW: &str = "\
Create, run, list, delete, or check status of a simple workflow with sequential step execution.

Usage:
- `action` determines the operation: 'create', 'run', 'status', 'list', 'delete'.
- `name` identifies the workflow.
- `steps` is an array of shell commands for the 'create' action.
- Steps execute sequentially — a failed step stops the workflow.

Notes:
- Workflows are useful for multi-step processes like build pipelines.
- Use 'status' to check progress of a running workflow.
- Each step's output is captured and available in the status response.";

/// Prompt for `daemon`.
pub const DAEMON: &str = "\
Manage background daemon processes: start, stop, status, list, restart, and logs.

Usage:
- `action` determines the operation: 'start', 'stop', 'status', 'list', 'restart', 'logs'.
- For 'start', provide `command` with the command to run.
- For 'stop', 'restart', 'logs', provide `id` with the daemon ID.
- For 'logs', optionally set `lines` to control output (default 50, max 500).

Notes:
- Daemons run persistently in the background until stopped.
- Use 'list' to see all running daemons and their IDs.
- Use 'logs' to inspect daemon output for debugging.
- Daemons are automatically cleaned up when the session ends.";

/// Prompt for `remote_trigger`.
pub const REMOTE_TRIGGER: &str = "\
Send an HTTP POST to trigger a remote event.

Usage:
- `url` is the endpoint to POST to.
- `event` is the event name or type.
- `payload` is an optional JSON object to send as the request body.

Notes:
- Requires network access to the target URL.
- Use this for webhook integrations and remote notifications.
- The target server must be configured to accept the event.";

/// Prompt for `enter_worktree`.
pub const ENTER_WORKTREE: &str = "\
Suggest a git worktree add command for a branch.

Usage:
- `branch` specifies the branch name for the worktree.
- Returns the suggested git worktree add command.

Notes:
- Worktrees allow working on multiple branches simultaneously.
- The branch must exist in the repository.
- Use `exit_worktree` to clean up when done.";

/// Prompt for `exit_worktree`.
pub const EXIT_WORKTREE: &str = "\
Suggest a git worktree remove command for a branch.

Usage:
- `branch` specifies the branch whose worktree to remove.
- Returns the suggested git worktree remove command.

Notes:
- Ensure all changes in the worktree are committed or stashed before removing.
- Use `list_worktrees` to see all active worktrees.";

/// Prompt for `list_worktrees`.
pub const LIST_WORKTREES: &str = "\
List all git worktrees in the current repository.

Usage:
- Returns a list of all worktrees with their paths, branches, and status.
- No parameters required.

Notes:
- Useful for understanding the current worktree layout.
- Use before creating or removing worktrees.";

// ── Other tools (P2) ─────────────────────────────────────────────────────────

/// Prompt for `powershell`.
pub const POWERSHELL: &str = "\
Execute a PowerShell command (Windows only).

Usage:
- The `command` parameter contains the PowerShell command to execute.
- Use `cwd` to run in a subdirectory instead of prefixing with Set-Location.
- Optionally provide a `description` of what the command does.
- Set `timeout_ms` for commands that may take longer (max 600000ms).
- Set `background` to true for long-running commands.

Notes:
- Only available on Windows systems.
- Prefer this over `bash_command` for Windows-specific operations.
- For cross-platform commands, prefer `bash_command`.
- Follows the same safety guidelines as `bash_command`.";

/// Prompt for `repl`.
pub const REPL: &str = "\
Execute code in a language REPL (python, node, or rust).

Usage:
- `language` selects the runtime: 'python', 'node', or 'rust'.
- `code` contains the code to execute.
- Returns the output (stdout) and any errors (stderr).

Notes:
- Each invocation runs in a fresh context — state is NOT preserved between calls.
- For multi-step computations, write a script file and run it with `bash_command` instead.
- Requires the selected runtime to be installed on the system.";

/// Prompt for `web_browser`.
pub const WEB_BROWSER: &str = "\
Enhanced web browser: fetch URL content, extract links, extract text, or take a screenshot.

Usage:
- `url` is the target URL.
- `action` determines the operation: 'fetch' (default), 'extract_links', 'extract_text', 'screenshot'.
- 'fetch' returns the full page content.
- 'extract_links' returns all hyperlinks on the page.
- 'extract_text' returns only the text content (no HTML).
- 'screenshot' captures a visual screenshot.

Notes:
- Requires network access.
- Some websites may block automated browsing.
- For simple content fetching, `web_fetch` may be sufficient.";

/// Prompt for `web_search`.
pub const WEB_SEARCH: &str = "\
Search the web for information using a search API.

Usage:
- `query` is the search query string.
- `max_results` controls the number of results (default 5, max 10).
- Returns titles, URLs, and summaries for each result.

Notes:
- Use this for finding documentation, solutions, or current information.
- For fetching specific URL content, use `web_fetch` or `web_browser`.
- Search results are summaries — use `web_fetch` for full page content.";

/// Prompt for `tungsten`.
pub const TUNGSTEN: &str = "\
Smart build/test/run engine that detects project type and executes the right commands.

Usage:
- `action` determines the operation: 'compile', 'run', 'test'.
- `target` specifies what to build/test/run (e.g., a package name, test filter, or binary).
- Automatically detects the project type (Rust, Node.js, Python, etc.) and uses appropriate tools.

Notes:
- Useful for running project-specific commands without memorizing build systems.
- For more control over command execution, use `bash_command` directly.
- Requires the appropriate build tools to be installed.";

/// Prompt for `overflow_test`.
pub const OVERFLOW_TEST: &str = "\
Generate test data for verifying context management edge cases.

Usage:
- `scenario` selects the test scenario: 'large_output', 'many_messages', 'deep_recursion'.
- Returns synthetic data designed to test context window handling.

Notes:
- This is a testing/debugging tool — do not use in normal workflows.
- Generated data can be very large — be mindful of context budget.
- Use `brief` to truncate output if needed.";

/// Prompt for `synthetic_output`.
pub const SYNTHETIC_OUTPUT: &str = "\
Generate synthetic test data in JSON, CSV, Markdown, or text format.

Usage:
- `type` selects the output format: 'json', 'csv', 'markdown', 'text'.
- `rows` controls the number of data rows (max 1000).
- Returns formatted synthetic data for testing and prototyping.

Notes:
- Data is randomly generated and not meaningful — for testing only.
- Useful for prototyping data processing pipelines.
- Combine with `write_file` to save generated data.";

/// Prompt for `voice_input`.
pub const VOICE_INPUT: &str = "\
Capture voice input via microphone, record audio, and transcribe to text.

Usage:
- `duration_secs` sets recording duration in seconds (default 5, max 60).
- `language` sets the language code for transcription (default 'en').
- Returns the transcribed text.

Notes:
- Requires microphone access and a supported transcription service.
- Longer recordings provide more context but take more time to transcribe.
- Use this for hands-free interaction or when typing is impractical.";

/// Prompt for `suggest_pr`.
pub const SUGGEST_PR: &str = "\
Analyze git diff and suggest a PR title and description.

Usage:
- No parameters required — automatically analyzes the current branch's changes.
- Returns a suggested PR title and description based on the diff.

Notes:
- Ensure all changes are committed before using this tool.
- The suggestion is based on diff analysis — review and adjust as needed.
- Use `bash_command` with `gh pr create` to actually create the PR.";

/// Prompt for `memory_read`.
pub const MEMORY_READ: &str = "\
Read persistent memory (RC.md) from global and/or project scope.

Usage:
- `scope` selects the memory scope: 'global', 'project', or 'all' (default: all).
- Returns the stored memory content for the selected scope.

Notes:
- Memory persists across sessions and is useful for storing project-specific knowledge.
- Global memory is shared across all projects.
- Project memory is specific to the current workspace.";

/// Prompt for `memory_write`.
pub const MEMORY_WRITE: &str = "\
Write or append to persistent memory (RC.md) in global or project scope.

Usage:
- `scope` selects the target: 'global' or 'project'.
- `content` is the text to write.
- `mode` controls write behavior: 'append' (default) or 'overwrite'.

Notes:
- Use append mode to add new information without losing existing content.
- Use overwrite mode to replace all memory content — be careful.
- Memory is persisted to disk and survives session restarts.";

/// Prompt for `list_peers`.
pub const LIST_PEERS: &str = "\
List all registered agents in the multi-agent system.

Usage:
- `team_name` optionally limits the listing to one team.
- Returns the visible peers with their names, roles, activity, and team metadata.

Notes:
- Use this to discover available agents for communication via `send_message`.
- Agents must be registered to appear in the list.";

/// Prompt for `discover_skills` (Phase 9).
pub const DISCOVER_SKILLS: &str = "\
Discover relevant skills using BM25 text search based on a task description query.

Usage:
- `query` is a task description to search for matching skills.
- `max_results` controls the number of results (default 10, max 20).
- Returns skills ranked by relevance to the query.

Notes:
- Uses BM25 text search for fuzzy matching.
- Provide a descriptive query for better results.
- Use `skill_execute` to load and follow a discovered skill's instructions.";

/// Prompt for `broadcast_message`.
pub const BROADCAST_MESSAGE: &str = "\
Broadcast a message to all agents in the multi-agent system.

Usage:
- `team_name` optionally selects the target team when multiple teams exist.
- `message` is the content to broadcast.
- `sender` optionally identifies the sender (default: coordinator).
- `priority` sets message priority: 'low', 'normal', or 'high' (default: normal).
- `recipients` optionally limits broadcast to specific agent names.

Notes:
- Without `recipients`, the message goes to all registered agents.
- Use `send_message` for direct one-to-one communication.
- High-priority messages may interrupt agent workflows.";

// ── Detailed prompt functions (Claude Code parity) ───────────────────────────

/// Returns the full Bash tool prompt matching Claude Code's BashTool/prompt.ts.
///
/// Includes background usage notes, commit and PR instructions, sandbox
/// section, and comprehensive shell usage guidelines.
#[must_use]
pub fn bash_tool_prompt() -> String {
    let background_note = "You can use the `run_in_background` parameter to run the command in \
        the background. Only use this if you don't need the result immediately and are OK being \
        notified when the command completes later. You do not need to check the output right \
        away — you'll be notified when it finishes. You do not need to use '&' at the end of the \
        command when using this parameter.";

    let commit_pr = "# Committing changes with git

Only create commits when requested by the user. If unclear, ask first. When the user asks you to \
create a new git commit, follow these steps carefully:

Git Safety Protocol:
- NEVER update the git config
- NEVER run destructive git commands (push --force, reset --hard, checkout ., restore ., clean \
-f, branch -D) unless the user explicitly requests these actions
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it
- NEVER run force push to main/master, warn the user if they request it
- CRITICAL: Always create NEW commits rather than amending, unless the user explicitly requests a \
git amend. When a pre-commit hook fails, the commit did NOT happen — so --amend would modify the \
PREVIOUS commit, which may result in destroying work or losing previous changes. Instead, after \
hook failure, fix the issue, re-stage, and create a NEW commit
- When staging files, prefer adding specific files by name rather than using \"git add -A\" or \
\"git add .\", which can accidentally include sensitive files (.env, credentials) or large binaries
- NEVER commit changes unless the user explicitly asks you to

1. Run git status, git diff, and git log in parallel to understand the current state.
2. Analyze all staged changes and draft a commit message focusing on the \"why\" rather than the \
\"what\".
3. Add relevant files, create the commit, and verify with git status.
4. If the commit fails due to pre-commit hook: fix the issue and create a NEW commit.

# Creating pull requests
Use the gh command via the Bash tool for ALL GitHub-related tasks. When creating a PR:
1. Run git status, git diff, git log, and git diff [base]...HEAD in parallel.
2. Analyze ALL commits (not just the latest) and draft a PR title and summary.
3. Create the branch, push, and create PR using gh pr create with a HEREDOC for the body.";

    let sandbox = "# Command sandbox
By default, your command will be run in a sandbox. This sandbox controls which directories and \
network hosts commands may access or modify without an explicit override. For temporary files, \
always use the $TMPDIR environment variable instead of /tmp directly.";

    format!(
        "Executes a given bash command and returns its output.\n\n\
        The working directory persists between commands, but shell state does not. The shell \
        environment is initialized from the user's profile (bash or zsh).\n\n\
        IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, \
        `awk`, or `echo` commands, unless explicitly instructed or after you have verified that \
        a dedicated tool cannot accomplish your task. Instead, use the appropriate dedicated tool:\n\
        - File search: Use Glob (NOT find or ls)\n\
        - Content search: Use Grep (NOT grep or rg)\n\
        - Read files: Use Read (NOT cat/head/tail)\n\
        - Edit files: Use Edit (NOT sed/awk)\n\
        - Write files: Use Write (NOT echo/cat)\n\n\
        While the Bash tool can do similar things, it's better to use the built-in tools as they \
        provide a better user experience and make it easier to review tool calls and give \
        permission.\n\n\
        # Instructions\n\
        - If your command will create new directories or files, first run `ls` to verify the \
        parent directory exists.\n\
        - Always quote file paths containing spaces with double quotes.\n\
        - Try to maintain your current working directory by using absolute paths and avoiding `cd`.\n\
        - You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes).\n\
        - {background_note}\n\
        - When issuing multiple commands, use && for sequential dependent commands, and make \
        multiple Bash calls for parallel independent commands.\n\
        - For git commands: prefer new commits over amending. Never skip hooks. Never force push \
        to main.\n\
        - Avoid unnecessary sleep commands. Use run_in_background for long-running tasks.\n\n\
        {sandbox}\n\n\
        {commit_pr}"
    )
}

/// Returns the file-edit tool prompt with detailed editing instructions.
#[must_use]
pub fn file_edit_tool_prompt() -> String {
    "Performs precise, targeted search/replace edits on an existing text file.\n\n\
    # Usage\n\
    - You MUST read the file at least once before editing. The tool will error otherwise.\n\
    - Each edit has a `search` string and a `replace` string. Edits are applied in sequence.\n\
    - The `search` string must match EXACTLY, including all whitespace and indentation.\n\
    - The edit will FAIL if any `search` string is not found in the file at its point of \
    application.\n\
    - Set `create_if_missing` to true to create the file if it does not exist.\n\n\
    # Best practices\n\
    - ALWAYS prefer editing existing files over creating new files.\n\
    - Use the smallest search string that is clearly unique — usually 2-4 adjacent lines is \
    sufficient.\n\
    - When editing text from read_file output, ensure you preserve the exact indentation.\n\
    - Never include line number prefixes in the search or replace strings.\n\
    - For a single replacement, use replace_in_file instead.\n\
    - When editing code, always consider the context in which the code is being used. Ensure that \
    your changes are compatible with the existing codebase and follow the project's coding \
    standards and best practices.\n\n\
    # Important\n\
    - Do NOT include `// rest of code unchanged` or similar placeholders.\n\
    - ALWAYS provide the COMPLETE intended content in each replace block.\n\
    - Partial updates or placeholders are STRICTLY FORBIDDEN."
        .to_owned()
}

/// Returns the file-read tool prompt with detailed reading instructions.
#[must_use]
pub fn file_read_tool_prompt() -> String {
    "Reads a UTF-8 text file from the local filesystem. You can access any file directly by using \
    this tool.\n\n\
    # Usage\n\
    - The `path` parameter must be a path relative to the current workspace.\n\
    - By default, it reads up to 2000 lines starting from the beginning of the file.\n\
    - Use `start_line` and `end_line` to read specific ranges, especially for large files.\n\
    - Results are returned with line numbers, starting at 1.\n\
    - This tool can read images (PNG, JPG, etc.) — contents are presented visually.\n\
    - This tool can read Jupyter notebooks (.ipynb) and returns all cells with outputs.\n\
    - This tool can only read files, not directories. Use list_directory for directories.\n\n\
    # Notes\n\
    - If you read a file that exists but has empty contents, a warning will be returned.\n\
    - It is okay to read a file that does not exist; an error will be returned.\n\
    - Always read a file before editing it — the edit tool requires a prior read.\n\
    - For very large files, read in chunks using start_line/end_line to avoid truncation.\n\
    - Supports text extraction from PDF and DOCX files.\n\
    - Lines longer than 2000 characters are truncated in the output."
        .to_owned()
}

/// Returns the file-write tool prompt with detailed writing instructions.
#[must_use]
pub fn file_write_tool_prompt() -> String {
    "Writes a file to the local filesystem. Creates the file if it does not exist; overwrites if \
    it does.\n\n\
    # Usage\n\
    - The `path` parameter is relative to the current workspace directory.\n\
    - The `content` parameter must contain the COMPLETE file content — partial writes are not \
    supported.\n\
    - Set `append` to true to append content to an existing file instead of overwriting.\n\
    - If this is an existing file, you MUST read it first to understand its current contents.\n\
    - Prefer the edit_file or replace_in_file tool for modifying existing files — it only sends \
    the diff.\n\n\
    # Important\n\
    - NEVER create documentation files (*.md) or README files unless explicitly requested.\n\
    - This tool automatically creates any intermediate directories needed.\n\
    - Do NOT use this tool for small edits to existing files — use edit_file instead.\n\
    - ALWAYS provide the COMPLETE intended content. Partial updates or placeholders are FORBIDDEN.\n\
    - Failure to do so will result in incomplete or broken code.\n\
    - When creating a new project, organize all new files within a dedicated project directory \
    unless the user specifies otherwise.".to_owned()
}

/// Returns the agent tool prompt with detailed sub-agent delegation instructions.
#[must_use]
pub fn agent_tool_prompt() -> String {
    "Launch a new agent to handle complex, multi-step tasks autonomously.\n\n\
    The Agent tool launches specialized agents (subprocesses) that autonomously handle complex \
    tasks. Each agent type has specific capabilities and tools available to it.\n\n\
    Available agent types are provided separately for the current session.\n\n\
    When using the Agent tool, specify a subagent_type parameter to select which agent type to \
    use. If omitted, the general-purpose agent is used.\n\n\
    When NOT to use the Agent tool:\n\
    - If you want to read a specific file path, use the ReadFile tool or the Glob tool instead of \
    the Agent tool, to find the match more quickly.\n\
    - If you are searching for a specific class definition like \"class Foo\", use the Glob tool \
    instead, to find the match more quickly.\n\
    - If you are searching for code within a specific file or set of 2-3 files, use the ReadFile \
    tool instead of the Agent tool, to find the match more quickly.\n\
    - Other tasks that are not related to the agent descriptions above.\n\n\
    Usage notes:\n\
    - Always include a short description (3-5 words) summarizing what the agent will do.\n\
    - When the agent is done, it will return a single message back to you. The result returned by \
    the agent is not visible to the user. To show the user the result, you should send a text \
    message back to the user with a concise summary of the result.\n\
    - You can optionally run agents in the background using the run_in_background parameter. When \
    an agent runs in the background, you will be automatically notified when it completes. Do NOT \
    sleep, poll, or proactively check on its progress.\n\
    - Foreground vs background: use foreground when you need the agent's results before you can \
    proceed. Use background when you have genuinely independent work to do in parallel.\n\
    - To continue a previously spawned agent, use SendMessage with the agent's ID or name as the \
    `to` field. Each Agent invocation starts fresh, so provide a complete task description.\n\
    - The agent's outputs should generally be trusted.\n\
    - Clearly tell the agent whether you expect it to write code or just to do research.\n\
    - If the user specifies that they want you to run agents in parallel, you MUST send a single \
    message with multiple Agent tool use content blocks.\n\
    - You can optionally set `isolation: \"worktree\"` to request an isolated git worktree for the \
    agent.\n\
    - You can optionally set `cwd` to run the agent in a specific working directory.\n\n\
    ## Writing the prompt\n\n\
    When spawning a fresh agent, it starts with zero context. Brief the agent like a smart \
    colleague who just walked into the room — it hasn't seen this conversation, doesn't know what \
    you've tried, and doesn't understand why this task matters.\n\
    - Explain what you're trying to accomplish and why.\n\
    - Describe what you've already learned or ruled out.\n\
    - Give enough context about the surrounding problem that the agent can make judgment calls \
    rather than just following a narrow instruction.\n\
    - If you need a short response, say so (\"report in under 200 words\").\n\
    - Lookups: hand over the exact command. Investigations: hand over the question.\n\n\
    Terse command-style prompts produce shallow, generic work.\n\n\
    Never delegate understanding. Don't write \"based on your findings, fix the bug\" or \
    \"based on the research, implement it.\" Write prompts that prove you understood: include file \
    paths, line numbers, and what specifically to change."
        .to_owned()
}

/// Lookup table: returns the detailed prompt for a tool by its internal name.
///
/// Returns an empty string for unknown tool names.
#[must_use]
pub fn get_prompt(tool_name: &str) -> &'static str {
    match tool_name {
        "list_directory" => LIST_DIRECTORY,
        "read_file" => READ_FILE,
        "search_text" => SEARCH_TEXT,
        "write_file" => WRITE_FILE,
        "replace_in_file" => REPLACE_IN_FILE,
        "edit_file" => EDIT_FILE,
        "bash_command" => BASH_COMMAND,
        "glob" => GLOB,
        "grep" => GREP,
        "web_fetch" => WEB_FETCH,
        "ask_user" => ASK_USER,
        "todo_write" => TODO_WRITE,
        "config_read" => CONFIG_READ,
        "agent" => AGENT,
        "web_search" => WEB_SEARCH,
        "lsp" => LSP,
        "task_create" => TASK_CREATE,
        "task_get" => TASK_GET,
        "task_list" => TASK_LIST,
        "task_stop" => TASK_STOP,
        "task_update" => TASK_UPDATE,
        "notebook_edit" => NOTEBOOK_EDIT,
        "skill_discover" => SKILL_DISCOVER,
        "skill_execute" => SKILL_EXECUTE,
        "send_message" => SEND_MESSAGE,
        "enter_plan_mode" => ENTER_PLAN_MODE,
        "exit_plan_mode" => EXIT_PLAN_MODE,
        "sleep" => SLEEP,
        "snip" => SNIP,
        "tool_search" => TOOL_SEARCH,
        "verify_plan" => VERIFY_PLAN,
        "terminal_capture" => TERMINAL_CAPTURE,
        "monitor" => MONITOR,
        "brief" => BRIEF,
        "ctx_inspect" => CTX_INSPECT,
        "send_user_file" => SEND_USER_FILE,
        "mcp_call" => MCP_CALL,
        "mcp_auth" => MCP_AUTH,
        "list_mcp_resources" => LIST_MCP_RESOURCES,
        "read_mcp_resource" => READ_MCP_RESOURCE,
        "team_create" => TEAM_CREATE,
        "team_delete" => TEAM_DELETE,
        "team_status" => TEAM_STATUS,
        "team_list" => TEAM_LIST,
        "review_artifact" => REVIEW_ARTIFACT,
        "schedule_cron" => SCHEDULE_CRON,
        "workflow" => WORKFLOW,
        "daemon" => DAEMON,
        "remote_trigger" => REMOTE_TRIGGER,
        "enter_worktree" => ENTER_WORKTREE,
        "exit_worktree" => EXIT_WORKTREE,
        "list_worktrees" => LIST_WORKTREES,
        "powershell" => POWERSHELL,
        "repl" => REPL,
        "web_browser" => WEB_BROWSER,
        "tungsten" => TUNGSTEN,
        "overflow_test" => OVERFLOW_TEST,
        "synthetic_output" => SYNTHETIC_OUTPUT,
        "voice_input" => VOICE_INPUT,
        "suggest_pr" => SUGGEST_PR,
        "memory_read" => MEMORY_READ,
        "memory_write" => MEMORY_WRITE,
        "list_peers" => LIST_PEERS,
        "discover_skills" => DISCOVER_SKILLS,
        "broadcast_message" => BROADCAST_MESSAGE,
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_prompts_are_non_empty() {
        let prompts = [
            LIST_DIRECTORY,
            READ_FILE,
            SEARCH_TEXT,
            WRITE_FILE,
            REPLACE_IN_FILE,
            EDIT_FILE,
            BASH_COMMAND,
            GLOB,
            GREP,
            WEB_FETCH,
            ASK_USER,
            TODO_WRITE,
            CONFIG_READ,
            AGENT,
            WEB_SEARCH,
            LSP,
            TASK_CREATE,
            TASK_GET,
            TASK_LIST,
            TASK_STOP,
            TASK_UPDATE,
            NOTEBOOK_EDIT,
            SKILL_DISCOVER,
            SKILL_EXECUTE,
            SEND_MESSAGE,
            ENTER_PLAN_MODE,
            EXIT_PLAN_MODE,
            SLEEP,
            SNIP,
            TOOL_SEARCH,
            VERIFY_PLAN,
            TERMINAL_CAPTURE,
            MONITOR,
            BRIEF,
            CTX_INSPECT,
            SEND_USER_FILE,
            MCP_CALL,
            MCP_AUTH,
            LIST_MCP_RESOURCES,
            READ_MCP_RESOURCE,
            TEAM_CREATE,
            TEAM_DELETE,
            TEAM_STATUS,
            TEAM_LIST,
            REVIEW_ARTIFACT,
            SCHEDULE_CRON,
            WORKFLOW,
            DAEMON,
            REMOTE_TRIGGER,
            ENTER_WORKTREE,
            EXIT_WORKTREE,
            LIST_WORKTREES,
            POWERSHELL,
            REPL,
            WEB_BROWSER,
            TUNGSTEN,
            OVERFLOW_TEST,
            SYNTHETIC_OUTPUT,
            VOICE_INPUT,
            SUGGEST_PR,
            MEMORY_READ,
            MEMORY_WRITE,
            LIST_PEERS,
            DISCOVER_SKILLS,
            BROADCAST_MESSAGE,
        ];
        for prompt in &prompts {
            assert!(!prompt.is_empty(), "Prompt must not be empty");
        }
    }

    #[test]
    fn all_prompts_under_max_length() {
        let prompts_with_names = [
            ("LIST_DIRECTORY", LIST_DIRECTORY),
            ("READ_FILE", READ_FILE),
            ("SEARCH_TEXT", SEARCH_TEXT),
            ("WRITE_FILE", WRITE_FILE),
            ("REPLACE_IN_FILE", REPLACE_IN_FILE),
            ("EDIT_FILE", EDIT_FILE),
            ("BASH_COMMAND", BASH_COMMAND),
            ("GLOB", GLOB),
            ("GREP", GREP),
            ("WEB_FETCH", WEB_FETCH),
            ("ASK_USER", ASK_USER),
            ("TODO_WRITE", TODO_WRITE),
            ("CONFIG_READ", CONFIG_READ),
            ("AGENT", AGENT),
            ("WEB_SEARCH", WEB_SEARCH),
            ("LSP", LSP),
            ("TASK_CREATE", TASK_CREATE),
            ("TASK_GET", TASK_GET),
            ("TASK_LIST", TASK_LIST),
            ("TASK_STOP", TASK_STOP),
            ("TASK_UPDATE", TASK_UPDATE),
            ("NOTEBOOK_EDIT", NOTEBOOK_EDIT),
            ("SKILL_DISCOVER", SKILL_DISCOVER),
            ("SKILL_EXECUTE", SKILL_EXECUTE),
            ("SEND_MESSAGE", SEND_MESSAGE),
            ("ENTER_PLAN_MODE", ENTER_PLAN_MODE),
            ("EXIT_PLAN_MODE", EXIT_PLAN_MODE),
            ("SLEEP", SLEEP),
            ("SNIP", SNIP),
            ("TOOL_SEARCH", TOOL_SEARCH),
            ("VERIFY_PLAN", VERIFY_PLAN),
            ("TERMINAL_CAPTURE", TERMINAL_CAPTURE),
            ("MONITOR", MONITOR),
            ("BRIEF", BRIEF),
            ("CTX_INSPECT", CTX_INSPECT),
            ("SEND_USER_FILE", SEND_USER_FILE),
            ("MCP_CALL", MCP_CALL),
            ("MCP_AUTH", MCP_AUTH),
            ("LIST_MCP_RESOURCES", LIST_MCP_RESOURCES),
            ("READ_MCP_RESOURCE", READ_MCP_RESOURCE),
            ("TEAM_CREATE", TEAM_CREATE),
            ("TEAM_DELETE", TEAM_DELETE),
            ("TEAM_STATUS", TEAM_STATUS),
            ("TEAM_LIST", TEAM_LIST),
            ("REVIEW_ARTIFACT", REVIEW_ARTIFACT),
            ("SCHEDULE_CRON", SCHEDULE_CRON),
            ("WORKFLOW", WORKFLOW),
            ("DAEMON", DAEMON),
            ("REMOTE_TRIGGER", REMOTE_TRIGGER),
            ("ENTER_WORKTREE", ENTER_WORKTREE),
            ("EXIT_WORKTREE", EXIT_WORKTREE),
            ("LIST_WORKTREES", LIST_WORKTREES),
            ("POWERSHELL", POWERSHELL),
            ("REPL", REPL),
            ("WEB_BROWSER", WEB_BROWSER),
            ("TUNGSTEN", TUNGSTEN),
            ("OVERFLOW_TEST", OVERFLOW_TEST),
            ("SYNTHETIC_OUTPUT", SYNTHETIC_OUTPUT),
            ("VOICE_INPUT", VOICE_INPUT),
            ("SUGGEST_PR", SUGGEST_PR),
            ("MEMORY_READ", MEMORY_READ),
            ("MEMORY_WRITE", MEMORY_WRITE),
            ("LIST_PEERS", LIST_PEERS),
            ("DISCOVER_SKILLS", DISCOVER_SKILLS),
            ("BROADCAST_MESSAGE", BROADCAST_MESSAGE),
        ];
        for (name, prompt) in &prompts_with_names {
            assert!(
                prompt.len() <= 2000,
                "Prompt {name} is {} chars, exceeds 2000 char limit",
                prompt.len()
            );
        }
    }

    #[test]
    fn get_prompt_returns_known_tools() {
        assert!(!get_prompt("bash_command").is_empty());
        assert!(!get_prompt("read_file").is_empty());
        assert!(!get_prompt("write_file").is_empty());
        assert!(!get_prompt("agent").is_empty());
    }

    #[test]
    fn get_prompt_returns_empty_for_unknown() {
        assert!(get_prompt("nonexistent_tool_xyz").is_empty());
    }

    #[test]
    fn prompt_count_covers_all_builtin_tools() {
        // Count all non-empty prompts returned by get_prompt for known tool names
        let known_tools = [
            "list_directory",
            "read_file",
            "search_text",
            "write_file",
            "replace_in_file",
            "edit_file",
            "bash_command",
            "glob",
            "grep",
            "web_fetch",
            "ask_user",
            "todo_write",
            "config_read",
            "agent",
            "web_search",
            "lsp",
            "task_create",
            "task_get",
            "task_list",
            "task_stop",
            "task_update",
            "notebook_edit",
            "skill_discover",
            "skill_execute",
            "send_message",
            "enter_plan_mode",
            "exit_plan_mode",
            "sleep",
            "snip",
            "tool_search",
            "verify_plan",
            "terminal_capture",
            "monitor",
            "brief",
            "ctx_inspect",
            "send_user_file",
            "mcp_call",
            "mcp_auth",
            "list_mcp_resources",
            "read_mcp_resource",
            "team_create",
            "team_delete",
            "team_status",
            "team_list",
            "review_artifact",
            "schedule_cron",
            "workflow",
            "daemon",
            "remote_trigger",
            "enter_worktree",
            "exit_worktree",
            "list_worktrees",
            "powershell",
            "repl",
            "web_browser",
            "tungsten",
            "overflow_test",
            "synthetic_output",
            "voice_input",
            "suggest_pr",
            "memory_read",
            "memory_write",
            "list_peers",
            "discover_skills",
            "broadcast_message",
        ];
        let covered = known_tools
            .iter()
            .filter(|t| !get_prompt(t).is_empty())
            .count();
        assert_eq!(
            covered,
            known_tools.len(),
            "Not all builtin tools have prompts"
        );
    }

    // ── Detailed prompt function tests ─────────────────────────────────

    #[test]
    fn bash_tool_prompt_is_non_empty() {
        let prompt = bash_tool_prompt();
        assert!(!prompt.is_empty(), "bash_tool_prompt must not be empty");
    }

    #[test]
    fn bash_tool_prompt_is_long_enough() {
        let prompt = bash_tool_prompt();
        assert!(
            prompt.len() > 500,
            "bash_tool_prompt should be >500 chars, got {}",
            prompt.len()
        );
    }

    #[test]
    fn bash_tool_prompt_contains_key_phrases() {
        let prompt = bash_tool_prompt();
        assert!(
            prompt.contains("run_in_background"),
            "should mention background usage"
        );
        assert!(
            prompt.contains("Committing changes"),
            "should mention committing"
        );
        assert!(prompt.contains("sandbox"), "should mention sandbox");
        assert!(
            prompt.contains("Git Safety Protocol"),
            "should mention git safety"
        );
        assert!(
            prompt.contains("pull request"),
            "should mention PR creation"
        );
    }

    #[test]
    fn file_edit_tool_prompt_is_non_empty_and_long() {
        let prompt = file_edit_tool_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.len() > 200,
            "should be >200 chars, got {}",
            prompt.len()
        );
        assert!(prompt.contains("search"), "should mention search");
        assert!(prompt.contains("replace"), "should mention replace");
    }

    #[test]
    fn file_read_tool_prompt_is_non_empty_and_long() {
        let prompt = file_read_tool_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.len() > 200,
            "should be >200 chars, got {}",
            prompt.len()
        );
        assert!(prompt.contains("start_line"), "should mention start_line");
        assert!(prompt.contains("end_line"), "should mention end_line");
    }

    #[test]
    fn file_write_tool_prompt_is_non_empty_and_long() {
        let prompt = file_write_tool_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.len() > 200,
            "should be >200 chars, got {}",
            prompt.len()
        );
        assert!(prompt.contains("COMPLETE"), "should mention COMPLETE");
        assert!(prompt.contains("append"), "should mention append");
    }

    #[test]
    fn agent_tool_prompt_is_non_empty_and_long() {
        let prompt = agent_tool_prompt();
        assert!(!prompt.is_empty());
        assert!(
            prompt.len() > 200,
            "should be >200 chars, got {}",
            prompt.len()
        );
        assert!(prompt.contains("Agent tool"), "should mention Agent tool");
        assert!(
            prompt.contains("run_in_background"),
            "should mention background execution"
        );
        assert!(prompt.contains("SendMessage"), "should mention SendMessage");
    }
}
