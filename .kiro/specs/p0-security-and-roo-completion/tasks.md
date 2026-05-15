# Implementation Plan: P0 Security and Roo Completion

## Overview

Convert the feature design into a series of prompts for a code-generation LLM that will implement each step with incremental progress. Make sure that each prompt builds on the previous prompts, and ends with wiring things together. There should be no hanging or orphaned code that isn't integrated into a previous step. Focus ONLY on tasks that involve writing, modifying, or testing code.

The implementation language is **Rust** (with TypeScript only for the unavoidable GUI front-end edits). Work proceeds in three layers:

1. **Foundation** — five new shared crates (`rc-secrets`, `rc-transport-validator`, `rc-tracing-redact`, `rc-agent-launcher`) plus the `PermissionDecision::AskAgain` variant in `rc-agent-protocol`. Property tests for the universal contracts ship alongside each crate.
2. **Security hardening** — apply the foundation pieces across the workspace: mask provider/control-plane/runner secrets, rebuild the mobile secure store on `keyring`, scrub the front-end bundle, guard MCP and runner paths, force TLS on transports, tighten CORS, verify agent-binary integrity, and validate MCP TLS by default.
3. **Roo deepening** — unify `PermissionDecision`, route Roo through the GUI permission dialog, replace the token-count approximation with `tiktoken-rs`, integrate MCP into the Roo `ToolDispatcher`, and wire the three adapters into a multi-agent E2E harness.

Property tests use `proptest` with `cases: 256` and a single test per property (Properties 1–18). Optional sub-tasks (`*`) are test-only.

## Tasks

- [ ] 1. Foundation: shared crates and PermissionDecision extension
  - [ ] 1.1 Scaffold cross-cutting shared crates
    - Create `crates/shared/rc-secrets/{Cargo.toml, src/lib.rs}` with stubs for `MaskedSecret` (depends on workspace `serde`, `zeroize`).
    - Create `crates/shared/rc-transport-validator/{Cargo.toml, src/lib.rs}` (depends on workspace `url`, `thiserror`).
    - Create `crates/shared/rc-tracing-redact/{Cargo.toml, src/lib.rs, src/home_path.rs}` (depends on workspace `tracing`, `tracing-subscriber`, `dirs`).
    - Create `crates/shared/rc-agent-launcher/{Cargo.toml, src/lib.rs}` (depends on workspace `serde`, `serde_json`, `sha2`, `thiserror`, `chrono`).
    - Add all four crate paths to root `Cargo.toml` `[workspace.members]`.
    - _Requirements: 3.3, 7.4, 8.1, 10.2, 24.1_

  - [ ] 1.2 Implement `MaskedSecret` newtype in `rc-secrets`
    - Write `MaskedSecret(String)` with `Clone`, `Serialize`/`Deserialize` (transparent), `Zeroize`, `ZeroizeOnDrop`.
    - Implement `Debug` and `Display` so a value of length `< 8` renders as `***` and otherwise as `***<last4>` (Unicode-safe via char-aware suffix slicing).
    - Expose `MaskedSecret::new`, `expose() -> &str`, `last4() -> &str`.
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ]* 1.3 Property test for `MaskedSecret` masking
    - **Property 2: `MaskedSecret` masking**
    - **Validates: Requirements 3.1, 3.2, 3.5**
    - For any `s`, assert `format!("{:?}", MaskedSecret::new(s))` equals `***` when `s.chars().count() < 8` and equals `***<last4>` otherwise. Generator emits arbitrary Unicode strings.
    - Place at `crates/shared/rc-secrets/src/lib.rs` test module under name `prop_masked_secret_debug_never_reveals_full`.

  - [ ] 1.4 Implement `validate_transport_url` in `rc-transport-validator`
    - Write `validate_transport_url(&str) -> Result<Url, TransportError>` accepting `wss`/`https` for any host and `ws`/`http` only for loopback (`localhost`, `::1`, any address in `127.0.0.0/8`).
    - Define `TransportError::PlaintextNotAllowed { scheme, host }` and `TransportError::InvalidUrl(String)` (via `thiserror`).
    - _Requirements: 8.1, 8.2, 8.3_

  - [ ]* 1.5 Property test for transport URL validation
    - **Property 7: Transport-URL validation**
    - **Validates: Requirements 8.1, 8.2, 8.3**
    - Assert `validate_transport_url(u)` is `Ok` iff scheme is `wss`/`https` or scheme is `ws`/`http` with a loopback host; otherwise returns `PlaintextNotAllowed` with the exact parsed scheme and host.
    - Place at `crates/shared/rc-transport-validator/src/lib.rs` test module under name `prop_transport_url_loopback_or_tls_only`.

  - [ ] 1.6 Implement `HomePathRedactionLayer` in `rc-tracing-redact`
    - Write a `tracing-subscriber::Layer` that walks event field values and replaces a leading `$HOME` (or `%USERPROFILE%` on Windows) prefix with `~`, preserving the trailing path components exactly.
    - Emit exactly one `tracing::warn!` per process when the home directory cannot be determined (latch via `OnceCell`).
    - _Requirements: 24.1, 24.2_

  - [ ]* 1.7 Property test for home-path redaction
    - **Property 18: Home-path redaction in tracing**
    - **Validates: Requirements 24.1, 24.2**
    - For any `p` and a synthesised home `h`, assert `redact(p)` equals `~` followed by `&p[h.len()..]` when `p` starts with `h`, otherwise equals `p`. When `h` cannot be determined, no path is rewritten.
    - Place at `crates/shared/rc-tracing-redact/src/home_path.rs` test module under name `prop_home_path_redaction_preserves_tail`.

  - [ ] 1.8 Implement `AgentBinaryManifest` schema and `AgentBinaryLauncher` in `rc-agent-launcher`
    - Define `AgentBinaryManifest { version: u32, generated_at, entries: Vec<AgentBinaryEntry> }` and `AgentBinaryEntry { kind, target_triple, path, sha256, size_bytes }` with `Serialize`/`Deserialize`.
    - Define `AgentKind { Claude, Codex, Roo }` with `serde(rename_all = "snake_case")`.
    - Implement `AgentBinaryLauncher::launch(kind, &Path) -> Result<Child, AgentLaunchError>`: load manifest, locate entry by `(kind, host_target_triple)`, recompute SHA-256, refuse on mismatch.
    - Define `AgentLaunchError { IntegrityCheckFailed, ManifestMissing, Io(io::Error) }`.
    - Gate the SHA check behind cargo feature `skip-agent-integrity`; when bypassed, emit a `tracing::warn!`.
    - _Requirements: 10.2, 10.3, 10.4, 10.5_

  - [ ]* 1.9 Property test for agent-binary integrity
    - **Property 9: Agent-binary integrity**
    - **Validates: Requirements 10.3, 10.4, 10.5**
    - For a synthetic on-disk binary `b` and manifest entry `e` with matching `(kind, target_triple)`, assert `launch` succeeds iff `sha256(b) == e.sha256` (or the bypass feature is active). Manifest missing/malformed → `IntegrityCheckFailed`/`ManifestMissing`.
    - Place at `crates/shared/rc-agent-launcher/tests/integrity.rs` under name `prop_launcher_accepts_iff_digest_matches`.

  - [ ] 1.10 Add `PermissionDecision::AskAgain` variant in `rc-agent-protocol`
    - Extend the existing enum with `AskAgain` while keeping `serde(rename_all = "snake_case")`. Existing JSON values `allow`, `deny`, `allow_all` continue to deserialise.
    - Add a doc comment explaining the broker-side three-retry cap.
    - _Requirements: 12.1, 12.6, 17.6_

  - [ ] 1.11 Upgrade workspace lints to deny `unwrap_used` in audited crates
    - Add `[lints.clippy] unwrap_used = "deny"` to every crate under `crates/adapters/`, to `apps/remote-code-runner/Cargo.toml`, and to `apps/remote-code-gui/src-tauri/Cargo.toml`.
    - Confirm root `Cargo.toml` keeps the workspace-wide `unwrap_used = "warn"` so `crates/codex/*` retains the documented exemption.
    - Replace any newly-surfaced `.unwrap()` call with `?` or `expect("invariant: ...")` on the audited crates.
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 19.1_

  - [ ]* 1.12 Smoke test for workspace lint configuration
    - Parse the root and per-crate `Cargo.toml` files and assert the per-crate `[lints.clippy].unwrap_used` value is `"deny"` for the audited crates and `"warn"` for `crates/codex/*`.
    - Place at `crates/shared/rc-secrets/tests/lint_config.rs` (any workspace-level test crate works).
    - _Requirements: 1.1, 1.2, 1.5_

