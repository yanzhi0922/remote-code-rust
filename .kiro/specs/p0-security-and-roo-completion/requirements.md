# Requirements Document

## Introduction

This feature closes the remaining P0-priority gaps in `remote-code-rust` before the project is declared production-stable. The work is organised into two themes:

1. **Security Hardening** — eleven items drawn from `AUDIT_CHECKLIST.md` Section 1 (S-03, S-05, S-06, S-07, S-08, S-11, S-12, S-13, S-14, S-15, S-16). These cover panic-discipline rollout, secret storage, log redaction, client-bundle leakage, path validation, transport TLS, CORS, agent binary integrity, and MCP transport TLS.
2. **Roo Agent Deepening** — completion of `ROADMAP.md` Phase 18: unifying the `PermissionDecision` enum across all three agents, wiring Roo into the GUI permission dialog, replacing the token-count approximation with native `tiktoken-rs`, integrating MCP into the Roo adapter, and adding end-to-end multi-Agent integration tests that exercise Claude / Codex / Roo through identical scenarios.

The goal is to bring all three agents (Claude, Codex, Roo) to functional and observable parity, eliminate every P0 security finding, and preserve every existing compatibility contract documented in `COMPATIBILITY.md`. The work must keep the workspace lint-clean, keep all 14,000+ existing tests green, and add new tests for every new behaviour.

## Glossary

- **AgentAdapter trait**: The Rust trait defined in `rc-agent-protocol` that every agent backend implements. It exposes `start`, `stop`, `send_message`, `cancel`, and `resolve_permission` and emits `UnifiedAgentEvent` values to the runtime.
- **AgentLoop**: The Roo-native conversation driver inside `rc-roo-adapter` that manages turn-by-turn provider calls, tool dispatch, and history compaction.
- **AppServerClient**: The Codex client component embedded in `rc-codex-adapter` that speaks the Codex App Server protocol over an in-process channel.
- **AskAgain**: One of the four `PermissionDecision` variants. The runtime requests the agent to re-emit the permission request after a state change (for example, after the user updates a session-level rule).
- **Agent_Binary_Launcher**: The component in `apps/remote-code-gui/src-tauri` and helper crates that resolves the path to a Claude, Codex, or Roo binary and spawns it as a child process when a session runs in subprocess mode.
- **BM25**: The ranking function used by `claude-tools` and `claude-skills` for keyword-based retrieval over the local tool and skill registries.
- **Circuit Breaker**: The failover state machine in `claude-provider/src/failover.rs` that tracks per-provider health and short-circuits requests to providers in a failing state.
- **Control_Plane_CORS_Policy**: The CORS configuration applied by `remote-code-control-plane` when serving the `/v1` REST API and the WebSocket event stream.
- **Frontend_Build**: The Vite build pipeline that produces the `apps/remote-code-gui` web bundle. Variables prefixed with `VITE_` are inlined into the bundle at build time.
- **GuiRuntimePermissionBroker**: The Tauri-side broker (already used by Claude and Codex) that converts an inbound `PermissionDecision` request into a GUI dialog and returns the user's decision back to the requesting adapter.
- **MCP transport (stdio | HTTP | WS)**: The three transports supported by the `claude-mcp` client for talking to MCP servers. `stdio` uses JSON-RPC over a child process, `HTTP` uses request/response, and `WS` uses a persistent WebSocket.
- **MCP_Tauri_Command_Layer**: The set of Tauri commands in `apps/remote-code-gui/src-tauri` that read and write `.mcp.json` configuration files for the active project.
- **Mobile_Secure_Store**: The Tauri command pair `mobile_secure_store_set` and `mobile_secure_store_get` in `apps/remote-code-gui/src-tauri/src/mobile.rs` that persists secrets used by the mobile shell.
- **Multi_Agent_Test_Harness**: A new test harness, introduced by this feature, that drives Claude, Codex, and Roo through identical scenarios and asserts equivalent `UnifiedAgentEvent` streams.
- **NDJSON transcript**: The newline-delimited JSON session format defined by `claude-session` and consumed by `remote-code export`.
- **PermissionDecision**: The enum defined in `rc-agent-protocol` with four variants — `Allow` (allow this single tool call), `AllowAll` (allow this tool for the remainder of the session), `Deny` (deny this tool call), and `AskAgain` (re-ask after the runtime updates state).
- **Provider**: A configured upstream LLM endpoint such as Anthropic, OpenAI, GLM, MiniMax, Bedrock, or Vertex.
- **QueryEngine**: The unified Claude-side execution engine in `claude-runtime` that runs prompts, dispatches tools, and emits `UnifiedAgentEvent` values.
- **RC.md persistent memory**: The per-project and global Markdown file used by `claude-tools` `memory_read` / `memory_write` to persist long-lived context.
- **Roo_Adapter**: The crate `rc-roo-adapter` and the `RooInProcessAdapter` type it exposes.
- **Roo_MCP_Bridge**: A new Roo-side component, introduced by this feature, that exposes MCP-discovered tools to the Roo `ToolDispatcher` using the same `McpServerConnection` API used by Claude.
- **Roo_Permission_Broker**: The internal Roo component that receives a tool-call permission request from `AgentLoop` and resolves it via the adapter's `resolve_permission` callback.
- **Roo_Token_Counter**: The token estimation component in `rc-roo-adapter` that emits the `context_usage` field on `UnifiedAgentEvent::ContextWindowUpdate`.
- **Runner_CORS_Policy**: The CORS configuration applied by `remote-code-runner` when serving its HTTP API to local and remote clients.
- **Runner_Path_Validator**: The path-bounding component (existing or newly introduced) inside `remote-code-runner` that rejects file operations outside the configured workspace root.
- **stream-json**: The headless JSON-line protocol documented in `COMPATIBILITY.md` and emitted when `remote-code -p --input-format stream-json --output-format stream-json` is used.
- **three-agent in-process architecture**: The architecture introduced in Phase 11 in which Claude, Codex, and Roo all run inside the same process as the GUI / runner via the `AgentAdapter` trait, rather than as separate child processes.
- **ToolDispatcher**: The component within each agent (Claude, Codex, Roo) responsible for routing a tool-call request to the matching tool implementation, including MCP-provided tools.
- **Tracing_Subsystem**: The `tracing` and `tracing_subscriber`-based logging facility used by every Rust crate in the workspace.
- **Transport_URL_Validator**: A validation component, introduced by this feature, that inspects WebSocket and SSE base URLs and rejects plaintext schemes for non-loopback hosts.
- **UnifiedAgentEvent**: The enum defined in `rc-agent-protocol` that all three adapters emit. Its variants form the parity contract that the multi-agent test harness asserts against.
- **Workspace_Lint_Config**: The `[workspace.lints.rust]` and `[workspace.lints.clippy]` blocks in the root `Cargo.toml` and the per-crate `lints` overrides.

