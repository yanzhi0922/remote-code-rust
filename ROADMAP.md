# Roadmap

This roadmap converts the approved rewrite strategy into implementation phases with concrete exit criteria.

## Phase 0: Foundation — ✅ COMPLETE

Objective: establish the repository baseline without pretending the runtime is already complete.

Deliverables:

- ✅ Cargo workspace with `apps/` and `crates/`
- ✅ initial application and library crate skeletons
- ✅ Windows and Linux CI
- ✅ `ARCHITECTURE.md`, `COMPATIBILITY.md`, `PROVENANCE.md`, and this roadmap
- ✅ fixture collection scripts and initial committed fixtures
- ✅ baseline smoke tests for CLI bootstrap and protocol serialization

Exit criteria:

- ✅ workspace builds on Windows and Linux
- ✅ lint and test commands exist and run in CI
- ✅ fixture strategy is documented and executable
- ✅ dependency direction is established and reviewable

## Phase 1: Replaceable Core Runtime — ✅ COMPLETE

Objective: produce a usable Rust runtime that can cover the main single-machine workflows of the current shell.

Deliverables:

- ✅ `remote-code` CLI with `doctor`, `sessions`, `resume`, and `export`
- ✅ headless `stream-json` mode via `-p --input-format stream-json --output-format stream-json`
- ✅ provider adapters for Anthropic-compatible and OpenAI-compatible APIs
- ✅ session persistence under `~/.remote-code-rust/`
- ✅ baseline permission engine (5 modes, 3 permission classes)
- ✅ core local tool families (7 built-in tools)
- ✅ initial TUI (session list and status dashboard)
- ✅ `remote-code migrate import`
- ✅ interactive shell with session management commands
- ✅ runtime hook system (SessionStart, PreToolUse, PostToolUse, PostToolUseFailure)
- ✅ provider streaming (SSE for both OpenAI and Anthropic)
- ✅ provider failover with multi-provider rotation

Exit criteria:

- ✅ a local user can start a session, persist it, resume it, and export it
- ✅ provider config works with both supported protocol families
- ✅ fixture tests verify the core `stream-json` compatibility layer
- ✅ Windows and Linux behavior is covered by automated tests for paths, shelling out, and permissions

## Phase 2: Advanced Local Runtime — ✅ COMPLETE

Objective: exceed the current lightweight runtime and close the gap with the richer backend behavior.

Deliverables:

- ✅ MCP client support over stdio (JSON-RPC)
- ✅ MCP client support over HTTP and WebSocket
- ✅ skill discovery and indexing (SKILL.md with TOML frontmatter)
- ✅ isolated plugin process model with JSON-RPC runtime adapters
- ✅ multi-agent scheduler and mailbox model
- ✅ hooks and context compaction
- ✅ 38+ built-in tools (file ops, search, web, agent, tasks, memory, etc.)
- ✅ BM25 tool search engine for intelligent tool discovery
- ✅ lazy tool loading (eager/lazy split for context optimization)
- ✅ cross-platform sandbox execution for bash commands
- ✅ context window management with auto-compaction
- ✅ Anthropic API cache optimization
- ✅ cost tracking per model
- ✅ memory system (RC.md persistent memory, global/project scoped)
- ✅ streaming tool execution with callbacks
- ✅ interactive TUI with Vim mode and slash commands
- ✅ full conversation loop (provider → tool → provider)
- ✅ SSH mode for remote host execution

Exit criteria:

- ✅ local advanced workflows no longer depend on the old TypeScript runtime
- ✅ multi-agent flows survive restarts and cleanup correctly
- ✅ plugin crashes are isolated from the main runtime
- ✅ approval and tool-budget behavior remain auditable

## Phase 3: Rust Remote Platform — ✅ COMPLETE

Objective: replace the TypeScript runner and control-plane backend while keeping current client surfaces viable.

Deliverables:

- ✅ `remote-code-runner` with HTTP API, control-plane registration, heartbeat sync
- ✅ `remote-code-control-plane` with REST API and WebSocket event streams
- ✅ versioned `/v1` HTTP API (runners, sessions, approvals, artifacts, events)
- ✅ versioned WebSocket event streams (timeline fan-out)
- ✅ runner registration with lease-based health tracking
- ✅ approval relay (create, list, show, respond)
- ✅ artifact indexing and downloads (base64 upload, binary download)
- ✅ session timeline fan-out
- ✅ remote CLI management commands (`remote-code remote *`)

Exit criteria:

- ✅ existing web and mobile clients can operate against the Rust backend with limited or no UI rewrites
- ✅ remote session creation, approval, reconnect, and export paths work end to end
- ✅ runner failures and reconnects are observable and recoverable

## Phase 4: Beyond Parity — ✅ COMPLETE (348 tests)

Objective: deliver improvements that are difficult or unsafe in the current architecture.

