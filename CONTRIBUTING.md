# Contributing

This repository is public source. Contributions are welcome only when they can
be reviewed and merged under the repository's current license terms.

## Development Rules

- Keep agent execution local by default; do not move provider keys or workspace
  files to the relay server.
- Prefer Rust ownership, borrowing, typed enums, and explicit `Result` errors
  over cloning, stringly typed state, or panics.
- Keep remote communication relay-only unless a direct mode is explicitly
  configured by the user.
- Add focused tests for protocol, auth, transport, and filesystem changes.
- Do not commit local profiles, logs, cache output, test keys, or `.env` files.

## Required Checks

```powershell
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets -j1 -- -D warnings
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

For release candidates:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-release.ps1 -IncludeAudit -IncludeDesktopBundle
```

Clean build caches after large local runs:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\clean-build-caches.ps1 -Aggressive
```