## Requirements

### Section 1 — Security Hardening

#### Requirement 1: Promote `unwrap_used` from warn to deny in audited crates (S-03 / S-04)

**User Story:** As a release engineer, I want the workspace lint configuration to forbid new `.unwrap()` calls in code paths that have already been audited clean, so that future contributions cannot reintroduce panic-on-error code in production binaries.

#### Acceptance Criteria

1. THE Workspace_Lint_Config SHALL set `clippy::unwrap_used = "deny"` for the crates in `crates/adapters/`, `apps/remote-code-runner`, and `apps/remote-code-gui/src-tauri`.
2. THE Workspace_Lint_Config SHALL keep `clippy::unwrap_used = "warn"` for crates under `crates/codex/` because they vendor upstream code outside the scope of this feature.
3. WHEN `cargo clippy --workspace -- -D warnings` is run, THE workspace SHALL produce zero warnings and zero errors.
4. WHEN a contributor adds a new `.unwrap()` call in any deny-listed crate, THE Workspace_Lint_Config SHALL cause `cargo clippy` to fail with a `clippy::unwrap_used` error pointing to the offending line.
5. THE feature SHALL document, in `AUDIT_CHECKLIST.md`, which crate paths are at `deny` and which remain at `warn`, with a one-line justification per `warn` exception.

#### Requirement 2: Replace plaintext mobile secret storage with platform Keychain (S-05)

**User Story:** As a mobile user of the remote-code GUI, I want secrets stored by the mobile shell to live in the platform secure enclave, so that my credentials are not readable as plaintext JSON if the device storage is inspected.

#### Acceptance Criteria

1. THE Mobile_Secure_Store SHALL persist every secret value through the platform secure-storage API (iOS Keychain on iOS, Android Keystore on Android, OS keyring on desktop) and SHALL NOT write the secret value to a plaintext file under the application data directory.
2. WHEN `mobile_secure_store_set(key, value)` is invoked, THE Mobile_Secure_Store SHALL store the value under the platform-specific account namespace `remote-code-rust` and the supplied `key`, and SHALL return `Ok(())` on success.
3. WHEN `mobile_secure_store_get(key)` is invoked for a previously stored key, THE Mobile_Secure_Store SHALL return the stored value, and SHALL return `Ok(None)` if the key has never been stored on this device.
4. IF the platform secure-storage API returns an error (locked keychain, missing entitlement, unsupported platform), THEN THE Mobile_Secure_Store SHALL return a typed error variant identifying the failure class and SHALL NOT silently fall back to plaintext storage.
5. WHEN `Mobile_Secure_Store` is exercised in unit and integration tests, THE Mobile_Secure_Store SHALL emit at least one `tracing::warn!` event per error path containing the operation name and key identifier but SHALL NOT include the secret value.
6. WHERE the build target is desktop (Windows or Linux), THE Mobile_Secure_Store SHALL use the workspace `keyring` crate as its backing store.

#### Requirement 3: Mask API keys in tracing output (S-06)

**User Story:** As an operator reviewing tracing output, I want API keys to appear masked, so that production logs and crash reports cannot leak provider credentials.

#### Acceptance Criteria