- [ ] 2. Apply `MaskedSecret` and register tracing redaction
  - [ ] 2.1 Replace API-key fields in `claude-provider` with `MaskedSecret`
    - Convert `ProviderConfig::api_key: Option<String>` to `Option<MaskedSecret>`.
    - Update every constructor and serialiser call-site, ensuring on-disk JSON round-trips unchanged via the transparent `serde` impl.
    - _Requirements: 3.3, 3.4, 17.6, 23.1_

  - [ ] 2.2 Replace API-key / OAuth / refresh-token fields in the Codex provider config with `MaskedSecret`
    - Convert `api_key`, `oauth_token`, `refresh_token` (Codex provider config struct) to `MaskedSecret`.
    - _Requirements: 3.3, 3.4, 17.6, 23.1_

  - [ ] 2.3 Replace API-key fields across Roo provider configs with `MaskedSecret`
    - Update every Roo provider config (Anthropic, OpenAI, OpenRouter, Bedrock, Vertex, MiniMax, etc.) to use `MaskedSecret` for the credential field.
    - _Requirements: 3.3, 3.4, 17.6, 23.1_

  - [ ] 2.4 Lock down the control-plane auth token in `claude-control-plane`
    - Convert `AuthConfig::token` from `String` to `MaskedSecret`.
    - Update the auth middleware to compare inbound `Authorization: Bearer` value against `secret.expose()` using `constant_time_eq`.
    - On mismatch, return `HTTP 401` with body `{"error":"unauthorized"}` and emit no token-related log line. On match, log only a `debug!("auth.ok")` event with no token data.
    - Update the runner registration call-site so the token is only sent in the `Authorization` header (never URL/body/span fields).
    - _Requirements: 4.1, 4.2, 4.3_

  - [ ] 2.5 Register `HomePathRedactionLayer` in every binary entry-point
    - Wire the layer into the `tracing-subscriber` registry in `apps/remote-code-runner/src/main.rs`, `apps/remote-code-control-plane/src/main.rs`, `apps/remote-code-gui/src-tauri/src/lib.rs`, and `apps/remote-code/src/main.rs`.
    - _Requirements: 24.1, 24.2_

  - [ ]* 2.6 Unit test that `Debug` never reveals raw secrets across all updated configs
    - For each adapter / control-plane / runner config struct touched in 2.1–2.4, assert `format!("{:?}", config)` does not contain the literal raw secret string used in the test (when `secret.len() >= 8`).
    - _Requirements: 3.5_

  - [ ]* 2.7 Integration test for control-plane auth-token confinement (Property 3)
    - **Property 3: Control-plane auth-token confinement**
    - **Validates: Requirements 4.1, 4.2, 4.3, 23.1**
    - Boot a stub control plane against the runner registration path, capture every `tracing` event at `INFO` and below for the duration, and assert the configured token sentinel appears in zero events. Assert outbound HTTP requests carry the token only in the `Authorization` header. Assert the rejection response body equals `{"error":"unauthorized"}` and contains no occurrence of the supplied token.
    - Place at `apps/remote-code-control-plane/tests/auth_confinement.rs` under name `prop_auth_token_never_observable_outside_authorization_header`.

