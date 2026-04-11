# Compatibility

This document defines what `remote-code-rust` keeps compatible with the current `remote-code` product family and what is intentionally redesigned.

Compatibility is a product decision, not an excuse to preserve internal debt.

## Compatibility Priorities

The rewrite preserves compatibility in this order:

1. External user and automation contracts
2. Session and migration safety
3. Provider configuration continuity
4. Remote runner and control-plane interoperability
5. Internal architecture only when there is no better Rust-native design

## CLI Surface

The primary command name remains `remote-code`.

### Core Commands

- `remote-code doctor` ✅ implemented — provider readiness checks
- `remote-code sessions list` / `sessions show` ✅ implemented — session management
- `remote-code resume <session-id>` ✅ implemented — resume an existing session
- `remote-code export <session-id>` ✅ implemented — export as JSON or NDJSON
- `remote-code -p --input-format stream-json --output-format stream-json` ✅ implemented — headless mode

### Extended Commands

- `remote-code remote` ✅ full remote CLI with meta/runners/sessions/approvals/artifacts/events subcommands
- `remote-code mcp list/call` ✅ MCP server discovery and tool invocation
- `remote-code plugins list/inspect/invoke` ✅ plugin runtime discovery and invocation
- `remote-code agents plan` ✅ multi-agent team planning
- `remote-code hooks list` ✅ lifecycle hook discovery
- `remote-code migrate` ✅ legacy profile migration tool

### Interactive Mode

- `remote-code tui` ✅ interactive TUI with Vim mode, Tab completion, Ctrl+R history search, and theme system (dark/light/monokai/solarized)
- `remote-code headless` ✅ headless mode with stdin/stdout piping
- `remote-code --ssh <host>` ✅ SSH remote execution mode with config file, agent forwarding, port forwarding, and timeout support

Additional flags may be introduced, but existing supported compatibility flags must remain additive rather than breaking.

Commands that were only historical compatibility escape hatches in the reference workspace are not automatically part of the Rust public surface. They are reintroduced only if they serve a clear product need.

## Headless Protocol

The Rust runtime emits the important `stream-json` message families used by the current headless shell and remote orchestration paths:

- `system` ✅
- `assistant` ✅
- `result` ✅
- `control_request` ✅
- `control_cancel_request` ✅
- `tool_progress` ✅

The compatibility layer preserves the meaning of:

- init metadata ✅
- session state transitions ✅
- permission requests and cancellations ✅
- success vs. error result framing ✅

Internally, the runtime uses typed Rust enums and structs. Only the serializer stays legacy-shaped.

## Session Compatibility

The current product stores active state under `~/.remote-code/`.

`remote-code-rust` does not reuse that directory directly. The new default profile root is:

```text
~/.remote-code-rust/
```

The rewrite provides import tooling for:

- provider config and profile settings ✅
- session indexes and exportable transcripts ✅
- history and state files that can be mapped safely ✅
- skill and plugin inventories that can be represented in the new model ✅

Storage model: SQLite for session metadata + NDJSON event logs per session.

The migration tool is intentionally explicit. The runtime does not silently mutate the old profile.

## Provider Environment Variables

The following variables remain first-class compatibility inputs:

- `REMOTE_CODE_PROVIDER` ✅
- `REMOTE_CODE_BASE_URL` ✅
- `REMOTE_CODE_API_KEY` ✅
- `REMOTE_CODE_MODEL` ✅
- `REMOTE_CODE_REQUEST_HEADERS_JSON` ✅

The Rust rewrite adds explicit configuration where the reference implementation inferred behavior:

- `REMOTE_CODE_PROTOCOL` ✅ (`openai` | `anthropic`)
- `REMOTE_CODE_PROFILE_DIR` ✅
- `REMOTE_CODE_COMPAT_MODE` ✅

Precedence rules:

1. CLI flags
2. Explicit new env vars
3. Existing compatibility env vars
4. Profile config files
5. Safe defaults

`REMOTE_CODE_PROTOCOL` wins over base URL heuristics when both are provided.

## Provider Protocol Families

The runtime supports five provider protocols:

| Provider | Protocol | Streaming | Status |
|----------|----------|-----------|--------|
| OpenAI | `openai` | ✅ SSE | ✅ Complete |
| Anthropic | `anthropic` | ✅ SSE | ✅ Complete |
| GLM/ZhipuAI | `openai` | ✅ SSE | ✅ Complete |
| AWS Bedrock | `anthropic` | ✅ SSE | ✅ Complete |
| Google Vertex AI | `anthropic` | ✅ SSE | ✅ Complete |

Additional features implemented:

- Automatic failover between providers ✅ (`rc-provider/src/failover.rs`)
- Retry with exponential backoff ✅ (`rc-provider/src/streaming.rs`)
- Health tracking and circuit-breaker logic ✅
- Anthropic API cache optimization with `cache_control` ✅
- Cost tracking per model ✅
- Streaming tool execution with callbacks ✅