1. WHEN any crate logs a struct containing an `api_key`, THE Tracing_Subsystem SHALL emit a value of the form `***<last4>` where `<last4>` is the final four characters of the key, and SHALL NOT emit the full key.
2. WHEN the underlying secret has fewer than eight characters, THE Tracing_Subsystem SHALL emit the literal string `***` and SHALL NOT emit any portion of the key.
3. THE feature SHALL provide a `MaskedSecret` newtype, defined once in a shared crate, whose `Debug` and `Display` implementations apply the masking rule defined in acceptance criteria 1 and 2.
4. THE feature SHALL replace every direct `Debug` derivation of a struct field that holds an API key, control-plane token, OAuth token, or refresh token with a `MaskedSecret`-typed field.
5. THE feature SHALL include a unit test that, for every adapter and every provider configuration struct touched, asserts that `format!("{:?}", config)` does not contain the literal raw secret string used in the test.

#### Requirement 4: Audit and lock down the control-plane auth token (S-07)

**User Story:** As an operator running the control plane, I want `REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN` to be unrecoverable from logs and error responses, so that an operator with log-read access cannot replay control-plane requests.

#### Acceptance Criteria

1. THE remote-code-control-plane SHALL load `REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN` once at startup into a `MaskedSecret` and SHALL NOT log the raw value at any log level.
2. WHEN the control plane rejects a request because of an invalid or missing token, THE remote-code-control-plane SHALL respond with HTTP 401 and a body whose JSON `error` field contains a constant string `unauthorized` and SHALL NOT echo the supplied token.
3. WHEN the runner registers itself, THE remote-code-runner SHALL transmit the token in the `Authorization` header only and SHALL NOT include it in URL query parameters, request bodies, or `tracing` spans.
4. THE feature SHALL include an integration test that boots the runner against a stub control plane and asserts that no log line emitted at `INFO` or below during a successful registration contains the configured token value.
5. THE feature SHALL document the token's lifecycle (where set, where consumed, rotation procedure) in a new section of `COMPATIBILITY.md` titled "Control-plane authentication".

#### Requirement 5: Eliminate `VITE_REMOTE_CONTROL_PLANE_TOKEN` from the client bundle (S-08)

**User Story:** As a security reviewer, I want the front-end bundle to contain no embedded control-plane token, so that an attacker who downloads the bundle cannot extract long-lived credentials.

#### Acceptance Criteria

1. THE Frontend_Build output SHALL contain zero occurrences of the literal substring `VITE_REMOTE_CONTROL_PLANE_TOKEN` in the emitted JavaScript and SHALL contain zero occurrences of any token value provided through that environment variable at build time.
2. THE feature SHALL remove every `import.meta.env.VITE_REMOTE_CONTROL_PLANE_TOKEN` reference from the front-end source tree.
3. WHEN the front-end needs to call a control-plane endpoint that requires the token, THE Frontend_Build SHALL route the call through a Tauri-invoked or runner-proxied request whose Rust side reads the token from the OS environment or the secure store, so that the token never reaches the browser context.
4. THE feature SHALL add a CI step that runs `rg --no-heading 'VITE_REMOTE_CONTROL_PLANE_TOKEN' apps/remote-code-gui/src/` and fails the build if any match is found.
5. THE feature SHALL update `apps/remote-code-gui/README.md` (or equivalent) to state that no front-end environment variable may carry a server-trust credential.

#### Requirement 6: Validate `project_path` in MCP Tauri commands (S-11)

**User Story:** As a desktop user, I want every MCP-config Tauri command to refuse to write to project paths I have not opened in the GUI, so that a malicious page or extension cannot drop `.mcp.json` files into arbitrary directories.

#### Acceptance Criteria

1. WHEN any MCP_Tauri_Command_Layer command (including but not limited to `mcp_config_path_for_scope`, `mcp_servers_list`, `mcp_servers_save`, `mcp_servers_delete`) receives a `project_path` argument, THE MCP_Tauri_Command_Layer SHALL validate that the path resolves (via `path_identity`) to a project that is currently registered in the GUI's managed-projects list before performing any disk write or read.
2. IF the supplied `project_path` does not match any managed project, THEN THE MCP_Tauri_Command_Layer SHALL return a typed error `McpCommandError::ProjectNotManaged` with the requested path masked to its file-name component only.
3. THE MCP_Tauri_Command_Layer SHALL apply the same validation to every helper that resolves an `.mcp.json` path, including the internal call sites of `mcp_config_path_for_scope`.
4. THE feature SHALL include unit tests that, for each MCP Tauri command, exercise both an accepted (managed) and a rejected (non-managed) `project_path` and assert the expected outcome.
5. WHEN a rejected request occurs, THE Tracing_Subsystem SHALL emit a `warn!` event with the command name, the rejection reason, and a redacted form of the requested path.

#### Requirement 7: Bound runner file operations by the configured workspace root (S-12)

**User Story:** As a runner operator, I want `remote-code-runner` to refuse to read or write outside the configured workspace root, so that a malicious session payload cannot use the runner to access arbitrary host files.

#### Acceptance Criteria

