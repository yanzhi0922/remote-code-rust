# Architecture

This document defines the architecture for `remote-code-rust`.

Each subsystem has one owner crate, one state model, and one boundary for integration.

## Top-Level Structure

The workspace is split into agent engines under `agents/`, application binaries under `apps/`, and libraries under `crates/`.

### Agent Sources

The `agents/` directory contains the AI agent engines:

- `agents/claudecode/`: **Claude Code Agent** — the Rust rewrite of Claude Code (formerly `apps/remote-code/`). This is the primary agent engine with CLI, TUI, headless, and interactive modes. It is a full workspace member.
- `agents/codex/`: OpenAI Codex source (`codex-rs/app-server`) — independent Git repository.
- `agents/roo-code/`: Roo Code source (`crates/roo-server`) — independent Git repository.

The codex and roo-code directories are excluded from the main repo via `.gitignore`. Build scripts in `scripts/` compile external agent binaries to `target/agent-binaries/`.

### Applications

- `agents/claudecode`: Claude Code Agent (Rust rewrite) — CLI, headless runtime, interactive shell, TUI, conversation loop
- `apps/remote-code-runner`: remote runner process that connects workspaces to the control plane
- `apps/remote-code-control-plane`: HTTP and WebSocket backend for sessions, approvals, artifacts, and runner coordination
- `apps/remote-code-migrate`: explicit migration and import tool

### Library Crates

- `rc-core`: shared runtime types, errors, conversation model, session states, hook types, tool types
- `rc-config`: CLI parsing, env loading, config precedence, profile resolution, provider config, legacy import
- `rc-protocol`: typed runtime events plus compatibility serializers for `stream-json`
- `rc-provider`: provider normalization, request shaping, transport, retries, streaming (SSE), failover, cost tracking, context management
- `rc-session`: session persistence (SQLite + NDJSON), indexes, exports, transcript appenders, resume loading, replay, memory system
- `rc-tools`: typed tool registry, 65+ built-in tools, tool execution with permission checks, BM25 search engine, lazy loading, sandbox execution
- `rc-permissions`: permission policies (5 modes), approval requests, tool classification, rule engine with wildcard matching, audit records
- `rc-mcp`: MCP client/server lifecycle, stdio/HTTP/WebSocket JSON-RPC transport, config discovery, tool projection
- `rc-skills`: `SKILL.md` discovery, TOML frontmatter parsing, indexing, lock file support
- `rc-plugins`: isolated plugin manifests, JSON-RPC process runtime, capability negotiation, bundled skills
- `rc-agents`: scheduler, mailbox, ownership, task lifecycle, tool budgets, team coordination, parallel execution
- `rc-tui`: `ratatui` screens, keyboard model (Vim mode), viewport state, and rendering
- `rc-runner`: runner protocol, HTTP API, workspace registration, heartbeat, session/approval management
- `rc-control-plane`: API models, runner registry, realtime fan-out (WebSocket), approvals, artifact routes, timeline events
- `rc-telemetry`: tracing setup, structured logging, JSON output, cost telemetry
- `rc-agent-protocol`: multi-agent protocol abstraction layer — `AgentAdapter` trait, `UnifiedAgentEvent` enum, `AgentRouter` for routing messages to different agent backends, `AgentType` enum
- `rc-query-engine`: unified query loop, state machine, streaming executor, token budget — execution path for Claude agent
- `rc-codex-adapter`: Codex in-process adapter — wraps `InProcessAppServerClient` with background event pump and `event_mapper` (754 lines, 50+ notification types)
- `rc-roo-adapter`: Roo in-process adapter — wraps Roo's `Provider` + `ToolDispatcher` with custom agent loop (12 provider backends)
- `rc-claude-adapter`: Claude in-process adapter — type alias for `QueryEngine`, re-exports all QueryEngine types

## Process Model

### Local Runtime

The `remote-code` (claudecode agent) process owns:

- CLI parsing
- session bootstrap
- provider connection setup
- local tool execution
- permission prompting
- TUI rendering
- headless protocol I/O
- interactive shell with Vim mode
- runtime hook execution
- MCP server discovery and invocation
- plugin discovery and runtime communication
- multi-agent task planning and parallel execution
- context window management and auto-compaction
- memory system (RC.md persistent memory, global/project scoped)
- cost tracking and telemetry

