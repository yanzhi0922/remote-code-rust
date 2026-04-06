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

Phase 1 compatibility targets:

- `remote-code doctor`
- `remote-code sessions`
- `remote-code resume <session-id>`
- `remote-code export <session-id>`
- `remote-code -p --input-format stream-json --output-format stream-json`

Additional flags may be introduced, but existing supported compatibility flags must remain additive rather than breaking.

Commands that were only historical compatibility escape hatches in the reference workspace are not automatically part of the Rust public surface. They are reintroduced only if they serve a clear product need.

## Headless Protocol

The Rust runtime will continue to emit the important `stream-json` message families used by the current headless shell and remote orchestration paths:

- `system`
- `assistant`
- `result`
- `control_request`
- `control_cancel_request`
- `tool_progress`

The compatibility layer will preserve the meaning of:

- init metadata
- session state transitions
- permission requests and cancellations
- success vs. error result framing

Internally, the runtime uses typed Rust enums and structs. Only the serializer stays legacy-shaped.

## Session Compatibility

The current product stores active state under `~/.remote-code/`.

`remote-code-rust` does not reuse that directory directly. The new default profile root is:

```text
~/.remote-code-rust/
```

The rewrite provides import tooling for:

- provider config and profile settings
- session indexes and exportable transcripts
- history and state files that can be mapped safely
- skill and plugin inventories that can be represented in the new model

The migration tool is intentionally explicit. The runtime does not silently mutate the old profile.

## Provider Environment Variables

The following variables remain first-class compatibility inputs:

- `REMOTE_CODE_PROVIDER`
- `REMOTE_CODE_BASE_URL`
- `REMOTE_CODE_API_KEY`
- `REMOTE_CODE_MODEL`
- `REMOTE_CODE_REQUEST_HEADERS_JSON`

The Rust rewrite adds explicit configuration where the reference implementation inferred behavior:

- `REMOTE_CODE_PROTOCOL`
- `REMOTE_CODE_PROFILE_DIR`
- `REMOTE_CODE_COMPAT_MODE`

Precedence rules:

1. CLI flags
2. Explicit new env vars
3. Existing compatibility env vars
4. Profile config files
5. Safe defaults

`REMOTE_CODE_PROTOCOL` wins over base URL heuristics when both are provided.

## Provider Protocol Families

The Phase 1 runtime must support both:

- Anthropic-compatible message APIs
- OpenAI-compatible chat completion APIs

Gateway-specific integrations such as GLM, MiniMax, ZAI, and private proxies are configuration variants on top of these protocol families. They must not fork the architecture.

## Permissions

The reference workspace supports modes such as `default`, `acceptEdits`, `bypassPermissions`, `dontAsk`, and `plan`.

The Rust rewrite keeps those user-facing compatibility modes where they are externally meaningful, but reimplements the decision engine behind a typed permission service.

Compatibility guarantees:

- explicit approval prompts remain possible in interactive and remote flows
- non-interactive denials are preserved when required
- blocked path reporting remains serializable through `stream-json`
- auditability is improved rather than reduced

## Tools

The reference lightweight runtime exposes a small tool set centered on file I/O and shell execution. The full backend exposes a much larger mixed surface.

The Rust rewrite does not preserve old handler internals. It preserves:

- the expectation that core local tools exist
- the distinction between read-like and edit-like operations
- the ability to route permission requests around mutable tools
- a clear path for MCP-provided tools to appear alongside local tools

Tool implementation details are intentionally new.

## Skills and Plugins

Compatibility targets:

- continue discovering skills from file-based `SKILL.md` roots
- keep skill indexing and invocation as a first-class workflow
- support legacy plugin ecosystems only through explicit adapter bridges

Non-goals:

- running legacy JavaScript plugin code directly inside the Rust process
- preserving plugin loading behavior that relies on implicit in-process side effects

## Runner and Control Plane

The Rust control plane exposes versioned `/v1` HTTP and WebSocket APIs.

Compatibility objectives:

- existing web and mobile clients should be able to migrate without a ground-up rewrite
- runner registration and session streaming semantics should remain familiar
- approval and artifact flows should map cleanly from the current `remote-hub` behavior

The API surface is allowed to become stricter and more explicit than the current TypeScript service as long as compatibility shims preserve expected workflows.

## Fixture Strategy

Compatibility is enforced by committed fixtures collected from the reference workspace.

Fixture categories:

- `stream-json` init and session-state flows
- permission request shapes
- session export behaviors
- error framing
- provider normalization edge cases

The old repository is read-only input for fixture collection. CI should validate against committed fixtures instead of shelling out to the old project during normal test runs.

## Deliberate Non-Compatibility

The following are intentionally not compatibility constraints:

- Bun-specific boot behavior
- internal TypeScript module layout
- runtime-specific implementation quirks that are not part of an external contract
- leaked or provenance-unclear code paths from third-party repositories
- fallback product surfaces that only existed as temporary migration scaffolding

Compatibility matters at the edges. The internals are free to become simpler and safer.
