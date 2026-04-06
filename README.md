# remote-code-rust

`remote-code-rust` is a clean-room Rust rewrite of the `C:\Users\Yanzh\Desktop\remote-code` workspace.

The goal is not a line-by-line port. The goal is to replace the current Bun/TypeScript runtime family with a Rust-native system that keeps the important external contracts, improves Windows and Linux behavior, and gives the project a maintainable long-term architecture.

## Scope

The rewrite covers these executable surfaces:

- `remote-code`: primary CLI, headless runtime, and TUI
- `remote-code-runner`: remote execution runner
- `remote-code-control-plane`: Rust control-plane backend for web and mobile clients
- `remote-code-migrate`: migration and import tooling

The first implementation focus is the local runtime family:

- headless `stream-json` compatibility
- provider adapters for Anthropic-compatible and OpenAI-compatible APIs
- state directories and session persistence
- permissions and tool execution
- baseline TUI

Later phases replace the remote runner and control-plane backend while keeping existing web, PWA, and mobile UI surfaces usable during the transition.

## Product Principles

- Clean-room implementation. The reference workspace is read-only input, not code to transplant.
- Compatibility where it matters. External CLI and protocol compatibility is preserved when it protects integrations and migration cost.
- Rust-native internals. State, concurrency, protocol handling, tools, and remote coordination are rebuilt around typed Rust boundaries instead of legacy dynamic flows.
- Windows and Linux first. Every core path must work natively on both platforms.
- Process isolation by default. Providers, tools, plugins, and remote actors must be isolated and auditable.

## Planned Workspace Layout

```text
apps/
  remote-code/
  remote-code-runner/
  remote-code-control-plane/
  remote-code-migrate/

crates/
  rc-core/
  rc-config/
  rc-protocol/
  rc-provider/
  rc-session/
  rc-tools/
  rc-permissions/
  rc-mcp/
  rc-skills/
  rc-plugins/
  rc-agents/
  rc-tui/
  rc-runner/
  rc-control-plane/
  rc-telemetry/

.github/
  workflows/

fixtures/
  reference/
```

The `apps/` crates expose user-facing binaries. The `crates/` directory holds reusable runtime components with narrow ownership and clear dependencies.

## Key Differences From the Reference Workspace

The current `remote-code` workspace mixes multiple product surfaces and compatibility layers:

- a lightweight headless runtime shell
- a Bun-backed full backend
- remote control and bridge entrypoints
- a separate `remote-hub` control plane

`remote-code-rust` separates those concerns into stable crates and explicit process boundaries:

- protocol and compatibility live in `rc-protocol`
- provider normalization and transport live in `rc-provider`
- sessions and persistent state live in `rc-session`
- permissions are centralized in `rc-permissions`
- tools live behind typed capability interfaces in `rc-tools`
- multi-agent lifecycle lives in `rc-agents`
- remote APIs and runner coordination live in `rc-control-plane` and `rc-runner`

## Compatibility Targets

The rewrite preserves the important contracts of the current product:

- `remote-code` remains the primary command name
- headless mode keeps `-p --input-format stream-json --output-format stream-json`
- session management keeps `doctor`, `sessions`, `resume`, and `export`
- environment setup continues to honor `REMOTE_CODE_PROVIDER`, `REMOTE_CODE_BASE_URL`, `REMOTE_CODE_API_KEY`, and `REMOTE_CODE_MODEL`
- old data under `~/.remote-code/` remains importable into `~/.remote-code-rust/`

Internal architecture is intentionally not compatibility-constrained. The compatibility layer exists to protect users and integrations, not to freeze the implementation model.

## State Model

The default profile root is `~/.remote-code-rust/`.

Planned top-level contents:

- `state.db`: SQLite metadata and indexes
- `sessions/`: append-only NDJSON session transcripts
- `artifacts/`: generated outputs and exports
- `logs/`: runtime and audit logs
- `profiles/`: named runtime profiles
- `skills/`: discovered or imported skill definitions
- `plugins/`: isolated plugin manifests and adapters

The database is for indexing and coordination. Transcript bodies stay on disk as append-only event logs.

## Tooling and CI

The workspace is expected to standardize on:

- Rust stable toolchain
- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- GitHub Actions for Windows and Linux

Reference-fixture collection is separate from CI. CI validates the Rust implementation against committed fixtures instead of depending on the old repository at runtime.

## Reference Inputs

Primary reference inputs for this repository:

- the local read-only workspace at `C:\Users\Yanzh\Desktop\remote-code`
- official Claude Code documentation for permissions, MCP, hooks, skills, and remote-control behavior
- public, provenance-clear architecture references such as `claw-code` and `opencode`

See `PROVENANCE.md` for the exact policy.

## Current Status

This repository starts with Phase 0 deliverables:

- workspace architecture and governance docs
- compatibility and provenance rules
- CI expectations for Windows and Linux
- fixture strategy for protocol and session regression coverage

Functional runtime crates are added incrementally on top of that baseline.