It does not directly embed plugin code from arbitrary JavaScript sources. External plugins are projected through child processes with a negotiated JSON-RPC protocol.

### Runner

The runner owns:

- workspace registration with the control plane
- launching local backend sessions
- streaming runtime events to the control plane
- forwarding approvals, messages, and shutdown requests
- periodic heartbeat with exponential backoff reconnect

### Control Plane

The control plane owns:

- authenticated REST API and WebSocket surfaces
- runner registration with lease-based health tracking
- session creation and viewer subscriptions
- approval workflows (create, list, show, respond)
- artifact metadata, upload (base64), and download
- timeline event fan-out over WebSocket

## Data Flow

### Local Session

1. `remote-code` resolves config from CLI, env, and profile files.
2. `rc-session` opens or creates a session and appends a bootstrap event.
3. `rc-provider` normalizes the configured backend protocol and builds a provider client.
4. `rc-tools`, `rc-mcp`, `rc-skills`, and `rc-plugins` register available capabilities.
5. `rc-permissions` decides whether a tool call is auto-allowed, denied, or needs approval.
6. `rc-protocol` emits typed events which are rendered either in the TUI, interactive shell, or serialized as `stream-json`.
7. `rc-session` persists all externally meaningful events as append-only NDJSON.

### Conversation Loop

The full conversation loop operates as follows:

1. User input → provider request (with context window management)
2. Provider response → parse tool calls
3. Tool calls → permission check → execute → collect results
4. Tool results → append to conversation → back to provider
5. Repeat until provider emits a text-only response (no tool calls)
6. Streaming callbacks fire at each stage for real-time UI updates

### Remote Session

1. A client asks the control plane to create or resume a session.
2. The control plane selects a runner that owns the requested workspace.
3. The runner launches a `remote-code` backend session with the correct profile and workspace mapping.
4. Runtime events flow from the backend to the runner, then to the control plane, then to subscribed clients.
5. Approval responses and follow-up prompts flow back through the same chain in reverse.

## Session Storage

### SQLite (`state.db`)

- session indexes
- profile metadata
- permission decisions that are safe to cache
- artifact metadata
- migration bookkeeping
- cost tracking records

### NDJSON Transcripts (`sessions/*.ndjson`)

Each session has a transcript file. The transcript is the source of truth for:

- user messages
- assistant messages
- tool requests and results
- permission prompts and decisions
- status transitions
- session context snapshots
- hook execution records
- remote control events that must survive restarts

## Protocol Boundaries

Internally, protocol data is strongly typed Rust.

Externally, the compatibility layer re-exposes:

- `system` messages (init, session state changes, status)
- `assistant` messages
- `result` messages (success/error, usage, duration)
- `control_request` and `control_cancel_request` (permission prompts, interrupts)
- `tool_progress`

The compatibility serializer is the only place where loosely structured legacy shapes are produced.

### Multi-Agent Protocol Boundary

Three independent in-process adapters, each tailored to its agent's native architecture:

- **Claude Code**: `QueryEngine` (via `rc-claude-adapter`) — full turn-based conversation loop with `GuiToolRunner`, `GuiQueryObserver`, `GuiRuntimePermissionBroker`, `ContextWindowManager`
- **Codex**: `CodexInProcessAdapter` (`rc-codex-adapter`) — wraps `InProcessAppServerClient` with background event pump and `event_mapper` (754 lines, 50+ notification types, 60+ RPC methods)
- **Roo Code**: `RooInProcessAdapter` (`rc-roo-adapter`) — wraps Roo's `Provider` + `ToolDispatcher` with custom agent loop (12 provider backends: Anthropic, OpenAI, DeepSeek, Google, Ollama, LMStudio, Mistral, Fireworks, BaseTen, LiteLLM, Moonshot, MiniMax)
- All adapters implement the `AgentAdapter` trait and emit `UnifiedAgentEvent` through `mpsc::Receiver`
- No external process spawning, no IPC overhead, no bridge binaries

## Provider Architecture

`rc-provider` standardizes provider access around a common request model:

- normalized base URL
- protocol family: `anthropic`, `openai`, `glm`, `bedrock`, `vertex`
- model identifier
- auth material (Bearer token + x-api-key)
- timeout policy
- header overrides
- retry and backoff policy (exponential with jitter, `Retry-After` support)

### Supported Providers