1. THE Runner_Path_Validator SHALL reject any file-system operation whose canonicalised target path is not a descendant of the configured `workspace_root` directory.
2. WHEN `remote-code-runner` receives a session create request whose payload references a file path, THE Runner_Path_Validator SHALL apply canonicalisation that resolves symbolic links, `..` segments, and Windows UNC prefixes before performing the descendant check.
3. IF the canonicalisation step fails (for example because the path does not exist on disk), THEN THE Runner_Path_Validator SHALL reject the request with a typed error `RunnerPathError::NotInWorkspace` and SHALL NOT fall back to the literal path.
4. THE Runner_Path_Validator SHALL share its implementation with the existing `path_validation` module in the main runtime so that both components apply identical rules.
5. THE feature SHALL include unit tests that cover, at minimum: a path inside the workspace root (accepted), a path outside the workspace root (rejected), a `..` traversal that re-enters the workspace root (accepted), a symlink that escapes the workspace root (rejected), and a path that does not exist (rejected).
6. THE Runner_Path_Validator SHALL run in well under 1 millisecond per call when the path resolves within the local file system, so that path validation does not become a hot-path bottleneck.

#### Requirement 8: Force TLS for non-loopback transport URLs (S-13)

**User Story:** As a remote user connecting to a runner over the public internet, I want plaintext WebSocket and SSE schemes to be rejected for non-loopback hosts, so that I cannot accidentally connect over an unencrypted channel.

#### Acceptance Criteria

1. WHEN the GUI or CLI is asked to construct a WebSocket or SSE connection from a base URL, THE Transport_URL_Validator SHALL inspect the URL host and reject the request if the scheme is `ws` or `http` and the host is not in the loopback set (`localhost`, `127.0.0.0/8`, `::1`).
2. WHEN the host is a loopback host, THE Transport_URL_Validator SHALL allow `ws` and `http` schemes for developer-experience reasons.
3. IF the validator rejects a URL, THEN THE Transport_URL_Validator SHALL return a typed error `TransportError::PlaintextNotAllowed` and the GUI SHALL surface it as a user-facing message that names the host and instructs the user to switch to `wss` or `https`.
4. THE feature SHALL include a unit test matrix that covers, at minimum: `wss://example.com` (accepted), `ws://example.com` (rejected), `ws://localhost:8080` (accepted), `ws://127.0.0.1:8080` (accepted), `http://192.168.1.10` (rejected), and `https://example.com` (accepted).
5. THE Transport_URL_Validator SHALL be invoked from every WebSocket and SSE construction site in the GUI (`apps/remote-code-gui/src/`) and the CLI (`apps/remote-code/src/remote/`).

#### Requirement 9: Tighten CORS for runner and control-plane (S-14)

**User Story:** As an operator deploying the runner and control plane, I want CORS to deny wildcard origins in production builds, so that a hostile origin cannot drive the API from a user's browser.

#### Acceptance Criteria

1. THE Runner_CORS_Policy SHALL build its allowed-origin list from the configuration value `runner.cors.allowed_origins` and SHALL NOT use `*` as a default value.
2. THE Control_Plane_CORS_Policy SHALL build its allowed-origin list from the configuration value `control_plane.cors.allowed_origins` and SHALL NOT use `*` as a default value.
3. IF the configuration value is empty, THEN THE Runner_CORS_Policy and THE Control_Plane_CORS_Policy SHALL deny all cross-origin requests (no `Access-Control-Allow-Origin` header on cross-origin responses) and SHALL emit a `warn!` event at startup advising the operator to configure at least one origin.
4. WHERE the build is a development profile (`cfg(debug_assertions)` or feature `dev-cors`), THE Runner_CORS_Policy and THE Control_Plane_CORS_Policy MAY accept `http://localhost:*` and `http://127.0.0.1:*` patterns by default to preserve developer ergonomics.
5. THE feature SHALL include integration tests that verify a release-profile build of the runner and control plane respond with no `Access-Control-Allow-Origin` header to a request from `https://attacker.example` while still serving an allowed origin successfully.

#### Requirement 10: Verify agent binary integrity before launch (S-15)

**User Story:** As a desktop user running the GUI in subprocess mode, I want each agent binary's SHA-256 to be verified against a manifest before launch, so that a swapped or tampered binary cannot run inside my session context.

#### Acceptance Criteria

1. WHEN the build script `scripts/build-agents.ps1` or `scripts/build-agents.sh` produces an agent binary, THE build script SHALL append the binary's SHA-256 digest, file size, and target triple to a manifest file at `target/agent-binaries/manifest.json`.
2. THE manifest schema SHALL be JSON, MUST include a `version` integer field starting at `1`, and SHALL list one entry per agent kind (`claude`, `codex`, `roo`) per target triple.
3. WHEN the Agent_Binary_Launcher prepares to spawn an agent binary, THE Agent_Binary_Launcher SHALL load the manifest, locate the entry whose `kind` and `target_triple` match the binary it is about to launch, recompute the SHA-256 of the on-disk file, and refuse to launch if the digest does not match.
4. IF the manifest is missing, malformed, or contains no entry for the requested kind and target, THEN THE Agent_Binary_Launcher SHALL refuse to launch the binary, emit a `tracing::error!` event naming the missing entry, and surface a typed error `AgentLaunchError::IntegrityCheckFailed` to the caller.
5. WHERE the workspace is built with the cargo feature `skip-agent-integrity` (intended only for local-development convenience), THE Agent_Binary_Launcher MAY bypass the SHA-256 check after emitting a `warn!` event and SHALL still load the manifest if present for diagnostic logging.
6. THE feature SHALL include a CI step that runs the build script in dry-run mode (no compilation) and asserts that the manifest emission code path produces a valid JSON document conforming to the schema described in acceptance criterion 2.

