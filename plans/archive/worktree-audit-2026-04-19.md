# Worktree Audit 2026-04-19

## Snapshot

- Current dirty worktree size: 114 tracked file changes, roughly `+6567 / -3986`.
- The tree is not dirty because of build caches anymore. Root `target/` and stray `tasks.json` were already removed.
- Current untracked items are source files, not disposable artifacts:
  - `crates/rc-session/src/plan_state.rs`
  - `crates/rc-session/src/runtime_context.rs`
  - `crates/rc-system-prompt/src/sections/ant_model_override.rs`
  - `crates/rc-tools/src/runtime_plan_mode.rs`
  - `crates/rc-tui/src/runtime_hooks.rs`

## Mainline Clusters

These clusters are consistent with the current parity/recovery program and should be treated as primary reconstruction work, not noise.

### Runtime / Compat / CLI Host

- `apps/remote-code/src/query_engine_compat.rs`
- `apps/remote-code/src/conversation.rs`
- `apps/remote-code/src/headless.rs`
- `apps/remote-code/src/agents.rs`
- `apps/remote-code/src/main.rs`
- `apps/remote-code/src/mcp_cli.rs`

This is the main host-runtime convergence layer. It currently carries system prompt assembly, provider-visible tool filtering, resume/compact recovery, plan-mode reinjection, and runtime MCP surface work.

### Tool Runtime / Plan Mode / Tool Surface

- `crates/rc-tools/src/lib.rs`
- `crates/rc-tools/src/plan_mode.rs`
- `crates/rc-tools/src/runtime_plan_mode.rs`
- `crates/rc-tools/src/specs.rs`
- `crates/rc-tools/src/system.rs`
- `crates/rc-tools/src/file_ops.rs`
- `crates/rc-tools/src/tasks.rs`
- `crates/rc-tools/src/send_message.rs`
- `crates/rc-tools/src/send_user_file.rs`
- `crates/rc-tools/src/team_tools.rs`

This cluster is clearly a real runtime replacement, not minor edits. It includes tool search, plan mode, message/file transport, team tools, and runtime tool gating.

### Provider / Query Engine

- `crates/rc-provider/src/lib.rs`
- `crates/rc-provider/src/streaming.rs`
- `crates/rc-provider/src/advanced_api.rs`
- `crates/rc-query-engine/src/engine.rs`
- `crates/rc-query-engine/src/preprocessing.rs`
- `crates/rc-query-engine/src/observer.rs`
- `crates/rc-query-engine/src/reactive_compact.rs`
- `crates/rc-query-engine/src/max_tokens_recovery.rs`
- `crates/rc-query-engine/src/config.rs`

This cluster now contains the provider wire-shape, compact/resume behavior, tool-result persistence, and query loop recovery path.

### Config / Settings / Session

- `crates/rc-config/src/lib.rs`
- `crates/rc-settings/src/loader.rs`
- `crates/rc-settings/src/merge.rs`
- `crates/rc-settings/src/types.rs`
- `crates/rc-settings/src/provider.rs`
- `crates/rc-settings/src/permissions.rs`
- `crates/rc-settings/src/mcp.rs`
- `crates/rc-session/src/lib.rs`
- `crates/rc-session/src/transcript.rs`
- `crates/rc-session/src/plan_state.rs`
- `crates/rc-session/src/runtime_context.rs`

This is the persistence and precedence layer for provider-aware auth, MCP discovery, resume state, and plan-mode state.

### Prompt / Core State / Permissions

- `crates/rc-system-prompt/src/lib.rs`
- `crates/rc-system-prompt/src/sections/using_tools.rs`
- `crates/rc-system-prompt/src/sections/tool_result.rs`
- `crates/rc-system-prompt/src/sections/mcp_instructions.rs`
- `crates/rc-system-prompt/src/sections/env_info.rs`
- `crates/rc-system-prompt/src/sections/ant_model_override.rs`
- `crates/rc-core/src/message.rs`
- `crates/rc-core/src/state.rs`
- `crates/rc-core/src/hook_executor.rs`
- `crates/rc-core/src/hook_types.rs`
- `crates/rc-permissions/src/lib.rs`
- `crates/rc-permissions/src/loader.rs`

This is the contract layer. Any parity work on runtime/tool/provider prompt behavior will keep touching these files.

### TUI

- `crates/rc-tui/src/lib.rs`
- `crates/rc-tui/src/app.rs`
- `crates/rc-tui/src/commands/mod.rs`
- `crates/rc-tui/src/commands/mode_commands.rs`
- `crates/rc-tui/src/commands/agent_commands.rs`
- `crates/rc-tui/src/commands/session_mgmt.rs`
- `crates/rc-tui/src/runtime_hooks.rs`

The TUI changes are not isolated cosmetic edits. They are coupled to runtime mode/session/agent changes and should be reviewed as a parity surface.

## Explicit Deletions

These are deliberate structural deletions and need focused review, not blind revert:

- `crates/rc-tools/src/enhanced_tool_system.rs`
- `crates/rc-tools/src/mcp_resource_tools.rs`

Both removals look like consolidation of duplicate or stale runtime paths.

## Secondary / Needs Separate Review

These files are plausible side effects of the mainline refactor, but they are not obviously part of the narrowest parity path and should be reviewed separately before any final push:

- `apps/remote-code-gui/src-tauri/src/lib.rs`
- `crates/rc-analytics/src/growthbook.rs`
- `crates/rc-auth/src/provider_auth.rs`
- `crates/rc-managed-settings/src/sync_engine.rs`
- `crates/rc-plugins/src/hint_recommendation.rs`
- `crates/rc-plugins/src/policy.rs`
- `crates/rc-plugins/src/zip_cache.rs`
- `crates/rc-file-history/src/backup.rs`
- `crates/rc-file-history/src/diff_stats.rs`
- `crates/rc-file-history/src/snapshot.rs`
- `crates/rc-file-history/src/state.rs`
- `crates/rc-swarm/src/backends/mod.rs`
- `crates/rc-swarm/src/constants.rs`
- `crates/rc-swarm/src/error.rs`
- `crates/rc-utils/src/memory_store.rs`
- `Cargo.lock`

## Safe Cleanup Status

Already cleaned:

- root `target/`
- stray root `tasks.json`

Not cleanup targets:

- tracked source deltas listed above
- untracked source files listed above
- CRLF warnings from Git line-ending normalization

## Working Rule

While this audit is active:

- Do not use destructive git cleanup on the worktree.
- Treat the runtime/tool/provider/prompt/session clusters as active parity work.
- Review secondary clusters separately instead of mixing them into mainline parity commits.
- Keep removing only true garbage artifacts, not source deltas.