| Provider | Protocol | Streaming | Notes |
|----------|----------|-----------|-------|
| OpenAI | `openai` | ✅ SSE | GPT-4, GPT-4o, etc. |
| Anthropic | `anthropic` | ✅ SSE | Claude 3.5 Sonnet, Opus, etc. |
| GLM/ZhipuAI | `openai` | ✅ SSE | GLM-4, ChatGLM |
| AWS Bedrock | `anthropic` | ✅ SSE | Claude on AWS |
| Google Vertex AI | `anthropic` | ✅ SSE | Claude on GCP |

### Failover Architecture

Multi-provider failover with automatic health tracking:

- Health status tracking per provider endpoint (Healthy / Degraded / Unhealthy)
- Automatic round-robin fallback on failure
- Circuit-breaker logic with configurable thresholds
- Configurable retry with exponential backoff and jitter
- `Retry-After` header support

### Streaming Callbacks

The streaming subsystem provides real-time callbacks:

- `on_token` — per-token text delta
- `on_tool_call` — tool call parsed from stream
- `on_tool_result` — tool execution completed
- `on_usage` — token usage statistics
- `on_error` — error during streaming

### Anthropic API Cache Optimization

- Prompt caching with `cache_control` breakpoints
- Automatic cache breakpoint insertion for system prompts and large tool definitions
- Cache hit/miss tracking in cost telemetry

## Tool System Architecture

`rc-tools` defines typed capability interfaces with 65+ built-in tools.

### Tool Categories

| Category | Tools | Permission Class |
|----------|-------|-----------------|
| File Operations | `read_file`, `write_file`, `edit_file`, `replace_in_file`, `list_directory` | Read / Edit |
| Search | `search_text`, `glob`, `grep`, `lsp` | Read |
| Execution | `bash_command` | Command |
| Web | `web_search`, `web_fetch`, `web_browser` | Read / Command |
| Agent System | `agent`, `send_message`, `team_create`, `team_status` | System |
| Task Management | `task_create`, `task_get`, `task_list`, `task_stop`, `task_update`, `todo_write` | System |
| Memory | `memory_read`, `memory_write` | Read / Edit |
| Other | `ask_user`, `config_read`, `sleep`, `snip`, `skill_discover`, `tool_search`, `verify_plan`, `terminal_capture`, `notebook_edit`, `enter_plan_mode`, `exit_plan_mode` | Various |

### BM25 Tool Search Engine

Tools are indexed with a BM25 search engine for intelligent discovery:

- Tool name, description, and category are indexed
- Fuzzy matching supports partial names and synonyms
- `tool_search` tool exposes the search API to the provider
- Reduces context window pressure by only loading relevant tool descriptions

### Lazy Tool Loading

Tools are split into eager and lazy categories:

- **Eager tools** (core): always loaded into context (file ops, search, bash)
- **Lazy tools** (extended): loaded on demand via `tool_search` or explicit request
- Reduces token usage by ~60% for typical conversations
- Provider can discover lazy tools through the `tool_search` tool

### Sandbox Execution

`bash_command` supports cross-platform sandboxed execution:

- Working directory restriction
- Environment variable filtering
- Timeout enforcement
- Output size limits
- Command allowlist/denylist (configurable)

## Permission System Architecture

`rc-permissions` owns the decision logic and audit log. No other crate can silently bypass it.

### Permission Modes

| Mode | Read | Edit | Command | Notes |
|------|------|------|---------|-------|
| `default` | ✅ auto | ❌ ask | ❌ ask | Safe default |
| `acceptEdits` | ✅ auto | ✅ auto | ❌ ask | CI-friendly |
| `bypassPermissions` | ✅ auto | ✅ auto | ✅ auto | Full automation |
| `dontAsk` | ✅ auto | ❌ deny | ❌ deny | Read-only |
| `plan` | ✅ auto | ❌ deny | ❌ deny | Planning mode |

### Rule Engine

Fine-grained permission rules with wildcard matching:

- Path-based rules: `src/**/*.rs` → allow read
- Command-based rules: `cargo *` → allow execution
- Tool-specific rules: per-tool allow/deny patterns
- Priority ordering: specific rules override general patterns
- Audit trail for all permission decisions

## Context Management Architecture

`rc-provider` includes intelligent context window management:

### Token Estimation

- Automatic token counting per message using provider-specific tokenizers
- Running total tracking against model context window limit
- Warning thresholds at 80% and 95% capacity