- [ ] 3. Mobile secure store (Keyring backend)
  - [ ] 3.1 Implement `SecureStoreBackend` trait and `KeyringBackend`
    - Define `SecureStoreError { Locked, MissingEntitlement, Unsupported, Backend(String) }` and `SecureStoreBackend { set, get, remove }` in `apps/remote-code-gui/src-tauri/src/secure_store/mod.rs`.
    - Implement `KeyringBackend` wrapping `keyring::Entry::new("remote-code-rust", key)` for desktop targets.
    - Stub `IosKeychainBackend` and `AndroidKeystoreBackend` behind `#[cfg(target_os = "ios")]` / `#[cfg(target_os = "android")]` for Phase 19 to fill in.
    - _Requirements: 2.1, 2.6_

  - [ ] 3.2 Wire backend into `mobile_secure_store_set/get/remove` Tauri commands and run legacy migration
    - Replace the existing implementation in `apps/remote-code-gui/src-tauri/src/mobile.rs` to call through `SecureStoreBackend`.
    - On the first `get` call, read any legacy `secure_store.json` plaintext file, copy each `(key, value)` into the keyring, and delete the file once migration succeeds.
    - On every error path emit `tracing::warn!(operation = "set"|"get"|"remove", key = %key)` (no value).
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [ ]* 3.3 Property test for secure-store round-trip and confidentiality (Property 1)
    - **Property 1: Secure-store round-trip and confidentiality**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.5**
    - For any sequence of `set`/`get` operations: after `set(k, v)` returns `Ok(())`, `get(k)` returns `Ok(Some(v))`; for any never-set key, `get` returns `Ok(None)`; no plaintext file under the application data directory contains the literal bytes of any `value`; the backing entry is namespaced under `"remote-code-rust"`.
    - Use a fake `SecureStoreBackend` to keep the test hermetic; cross-check with a real `KeyringBackend` on desktop runners only.
    - Place at `apps/remote-code-gui/src-tauri/src/secure_store/tests.rs` under name `prop_secure_store_round_trip_and_no_plaintext`.

- [ ] 4. Front-end bundle scrub
  - [ ] 4.1 Remove `VITE_REMOTE_CONTROL_PLANE_TOKEN` from the front-end source tree
    - Delete every `import.meta.env.VITE_REMOTE_CONTROL_PLANE_TOKEN` reference under `apps/remote-code-gui/src/`.
    - Replace each call site with a Tauri-invoked authenticated request (see 4.2).
    - _Requirements: 5.1, 5.2_

  - [ ] 4.2 Add `control_plane_authenticated_request` Tauri command
    - In `apps/remote-code-gui/src-tauri/src/desktop.rs`, add a new Tauri command whose Rust side reads the token from the OS environment or the runtime config and forwards the request with the `Authorization: Bearer …` header.
    - Update the front-end TS client to call the new command instead of fetching directly.
    - _Requirements: 5.3_

  - [ ]* 4.3 Front-end bundle scrub test
    - Run `rg --no-heading 'VITE_REMOTE_CONTROL_PLANE_TOKEN' apps/remote-code-gui/src/` from a CI-invoked test that fails on any match.
    - Add a post-`vite build` scan of the emitted `dist/` directory for the literal `VITE_REMOTE_CONTROL_PLANE_TOKEN` substring and any test-injected token value; fail on hit.
    - _Requirements: 5.1, 5.4, 5.5_

- [ ] 5. MCP project-path guard
  - [ ] 5.1 Implement `validate_managed_project` and `McpCommandError`, apply to every MCP Tauri command
    - In `apps/remote-code-gui/src-tauri/src/desktop.rs`, define `McpCommandError { ProjectNotManaged { name }, Internal(String) }`.
    - Implement `validate_managed_project(project_path, &runtime.projects) -> Result<&ProjectEntry, McpCommandError>` using the existing `path_identity` helper.
    - Call the validator from every MCP command (`mcp_servers_list`, `mcp_servers_save`, `mcp_servers_delete`, `mcp_servers_toggle`, `mcp_servers_reset`, and the internal `mcp_config_path_for_scope`) before any disk I/O.
    - On rejection, emit `tracing::warn!(command, redacted_path, reason = "project_not_managed")` where `redacted_path` is only the file-name component.
    - _Requirements: 6.1, 6.2, 6.3, 6.5_

  - [ ]* 5.2 Property test for MCP project-path guard (Property 5)
    - **Property 5: MCP project-path guard**
    - **Validates: Requirements 6.1, 6.2, 6.5**
    - For any MCP Tauri command and any `project_path`, assert: when the path canonicalises to a managed project, the command proceeds; otherwise it returns `McpCommandError::ProjectNotManaged { name }` (`name` equals only the file-name component), no disk read or write occurs, and exactly one `tracing::warn!` event fires.
    - Place at `apps/remote-code-gui/src-tauri/src/mcp/tests.rs` under name `prop_mcp_project_path_guard_rejects_unmanaged`.

  - [ ]* 5.3 Unit tests per MCP command (one accepted, one rejected)
    - For `mcp_servers_list`, `mcp_servers_save`, `mcp_servers_delete`, `mcp_servers_toggle`, and `mcp_servers_reset`, exercise both an accepted (managed) and a rejected (non-managed) `project_path` and assert the expected outcome.
    - _Requirements: 6.4_