#### Requirement 11: Validate MCP server certificates by default (S-16)

**User Story:** As a user configuring an MCP server over HTTP or WebSocket, I want TLS certificates to be validated by default with an explicit opt-out per server, so that a man-in-the-middle cannot impersonate an MCP server I trust.

#### Acceptance Criteria

1. WHEN the claude-mcp client opens a connection over the HTTP or WebSocket transport, THE MCP transport SHALL validate the remote server's TLS certificate against the system trust store and SHALL refuse the connection on validation failure.
2. WHERE the per-server configuration entry sets `insecure: true`, THE MCP transport MAY skip certificate validation, and THE Tracing_Subsystem SHALL emit a `warn!` event at every connection attempt naming the server and the security implication.
3. WHEN any MCP server is configured with `insecure: true`, THE GUI SHALL display a non-dismissible badge next to that server in the MCP servers list with the text "TLS verification disabled".
4. IF the configuration file omits the `insecure` field, THEN THE MCP transport SHALL treat the field as `false` (validation enabled).
5. THE stdio transport SHALL be unaffected by this requirement because it does not transit the network.
6. THE feature SHALL include integration tests that exercise: a self-signed server with `insecure: false` (rejected), the same server with `insecure: true` (accepted with warning), and a valid certificate chain with `insecure: false` (accepted, no warning).

### Section 2 — Roo Agent Deepening

#### Requirement 12: Unify `PermissionDecision` across the Roo adapter (Phase 18 / R-09)

**User Story:** As a Roo user, I want my `AllowAll`, `Deny`, and `AskAgain` decisions to behave the same as they do in Claude and Codex, so that all three agents share one approval model.

#### Acceptance Criteria

1. THE Roo_Adapter SHALL expose a `resolve_permission` callback whose return type is `PermissionDecision` and SHALL NOT use `oneshot::Sender<bool>` on its public boundary.
2. WHEN the Roo `AgentLoop` requires permission for a tool call, THE Roo_Permission_Broker SHALL serialise the request as a `PermissionResolutionRequest` value identical in shape to the one used by Claude and Codex, including `tool_name`, `tool_input`, `session_id`, and the call origin.
3. WHEN the runtime returns `PermissionDecision::Allow`, THE Roo_Permission_Broker SHALL allow only the current tool call and SHALL re-prompt for the next equivalent tool call.
4. WHEN the runtime returns `PermissionDecision::AllowAll`, THE Roo_Permission_Broker SHALL record a session-level allow rule for the matching `(tool_name, tool_input shape)` pair such that subsequent equivalent requests resolve immediately without re-prompting.
5. WHEN the runtime returns `PermissionDecision::Deny`, THE Roo_Permission_Broker SHALL block the current tool call, propagate a denial result back to the Roo `AgentLoop`, and SHALL NOT modify any persistent rule.
6. WHEN the runtime returns `PermissionDecision::AskAgain`, THE Roo_Permission_Broker SHALL re-emit the same `PermissionResolutionRequest` after the runtime signals that state has been updated, up to a maximum of three retries before failing the call.
7. THE feature SHALL include a unit test for each of the four `PermissionDecision` variants that asserts the expected behaviour described in acceptance criteria 3 through 6.

#### Requirement 13: Wire the Roo adapter into the GUI permission dialog

**User Story:** As a desktop user running Roo, I want tool approval to surface the same interactive permission dialog that Claude and Codex already use, so that my approval workflow is consistent regardless of which agent is selected.

#### Acceptance Criteria

1. THE Roo_Adapter SHALL accept a `GuiRuntimePermissionBroker` (or its equivalent trait object) at construction time and SHALL route every `PermissionResolutionRequest` through that broker when the runtime is the GUI.
2. WHEN the GUI receives a Roo permission request, THE GUI SHALL render the same approval dialog component that Claude and Codex use, with the agent identifier shown as `roo`.
3. WHEN the user resolves the dialog, THE GUI SHALL forward the user's `PermissionDecision` to the Roo_Adapter unchanged.
4. IF the user closes the GUI window without resolving the dialog, THEN THE Roo_Adapter SHALL receive `PermissionDecision::Deny` after a configurable timeout (default 60 seconds) and SHALL terminate the in-flight tool call.
5. WHEN the GUI is not the active runtime (CLI, headless, automation), THE Roo_Adapter SHALL fall back to the same default broker policy used by Claude and Codex (auto-allow read tools per permission mode, prompt on stdin for interactive runtimes, deny otherwise).
6. THE feature SHALL include an integration test that drives the Roo_Adapter against a fake `GuiRuntimePermissionBroker`, asserts that one approval dialog is rendered per tool call, and asserts that `AllowAll` suppresses subsequent dialogs for the same tool name.