### Auto-Compaction

When context approaches the window limit:

1. Summarize older conversation turns
2. Retain recent turns verbatim
3. Preserve all tool call/result pairs for active tasks
4. Emit compaction event to session transcript
5. Continue conversation with compressed context

### Context Strategy

- System prompt always retained (never compacted)
- Recent N turns kept verbatim (configurable)
- Tool results compacted to summaries
- User messages preserved with higher priority

## Cost Tracking Architecture

`rc-provider` tracks token usage and costs across all models:

- Per-request token counting (input, output, cache read, cache write)
- Per-model cost accumulation
- Session-level cost aggregation
- Provider-level cost breakdown
- Cost reporting via telemetry

## Memory System Architecture

`rc-session` implements RC.md persistent memory:

- `memory_read` — load memories from the memory store
- `memory_write` — persist observations and facts
- Memories scoped per project (workspace-relative)
- Automatic memory loading on session start
- Memory compaction when store grows large

## Multi-Agent System Architecture

### Internal Swarm (`rc-agents`)

`rc-agents` is the single owner of multi-agent state:

- agent identities with labels and ownership paths
- task scheduling with state machine (Pending → Assigned → Running → Completed/Failed)
- ownership and mailbox routing
- parallel task execution with capacity-aware scheduling
- shutdown and cleanup
- token, tool, and context budgets per task
- team lifecycle with lead agent and objective tracking
- lifecycle event recording
- inter-agent messaging via mailbox system

### Agent Tools

- `agent` — spawn a new agent for a subtask
- `send_message` — send a message to another agent's mailbox
- `team_create` — create a team of agents with a shared objective
- `team_status` — query the status of a team and its agents

### Multi-Agent Adapter Architecture

The GUI supports three independent AI agent backends, each with its own in-process adapter tailored to the agent's native architecture. See [plans/multi-agent-architecture.md](plans/multi-agent-architecture.md) for the full design.

**Three Independent Adapter Architecture:**

```mermaid
graph TB
    subgraph Frontend
        UI[React UI<br/>AgentSelector]
    end

    subgraph Tauri Backend — send_prompt routing
        CMD[Tauri Commands]
        ROUTING{agent_type match}
        
        subgraph crates/adapters/
            CA[rc-claude-adapter<br/>ClaudeInProcessAdapter<br/>= QueryEngine]
            CXA[rc-codex-adapter<br/>CodexInProcessAdapter<br/>AppServerClient + event_pump]
            RA[rc-roo-adapter<br/>RooInProcessAdapter<br/>Provider + ToolDispatcher]
        end
    end

    subgraph Agent Runtimes
        QE[QueryEngine<br/>GuiToolRunner + Observer]
        CX_RT[Codex AppServer<br/>60+ RPC methods]
        RO_RT[Roo Agent Loop<br/>12 Provider backends]
    end

    UI --> CMD
    CMD --> ROUTING
    ROUTING -->|remote_claude| CA
    ROUTING -->|remote_codex| CXA
    ROUTING -->|remote_roo| RA
    
    CA --> QE
    CXA --> CX_RT
    RA --> RO_RT
```

**Supported Agents:**

| Agent | Crate | Transport | Implementation |
|-------|-------|-----------|----------------|
| Claude Code | `rc-claude-adapter` | In-process QueryEngine | `QueryEngine` + `GuiToolRunner` + `GuiQueryObserver` + `GuiRuntimePermissionBroker` |
| OpenAI Codex | `rc-codex-adapter` | In-process AppServer | `CodexInProcessAdapter` + `InProcessAppServerClient` + `event_mapper` (754 lines) |
| Roo Code | `rc-roo-adapter` | In-process Provider | `RooInProcessAdapter` + `Provider` + `ToolDispatcher` + custom agent loop |

**Core Abstractions:**

- `AgentAdapter` trait — async interface: `start()`, `send_message()`, `cancel()`, `resolve_permission()`, `stop()`, `is_alive()`
- `AgentRouter` — routes sessions to the correct adapter based on `agent_type`
- `UnifiedAgentEvent` — normalized event model for all agent protocols
- `rc-agent-protocol` — shared trait, event definitions, types (no adapter implementations)
- `rc-claude-adapter` — Claude adapter: `ClaudeInProcessAdapter` = type alias for `QueryEngine`
- `rc-codex-adapter` — Codex adapter: `CodexInProcessAdapter` with `event_mapper` (AppServerEvent → UnifiedAgentEvent)
- `rc-roo-adapter` — Roo adapter: `RooInProcessAdapter` with `Provider` + `ToolDispatcher`