- [ ] 6. Runner path validator
  - [ ] 6.1 Promote `claude-permissions::path_validation` and add `RunnerPathValidator` wrapper
    - Widen the visibility of `claude-permissions::path_validation::{PathValidation, validate_path}` so the runner crate can consume the same rules.
    - Create `apps/remote-code-runner/src/path_guard.rs` with `RunnerPathValidator { workspace_root: PathBuf }`, `RunnerPathValidator::check<P: AsRef<Path>>(&self, target: P) -> Result<PathBuf, RunnerPathError>` using `dunce::canonicalize`, and `RunnerPathError::NotInWorkspace`.
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [ ] 6.2 Wire `RunnerPathValidator` into every runner file-op site
    - Apply the validator before any session-create, artifact-upload, or file-read code path inside `apps/remote-code-runner/src/`.
    - Map `RunnerPathError::NotInWorkspace` to `HTTP 400` with a typed JSON body and emit `tracing::warn!` with the redacted (file-name only) path.
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ]* 6.3 Property test for runner workspace-path containment (Property 6)
    - **Property 6: Runner workspace-path containment**
    - **Validates: Requirements 7.1, 7.2, 7.3**
    - For any `p` and any configured workspace root `r`, assert `RunnerPathValidator::check(p)` returns `Ok(canonical)` iff `dunce::canonicalize(p)` succeeds and is a descendant of `dunce::canonicalize(r)`; otherwise returns `RunnerPathError::NotInWorkspace`. Generators must produce `..` traversals, symlinks pointing in/out, UNC prefixes, and non-existent paths.
    - Place at `apps/remote-code-runner/src/path_guard.rs` test module under name `prop_runner_path_validator_descendancy`.

  - [ ]* 6.4 Unit tests for runner path validator edge cases
    - Cover (per Requirement 7.5): a path inside the workspace root (accepted), a path outside (rejected), a `..` traversal that re-enters the workspace root (accepted), a symlink that escapes (rejected), and a path that does not exist (rejected).
    - _Requirements: 7.5_

  - [ ]* 6.5 Criterion bench for runner path validator latency
    - Add `apps/remote-code-runner/benches/path_guard.rs` measuring median per-call wall-clock for in-workspace paths; assert median is well under 1 ms.
    - _Requirements: 7.6_

- [ ] 7. Transport URL validator integration
  - [ ] 7.1 Wire `validate_transport_url` into the Tauri `connect_runner` command
    - In `apps/remote-code-gui/src-tauri/src/desktop.rs`, call `validate_transport_url` before any WebSocket/SSE construction and surface the typed error as a Tauri error variant the front-end renders as a toast.
    - _Requirements: 8.3, 8.5_

  - [ ] 7.2 Wire `validate_transport_url` into the CLI transport
    - Apply the validator at every WebSocket/SSE construction site in `apps/remote-code/src/remote/ws.rs` and `apps/remote-code/src/remote/sse.rs`.
    - Map `TransportError::PlaintextNotAllowed` to CLI exit code 64 with a one-line message naming the host.
    - _Requirements: 8.3, 8.5_

  - [ ] 7.3 Wire the validator into the front-end transport layer
    - In `apps/remote-code-gui/src/lib/transport.ts`, route every transport URL through a Tauri command that delegates to `validate_transport_url`.
    - Surface the error as a user-facing toast naming the host and recommending `wss://` or `https://`.
    - _Requirements: 8.3, 8.5_

- [ ] 8. CORS hardening
  - [ ] 8.1 Update the control-plane CORS layer with deny-by-default
    - In `claude-control-plane::auth`, build `CorsLayer` from `control_plane.cors.allowed_origins`. Empty list in a release build → `CorsLayer` that emits no `Access-Control-Allow-Origin` header on cross-origin responses, plus exactly one startup `tracing::warn!(component="cors", action="deny_all_cross_origin")`.
    - Under `cfg(debug_assertions)` or feature `dev-cors`, allow `http://localhost:*` and `http://127.0.0.1:*` patterns by default.
    - Reject `*` as a configurable value.
    - _Requirements: 9.2, 9.3, 9.4_

  - [ ] 8.2 Implement a runner CORS module mirroring the same logic
    - Create `apps/remote-code-runner/src/cors.rs` with `build_cors(origins: &[String]) -> CorsLayer` keyed on `runner.cors.allowed_origins` and apply the same deny-by-default rule.
    - Wire the module into the runner HTTP server bootstrap.
    - _Requirements: 9.1, 9.3, 9.4_

  - [ ]* 8.3 Property test for CORS allowed-origin configuration (Property 8)
    - **Property 8: CORS allowed-origin configuration**
    - **Validates: Requirements 9.1, 9.2, 9.3**
    - For any `origins` list, assert the resulting `CorsLayer` allows exactly those origins (with optional debug-only loopback wildcards) and never `"*"` by default. Assert that an empty `origins` in a release build produces no `Access-Control-Allow-Origin` header on cross-origin responses and emits exactly one `warn!(component="cors", action="deny_all_cross_origin")`.
    - Place at `apps/remote-code-runner/src/cors.rs` test module under name `prop_cors_layer_matches_config`.

  - [ ]* 8.4 Release-profile integration test for CORS
    - Build the runner and control plane in release profile, send a request from `Origin: https://attacker.example`, and assert the response carries no `Access-Control-Allow-Origin`. Send the same request from a configured allowed origin and assert success.
    - _Requirements: 9.5_