Gateway-specific integrations such as GLM, MiniMax, ZAI, and private proxies are configuration variants on top of these protocol families. They must not fork the architecture.

## Permissions

The reference workspace supports modes such as `default`, `acceptEdits`, `bypassPermissions`, `dontAsk`, and `plan`.

The Rust rewrite implements 5 permission modes via a typed permission service with rule engine:

| Mode | Read | Edit | Command | Notes |
|------|------|------|---------|-------|
| `default` | ✅ auto | ❌ ask | ❌ ask | Safe default |
| `acceptEdits` | ✅ auto | ✅ auto | ❌ ask | CI-friendly |
| `bypassPermissions` | ✅ auto | ✅ auto | ✅ auto | Full automation |
| `dontAsk` | ✅ auto | ❌ deny | ❌ deny | Read-only |
| `plan` | ✅ auto | ❌ deny | ❌ deny | Planning mode |

Compatibility guarantees:

- explicit approval prompts remain possible in interactive and remote flows ✅
- non-interactive denials are preserved when required ✅
- blocked path reporting remains serializable through `stream-json` ✅
- auditability is improved rather than reduced ✅
- fine-grained rules with wildcard matching ✅
- path-based and command-based rule patterns ✅

## Tools

The Rust rewrite implements 38+ built-in tools across multiple categories:

### File Operations

| Tool | Permission | Description |
|------|-----------|-------------|
| `read_file` | Read | Read file contents |
| `write_file` | Edit | Write or create files |
| `edit_file` | Edit | Multi-edit search/replace blocks |
| `replace_in_file` | Edit | Simple text replacement |
| `list_directory` | Read | List directory contents |

### Search

| Tool | Permission | Description |
|------|-----------|-------------|
| `search_text` | Read | Regex search across files |
| `glob` | Read | Glob pattern file matching |
| `grep` | Read | Fast content search |
| `lsp` | Read | Simplified LSP operations |

### Execution

| Tool | Permission | Description |
|------|-----------|-------------|
| `bash_command` | Command | Shell command execution with sandbox |

### Web

| Tool | Permission | Description |
|------|-----------|-------------|
| `web_search` | Read | Web search queries |
| `web_fetch` | Read | Fetch URL content |
| `web_browser` | Command | Browser automation |

### Agent System

| Tool | Permission | Description |
|------|-----------|-------------|
| `agent` | System | Spawn subtask agents |
| `send_message` | System | Inter-agent messaging |
| `team_create` | System | Create agent teams |
| `team_status` | Read | Query team status |

### Task Management

| Tool | Permission | Description |
|------|-----------|-------------|
| `task_create` | System | Create background tasks |
| `task_get` | Read | Get task status |
| `task_list` | Read | List all tasks |
| `task_stop` | System | Stop a running task |
| `task_update` | System | Update task state |
| `todo_write` | Edit | Write todo checklist |

### Memory

| Tool | Permission | Description |
|------|-----------|-------------|
| `memory_read` | Read | Read persistent memories |
| `memory_write` | Edit | Write persistent memories |

### Other

| Tool | Permission | Description |
|------|-----------|-------------|
| `ask_user` | System | Prompt user for input |
| `config_read` | Read | Read configuration |
| `sleep` | System | Delay execution |
| `snip` | Read | Code snippet extraction |
| `skill_discover` | Read | Discover available skills |
| `tool_search` | Read | BM25 tool search |
| `verify_plan` | System | Plan verification |
| `terminal_capture` | Read | Capture terminal output |
| `notebook_edit` | Edit | Edit Jupyter notebooks |
| `enter_plan_mode` | System | Enter planning mode |
| `exit_plan_mode` | System | Exit planning mode |

The rewrite preserves:

- the expectation that core local tools exist ✅
- the distinction between read-like and edit-like operations ✅
- the ability to route permission requests around mutable tools ✅
- a clear path for MCP-provided tools to appear alongside local tools ✅
- BM25 intelligent tool search ✅
- lazy tool loading for context window optimization ✅

Tool implementation details are intentionally new.

## Skills and Plugins

Compatibility targets:

- continue discovering skills from file-based `SKILL.md` roots ✅ (`rc-skills`)
- keep skill indexing and invocation as a first-class workflow ✅
- support legacy plugin ecosystems only through explicit adapter bridges ✅
- runtime skill discovery via `skill_discover` tool ✅

Plugin runtime (`rc-plugins`):

- JSON-RPC over stdio protocol ✅
- Plugin manifest with capabilities, actions, and runtime config ✅
- Isolated subprocess execution model ✅
- Bundled skill and MCP config discovery ✅

Non-goals:

- running legacy JavaScript plugin code directly inside the Rust process
- preserving plugin loading behavior that relies on implicit in-process side effects

