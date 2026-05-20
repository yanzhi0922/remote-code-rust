# Release Validation Snapshot - 2026-05-20

This snapshot records the local evidence produced on 2026-05-20 in
`D:\remote-code-rust`. It is not a production sign-off by itself; the
requirements in [requirements.md](requirements.md) remain authoritative.

## Automated Gates Run

| Area | Result | Evidence |
| --- | --- | --- |
| Rust format | PASS | `cargo fmt --all -- --check` |
| Git whitespace | PASS | `git diff --check` |
| Claude check slice | PASS | `python scripts\cargo_workspace_slice.py check claude` |
| Claude clippy slice | PASS | `python scripts\cargo_workspace_slice.py clippy claude` |
| Claude test slice | PASS | `python scripts\cargo_workspace_slice.py test claude` |
| Focused MCP cleanup tests | PASS | `cargo test -p claude-mcp --lib -- --nocapture` |
| Runner focused tests | PASS | `cargo test -p remote-code-runner --all-targets -j 1 -- --nocapture` |
| Control-plane focused tests | PASS | `cargo test -p claude-control-plane --all-targets -j 1 -- --nocapture` |
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

| Provider | Model | Protocol | Result |
| --- | --- | --- | --- |
| MiniMax Token Plan | `minimax-m2.7` | Anthropic-compatible | PASS |
| KuaiKAT Coding Plan | `kat-coder-pro-v2` | Anthropic-compatible | PASS |
| DeepSeek | `deepseek-v4-flash` | Anthropic-compatible | PASS |

## MCP Acceptance

Ran with `scripts\acceptance-release.ps1 -IncludeMcpMatrix -UseProxy`.

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

## Remaining Environment Sign-Off

The following requirements still need environment-specific sign-off before a
production release can be claimed:

- Installed Windows desktop first launch with embedded runner online.
- Full relay + runner + control-plane remote E2E on the target host.
- Mobile/PWA pairing, refresh restore, prompt, interrupt, approval, artifact
  download, and timeline validation on real devices.
- Relay, Direct WS, Outbound Poll, and QUIC controlled E2E, including QUIC
  certificate/fingerprint and failure diagnostics.
- Optional Tailscale tailnet direct path, ACL/device-trust evidence, E2EE,
  approval, and artifact validation if that path is enabled.
- Relay host inspection proving it contains no source tree, runner, agent,
  workspace, provider keys, or MCP keys.

Any public release notes must either attach a completed redacted evidence report
for these items or explicitly mark the release as not production-ready.