- [ ] 9. Agent binary integrity
  - [ ] 9.1 Update `scripts/build-agents.{ps1,sh}` to emit and dry-run the manifest
    - Append, to each successful agent build, a manifest entry `{ kind, target_triple, path, sha256, size_bytes }` to `target/agent-binaries/manifest.json` (read, mutate, write atomically).
    - Add a `--dry-run` mode that emits the entry without invoking `cargo build` (used by CI to validate the emission code path).
    - _Requirements: 10.1, 10.2_

  - [ ] 9.2 Replace the GUI subprocess spawn with `AgentBinaryLauncher::launch`
    - In `apps/remote-code-gui/src-tauri/src/desktop.rs`, replace the existing `Command::spawn(...)` agent-binary path with `AgentBinaryLauncher::launch(kind, &binary)`.
    - Surface `AgentLaunchError` to the front-end as the existing Tauri error envelope and log `tracing::error!(?kind, ?target, "integrity check failed")` on failure.
    - _Requirements: 10.3, 10.4, 10.5_

  - [ ]* 9.3 Manifest dry-run schema validation test
    - Run `scripts/build-agents.sh --dry-run` (and the PowerShell equivalent on Windows runners) and validate the emitted JSON against the documented schema (`version: u32 == 1`, required entry fields).
    - _Requirements: 10.6_

- [ ] 10. MCP TLS validation
  - [ ] 10.1 Add the `insecure: bool` field to `McpServerEntry` deserialisation
    - Update the `McpServerEntry` struct in the MCP config crate so `insecure` defaults to `false` (`#[serde(default)]`).
    - Confirm existing configs without the field round-trip unchanged (Requirement 11.4).
    - _Requirements: 11.4, 17.6_

  - [ ] 10.2 Update `claude-mcp` HTTP/WS clients to use `rustls-tls-native-roots` and the `insecure` flag
    - Update the shared `reqwest::Client` and `tokio_tungstenite::connect_async` builders so TLS validation runs against the system trust store by default.
    - When `insecure == true`, build a `dangerous_accept_invalid_certs(true)` client and emit `tracing::warn!(server, "tls verification disabled — connection is insecure")` per connection attempt.
    - Leave the stdio transport unchanged.
    - _Requirements: 11.1, 11.2, 11.5_

  - [ ] 10.3 Add the GUI badge in `McpServerList`
    - Update `apps/remote-code-gui/src/components/McpServerList.tsx` to render `<Badge variant="warning" aria-label="TLS verification disabled">TLS verification disabled</Badge>` next to any server with `insecure: true`. Make the badge non-dismissible.
    - _Requirements: 11.3_

  - [ ]* 10.4 Property test for MCP TLS validation (Property 10)
    - **Property 10: MCP TLS validation**
    - **Validates: Requirements 11.1, 11.2, 11.4**
    - For any TLS config `(cert, insecure)`: when `insecure == false`, the connection succeeds iff `cert` validates; when `insecure == true`, succeeds regardless and emits exactly one `warn!` per attempt naming the server. Absent `insecure` deserialises to `false`.
    - Place at `crates/claude/claude-mcp/tests/tls.rs` under name `prop_mcp_tls_validation_and_warn`.

  - [ ]* 10.5 Integration tests for MCP TLS scenarios
    - Self-signed server with `insecure: false` (rejected); same server with `insecure: true` (accepted with warn); valid certificate chain with `insecure: false` (accepted, no warn).
    - _Requirements: 11.6_

- [ ] 11. Checkpoint - Security hardening complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 12. Roo permission unification
  - [ ] 12.1 Implement `RooPermissionBroker` in `rc-roo-adapter`
    - Create `crates/adapters/rc-roo-adapter/src/permission_broker.rs` with `RooPermissionBroker { inflight, session_rules }`.
    - Implement `request(session_id, PermissionResolutionRequest) -> Result<PermissionDecision, RooPermissionError>`: check `session_rules` first; otherwise register a `oneshot`, emit `UnifiedAgentEvent::PermissionRequest`, and await with up to three `AskAgain` retries.
    - Implement `resolve(session_id, request_id, decision)` to fan the decision back to the awaiting `oneshot`.
    - Decision semantics: `Allow` → resolve current call only; `AllowAll` → record `(tool_name, input_shape_hash)` in `session_rules`; `Deny` → fail with `PermissionError::Denied`; `AskAgain` → drop resolver, wait for `state_updated`, re-emit (max 3 retries → `PermissionError::Exhausted`).
    - Define `RooPermissionError { Denied, Exhausted, Cancelled, Internal(String) }`.
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 12.6_

  - [ ] 12.2 Wire `RooPermissionBroker` into `RooInProcessAdapter`
    - Rewrite `RooInProcessAdapter::resolve_permission(...)` so its public boundary returns `PermissionDecision` (no `oneshot::Sender<bool>`).
    - Add `RooInProcessAdapter::with_permission_broker<B: GuiRuntimePermissionBroker + 'static>(self, broker: Arc<B>) -> Self` builder.
    - When the runtime is not the GUI, fall back to the same default broker policy used by Claude and Codex (auto-allow read tools per permission mode, prompt on stdin for interactive runtimes, deny otherwise).
    - _Requirements: 12.1, 13.1, 13.5_

  - [ ]* 12.3 Property test for `PermissionDecision::Allow` (Property 11)
    - **Property 11: `Allow` allows exactly one call**
    - **Validates: Requirements 12.3**
    - For any tool `t`, input `i`, and `n >= 1` equivalent calls in sequence: receiving `Allow` for the first call produces exactly `n` `PermissionResolutionRequest` events; the first call resolves allowed; `session_rules` is unchanged.
    - Place at `crates/adapters/rc-roo-adapter/src/permission_broker.rs` test module under name `prop_allow_allows_exactly_one_call`.

  - [ ]* 12.4 Property test for `PermissionDecision::AllowAll` (Property 12)
    - **Property 12: `AllowAll` records a session rule**
    - **Validates: Requirements 12.4**
    - For any tool `t`, input shape `i`, and `n >= 1` equivalent subsequent calls: receiving `AllowAll` for the first call produces exactly one `PermissionResolutionRequest` in total; subsequent equivalent calls resolve allowed without re-prompting.
    - Place in the same file under name `prop_allow_all_records_session_rule`.

  - [ ]* 12.5 Property test for `PermissionDecision::Deny` (Property 13)
    - **Property 13: `Deny` blocks the call without altering rules**
    - **Validates: Requirements 12.5**
    - For any tool `t`, input `i`, and `n >= 1` equivalent subsequent calls: receiving `Deny` for the first call produces `PermissionError::Denied`; each of the `n - 1` subsequent equivalent calls re-emits a fresh `PermissionResolutionRequest`; `session_rules` is unchanged.
    - Place in the same file under name `prop_deny_blocks_and_does_not_mutate_rules`.

  - [ ]* 12.6 Property test for `PermissionDecision::AskAgain` (Property 14)
    - **Property 14: `AskAgain` retries up to three times**
    - **Validates: Requirements 12.6**
    - For any sequence of `m` consecutive `AskAgain` responses to the same request followed by terminal decision `d`: when `m <= 3`, the request resolves with `d`; when `m > 3`, the request fails with `PermissionError::Exhausted` and emits no further re-emissions.
    - Place in the same file under name `prop_ask_again_bounded_retries`.

  - [ ]* 12.7 Unit tests for each `PermissionDecision` variant
    - One deterministic example per variant covering Requirements 12.3 through 12.6 alongside the property tests.
    - Place at `crates/adapters/rc-roo-adapter/tests/permission_examples.rs`.
    - _Requirements: 12.7_