## MCP (Model Context Protocol)

The Rust rewrite implements a full MCP client:

- stdio JSON-RPC transport ✅
- HTTP transport ✅ (config parsing + runtime)
- WebSocket transport ✅ (config parsing + runtime)
- Server discovery from config files, profile, and plugins ✅
- Tool listing and invocation ✅
- Structured content and error handling ✅

## Runner and Control Plane

The Rust control plane exposes versioned HTTP and WebSocket APIs.

Implemented API surface:

- `GET /health` ✅
- `POST /v1/runners` ✅ (runner registration)
- `PUT /v1/runners/:id/heartbeat` ✅
- `GET /v1/runners` ✅
- `GET /v1/runners/:id` ✅
- `POST /v1/sessions` ✅ (session creation with auto-dispatch)
- `GET /v1/sessions` ✅ (filtered listing)
- `GET /v1/sessions/:id` ✅
- `PUT /v1/sessions/:id/state` ✅ (state relay to runner)
- `POST /v1/artifacts` ✅ (upload)
- `GET /v1/artifacts` ✅ (listing)
- `GET /v1/artifacts/:id/download` ✅
- `POST /v1/approvals` ✅ (approval creation with relay)
- `PUT /v1/approvals/:id` ✅ (decision with relay)
- `GET /v1/events` ✅ (timeline with WebSocket streaming)
- Per-runner and per-session scoped endpoints ✅

Compatibility objectives:

- existing web and mobile clients should be able to migrate without a ground-up rewrite ✅
- runner registration and session streaming semantics remain familiar ✅
- approval and artifact flows map cleanly from the current `remote-hub` behavior ✅

The API surface is allowed to become stricter and more explicit than the current TypeScript service as long as compatibility shims preserve expected workflows.

## Multi-Agent System

The Rust rewrite introduces a multi-agent scheduler (`rc-agents`):

- Agent identity with name, role, and path ownership ✅
- Task lifecycle with budget scopes ✅
- Team planning with lead/coordinator roles ✅
- Mailbox-based inter-agent messaging ✅
- Capacity-aware scheduling ✅
- Parallel task execution ✅

This is a new capability not present in the reference implementation.

## Context Management

New capabilities for intelligent context window usage:

- Automatic token estimation per message ✅
- Context auto-compaction when approaching window limits ✅
- Anthropic API cache optimization with `cache_control` breakpoints ✅
- Lazy tool loading to reduce context pressure ✅

## Cost Tracking

New capability for monitoring AI spending:

- Per-request token counting (input, output, cache read, cache write) ✅
- Per-model cost accumulation ✅
- Session-level cost aggregation ✅
- Cost reporting via telemetry ✅

## Memory System

New capability for persistent memory:

- RC.md persistent memory (global/project scoped) ✅
- `memory_read` / `memory_write` tools ✅
- Per-project memory scoping ✅
- Automatic memory loading on session start ✅

## Fixture Strategy

Compatibility is enforced by committed fixtures collected from the reference workspace.

Fixture categories:

- `stream-json` init and session-state flows ✅
- permission request shapes ✅
- session export behaviors ✅
- error framing ✅
- provider normalization edge cases ✅

The old repository is read-only input for fixture collection. CI validates against committed fixtures instead of shelling out to the old project during normal test runs.

## Test Coverage

Current test suite: **200+ tests** passing across all crates.

| Crate | Tests | Category |
|-------|-------|----------|
| rc-agents | 5 | Scheduler, mailbox, capacity |
| rc-config | 6 | Config loading, hooks, normalization |
| rc-control-plane | 24 | Full API round-trip, WebSocket streaming |
| rc-core | 2 | Hook events, upstream shapes |
| rc-mcp | 9 | Config parsing, stdio transport, tool invocation |
| rc-permissions | 19 | Tool classification, permission modes, broker |
| rc-plugins | 8 | Manifest loading, runtime inspection, invocation |
| rc-runner | 11 | Runner API, sessions, approvals, health |
| rc-session | 11 | CRUD, conversation, events, export, bundle |
| rc-skills | 4 | Skill loading, front matter, lock file |
| rc-tools | 1 | Tool registry |
| remote-code | 31 | CLI parsing, remote helpers, MCP/Plugin CLI |
| remote-code-runner | 3 | Heartbeat, retry, control plane sync |
| Apps (doctor) | 2 | Runner + control plane doctor |
| **Total** | **200+** | |

## Deliberate Non-Compatibility

The following are intentionally not compatibility constraints:

- Bun-specific boot behavior
- internal TypeScript module layout
- runtime-specific implementation quirks that are not part of an external contract
- leaked or provenance-unclear code paths from third-party repositories
- fallback product surfaces that only existed as temporary scaffolding

Compatibility matters at the edges. The internals are free to become simpler and safer.
