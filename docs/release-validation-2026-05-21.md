# Release Validation Snapshot - 2026-05-21

This snapshot records the full local release acceptance run produced on
2026-05-21 in `D:\remote-code-rust`. Secrets were supplied only through process
environment variables and are not stored in this document.

## Full Acceptance Command

```powershell
powershell -ExecutionPolicy Bypass -File scripts\acceptance-release.ps1 `
  -RunBaseGates `
  -IncludeWorkspaceTests `
  -IncludeDesktopBundle `
  -IncludeProviderMatrix `
  -IncludeMcpMatrix `
  -IncludeRemoteE2E `
  -IncludeMobilePwaE2E `
  -IncludeTransportE2E `
  -IncludeTailscaleE2E `
  -RelayHostAuditReport .release-evidence\relay-host-audit-20260520.txt `
  -RequireComplete `
  -UseProxy
```

Result: PASS, evidence root `.release-evidence\20260521-122522`.

## Acceptance Matrix

| Area | Item | Result | Evidence |
| --- | --- | --- | --- |
| 14.1 | Base gates | PASS | `.release-evidence\20260521-122522\logs\14-1-base-gates.log` |
| Provider | MiniMax Token Plan / `minimax-m2.7` | PASS | `.release-evidence\20260521-122522\logs\provider-minimax-token-plan.log` |
| Provider | KuaiKAT Coding Plan / `kat-coder-pro-v2` | PASS | `.release-evidence\20260521-122522\logs\provider-kuaikat-coding.log` |
| Provider | DeepSeek / `deepseek-v4-flash` | PASS | `.release-evidence\20260521-122522\logs\provider-deepseek-anthropic.log` |
| MCP | MiniMax discovery + tool call | PASS | `.release-evidence\20260521-122522\logs\mcp-minimax-*.log` |
| MCP | context7 discovery + tool call | PASS | `.release-evidence\20260521-122522\logs\mcp-context7-*.log` |
| MCP | sequentialthinking discovery + tool call | PASS | `.release-evidence\20260521-122522\logs\mcp-sequentialthinking-*.log` |
| MCP | memory discovery + tool call | PASS | `.release-evidence\20260521-122522\logs\mcp-memory-*.log` |
| MCP | puppeteer discovery + tool call | PASS | `.release-evidence\20260521-122522\logs\mcp-puppeteer-*.log` |
| Remote E2E | Control-plane + runner local loop | PASS | `.release-evidence\20260521-122522\logs\remote-e2e-control-plane-runner-local.log` |
| Mobile/PWA | Pairing, prompt, approval, artifact flow | PASS | `.release-evidence\20260521-122522\logs\mobile-pwa-pairing-prompt-approval-artifact.log` |
| Transport | Relay, direct/outbound, QUIC gate | PASS | `.release-evidence\20260521-122522\logs\transport-relay-direct-outbound-quic.log` |
| Tailscale | Optional path disabled for this candidate | N/A | Recorded in `.release-evidence\20260521-122522\release-acceptance.md` |
| Secure deployment | Relay host audit | PASS | `.release-evidence\20260521-122522\logs\secure-deployment-relay-host-audit.log` |

## Base Gate Coverage

The `14.1/base-gates` step passed all of the following:

| Gate | Result |
| --- | --- |
| `git diff --check` | PASS |
| `cargo fmt --all -- --check` | PASS |
| Cargo check slices: `claude`, `codex`, `roo`, `apps-shared` | PASS |
| Cargo clippy slices with `-D warnings`: `claude`, `codex`, `roo`, `apps-shared` | PASS |
| Cargo test slices: `claude`, `codex`, `roo`, `apps-shared` | PASS |
| `cargo audit --quiet` | PASS |
| `gitleaks detect --source . --redact` | PASS |
| GUI `npm ci`, `npm audit`, `npm test`, `npm run build` | PASS |
| Windows desktop bundle via `npm run desktop:build` | PASS |

## Desktop Package

| Artifact | Value |
| --- | --- |
| Installer | `target\release\bundle\nsis\Remote Code_0.1.0_x64-setup.exe` |
| Size | 60,008,193 bytes |
| SHA256 | `7894758C7D3FDFDE8A434A2978A933DA979B7FA250C7F04CD70227239721B476` |

## Notes

`-RequireComplete` exited 0. The only non-PASS row is the optional Tailscale
candidate path recorded as `N/A` because it is not enabled for this release
candidate; standard relay, direct/outbound, mobile/PWA, and QUIC gates passed.