- [ ] 13. Roo GUI permission dialog wiring
  - [ ] 13.1 Update the GUI `PermissionDialog` for the Roo agent identifier
    - In `apps/remote-code-gui/src/components/PermissionDialog.tsx`, render the `agent` field with value `roo` alongside Claude and Codex.
    - Forward the user's `PermissionDecision` value (now including `ask_again`) unchanged.
    - _Requirements: 13.2, 13.3_

  - [ ] 13.2 Implement window-close timeout and non-GUI fallback in the GUI permission broker
    - In the `GuiRuntimePermissionBroker` implementation (Tauri side), when the dialog is unresolved and the configurable timeout `RC_PERMISSION_TIMEOUT_SECS` (default 60 seconds) elapses, resolve as `PermissionDecision::Deny` and surface that to the adapter.
    - When the GUI is not the active runtime (CLI / headless / automation), instantiate the same default broker policy used by Claude and Codex.
    - _Requirements: 13.4, 13.5_

  - [ ]* 13.3 Integration test for Roo + GUI broker dialog flow
    - Drive `RooInProcessAdapter::with_permission_broker(...)` against a fake `GuiRuntimePermissionBroker` and assert (a) one approval dialog rendered per tool call, (b) `AllowAll` suppresses subsequent dialogs for the same tool name, (c) the window-close path resolves to `Deny` after the configured timeout.
    - _Requirements: 13.6_

- [ ] 14. Roo token counter via `tiktoken-rs`
  - [ ] 14.1 Implement `RooTokenCounter` with the model-encoding map
    - Create `crates/adapters/rc-roo-adapter/src/token_counter.rs` with `RooTokenCounter::for_model(provider, model) -> Self` selecting `cl100k_base` or `o200k_base` per the design's mapping (OpenAI-compatible GPT-4o/4.1 → `o200k_base`; OpenAI-compatible GPT-3.5/4 → `cl100k_base`; Anthropic-compatible → `cl100k_base` plus `tracing::debug!`; unknown → `cl100k_base` plus `tracing::debug!`).
    - Add `count(&[ApiMessage]) -> usize` returning `0` on empty input and otherwise summing `encode_with_special_tokens(...)` over messages.
    - Add the `provider+model → encoding` table at `crates/adapters/rc-roo-adapter/src/token_counter/model_map.rs`.
    - _Requirements: 14.1, 14.2, 14.3, 14.6_

  - [ ] 14.2 Wire `RooTokenCounter` into `RooInProcessAdapter`
    - Initialise `Arc<RooTokenCounter>` at session start from the active provider config.
    - Set `UnifiedAgentEvent::ContextWindowUpdate::tokens_used` to `counter.count(&conversation_history)` on every emission.
    - _Requirements: 14.4_

  - [ ]* 14.3 Property test for Roo token counting (Property 16)
    - **Property 16: Roo token counting matches `tiktoken-rs`**
    - **Validates: Requirements 14.1, 14.2, 14.4, 14.6**
    - For any sequence of `ApiMessage` and any `(provider, model)` pair selecting encoding `enc`: assert `RooTokenCounter::for_model(provider, model).count(&messages)` equals `messages.iter().map(|m| enc.encode_with_special_tokens(&m.text()).len()).sum()`. Empty `messages` → `0`.
    - Place at `crates/adapters/rc-roo-adapter/src/token_counter.rs` test module under name `prop_roo_token_counter_matches_tiktoken`.

  - [ ]* 14.4 Hand-computed token-counter unit tests
    - For at least three representative provider+model pairs (one OpenAI, one Anthropic, one MiniMax), compare the count returned by `RooTokenCounter` against a hand-computed `tiktoken-rs` reference and assert exact equality.
    - Place at `crates/adapters/rc-roo-adapter/tests/token_counter_examples.rs`.
    - _Requirements: 14.5_

