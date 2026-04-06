# Architecture

This document defines the target architecture for `remote-code-rust`.

It is intentionally stricter than the reference TypeScript workspace. The main design rule is that each subsystem must have one owner crate, one state model, and one boundary for integration.

## Top-Level Structure

The workspace is split into binaries under `apps/` and libraries under `crates/`.

### Applications

- `apps/remote-code`: CLI, headless runtime entrypoint, session commands, and TUI launcher
- `apps/remote-code-runner`: remote runner process that connects workspaces to the control plane
- `apps/remote-code-control-plane`: HTTP and WebSocket backend for sessions, approvals, artifacts, and runner coordination
- `apps/remote-code-migrate`: explicit migration and import tool

### Library Crates

- `rc-core`: shared runtime types, errors, service traits, and app bootstrap helpers
- `rc-config`: CLI parsing, env loading, config precedence, and profile resolution
- `rc-protocol`: typed runtime events plus compatibility serializers for `stream-json`
- `rc-provider`: provider normalization, request shaping, transport, retries, and streaming adapters
- `rc-session`: session persistence, indexes, exports, transcript appenders, and resume loading
- `rc-tools`: typed tool registry and tool execution services
- `rc-permissions`: permission policies, approval requests, blocked-path logic, and audit records
- `rc-mcp`: MCP client/server lifecycle and tool projection
- `rc-skills`: `SKILL.md` discovery, parsing, indexing, and invocation metadata
- `rc-plugins`: isolated plugin manifests, adapter processes, and capability negotiation
- `rc-agents`: scheduler, mailbox, ownership, task lifecycle, and team coordination
- `rc-tui`: `ratatui` screens, keyboard model, viewport state, and rendering
- `rc-runner`: runner protocol, workspace registration, backend process control, and reconnect behavior
- `rc-control-plane`: API models, runner registry, realtime fan-out, approvals, and artifact routes
- `rc-telemetry`: tracing setup, metrics, budgets, audit sinks, and structured event exports

## Process Model

The rewrite uses explicit process boundaries.

### Local Runtime

The `remote-code` process owns:

- CLI parsing
- session bootstrap
- provider connection setup
- local tool execution
- permission prompting
- TUI rendering
- headless protocol I/O

It does not directly embed plugin code from arbitrary JavaScript sources. External plugins are projected through child processes with a negotiated protocol.

### Runner

The runner owns:

- workspace registration
- launching local backend sessions
- streaming runtime events to the control plane
- forwarding approvals, messages, and shutdown requests

The runner is not the source of truth for session persistence. It is a transport and execution coordinator.

### Control Plane

The control plane owns:

- authenticated API and WebSocket surfaces
- runner registration and health
- session creation and viewer subscriptions
- approval workflows
- artifact metadata and download authorization

The control plane is not allowed to execute coding tools itself.

## Data Flow

### Local Session

1. `remote-code` resolves config from CLI, env, and profile files.
2. `rc-session` opens or creates a session and appends a bootstrap event.
3. `rc-provider` normalizes the configured backend protocol and builds a provider client.
4. `rc-tools`, `rc-mcp`, `rc-skills`, and `rc-plugins` register available capabilities.
5. `rc-permissions` decides whether a tool call is auto-allowed, denied, or needs approval.
6. `rc-protocol` emits typed events which are rendered either in the TUI or serialized as `stream-json`.
7. `rc-session` persists all externally meaningful events as append-only NDJSON.

### Remote Session

1. A client asks the control plane to create or resume a session.
2. The control plane selects a runner that owns the requested workspace.
3. The runner launches a `remote-code` backend session with the correct profile and workspace mapping.
4. Runtime events flow from the backend to the runner, then to the control plane, then to subscribed clients.
5. Approval responses and follow-up prompts flow back through the same chain in reverse.

## Session Storage

Persistent state is split between SQLite metadata and append-only files.

### SQLite

`state.db` stores:

- session indexes
- profile metadata
- runner and workspace registration state
- permission decisions that are safe to cache
- artifact metadata
- migration bookkeeping