Deliverables:

- ✅ deterministic replay (session replay with verification)
- ✅ provider failover and routing policy
- ✅ fine-grained permission rules with wildcard matching
- ✅ cost tracking and telemetry per model
- ✅ memory system with persistent RC.md storage (global/project scoped)
- ✅ multi-agent parallel execution
- ✅ BM25 tool search engine
- ✅ lazy tool loading
- ✅ sandbox execution
- ✅ context auto-compaction
- ✅ Anthropic API cache optimization
- ✅ streaming callbacks for tool execution
- ✅ 348 tests passing, clippy clean with -D warnings

Exit criteria:

- ✅ the Rust runtime is not just compatible, but materially more robust than the reference implementation
- ✅ remote and local flows share one coherent typed event model
- ✅ all core modules have test coverage

## Phase 5: Competitive Parity Enhancement — ✅ COMPLETE

Objective: close all gaps identified by competitive research against 15 reference implementations.

Deliverables:

- ✅ P0: 7 blocking fixes — streaming SSE parser, incremental rendering, model info DB (100+ models), 7 compaction strategies, error classification with retry, first-run wizard, doctor diagnostics
- ✅ P1: 9 important improvements — BM25 tool search, lazy tool loading, Anthropic cache optimization, cost tracking, memory system, multi-agent scheduler, sandbox execution, context auto-compact, streaming callbacks
- ✅ P2: 9 enhancement features — workflow CRUD tool, cron scheduler CRUD, daemon process manager (spawn/stop/status/logs), SSH enhanced mode (config/forward/timeout), REPL tool, PowerShell tool, monitor tool, remote trigger, PR suggester
- ✅ P3: 6 polish features — Tab auto-completion (tools + files), Ctrl+R history search, theme system (4 presets), voice input (sox/ffmpeg + whisper), cross-compilation CI (8 targets), SHA256 artifact signing
- ✅ Clippy zero-warning across entire workspace (-D warnings)
- ✅ 348 tests passing on Windows, macOS, Linux

Exit criteria:

- ✅ all P0–P3 items from competitive research report are implemented
- ✅ clippy -- -D warnings passes cleanly
- ✅ all 348 tests pass on Windows
- ✅ CI covers 8 cross-compilation targets with SHA256 checksums

## Ongoing Tracks

### Compatibility Track

- keep committed fixtures current
- add regression coverage for every externally meaningful behavior change
- avoid accidental CLI or protocol breakage

### Migration Track

- improve import tooling from `~/.remote-code/`
- document one-way and reversible migration boundaries
- keep exported session formats stable

### Quality Track

- keep clippy warning-free
- avoid cyclic crate dependencies
- constrain third-party dependencies
- maintain Windows and Linux parity
- maintain test coverage for all core modules

### Performance Track

- benchmark token estimation accuracy
- profile context compaction overhead
- measure tool search latency
- track memory usage across long sessions

## Phase 6: Desktop GUI — 🔄 IN PROGRESS

Objective: provide a native desktop GUI for users who prefer graphical interfaces over the CLI/TUI.

Deliverables:

- ✅ Tauri v2 + React 19 + TypeScript + Vite + Tailwind CSS scaffold
- ✅ Multi-project sidebar with folder picker and collapsible sessions
- ✅ Chat interface with Markdown rendering (KaTeX math, code highlighting, GFM)
- ✅ Collapsible tool calls, thinking blocks, and subtask expansion
- ✅ Multi-Provider management (add/edit/delete/switch providers)
- ✅ Settings panel with Provider, Model, Permissions, and Advanced tabs
- ✅ Permission modal for tool execution approval
- ✅ Quick selectors for Provider/model/permission mode below chat input
- ✅ Project path normalization (Windows UNC prefix handling)
- ✅ Session persistence and conversation history
- ✅ CI frontend job (tsc + vite build)
- ✅ GUI CLI feature parity (doctor, export, MCP management, runtime status)
- ✅ Frontend test infrastructure (vitest + React Testing Library baseline)
- ⬜ Headless browser integration for web_browser screenshot tool

Exit criteria:

- GUI builds and runs on Windows, macOS, and Linux
- All core workflows (chat, settings, project management) work without CLI
- CI covers both Rust and frontend code paths
- Frontend has baseline test coverage

## Future Considerations

Potential areas for future development:

- richer telemetry and audit snapshots
- stronger plugin sandboxing (seccomp, namespaces)
- more capable Windows-native process and terminal behavior
- macOS parity work
- performance and memory budgets enforced by CI
- background sessions with persistent state
- remote bridge entrypoints for hybrid workflows

## Definition of Done

A phase is complete only when:

- code exists
- tests exist
- docs describe the intended behavior
- CI enforces the behavior

A checklist without enforceable validation is not considered done in this repository.