- [ ] 15. Roo MCP bridge
  - [ ] 15.1 Implement `RooMcpBridge` and tool-dispatcher integration
    - Create `crates/adapters/rc-roo-adapter/src/mcp_bridge.rs` with `RooMcpBridge { connections: Vec<McpServerConnection> }` and `append_to_dispatcher(&self, dispatcher: &mut ToolDispatcher)`.
    - Each tool is registered as `mcp::<server>::<tool>`; routing strips the prefix before forwarding to the connection.
    - On `AgentLoop` invocation of an `mcp::`-prefixed tool, route through the matching connection and propagate structured results / errors byte-for-byte unchanged.
    - _Requirements: 15.1, 15.2, 15.3_

  - [ ] 15.2 Wire MCP loading at session start and update `McpSupport` advertising
    - In `RooInProcessAdapter`, consume `build_mcp_server_entries()` (already passed via `set_external_mcp_servers`), construct `RooMcpBridge`, and call `append_to_dispatcher` at session start.
    - On a server failing to connect at session start, emit `tracing::warn!(server, "mcp connect failed")` and let the session continue with the remaining tools.
    - Update `RooInProcessAdapter::info()` so the advertised `McpSupport` capability is backed by the live bridge with `provider: "roo_mcp_bridge"` recorded in capability metadata.
    - _Requirements: 15.4, 15.5_

  - [ ]* 15.3 Property test for Roo MCP bridge naming and routing (Property 15)
    - **Property 15: Roo MCP-tool naming and routing**
    - **Validates: Requirements 15.2, 15.3**
    - For any MCP server `s` with a tool named `tool_name` and any input `i`: `ToolDispatcher::list_tools()` includes `format!("mcp::{}::{}", s, tool_name)`; invoking the prefixed tool causes the matching `McpServerConnection` to receive an unprefixed request with input `i`; structured result / error round-trips unchanged.
    - Place at `crates/adapters/rc-roo-adapter/src/mcp_bridge.rs` test module under name `prop_mcp_bridge_naming_and_routing`.

  - [ ]* 15.4 Roo + MCP integration test (`mcp-server-stub`)
    - Under `cargo test --features mcp-stub`, spawn the in-tree `mcp-server-stub`, attach Roo, list tools, invoke one tool, and assert the structured result reaches the test harness. If `mcp-server-everything` is reachable, run an additional flavour against it.
    - If neither server is present, skip with a clear `eprintln!` instead of failing.
    - _Requirements: 15.6_

- [ ] 16. Checkpoint - Roo deepening core complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 17. Multi-agent E2E test harness
  - [ ] 17.1 Scaffold `tests/multi_agent_e2e/` workspace test crate
    - Create `tests/multi_agent_e2e/{Cargo.toml, src/lib.rs, README.md}` and `tests/multi_agent_e2e/tests/multi_agent_e2e.rs`. Add the path to root `Cargo.toml` `[workspace.members]`.
    - Declare dev-dependencies on all three adapters and `proptest`.
    - _Requirements: 16.7_

  - [ ] 17.2 Implement the hermetic `MockProvider`
    - Add `tests/multi_agent_e2e/src/mock_provider.rs` exposing a deterministic provider that emits canned responses for the four shared scenarios.
    - Default provider; opt into a real provider via `RC_E2E_REAL_PROVIDER=1`.
    - _Requirements: 16.3, 16.4_

  - [ ] 17.3 Implement `scenarios.rs` (four scenarios) and `ScenarioSpec`
    - Add `tests/multi_agent_e2e/src/scenarios.rs` defining `Scenario { SimplePrompt, ToolCallWithPermission, McpToolCall, Cancellation }` and the matching `ScenarioSpec { name, expected_kinds, agent_specific_exemptions }`.
    - Document the per-scenario expected event-kind sequences with `ContextCompacted` (Claude) and `CodexAppServerNotification` (Codex) tolerated as agent-specific exemptions.
    - _Requirements: 16.1, 16.2, 25.1, 25.2_

  - [ ] 17.4 Implement `assertions.rs` event-sequence diff
    - Add `tests/multi_agent_e2e/src/assertions.rs` producing a unified diff between observed and expected event sequences, including agent identifier and scenario name in failure messages.
    - Assert every `ToolCallStarted` carries `tool_name`, `tool_input`, `call_id`, and `agent_kind`.
    - _Requirements: 16.5, 25.3_

  - [ ] 17.5 Wire all three adapters into the harness entry point
    - In `tests/multi_agent_e2e/tests/multi_agent_e2e.rs`, run each scenario against `ClaudeInProcessAdapter`, `CodexInProcessAdapter`, and `RooInProcessAdapter`, asserting the observed event-kind sequence equals the per-scenario `ScenarioSpec`.
    - Cap wall-clock at 60 seconds in mock-provider mode.
    - Make the test invokable through `cargo test --workspace --test multi_agent_e2e`.
    - _Requirements: 16.6, 16.7_

  - [ ]* 17.6 Property test for three-agent event parity (Property 17)
    - **Property 17: Three-agent event parity for shared scenarios**
    - **Validates: Requirements 16.2, 25.1, 25.3**
    - For any scenario `s` ∈ {`simple_prompt`, `tool_call_with_permission`, `mcp_tool_call`, `cancellation`} and any adapter `a` ∈ {Claude, Codex, Roo}: the ordered sequence of `UnifiedAgentEvent` variant kinds emitted by `a` running `s` equals the per-scenario reference sequence, modulo documented exemptions. Every `ToolCallStarted` carries the four required fields.
    - Place at `tests/multi_agent_e2e/tests/multi_agent_e2e.rs` under name `prop_event_kind_sequence_matches_per_scenario_spec`.

  - [ ]* 17.7 Per-scenario hand-authored example tests
    - Add deterministic example tests covering each (scenario × agent) combination with hand-authored expected event-kind sequences.
    - _Requirements: 16.1, 16.5_

- [ ] 18. Compatibility property test
  - [ ]* 18.1 Property test for new optional JSON fields (Property 4)
    - **Property 4: New JSON fields are optional**
    - **Validates: Requirements 17.6**
    - For any JSON payload of an existing `COMPATIBILITY.md`-documented shape (CLI flag set, env var output, `stream-json` message family, `/v1` REST request/response, NDJSON transcript record), removing every field added by this feature still produces a payload the receiving deserialiser accepts and that yields the documented default for each missing field.
    - Place at `tests/compatibility/optional_fields.rs` under name `prop_new_optional_fields_deserialise_when_absent`.