### NDJSON Transcripts

Each session has a transcript file under `sessions/`.

The transcript is the source of truth for:

- user messages
- assistant messages
- tool requests and results
- permission prompts and decisions
- status transitions
- remote control events that must survive restarts

This avoids overloading SQLite with large transcript blobs and makes export/replay simpler.

## Protocol Boundaries

Internally, protocol data is strongly typed Rust.

Externally, the compatibility layer re-exposes:

- `system` messages such as init and session state changes
- `assistant` messages
- `result` messages
- `control_request` and `control_cancel_request`
- `tool_progress`

The compatibility serializer is the only place where loosely structured legacy shapes are produced.

## Provider Architecture

`rc-provider` standardizes provider access around a common request model:

- normalized base URL
- protocol family: `anthropic` or `openai`
- model identifier
- auth material
- timeout policy
- header overrides
- retry and backoff policy

Concrete transports are layered behind the same trait so that:

- Anthropic-compatible and OpenAI-compatible backends share the same upper runtime
- GLM, MiniMax, ZAI, and custom gateways can be added as configuration, not architecture forks
- mock and regression providers can be used in tests without special runtime code

## Tool and Permission Architecture

`rc-tools` defines typed capability interfaces instead of stringly typed ad hoc handlers.

Initial core tool families:

- filesystem
- search
- edit
- process execution
- Git inspection
- HTTP fetch
- environment and time

Each tool declares:

- capability class
- mutability
- path scope rules
- timeout class
- whether approval is required

`rc-permissions` owns the decision logic and audit log. No other crate can silently bypass it.

## MCP, Skills, and Plugins

These three surfaces are treated differently on purpose.

### MCP

MCP is a first-class transport and tool source. `rc-mcp` handles:

- stdio clients
- WebSocket clients
- lifecycle and reconnects
- capability projection into the runtime tool registry

### Skills

Skills remain file-based and human-editable. `rc-skills` handles:

- `SKILL.md` discovery
- frontmatter or manifest parsing
- indexing and lookup
- invocation metadata

### Plugins

Plugins are isolated processes. `rc-plugins` handles:

- plugin manifest loading
- capability negotiation
- stdio JSON-RPC transport
- crash isolation
- adapter bridges for legacy plugin ecosystems

Plugins do not run as in-process arbitrary scripts.

## Multi-Agent Architecture

`rc-agents` is the single owner of multi-agent state.

It manages:

- agent identities
- task scheduling
- ownership and mailbox routing
- shutdown and cleanup
- token, tool, and context budgets
- team lifecycle

This prevents the state duplication that happens when CLI, remote control, runner, and UI layers each invent their own coordination logic.

## TUI Architecture

`rc-tui` is a client over the same typed session events used by headless mode.

Initial UI responsibilities:

- rendering session timeline
- displaying current status and provider identity
- showing pending approvals
- showing session metadata and profile info
- launching command actions for export, resume, and doctor flows

The TUI does not own business logic. It consumes services and event streams from the other crates.

## Dependency Direction

The intended dependency flow is inward and acyclic:

- `apps/*` depend on `rc-*` crates
- UI-facing crates depend on core crates, not the reverse
- remote crates depend on protocol, config, session, and telemetry crates
- compatibility code depends on internal typed models, not the reverse

Examples of allowed direction:

- `rc-tui -> rc-core, rc-protocol, rc-session`
- `rc-control-plane -> rc-runner, rc-protocol, rc-session`
- `rc-provider -> rc-core, rc-config`

Examples of disallowed direction:

- `rc-core -> rc-tui`
- `rc-permissions -> apps/remote-code`
- `rc-session -> rc-control-plane`

## CI Expectations

Windows and Linux CI must validate the same architectural promises:

- workspace builds cleanly
- tests pass
- clippy is warning-clean
- fixture compatibility tests pass
- platform-specific path and process tests do not regress

CI enforces the baseline. It is not optional polish.