**Key Design Decisions:**

1. All agents run in-process — no subprocess spawning, no IPC overhead, no bridge binaries
2. Each agent has its own dedicated adapter crate under `crates/adapters/`
3. Adapters are architecturally symmetric but implementation differs per agent's native architecture
4. Sessions are bound to a single agent type at creation time
5. Permission requests from all agents are routed through the same GUI approval flow
6. Claude uses QueryEngine; Codex uses AppServer protocol; Roo uses Provider+ToolDispatcher

## Three-Agent In-Process Architecture

### 概览

三个 Agent 各自拥有独立的适配器 crate，全部进程内执行，无需子进程或桥接二进制：

| Agent | Adapter Crate | 适配器类型 | 运行时依赖 |
|-------|--------------|-----------|-----------|
| Claude Code | `crates/adapters/rc-claude-adapter` | `ClaudeInProcessAdapter` (= `QueryEngine`) | `rc-query-engine`, `rc-core`, `rc-provider`, `rc-tools`, `rc-session` |
| Codex | `crates/adapters/rc-codex-adapter` | `CodexInProcessAdapter` | `codex-app-server-client`, `codex-core`, `codex-protocol` |
| Roo Code | `crates/adapters/rc-roo-adapter` | `RooInProcessAdapter` | `roo-provider`, `roo-task`, `roo-tools`, `roo-types` |

### Adapter Integration Status

| Agent | Core Path | Tool Execution | Permissions | Context Mgmt | Streaming | MCP |
|-------|-----------|---------------|-------------|-------------|-----------|-----|
| Claude | ✅ QueryEngine | ✅ All native tools | ✅ Full GUI broker | ✅ Auto compaction | ✅ | ✅ |
| Codex | ✅ AppServer | ✅ AppServer tools | ✅ Mapped to GUI | ✅ AppServer managed | ✅ | ✅ |
| Roo | ✅ Provider+Dispatcher | ✅ ToolDispatcher | ⚠️ No-op stub | ⚠️ Rough estimate | ✅ | ❌ Not yet |

### CodexInProcessAdapter Architecture

`CodexInProcessAdapter` 直接包装 Codex 的 `InProcessAppServerClient`，通过后台事件泵实现实时流式传输：

```text
┌──────────────────────────────────────────────┐
│  CodexInProcessAdapter                       │
│  ┌──────────────┐  ┌───────────────────────┐ │
│  │ request_handle│  │ event_pump (bg task)  │ │
│  │ (Clone)       │  │ owns AppServerClient  │ │
│  │               │  │ loops next_event()    │ │
│  │ - request()   │  │ maps via event_mapper │ │
│  │ - resolve()   │  │ forwards to event_tx  │ │
│  │ - reject()    │  └───────────┬───────────┘ │
│  └──────┬───────┘              │             │
│         │          ┌───────────▼───────────┐ │
│         │          │ Arc<Mutex<Option<tx>>> │ │
│         │          │ (shared event router)  │ │
│         │          └───────────┬───────────┘ │
│  send_message() installs new rx│             │
│  cancel() sends TurnInterrupt  │             │
│  resolve_permission() resolves │             │
└──────────────────────────────────────────────┘
```

### RooInProcessAdapter Architecture

`RooInProcessAdapter` 包装 Roo 的 `Provider` + `ToolDispatcher`，在后台任务中运行自定义 agent loop：

```text
┌──────────────────────────────────────────────┐
│  RooInProcessAdapter                         │
│  ┌──────────────┐  ┌───────────────────────┐ │
│  │ build_handler │  │ run_agent_loop (task) │ │
│  │ 12 providers  │  │ Provider.create_msg   │ │
│  │               │  │ collect_stream+fwd    │ │
│  │               │  │ ToolDispatcher.dispatch│ │
│  └──────────────┘  └───────────┬───────────┘ │
│                                  │             │
│  send_message() spawns worker    │             │
│  cancel() via CancellationToken  │             │
└──────────────────────────────────────────────┘
```

### 构建系统

