# `plans/archive/` — Historical Design Documents

This directory contains design documents, gap analyses, and architectural
decisions that are **no longer in active use** but are kept for institutional
memory and archaeological reference. Files here are NOT the source of truth
for the current state of the codebase — they describe plans that have
either shipped, been superseded, or been abandoned.

## When to read these

- Onboarding a new contributor: skim the **one-paragraph summary** for each
  document below to understand *why* the codebase is shaped the way it is.
- Reviewing a large refactor: check the **supersedes** field to find prior
  attempts and their conclusions.
- Re-activating an abandoned feature (e.g. the ZCode integration): read
  the corresponding `*-plan.md` to recover design context that is no
  longer in the code.

## When NOT to read these

- Implementing against the current API: read the source, not these docs.
  Code in `crates/`, `apps/`, and `agents/` is authoritative; design
  documents here may be months or years out of date.

## Index

### Active historical record (created 2026-06-02)

| File | One-line summary | Status |
| --- | --- | --- |
| `zcode-checkpoints-archive.md` | Metadata for the 138 ZCode checkpoint refs that were removed on 2026-06-02. | Reference only. |

### Architectural plans and gap analyses (kept for context)

| File | One-line summary | Status |
| --- | --- | --- |
| `tauri-gui-architecture-design.md` | Initial Tauri 2 GUI architecture for the desktop app. | Superseded by the current `apps/remote-code-gui/` layout. |
| `claude-code-rust-full-clone-plan.md` | Plan to port Claude Code (TypeScript) to Rust. | Shipped; current code reflects this plan. |
| `claude-code-deep-comparison.md` | Feature parity comparison: Claude Code (TS) vs `rc-claude-adapter` (Rust). | Reference for ongoing parity work. |
| `three-agent-integration.md` | Architecture for hosting Claude + Codex + Roo in one desktop app. | Shipped; current `apps/remote-code-gui/` runs all three in-process. |
| `phase5-gui-agent-integration.md` | Phase 5 work: GUI ↔ agent integration, Tauri command surface. | Shipped. |
| `gui-remote-advanced-optimization-v2.md` | GUI ↔ remote-control-plane advanced optimization design. | Shipped. |
| `query-engine-unified-path-design.md` | Single query path that handles both local and remote sessions. | Shipped. |
| `mobile-app-research-report.md` | Tauri mobile research and feasibility analysis. | Shipped; current `apps/remote-code-gui/` mobile target uses this. |

### Test plans and stress reports (reference only)

| File | One-line summary | Status |
| --- | --- | --- |
| `comprehensive-test-plan-500.md` | 500-test acceptance plan used during v1.0.0. | Completed; tests live in `crates/claude/claude-integration-tests/`. |
| `cli-stress-test-report.md` | Results of the CLI stress test pass. | Snapshot; numbers may be stale. |
| `worktree-audit-2026-04-19.md` | Git worktree audit as of 2026-04-19. | Snapshot; subsequent audits in `plans/PROJECT_STATUS.md`. |

### Working agreements and audits

| File | One-line summary | Status |
| --- | --- | --- |
| `ACCEPTANCE_AUDIT_REPORT.md` | External-acceptance audit report. | Snapshot; see `ROADMAP.md` for current state. |
| `REMOTE_PLAN.md` | Remote-control-plane feature plan. | Shipped. |
| `gap-analysis-and-restructure.md` | One-off gap analysis that triggered a workspace restructure. | Reference for the `crates/roo/` merge. |

## How to add to this directory

1. Move the document to `plans/archive/<filename>.md` (do NOT delete from
   `plans/` — keep the original path broken with a redirect comment if
   the file was linked from elsewhere).
2. Add an entry to the **Index** table above with a one-line summary.
3. Update `plans/PROJECT_STATUS.md` if the document is referenced there.

## How to retire a document

If a document is found to be entirely wrong or irrelevant:

1. Mark it `[OBSOLETE]` at the top of the file.
2. Add a `**Obsoleted by**: <PR or commit>` line.
3. Leave the file in place — do NOT delete; some tools and external links
   may still reference it.