#### Requirement 14: Replace the Roo token-count approximation with native `tiktoken-rs`

**User Story:** As a Roo user watching the context-usage indicator, I want the displayed token count to match the count the provider will charge me for, so that I can make informed compaction decisions.

#### Acceptance Criteria

1. THE Roo_Token_Counter SHALL count tokens using the workspace `tiktoken-rs` crate with the encoding that matches the active provider's tokenizer family, and SHALL NOT use the `text.len() / 4` approximation.
2. WHEN the active provider is OpenAI-compatible, THE Roo_Token_Counter SHALL select the `cl100k_base` encoding for GPT-3.5 and GPT-4 family models, and the `o200k_base` encoding for GPT-4o and GPT-4.1 family models.
3. WHEN the active provider is Anthropic-compatible, THE Roo_Token_Counter SHALL use the `cl100k_base` encoding as a documented approximation and SHALL emit a `tracing::debug!` event explaining the choice.
4. WHEN the Roo_Adapter emits a `UnifiedAgentEvent::ContextWindowUpdate`, THE event's `tokens_used` field SHALL equal the result returned by `Roo_Token_Counter::count(messages)` for the same input.
5. THE feature SHALL include unit tests that, for at least three representative provider models (one OpenAI, one Anthropic, one MiniMax), compare the count returned by `Roo_Token_Counter` against a hand-computed `tiktoken-rs` reference and assert exact equality.
6. THE Roo_Token_Counter SHALL count an empty message list as zero tokens.

#### Requirement 15: Integrate MCP into the Roo adapter

**User Story:** As a Roo user with MCP servers configured in the GUI, I want Roo to discover and call those tools, so that my MCP investment works across all three agents.

#### Acceptance Criteria

1. WHEN the Roo_Adapter starts a session, THE Roo_MCP_Bridge SHALL load the same MCP server entries produced by `build_mcp_server_entries()` that Claude and Codex consume.
2. WHEN the Roo `ToolDispatcher` lists available tools, THE Roo_MCP_Bridge SHALL append the MCP-discovered tools to the local tool list with a `mcp::<server>::<tool>` naming prefix consistent with Claude and Codex.
3. WHEN the Roo `AgentLoop` invokes an MCP-prefixed tool, THE Roo_MCP_Bridge SHALL route the call through the matching `McpServerConnection` and SHALL return the structured result and any error to the `AgentLoop` unchanged.
4. WHEN an MCP server fails to connect at session start, THE Roo_MCP_Bridge SHALL emit a `tracing::warn!` event naming the server and SHALL allow the session to continue with the remaining tools available.
5. WHEN the Roo_Adapter advertises capabilities, THE `McpSupport` capability SHALL be reported as backed by the live Roo_MCP_Bridge implementation rather than as a no-op stub.
6. WHERE the test environment provides the npm package `mcp-server-everything` or the in-tree stub `mcp-server-stub`, THE feature SHALL include an integration test that connects Roo to that server, lists tools, invokes one tool, and asserts that the structured result reaches the test harness.

#### Requirement 16: End-to-end multi-Agent integration tests

**User Story:** As a maintainer, I want one test harness that drives Claude, Codex, and Roo through identical scenarios, so that any future divergence in their event streams is caught by CI.

#### Acceptance Criteria

1. THE Multi_Agent_Test_Harness SHALL define exactly four shared scenarios — `simple_prompt`, `tool_call_with_permission`, `mcp_tool_call`, and `cancellation` — and SHALL run each scenario against each of the three agents.
2. WHEN a scenario runs, THE Multi_Agent_Test_Harness SHALL collect the ordered sequence of `UnifiedAgentEvent` values emitted by the agent and SHALL assert that the sequence matches a per-scenario specification described in terms of variant names and ordering, with provider-specific payload differences explicitly tolerated.
3. THE Multi_Agent_Test_Harness SHALL run by default against a hermetic mock provider so that CI does not require external API keys.
4. WHERE the environment variable `RC_E2E_REAL_PROVIDER=1` is set, THE Multi_Agent_Test_Harness MAY additionally run the same scenarios against a real configured provider for opt-in validation.
5. WHEN any scenario fails, THE Multi_Agent_Test_Harness SHALL produce a diff between the observed and expected event sequences and SHALL include the agent identifier and scenario name in the test failure message.
6. THE Multi_Agent_Test_Harness SHALL complete the default (mock-provider) suite within 60 seconds on a CI machine to keep PR feedback latency bounded.
7. THE Multi_Agent_Test_Harness SHALL be invokable through `cargo test --workspace --test multi_agent_e2e` so that it integrates with existing CI commands.

## Non-Functional Requirements

#### Requirement 17: Backward compatibility with existing contracts (NFR-1)

**User Story:** As a downstream user of `remote-code`, I want every published contract to keep working unchanged after this feature ships, so that my CI scripts, mobile apps, and automation do not break.