- `Makefile` — GNU Make 统一构建入口
- `scripts/build-agents.ps1` — PowerShell 构建脚本（Windows）
- `scripts/build-agents.sh` — Bash 构建脚本（Linux/macOS）

## QueryEngine Execution Path (Claude Agent)

`rc-query-engine` provides the execution path for the Claude agent, with full tool execution, permission brokering, and context management:

```mermaid
graph LR
    A[AgentAdapter.send_message] --> B[QueryEngine.run]
    B --> C[Provider Request]
    C --> D[Parse Response]
    D --> E{Has Tool Calls?}
    E -->|Yes| F[Permission Check]
    F --> G[Execute Tool]
    G --> H[Append Result]
    H --> C
    E -->|No| I[Emit Completed Event]
```

**Key properties:**

- Single state machine for all agent types
- Streaming event emission via `mpsc::Receiver<UnifiedAgentEvent>`
- Token budget tracking and context window management
- Observer pattern for checkpoint and recovery
- Shared tool execution loop with permission broker

## MCP, Skills, and Plugins

### MCP

MCP is a first-class transport and tool source. `rc-mcp` handles:

- stdio JSON-RPC clients with configurable timeouts
- HTTP transport for remote MCP servers
- WebSocket transport for persistent connections
- lifecycle management (initialize, tools/list, tools/call)
- capability projection into the runtime tool registry
- config discovery from `mcp.toml` files

### Skills

Skills remain file-based and human-editable. `rc-skills` handles:

- `SKILL.md` discovery with recursive directory walk
- TOML frontmatter parsing (`+++` delimited)
- heading and summary extraction
- trigger keyword extraction
- reference, script, and asset path discovery
- lock file support for installed skills
- `skill_discover` tool for runtime skill search

### Plugins

Plugins are isolated processes. `rc-plugins` handles:

- plugin manifest loading (`plugin.json`)
- capability negotiation
- stdio JSON-RPC runtime adapter
- crash isolation
- bundled skill discovery
- MCP config inheritance

## TUI Architecture

`rc-tui` is a client over the same typed session events used by headless mode.

Current UI responsibilities:

- rendering session timeline (recent sessions list)
- displaying current status and provider identity
- showing session metadata and profile info
- Vim mode key bindings (Normal/Insert mode)
- Slash command handling
- Real-time streaming output display

The TUI does not own business logic. It consumes services and event streams from the other crates.

## Dependency Direction

The intended dependency flow is inward and acyclic:

```
apps/* → rc-* crates
UI-facing crates → core crates (not the reverse)
remote crates → protocol, config, session, telemetry
compatibility code → internal typed models (not the reverse)
rc-agent-protocol → rc-core (shared types only)
```

Examples of allowed direction:

- `rc-tui → rc-core, rc-config, rc-session`
- `rc-control-plane → rc-runner, rc-config`
- `rc-provider → rc-core, rc-config, rc-tools`
- `rc-plugins → rc-mcp, rc-skills`
- `rc-agent-protocol → rc-core` (shared types, events)
- `rc-query-engine → rc-provider, rc-tools, rc-session` (unified execution)

Examples of disallowed direction:

- `rc-core → rc-tui`
- `rc-permissions → apps/remote-code`
- `rc-session → rc-control-plane`
- `rc-core → rc-agent-protocol`

## CI Expectations

CI enforces on every push and PR:

- workspace builds cleanly (Linux + Windows)
- `cargo fmt --all -- --check` passes
- `cargo clippy --workspace --all-targets -- -D warnings` passes
- `cargo test --workspace` passes
- platform-specific path and process tests do not regress

Release builds are triggered by tags and produce binaries for 5 platforms.

## Known Limitations

| Limitation | Description |
|------------|-------------|
| TTS Mock | `rc-voice::tts` returns placeholder responses, not connected to a real TTS service |
| Roo Permission Stub | `RooInProcessAdapter::resolve_permission()` is a no-op — Roo's tool approval flow is not yet wired to the GUI interactive permission dialog |
| Roo Token Estimation | Roo adapter uses `text.len() / 4` for approximate token counting instead of Roo's native tiktoken |
| Roo MCP Not Wired | Roo adapter declares `McpSupport` capability but does not integrate `McpServerConnection` in `send_message()` |
| Alpha Dependencies | `rama-*` crates pinned to `0.3.0-alpha.4` — pre-release quality, will need migration when stable releases |
