# Roadmap

This roadmap converts the approved rewrite strategy into implementation phases with concrete exit criteria.

## Phase 0: Foundation

Objective: establish the repository baseline without pretending the runtime is already complete.

Deliverables:

- Cargo workspace with `apps/` and `crates/`
- initial application and library crate skeletons
- Windows and Linux CI
- `ARCHITECTURE.md`, `COMPATIBILITY.md`, `PROVENANCE.md`, and this roadmap
- fixture collection scripts and initial committed fixtures
- baseline smoke tests for CLI bootstrap and protocol serialization

Exit criteria:

- workspace builds on Windows and Linux
- lint and test commands exist and run in CI
- fixture strategy is documented and executable
- dependency direction is established and reviewable

## Phase 1: Replaceable Core Runtime

Objective: produce a usable Rust runtime that can cover the main single-machine workflows of the current shell.

Deliverables:

- `remote-code` CLI with `doctor`, `sessions`, `resume`, and `export`
- headless `stream-json` mode via `-p --input-format stream-json --output-format stream-json`
- provider adapters for Anthropic-compatible and OpenAI-compatible APIs
- session persistence under `~/.remote-code-rust/`
- baseline permission engine
- core local tool families
- initial TUI
- `remote-code migrate import`

Exit criteria:

- a local user can start a session, persist it, resume it, and export it
- provider config works with both supported protocol families
- fixture tests verify the core `stream-json` compatibility layer
- Windows and Linux behavior is covered by automated tests for paths, shelling out, and permissions

## Phase 2: Advanced Local Runtime

Objective: exceed the current lightweight runtime and close the gap with the richer backend behavior.

Deliverables:

- MCP client support over stdio and WebSocket
- skill discovery and indexing
- isolated plugin process model plus legacy adapter bridge
- multi-agent scheduler and mailbox model
- background sessions
- hooks and context compaction
- remote bridge entrypoints

Exit criteria:

- local advanced workflows no longer depend on the old TypeScript runtime
- multi-agent flows survive restarts and cleanup correctly
- plugin crashes are isolated from the main runtime
- approval and tool-budget behavior remain auditable

## Phase 3: Rust Remote Platform

Objective: replace the TypeScript runner and control-plane backend while keeping current client surfaces viable.

Deliverables:

- `remote-code-runner`
- `remote-code-control-plane`
- versioned `/v1` HTTP API
- versioned WebSocket event streams
- runner registration and workspace ownership
- approval relay
- artifact indexing and downloads
- session timeline fan-out

Exit criteria:

- existing web and mobile clients can operate against the Rust backend with limited or no UI rewrites
- remote session creation, approval, reconnect, and export paths work end to end
- runner failures and reconnects are observable and recoverable

## Phase 4: Beyond Parity

Objective: deliver improvements that are difficult or unsafe in the current architecture.

Deliverables:

- deterministic replay
- provider failover and routing policy
- richer telemetry and audit snapshots
- finer-grained tool budgets
- stronger plugin sandboxing
- more capable Windows-native process and terminal behavior
- macOS parity work once Windows and Linux are stable

Exit criteria:

- the Rust runtime is not just compatible, but materially more robust than the reference implementation
- performance and memory budgets are enforced by CI
- remote and local flows share one coherent typed event model

## Ongoing Tracks

These tracks continue across phases rather than belonging to a single milestone.

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

## Definition of Done

A phase is complete only when:

- code exists
- tests exist
- docs describe the intended behavior
- CI enforces the behavior

A checklist without enforceable validation is not considered done in this repository.