- [ ] 19. Performance benchmarks (NFR-5, NFR-6)
  - [ ]* 19.1 Criterion bench for Claude in-process startup latency
    - Add `crates/adapters/rc-claude-adapter/benches/startup.rs` measuring `RemoteClaudeAdapter::new` → first `UnifiedAgentEvent::Ready` median wall-clock. Fail CI when the median exceeds 100 ms or regresses by more than 10 % vs the recorded baseline.
    - _Requirements: 21.1, 21.2_

  - [ ]* 19.2 GUI idle RSS measurement script
    - Add `scripts/measure-gui-rss.{ps1,sh}` that launches the GUI, idles for 10 s, samples RSS via `Get-Process` / `ps`, and asserts < 60 MB and ≤ 10 % regression vs baseline.
    - _Requirements: 22.1, 22.2_

- [ ] 20. Documentation updates
  - [ ] 20.1 Update `AUDIT_CHECKLIST.md` with lint deny/warn justifications
    - Document which crate paths are at `deny` and which remain at `warn`, with a one-line justification per `warn` exception (notably `crates/codex/*`).
    - _Requirements: 1.5_

  - [ ] 20.2 Add the "Control-plane authentication" section to `COMPATIBILITY.md`
    - Document the env var `REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN`, the header format `Authorization: Bearer <token>`, and the rotation procedure.
    - Document any new optional JSON fields introduced by this feature.
    - _Requirements: 4.5, 17.6_

  - [ ] 20.3 Update `apps/remote-code-gui/README.md` with the front-end secrets rule
    - Add a note stating "no front-end environment variable may carry a server-trust credential" and pointing operators at the Tauri-mediated path.
    - _Requirements: 5.5_

  - [ ] 20.4 Append the "Event parity matrix" to `ARCHITECTURE.md`
    - Enumerate the `UnifiedAgentEvent` variants covered by parity and document the `ContextCompacted` (Claude) and `CodexAppServerNotification` (Codex) exemptions.
    - _Requirements: 25.2_

- [ ] 21. CI workflow updates
  - [ ] 21.1 Add the front-end VITE_ scan CI step
    - Append a step to `.github/workflows/ci.yml` that runs `rg --no-heading 'VITE_REMOTE_CONTROL_PLANE_TOKEN' apps/remote-code-gui/src/` and fails the build on any hit.
    - _Requirements: 5.4_

  - [ ] 21.2 Add the manifest dry-run CI step
    - Append a step to `.github/workflows/ci.yml` that runs the build script in `--dry-run` mode and validates the emitted JSON against the documented schema.
    - _Requirements: 10.6_

  - [ ] 21.3 Add the log-secret-pattern scan CI step (NFR-7)
    - Append a step that runs `rg --no-heading 'sk-[A-Za-z0-9]{16,}|ghp_[A-Za-z0-9]{16,}|xoxb-[A-Za-z0-9]+'` over the latest CI log artefact directory and fails on any hit.
    - _Requirements: 23.2_

- [ ] 22. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional (test-only) and can be skipped for faster MVP, but every Property 1–18 has a dedicated test sub-task to maintain traceability.
- Each task references the granular requirement clause it satisfies (for example `_Requirements: 6.1, 6.2, 6.5_`), not just the top-level user story.
- Property test sub-tasks include the property's title in bold and the requirement clauses they validate.
- Two checkpoints (Tasks 11 and 16) and a final checkpoint (Task 22) provide review gates after the security hardening, Roo deepening, and full feature respectively.
- Property tests use `proptest` configured with `cases: 256` and the `// Feature: p0-security-and-roo-completion, Property N: ...` doc comment so each property can be located by grep.
- The dependency graph respects same-file constraints: tasks that touch the same file (notably `apps/remote-code-gui/src-tauri/src/desktop.rs` and `crates/adapters/rc-roo-adapter/src/adapter.rs`) are sequenced into separate waves.

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.10"] },
    { "id": 1, "tasks": ["1.2", "1.4", "1.6", "1.8"] },
    { "id": 2, "tasks": ["1.3", "1.5", "1.7", "1.9", "1.11"] },
    { "id": 3, "tasks": ["1.12", "2.1", "2.2", "2.3", "2.4", "2.5", "3.1", "4.1", "5.1", "6.1", "7.2", "8.1", "8.2", "9.1", "10.1"] },
    { "id": 4, "tasks": ["2.6", "3.2", "4.2", "5.2", "5.3", "6.2", "7.3", "8.3", "8.4", "10.2", "10.3"] },
    { "id": 5, "tasks": ["2.7", "3.3", "4.3", "6.3", "6.4", "6.5", "7.1", "9.3", "10.4", "10.5"] },
    { "id": 6, "tasks": ["9.2", "12.1", "14.1", "15.1"] },
    { "id": 7, "tasks": ["12.2"] },
    { "id": 8, "tasks": ["14.2"] },
    { "id": 9, "tasks": ["15.2"] },
    { "id": 10, "tasks": ["12.3", "12.7", "13.1", "13.2", "14.3", "14.4", "15.3", "15.4"] },
    { "id": 11, "tasks": ["12.4"] },
    { "id": 12, "tasks": ["12.5"] },
    { "id": 13, "tasks": ["12.6", "13.3", "17.1"] },
    { "id": 14, "tasks": ["17.2", "17.3", "17.4"] },
    { "id": 15, "tasks": ["17.5"] },
    { "id": 16, "tasks": ["17.6", "17.7", "18.1", "19.1", "19.2", "20.1", "20.2", "20.3", "20.4", "21.1", "21.2", "21.3"] }
  ]
}
```