#### Acceptance Criteria

1. THE feature SHALL preserve every CLI flag, subcommand, and exit code documented in `COMPATIBILITY.md` § "CLI Surface".
2. THE feature SHALL preserve every environment variable name and precedence rule documented in `COMPATIBILITY.md` § "Provider Environment Variables".
3. THE feature SHALL preserve the `stream-json` message families documented in `COMPATIBILITY.md` § "Headless Protocol", including their JSON field names and types.
4. THE feature SHALL preserve every endpoint shape (path, method, request body, response body) of the `/v1` REST API documented in `COMPATIBILITY.md` § "Runner and Control Plane".
5. THE feature SHALL preserve the NDJSON transcript schema produced by `remote-code export`.
6. WHERE this feature must add new fields to existing JSON payloads, THE feature SHALL only add optional fields and SHALL document them in `COMPATIBILITY.md`.

#### Requirement 18: Test gate (NFR-2)

**User Story:** As a release manager, I want every existing test to keep passing alongside the new tests this feature introduces, so that the production-readiness baseline does not regress.

#### Acceptance Criteria

1. WHEN `cargo test --workspace` is run on a CI Windows or Linux runner, THE workspace SHALL produce zero failures.
2. THE feature SHALL add at least one unit or integration test for every new behaviour introduced in Requirements 1 through 16.
3. THE feature SHALL keep the historical pass count at or above 14,000 tests, counting both pre-existing and newly added tests.

#### Requirement 19: Lint gate (NFR-3)

**User Story:** As a maintainer, I want the workspace to stay clippy-clean at deny-warnings, so that style and panic-discipline regressions are caught at PR time.

#### Acceptance Criteria

1. WHEN `cargo clippy --workspace --all-targets -- -D warnings` is run, THE workspace SHALL produce zero warnings and zero errors.
2. THE feature SHALL keep the existing per-crate lint overrides intact except where Requirement 1 explicitly upgrades a level.

#### Requirement 20: Format gate (NFR-4)

**User Story:** As a maintainer, I want the workspace to stay rustfmt-clean, so that style remains consistent across crates.

#### Acceptance Criteria

1. WHEN `cargo fmt --check` is run on the workspace, THE workspace SHALL exit with status code 0.

#### Requirement 21: Claude in-process startup latency budget (NFR-5)

**User Story:** As a desktop user opening a Claude session, I want the agent to be ready in under 100 milliseconds, so that the GUI feels responsive.

#### Acceptance Criteria

1. WHEN a benchmark measures the wall-clock time between `RemoteClaudeAdapter::new` returning and the first `UnifiedAgentEvent::Ready` event being emitted, THE measured duration SHALL be less than 100 milliseconds on a CI Windows or Linux runner.
2. THE feature SHALL NOT regress this measurement by more than 10 percent compared to the baseline taken immediately before this feature's first commit.

#### Requirement 22: GUI baseline memory budget (NFR-6)

**User Story:** As a desktop user, I want the GUI's idle memory footprint to stay under 60 megabytes, so that the application is suitable for low-memory laptops.

#### Acceptance Criteria

1. WHEN the GUI is launched and left idle on a fresh project for 10 seconds, THE GUI process resident set size SHALL be less than 60 megabytes on a CI Windows or Linux runner.
2. THE feature SHALL NOT regress this measurement by more than 10 percent compared to the baseline taken immediately before this feature's first commit.

#### Requirement 23: No plaintext secrets in observables (NFR-7)

**User Story:** As a security reviewer, I want every observable surface (logs, error messages, transcripts, telemetry) to be free of raw secret values, so that operational diagnostics cannot leak credentials.

#### Acceptance Criteria

1. THE Tracing_Subsystem, the NDJSON transcript writer, and the telemetry exporter SHALL NOT emit raw API keys, control-plane tokens, OAuth tokens, refresh tokens, or platform-secure-storage values.
2. THE feature SHALL include a workspace-level CI step that runs `rg --no-heading 'sk-[A-Za-z0-9]{16,}|ghp_[A-Za-z0-9]{16,}|xoxb-[A-Za-z0-9]+'` over the latest CI log artefact directory and fails the build on any hit.

#### Requirement 24: User-home path redaction in tracing (NFR-8)

**User Story:** As a privacy-conscious user, I want tracing output to redact paths under my home directory, so that crash reports and logs do not leak my username or directory layout.

#### Acceptance Criteria

1. WHEN the Tracing_Subsystem logs a file system path that begins with the current user's home directory, THE Tracing_Subsystem SHALL replace the home prefix with the literal `~` and SHALL preserve the trailing path components.
2. WHERE redaction is impossible because the home directory cannot be determined, THE Tracing_Subsystem MAY emit the original path unchanged and SHALL emit a single `tracing::warn!` event per process explaining why redaction was skipped.
3. THE feature SHALL include a unit test that asserts a sample log line containing `/home/alice/projects/foo/bar.rs` (Linux) or `C:\\Users\\alice\\projects\\foo\\bar.rs` (Windows) is redacted to `~/projects/foo/bar.rs` or `~\\projects\\foo\\bar.rs` respectively.

