# Changelog

All notable changes to this project are documented in this file. The format
is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Repository hygiene

- **P0**: Untracked 848 `.codex-logs/` files (Chrome cache, browser data,
  PowerShell scripts) — these were erroneously committed before the matching
  `.gitignore` rule was added. Files remain on disk.
- **P0**: Removed 138 `refs/zcode/checkpoints/...` refs whose commits
  carried author `ZCode Checkpoint <checkpoint@zcode.local>`, then ran
  `git gc --prune=now --aggressive`. 927 MB of orphaned objects reclaimed
  (`.git/objects/` 954 MB → 27 MB).  Metadata archived at
  `plans/archive/zcode-checkpoints-archive.md`.
- **P0**: Cleaned 2 garbage `tmp_obj_*` files in `.git/objects/a3/`.
- **P0**: Set global `i18n.commitencoding=utf-8`,
  `i18n.logoutputencoding=utf-8`, `core.quotepath=off`, `core.autocrlf=false`
  to prevent future GBK-misinterpreted commit messages.
- **P0**: Added `.zcode-session/`, `.zcode-cache/`, `.zcode-checkpoints/`
  rules to `.gitignore` so the 138-ref pollution cannot recur. Future
  zcode integration MUST use `refs/notes/zcode-archive/` namespace.

### Security hardening

- **P2**: `probe_provider_model` now wraps the API key in
  `secrecy::SecretString` so the inner buffer is wiped on drop; the
  shared `reqwest::Client` is reused across calls (no per-call
  `Client::builder()`), and a 1-second per-process rate-limit gate
  prevents upstream 429s from spam-clicks of the "Plug" icon. Added
  `secrecy = "0.10"` to workspace deps and the `ProbeClient` struct
  to `provider_commands.rs`.

### CI / tooling

- **P2**: `scripts/check_panic_macros.py` + `panic-macro-budget.json`
  enforce production-code panic-macro budget (initial: 7 600 unwrap/expect
  + 500 `panic!()`). New `panic-macros` CI job.
- **P2**: `scripts/audit_unsafe_blocks.py` + `add_safety_comments.py`
  add `// SAFETY: ...` to every `unsafe {}` block; new
  `unsafe-audit` CI job fails on missing comments.
- **P2**: `deny.toml` + new `cargo-deny` CI job enforce license
  policy, ban `openssl` / `libgit2-sys` / `git2`, and check the
  RustSec advisory database.  Strong-copyleft (GPL/AGPL/SSPL) denied.
- **P2**: `gitleaks` now runs on a nightly cron + `workflow_dispatch`,
  not only on `main` push/PR — catches long-lived feature branches.
- **P2**: `playwright` CI job runs the existing `e2e/` specs against
  `vite preview` (no Tauri runtime required for UI tests).

### Documentation

- **P3**: `plans/archive/README.md` indexes the 17 design documents
  in that directory with one-line summaries and status.

## [1.0.0] — 2026-06-01

Initial v1.0.0 release. The bullet list below is the original
"Unreleased" section retroactively assigned to v1.0.0 since the
`v1.0.0` tag predates this CHANGELOG entry.

- Hardened remote transport defaults: relay-only web clients unless direct
  runner mode is explicitly enabled.
- Added pinned self-signed TLS enforcement and real TLS signature verification
  for remote transport.
- Completed QUIC command framing, frame-size limits, typed approval decisions,
  and parity with HTTP approval relay/commit/publish behavior.
- Fixed Windows test stack limits for Codex CLI/app-server binaries and reduced
  test debug artifact pressure.
- Isolated app-server integration tests from host home directories and disabled
  unrelated Apps MCP startup in account auth-refresh tests.
- Added release workflow support for Web/PWA relay artifacts and Windows NSIS
  desktop installer artifacts.
- Added one-line relay installer and Windows installer helper scripts.
- Added English/Chinese quick-start docs, security policy, contribution guide,
  and root license notice.

[v1.0.0]: https://github.com/yanzhi0922/remote-code-rust/releases/tag/v1.0.0
[Unreleased]: https://github.com/yanzhi0922/remote-code-rust/compare/v1.0.0...HEAD
