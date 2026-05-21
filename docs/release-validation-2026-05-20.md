# Release Validation Snapshot - 2026-05-20

This snapshot records the local evidence produced on 2026-05-20 in
`D:\remote-code-rust`. It is not a production sign-off by itself; the
requirements in [requirements.md](requirements.md) remain authoritative.

For the latest full `-RequireComplete` release acceptance run, see
[release-validation-2026-05-21.md](release-validation-2026-05-21.md).

## Automated Gates Run

| Area | Result | Evidence |
| --- | --- | --- |
| Rust format | PASS | `cargo fmt --all -- --check` |
| Git whitespace | PASS | `git diff --check` |
| Claude check slice | PASS | `python scripts\cargo_workspace_slice.py check claude` |
| Claude clippy slice | PASS | `python scripts\cargo_workspace_slice.py clippy claude` |
| Claude test slice | PASS | `python scripts\cargo_workspace_slice.py test claude` |
| Apps-shared check slice | PASS | `python scripts\cargo_workspace_slice.py check apps-shared` |
| Focused MCP cleanup tests | PASS | `cargo test -p claude-mcp --lib -- --nocapture` |
| Runner focused tests | PASS | `cargo test -p remote-code-runner --all-targets -j 1 -- --nocapture` |
| Control-plane focused tests | PASS | `cargo test -p claude-control-plane --all-targets -j 1 -- --nocapture` |
| QUIC transport E2E | PASS | `cargo test -p claude-control-plane --test quic_transport -- --nocapture`; covers cert fingerprint pinning, timeline event, prompt, approval |
| Transport unit gates | PASS | `cargo test -p rc-remote-transport --features quic --lib -- --nocapture` |
| Remote/mobile/transport acceptance subset | PASS | `scripts\acceptance-release.ps1 -IncludeRemoteE2E -IncludeMobilePwaE2E -IncludeTransportE2E -IncludeTailscaleE2E`; evidence `.release-evidence\20260520-205850` |
| RustSec audit | PASS | `cargo audit --quiet` with proxy, exit code 0 |
| Secret scan | PASS | `gitleaks detect --source /repo --redact --no-git` against a temporary copy of Git-tracked files plus this change set |
| GUI dependency audit | PASS | `npm audit --audit-level=moderate --registry=https://registry.npmjs.org/` |
| GUI tests | PASS | `npm test` |
| GUI build | PASS | `npm run build` |
| Windows desktop bundle | PASS | `npm run desktop:build`; `target\release\bundle\nsis\Remote Code_0.1.0_x64-setup.exe`, 60,023,498 bytes, 2026-05-20 20:27 local |

## Provider Acceptance

Ran in `C:\Users\Yanzh\Desktop\cli-stress-test` with provider keys supplied
only through process environment variables. Logs were written under ignored
`.release-evidence/` and sanitized by `scripts\acceptance-release.ps1`.
The latest rerun was `.release-evidence\20260520-210153`.

| Provider | Model | Protocol | Result |
| --- | --- | --- | --- |
| MiniMax Token Plan | `minimax-m2.7` | Anthropic-compatible | PASS |
| KuaiKAT Coding Plan | `kat-coder-pro-v2` | Anthropic-compatible | PASS |
| DeepSeek | `deepseek-v4-flash` | Anthropic-compatible | PASS |

## MCP Acceptance

Ran with `scripts\acceptance-release.ps1 -IncludeMcpMatrix -UseProxy`.
The latest rerun was `.release-evidence\20260520-210153`; MiniMax now uses a
real `web_search` tool call rather than treating a second discovery as a call.

| MCP server | Discovery | Tool call | Result |
| --- | --- | --- | --- |
| MiniMax | PASS | PASS | PASS |
| context7 | PASS | PASS | PASS |
| sequentialthinking | PASS | PASS | PASS |
| memory | PASS | PASS | PASS |
| puppeteer | PASS | PASS | PASS |

The puppeteer MCP initially exposed a shutdown bug: the tool returned a
successful navigation response, but the stdio child process and browser stayed
alive. `claude-mcp` now terminates stdio MCP process trees on Windows and caps
shutdown wait time, then the matrix passed.

## Relay Host Audit

Ran against `remote-code.yz520gzy.top` on 2026-05-20 after installing
`deploy/tencent-cloud/audit-relay-host.sh` on the host and setting
`REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY=true`.

| Check | Result |
| --- | --- |
| Public `/healthz` | PASS: `ok=true`, `auth_required=true`, `bootstrap_secret_configured=true`, `owner_claimed=true` |
| Systemd boundary | PASS: only `remote-code-control-plane.service` running |
| Process boundary | PASS: no runner, CLI, cargo/rustc, or agent process detected |
| Source boundary | PASS: no Rust source tree under `/opt/remote-code` |
| Secret boundary | PASS: env file contains no provider, cloud, or MCP key variables |
| Query-token legacy switches | PASS: disabled or unset |
| Audit summary | PASS: 13 pass, 0 warning, 0 failure |

## Release Boundary Enforcement

`scripts\acceptance-release.ps1` now supports `-RequireComplete`. In that mode
any `FAIL`, `SKIP`, or `MANUAL` status is release-blocking; disabled optional
Tailscale closes as `N/A`. Relay host evidence must be supplied with
`-RelayHostAuditReport`, and an enabled Tailscale path must supply
`-TailscaleEvidenceReport`. Public release notes must attach the redacted
evidence bundle for the exact commit being released.
