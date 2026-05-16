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
- ✅ 62 built-in tools (file ops, search, web, agent, tasks, memory, etc.)
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

## Phase 4: Beyond Parity — ✅ COMPLETE

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
- ✅ P2: 9 enhancement features — workflow CRUD tool, cron scheduler CRUD, daemon process manager, SSH enhanced mode, REPL tool, PowerShell tool, monitor tool, remote trigger, PR suggester
- ✅ P3: 6 polish features — Tab auto-completion, Ctrl+R history search, theme system, voice input, cross-compilation CI, SHA256 signing
- ✅ Clippy zero-warning across entire workspace (-D warnings)
- ✅ 348 tests passing on Windows, macOS, Linux

## Phase 6: Desktop GUI — ✅ COMPLETE

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

## Phase 7-8: Multi-Agent Architecture — ✅ COMPLETE

Objective: support multiple AI agent backends (Remote Code, Roo Code, OpenAI Codex) in the GUI.

Deliverables:

- ✅ `rc-agent-protocol` crate — `AgentAdapter` trait, `UnifiedAgentEvent`, `AgentRouter`
- ✅ Initial adapter implementations (subprocess JSON-RPC for Roo/Codex)
- ✅ GUI frontend Agent selector component
- ✅ Event translation layer (UnifiedAgentEvent → Tauri events)

## Phase 9-10: Full Audit + 38 Fixes — ✅ COMPLETE

Objective: comprehensive codebase audit and fix all identified issues before production.

Deliverables:

- ✅ Full codebase scan: todo!, unimplemented!, hardcoded secrets, unsafe, TypeScript any, unwrap, expect
- ✅ 38 issues fixed (P1 functional defects + P2 code quality + P3 docs/tests)
- ✅ Clippy warnings reduced from 15 to 0
- ✅ Production `console.log` removed
- ✅ Dead code annotations reviewed

## Phase 11: InProcessAdapter Conversion — ✅ COMPLETE

Objective: convert RooCode/Codex from subprocess JSON-RPC to in-process callback model.

Deliverables:

- ✅ `InProcessAdapter` unified implementation in `crates/claude/rc-agent-protocol/src/adapters/in_process.rs`
- ✅ Type aliases: `RemoteClaudeAdapter`, `RemoteRooAdapter`, `RemoteCodexAdapter`
- ✅ Builder-pattern callback injection (`with_send_message`, `with_cancel`, `with_resolve_permission`)
- ✅ Subprocess management code removed
- ✅ 860+ tests passing (historical; current: 14,000+)

## Phase 12: E2E Real Testing — ✅ COMPLETE

Objective: validate with real API calls, not just mocks.

Deliverables:

- ✅ MiniMax Provider (anthropic-compatible) real API call testing
- ✅ MCP end-to-end integration testing
- ✅ Headless `--print` and `stream-json` smoke tests passing

## Phase 13: QueryEngine Unified Path — ✅ COMPLETE

Objective: unify the dual execution paths into a single QueryEngine path shared by all agents.

Deliverables:

- ✅ Single `QueryEngine` execution path for all three agent types
- ✅ Eliminated dual-path divergence (`run_gui_prompt` vs `run_agent_prompt`)
- ✅ Unified state machine + streaming executor + token budget
- ✅ Observer pattern for checkpoint and recovery

## Phase 14: Code Cleanup — ✅ COMPLETE

Objective: remove dead code and simplify architecture after unification.

Deliverables:

- ✅ Deleted 880 lines of old code (AgentRouter, health checker, restart tracker)
- ✅ AppState simplified
- ✅ `InProcessAdapter` extracted as independent module
- ✅ Code pushed (commit `05b05ec`)

## Phase 15: Three-Agent Independent Adapter Architecture — ✅ COMPLETE

Objective: split each agent into its own dedicated adapter crate.

Deliverables:

- ✅ Deleted bridge residue (subprocess.rs 730 lines + bridge_proto.rs)
- ✅ `rc-codex-adapter` moved from `crates/claude/` to `crates/adapters/`
- ✅ `rc-roo-adapter` moved from `crates/claude/` to `crates/adapters/`
- ✅ New `rc-claude-adapter` (ClaudeInProcessAdapter = QueryEngine)
- ✅ Roo adapter upgraded to native `AgentLoop` with 26 provider backends
- ✅ All three adapters compile and pass tests
- ✅ 860+ tests passing (historical; current: 14,000+)

## Phase 16: GUI Redesign — ✅ COMPLETE

Objective: modernize the desktop GUI with professional IDE-like layout.

Deliverables:

- ✅ Phase 1: Design System Foundation — CSS tokens, Tailwind config, ThemeProvider
- ✅ Phase 2: Layout Overhaul — ActivityBar, SplitPane, StatusBar, tab sidebar
- ✅ Phase 3: Chat Experience — streaming animation, inline diff, slash commands, message actions
- ✅ Phase 4: Integrated Tool Panes — Terminal (xterm.js), Diff, Preview, PaneHost
- ✅ Phase 5: Command Palette — keyboard shortcuts overlay

## Phase 17: ZCode-Inspired Features — ✅ COMPLETE

Objective: implement features inspired by ZCode (Z.AI) analysis — conversation-level version control, specialized agents, built-in Git panel, and permission mode switching.

Deliverables:

- ✅ `claude-checkpoint` crate — snapshot scanner (SHA256 hashes), SQLite storage (3 tables), unified diff via `similar` crate, restore engine (undo/rollback/preview), workspace exclusion patterns
- ✅ `claude-specialized-agents` crate — Markdown+YAML frontmatter agent definitions, 3-layer discovery (built-in/user/project), `@agent-name` mention parsing, 5 built-in agents (code-reviewer, bug-analyzer, dev-planner, architect, test-writer)
- ✅ `claude-git` crate — `gix` for branch resolution, CLI-based status/staging/commit/diff/log/branch switching, structured types for `GitStatus`/`GitDiff`/`CommitInfo`
- 🧹 Deprecated GUI prototypes removed — `PermissionModeSwitch`, `GitPanel`, `CheckpointTimeline`, and `AgentPicker` were not wired into the production app and were removed during dead-code cleanup.
- ✅ All three new crates compile and pass unit tests

## Phase 18: Roo Agent Deepening — 📋 PLANNED

Objective: deepen Roo adapter integration with full native capabilities.

Deliverables:

- [ ] Roo 权限系统 — 将 `resolve_permission()` 完全接入 GUI 交互式权限弹窗
- [ ] Roo Token 精确计算 — 使用 Roo 原生 tiktoken 替代粗略估算
- [ ] Roo MCP 集成 — 在 `send_message()` 中集成 `McpServerConnection`
- [ ] 端到端多 Agent 集成测试

## Phase 18.5: Provider API Contract Hardening — ✅ COMPLETE

Objective: fix critical API contract violations causing 400 errors in production.

Deliverables:

- ✅ Message normalization pipeline (`claude-provider/src/normalize.rs`) — 6-pass pipeline ensuring Anthropic API contract: role alternation, tool_use/tool_result pairing, orphaned thinking removal, trailing thinking strip, whitespace-only filter, non-empty content guarantee (11 unit tests)
- ✅ Stream idle watchdog — 90s configurable timeout (`CLAUDE_STREAM_IDLE_TIMEOUT_MS`) for all SSE streaming paths (OpenAI, Anthropic, Bedrock, Vertex), triggers fallback/retry on timeout (4 unit tests)
- ✅ Thinking budget clamping — `min(budget, maxTokens - 1)` prevents API errors when extended thinking budget exceeds output token space (4 unit tests)
- ✅ Model family references updated — Opus 4.6 → 4.7, Haiku model ID updated, knowledge cutoff added
- ✅ Grep tool schema expanded — from 4 fields to 14+ (glob, context lines, case-insensitive, type filtering, multiline, pagination, offset) matching TS reference
- ✅ TodoWrite schema enhanced — added `activeForm` field for spinner text during `in_progress` status
- ✅ Compact engine improvements — expanded auto strategy, attachment handling, snip logic, session memory extraction
- ✅ Roo provider improvements — MCP hub expansion, OpenAI-compatible streaming, tool dispatcher enhancements, file search helpers
- ✅ Full workspace compilation — 154 crates, 0 errors
- ✅ E2E real testing — MiniMax provider (anthropic-compatible) tested successfully with simple prompt + tool use

Exit criteria:

- ✅ `cargo check --workspace` passes with 0 errors
- ✅ 14,000+ tests passing (19 new: 11 normalize + 4 watchdog + 4 thinking budget)
- ✅ Real API calls to MiniMax provider succeed (simple prompt + file read tool use)

## Phase 19: Enhanced Remote Interaction + Tauri v2 Mobile — 📋 PLANNED

Objective: improve remote user experience with real-time features and ship the Tauri v2 mobile app.

Deliverables:

- [x] Terminal Stream — local xterm.js terminal with GitHub Dark theme, auto-fit, ResizeObserver (TerminalPane)
- [x] File Preview — multi-format preview: Markdown, HTML (sandboxed iframe), code, images (PreviewPane)
- [x] Diff Viewer — unified + side-by-side modes, inline diff algorithm, collapse unchanged regions (DiffPane)
- [ ] Remote Terminal Stream — real-time terminal output from remote sessions
- [ ] Remote File Preview — remote file content browsing
- [ ] Tauri v2 Mobile Init — `tauri android init` / `tauri ios init` to generate native projects
- [ ] Mobile UI Adaptation — responsive layout tuning for touch/small screens, mobile-specific gestures
- [ ] Push Notifications — mobile approval reminders via Tauri notification plugin (mobile.rs backend ready)
- [ ] Mobile Deep Link — `remotecode://` URL scheme for pairing and session links (mobile.rs backend ready)

## Phase 20: Competitive Advantage — 📋 PLANNED

Objective: exceed all competitors with unique capabilities.

Deliverables:

- [ ] Deep subtask delegation — multi-level sub-agents + parallel execution
- [ ] Session rollback — revert to any historical point (powered by `claude-checkpoint`)
- [ ] Shadow Git checkpoints — automatic git checkpoint (powered by `claude-git`)
- [ ] Task Flow visualization — task dependency graph + progress tracking
- [ ] TTS real implementation — connect to speech synthesis service

## Future Considerations

Potential areas for future development:

- richer telemetry and audit snapshots
- stronger plugin sandboxing (seccomp, namespaces)
- more capable Windows-native process and terminal behavior
- macOS parity work
- performance and memory budgets enforced by CI
- background sessions with persistent state
- remote bridge entrypoints for hybrid workflows
- cloud Runner — execute code on Tencent Cloud
- multi-workstation scheduling
- team collaboration — multi-user shared sessions

## Definition of Done

A phase is complete only when:

- code exists
- tests exist
- docs describe the intended behavior
- CI enforces the behavior

A checklist without enforceable validation is not considered done in this repository.
