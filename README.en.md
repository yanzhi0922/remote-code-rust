# Remote Code Rust

Remote Code Rust is a Rust-native AI coding workspace for desktop-first development and secure mobile remote control. The product goal is a one-click Windows desktop app: run Codex, Roo, and Claude-compatible coding agents locally on the user's machine, then control that same desktop session from a phone or PWA through a relay-only cloud control plane.

Languages: [English](README.en.md) | [简体中文](README.zh-CN.md) | [Root README](README.md)

## What It Does

- Runs coding agents on the desktop, not on the cloud relay.
- Provides a Tauri v2 + React 19 desktop GUI with provider, model, session, approval, artifact, and remote-control surfaces.
- Integrates three agent engines behind native Rust adapter traits: Codex, Roo, and Claude.
- Uses a relay-only control plane for pairing, authentication, heartbeats, command polling, and event streaming.
- Defaults remote clients to relay-only mode; direct runner access is an explicit advanced opt-in.
- Keeps provider keys, workspace files, tool execution, and agent loops on trusted user machines.

## Quick Install

Windows desktop app from the latest GitHub Release:

```powershell
iwr -UseB https://raw.githubusercontent.com/yanzhi0922/remote-code-rust/main/scripts/install-windows.ps1 | iex
```

Silent install:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1 -Silent
```

Relay server on Ubuntu 22.04:

```bash
curl -fsSL https://raw.githubusercontent.com/yanzhi0922/remote-code-rust/main/deploy/install-relay.sh | sudo REMOTE_CODE_DOMAIN=remote-code.example.com REMOTE_CODE_ACME_EMAIL=admin@example.com bash
```

The relay installer downloads the release artifact, installs `remote-code-control-plane`, serves the Web/PWA frontend, binds the control plane to `127.0.0.1`, configures nginx when TLS certificates are available, and refuses to run local coding agents on the server.

## Local Development

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -j1 -- -D warnings
cargo test -p codex-app-server --test all -j1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

Use the cache cleaner after large builds:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\clean-build-caches.ps1 -Aggressive
```

## Security Model

- The relay server is not a runner and must not host provider keys or workspaces.
- Runner/control-plane credentials are separate.
- Long-lived access tokens are not accepted in WebSocket query strings by default.
- Self-signed QUIC/TLS requires certificate pinning.
- Direct runner mode is disabled by default in the web client.
- Secret scanning is part of release gating; rotate any credential that was ever committed or shared in local test files.

## Release Status

This repository is moving toward a production Windows desktop release. Before publishing a release, run the release gates in `scripts/verify-release.ps1`, `cargo audit`, npm audit, gitleaks, and the Windows installer build. Android/iOS native packaging and store hardening are tracked separately from the Web/PWA flow.

## License

This repository is public source, but not open-source licensed unless a separate written license says otherwise. See [LICENSE](LICENSE).
