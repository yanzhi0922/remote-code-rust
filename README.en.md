# Remote Code Rust

[Root README](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

Remote Code Rust is a local-first AI coding workspace for Claude-compatible, Codex, and Roo agents. It combines a Rust runtime, native in-process adapters, a Tauri desktop app, a trusted local runner, a relay-only control plane, and Web/PWA remote control.

## Current Status

This repository is public preview. It is useful for development, auditing, and controlled dogfooding, but it is not yet ready for direct production deployment. The release baseline is [docs/requirements.md](docs/requirements.md).

Production release requires the full checklist: Rust slices, clippy, audit, GUI npm checks, desktop bundle, runner/control-plane E2E, Mobile/PWA pairing, approvals, artifacts, QUIC, provider matrix, MCP calls, and secret scanning.

## Product Boundary

- Coding agents run locally on the desktop or on a trusted runner.
- Provider credentials stay local or on the trusted runner.
- Workspace files stay local or on the trusted runner.
- The cloud relay handles auth, pairing, heartbeats, command relay, event streaming, approvals, artifacts, and Web/PWA assets only.
- The relay must not run agents, workspace tooling, provider SDK loops, Cargo, or `remote-code-runner`.
- Direct runner access is an explicit advanced mode with a separate runner token.

## Main Components

| Area | Location |
| --- | --- |
| CLI / TUI / headless runtime | `agents/claudecode` |
| Desktop GUI and Web/PWA | `apps/remote-code-gui` |
| Control plane / relay | `apps/remote-code-control-plane` |
| Local runner | `apps/remote-code-runner` |
| Agent protocol | `crates/shared/rc-agent-protocol` |
| Claude adapter | `crates/adapters/rc-claude-adapter` |
| Codex adapter | `crates/adapters/rc-codex-adapter` |
| Roo adapter | `crates/adapters/rc-roo-adapter` |
| Claude runtime crates | `crates/claude` |
| Codex crates | `crates/codex` |
| Roo crates | `crates/roo` |

## Local Checks

```powershell
cargo fmt --all -- --check
git diff --check
cargo check --workspace -j 1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

For release candidates, use the sliced gates documented in [docs/requirements.md](docs/requirements.md), `scripts/verify-release.ps1`, and the redacted evidence template in [docs/release-acceptance-evidence.md](docs/release-acceptance-evidence.md).

## Release Artifacts

Tag-triggered releases (`v*`) build workspace tool archives, a relay-only Linux package, the Windows NSIS installer, Web/PWA assets, and `SHA256SUMS.txt`. The `cloud-relay.yml` workflow on `main` builds a relay package without the frontend; the full `release.yml` workflow includes Web/PWA assets.

Do not publish a release solely because artifacts built successfully. Requirements 14/17 need a completed, redacted evidence report.

## Provider And MCP Validation

Primary release paths must remain separate from supplemental test providers. Supplemental validation currently covers MiniMax Token Plan, KuaiKAT Coding Plan, and DeepSeek where applicable. MCP validation must cover MiniMax, context7, sequentialthinking, memory, and puppeteer.

Never commit provider keys, MCP keys, runner tokens, OAuth tokens, local settings, screenshots containing secrets, or logs with raw credentials.

## Security

Read [SECURITY.md](SECURITY.md) before deploying or reporting a vulnerability. If any credential has appeared in chat, logs, reports, or Git history, rotate it before public use.

## License

Public source, proprietary by default. This is not OSI open source and does not grant redistribution, hosted SaaS, or commercial reuse rights without a separate written license. See [LICENSE](LICENSE).

Third-party source mirrors and fixtures under `agents/` and `crates/codex/` retain their upstream notices.