#### Requirement 25: Agent parity (NFR-9)

**User Story:** As a developer integrating against `UnifiedAgentEvent`, I want Claude, Codex, and Roo to emit equivalent event shapes for equivalent operations, so that downstream code does not need agent-specific branches.

#### Acceptance Criteria

1. WHEN any of the four shared scenarios defined in Requirement 16 runs against any of the three agents, THE adapter SHALL emit the same set of `UnifiedAgentEvent` variant kinds in the same order.
2. WHERE an event variant is documented as agent-specific (`ContextCompacted` for Claude, `CodexAppServerNotification` for Codex), THE feature SHALL document the exemption in a new "Event parity matrix" table appended to `ARCHITECTURE.md`.
3. WHEN an adapter emits a `ToolCallStarted` event, THE event SHALL include `tool_name`, `tool_input`, `call_id`, and `agent_kind` fields with consistent naming and types across all three adapters.

## Out of Scope

The following items are explicitly deferred to future specs and SHALL NOT be addressed by this feature:

- **Phase 19 mobile native init** — `tauri android init`, `tauri ios init`, mobile UI adaptation, push notifications, deep links, remote terminal stream, remote file preview. Reason: requires native Android Studio / Xcode setup beyond this feature's CI envelope.
- **Phase 20 advanced features** — deep subtask delegation, session rollback, shadow Git checkpoints, Task Flow visualisation, real TTS. Reason: each is a multi-week feature in its own right.
- **AUDIT_CHECKLIST P1 / P2 / P3 items** — `lib.rs` (7,264 lines) split, `useAppStore.ts` split, `types.ts` split, `tauri.ts` split, i18n extraction, performance hot-path clone audit, frontend a11y polish, Sentry integration, OpenTelemetry completion, env var documentation, `CONTRIBUTING.md`, `cargo-audit` / `npm-audit`, sccache, frontend bundle-size optimisation, rustdoc rollout, classification of the 1,329 outstanding `TODO/FIXME` markers. Reason: these are quality-of-life and documentation improvements that do not block production-readiness.
- **Phase 21 cloud runner** — multi-workstation scheduling, team collaboration, multi-user shared sessions. Reason: requires backend infrastructure outside this feature's scope.
- **`rama-*` alpha → stable migration**. Reason: blocked on upstream availability.
- **macOS parity work**. Reason: CI does not yet include a macOS runner; this feature targets the Windows + Linux baseline.

## Assumptions

- The workspace already declares `tiktoken-rs = "0.7"` (or compatible) in `[workspace.dependencies]`. Requirement 14 consumes that declaration.
- The workspace already declares `keyring` (v3.x) in `[workspace.dependencies]`. Requirement 2 consumes that declaration on desktop targets.
- The Tauri secure-storage plugin (or an equivalent platform binding) is available on the chosen mobile targets. Requirement 2 consumes that plugin on iOS and Android.
- The unwrap audit summarised in `AUDIT_CHECKLIST.md` (S-04) is current — namely, that crates under `crates/adapters/`, `apps/remote-code-runner`, and `apps/remote-code-gui/src-tauri` produce zero `clippy::unwrap_used` warnings under `warn`. If this assumption is invalidated when the lint is upgraded to `deny`, Requirement 1 includes the work to fix the offending sites.
- The `path_validation` module in the main runtime exposes a public API that the runner can consume. Requirement 7 consumes this API.
- The npm package `mcp-server-everything` (or the equivalent in-tree stub `mcp-server-stub`) is reachable from the test environment. Requirement 15 consumes this server.
- The CI Windows and Linux runners have at least 4 GiB of RAM available to the build process. Requirements 21 and 22 assume this baseline when measuring latency and memory.

## Dependencies

- **D-1 — Build-script update**: `scripts/build-agents.ps1` and `scripts/build-agents.sh` MUST be updated as part of Requirement 10 to emit `target/agent-binaries/manifest.json`. This requires running the build scripts on a developer or CI machine; CI need only verify manifest emission via a dry-run mode.
- **D-2 — Mobile platform projects**: Requirement 2 only ships the desktop path under CI. The iOS and Android paths require the mobile native project scaffolding which is part of Phase 19; this feature stubs the platform-keychain calls behind a trait so that the Phase 19 work can fill them in without a new API change.
- **D-3 — Real-provider opt-in test secrets**: The `RC_E2E_REAL_PROVIDER=1` mode of Requirement 16 requires at least one provider API key in the runner's environment. The default (mock) mode does not.
- **D-4 — `tiktoken-rs` model maps**: Requirement 14's encoding-selection logic depends on a maintained mapping from provider+model strings to `tiktoken-rs` encodings. The feature MUST add this mapping under `crates/adapters/rc-roo-adapter` and SHALL keep it in sync with `claude-provider`'s own model registry.
- **D-5 — Stub MCP server for tests**: Requirement 15's integration test depends on the presence of either `mcp-server-everything` (npm) or an in-tree stub. If neither is present at test time, the test SHALL be skipped with a clear `eprintln!` message rather than fail.
