# Remote Code Rust

**Languages:** [English](README.en.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

Remote Code Rust is a local-first AI coding workspace. It brings Claude-compatible, Codex, and Roo coding agents into one Rust workspace, with a Tauri desktop app, a local runner, a relay-only control plane, and Web/PWA remote control.

> Status: public preview. The repository is public, but the product is not yet production-ready. Use [docs/requirements.md](docs/requirements.md) as the release gate, not a successful build alone.

## What This Project Is

- A Rust-native runtime for local AI coding sessions.
- A desktop GUI built with Tauri v2, React 19, TypeScript, and Vite.
- Three independent in-process agent adapters:
  - `remote_claude`
  - `remote_codex`
  - `remote_roo`
- A local runner that hosts trusted workspace sessions.
- A control plane that relays auth, pairing, events, approvals, artifacts, and Web/PWA traffic.
- MCP, Skills, Plugins, permissions, transcript persistence, context management, and remote transport experiments.

## Security Boundary

The default product boundary is strict:

- Agent loops run on the user desktop or another trusted runner.
- Provider keys stay on the user machine or trusted runner.
- Workspace files stay on the user machine or trusted runner.
- The cloud relay must not run coding agents, provider SDK loops, `remote-code-runner`, Cargo, or workspace tooling.
- Direct runner access is opt-in and must use a separate runner API token.
- WebSocket event streams use short-lived stream tickets instead of long-lived URL tokens.

See [SECURITY.md](SECURITY.md) for reporting and operational rules.

## Release Readiness

This project should not be treated as production-ready until the release checklist in [docs/requirements.md](docs/requirements.md) is fully satisfied.

At minimum, release candidates must pass:

```powershell
cargo fmt --all -- --check
git diff --check
python scripts/cargo_workspace_slice.py check claude
python scripts/cargo_workspace_slice.py check codex
python scripts/cargo_workspace_slice.py check roo
python scripts/cargo_workspace_slice.py check apps-shared
python scripts/cargo_workspace_slice.py clippy claude
python scripts/cargo_workspace_slice.py clippy codex
python scripts/cargo_workspace_slice.py clippy roo
python scripts/cargo_workspace_slice.py clippy apps-shared
cargo audit --quiet
cd apps\remote-code-gui
npm ci
npm audit --audit-level=moderate --registry=https://registry.npmjs.org/
npm test
npm run build
```

The project also requires real end-to-end validation for desktop GUI, runner, control plane, Mobile/PWA, approvals, artifacts, QUIC, provider matrices, and MCP.

## Provider And MCP Test Matrix

Release validation must keep the primary paths and supplemental providers separate:

- Primary paths:
  - Claude / Anthropic-compatible
  - Codex / OpenAI-compatible
  - Roo / Roo-native provider stack
- Supplemental validation providers:
  - MiniMax Token Plan
  - KuaiKAT Coding Plan
  - DeepSeek where applicable
- MCP validation servers:
  - MiniMax
  - context7
  - sequentialthinking
  - memory
  - puppeteer

Real provider keys and MCP keys must never be committed, placed in reports, copied into screenshots, or stored in Git-tracked files. Use environment variables, OS keychains, or local untracked config.

## Repository Layout

```text
remote-code-rust/
├── agents/
│   ├── claudecode/                 # remote-code CLI/TUI/headless runtime
│   └── codex/                      # vendored Codex source mirror
├── apps/
│   ├── remote-code-gui/            # Tauri desktop and Web/PWA frontend
│   ├── remote-code-control-plane/  # relay/control plane
│   ├── remote-code-runner/         # trusted local runner
│   └── remote-code-migrate/        # profile migration
├── crates/
│   ├── adapters/                   # rc-claude/codex/roo adapters
│   ├── claude/                     # Claude-compatible Rust runtime crates
│   ├── codex/                      # Codex Rust crates
│   ├── roo/                        # Roo Rust rewrite crates
│   └── shared/                     # shared protocol and event crates
├── deploy/                         # relay deployment scripts
├── docs/                           # requirements and architecture assets
├── plans/                          # design notes and archived audit material
└── scripts/                        # release, validation, and cleanup scripts
```

## Local Development

```powershell
cargo fmt --all -- --check
cargo check --workspace -j 1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

For Windows builds with less disk pressure:

```powershell
$env:CARGO_INCREMENTAL='0'
$env:RUSTFLAGS='-C debuginfo=0'
cargo build --workspace -j 1
```

After large builds:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\clean-build-caches.ps1 -Aggressive
```

## Documentation

- [Requirements](docs/requirements.md): release baseline and acceptance criteria.
- [Architecture](ARCHITECTURE.md): system topology and module boundaries.
- [Compatibility](COMPATIBILITY.md): compatibility notes.
- [Roadmap](ROADMAP.md): planned work.
- [Security](SECURITY.md): reporting and security model.
- [Contributing](CONTRIBUTING.md): contribution and local checks.

## Public Repository Hygiene

- `.research/`, local MCP config, local profiles, logs, build output, and test keys are intentionally untracked.
- Do not commit `.env`, `.mcp.json`, local provider settings, runner tokens, SQLite state, screenshots containing secrets, or generated logs.
- Before release or visibility changes, run a real secret scanner such as gitleaks over the current tree and full history.
- Any credential that was ever pasted into chat, logs, local reports, or Git history must be rotated before public use.

## License

This repository is public source, not open-source licensed unless a separate written license says otherwise. See [LICENSE](LICENSE).
