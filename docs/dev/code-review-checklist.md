# Code Review Checklist

Use this checklist for every PR that touches a `.rs` file under
`crates/`, `apps/`, or `agents/`.  Adapted from past regressions we have
hit; each item is a real bug class that has cost us a hotfix.

## Security

- [ ] **No new `unsafe` without `// SAFETY:` comment** above the block.
      The CI `unsafe-audit` job fails on missing comments.
- [ ] **No new `unwrap()` / `expect()` / `panic!()` in production code**
      without a comment justifying it.  See `scripts/panic-macro-budget.json`
      for the current budget; tighten it in follow-up PRs.
- [ ] **No secrets in code or logs.** API keys, runner tokens, JWT
      secrets, and pairing codes must go through `secrecy::SecretString`
      or `keyring`.  Never `format!("{key}")` into a log line.
- [ ] **No new direct `git refdb` writes outside the ZCode notes
      namespace.** `refs/zcode/checkpoints/...` is rejected by the
      pre-receive hook; if you really need to persist agent state in
      git, use `refs/notes/<session>/<id>` instead.
- [ ] **No new Tauri capability without a matching permission in
      `tauri.conf.json`.** Run `cargo check --package remote-code-gui`
      and confirm the build does not warn about missing capabilities.
- [ ] **No new `dangerouslyDisableTauri` or `dangerouslySetInnerHTML`**
      in TS/TSX without a code comment explaining why this is OK.

## Performance

- [ ] **No new `reqwest::Client::builder()` per call.**  Reuse the
      shared client from `AppState::probe_client` (probe) or
      `ProviderClient` (everything else).
- [ ] **No new `tokio::sync::Mutex` held across an `.await`.**  Use
      `parking_lot::Mutex` for short critical sections, or
      `tokio::sync::Mutex` only when the critical section itself is
      async.
- [ ] **No new `Vec::clone()` on a 1k+ element vector.**  Pass a
      `&[T]` slice instead.

## Correctness

- [ ] **Match arms cover all variants of the new enum.**  A
      non-exhaustive match that "compiles" today is a runtime
      `match!()` panic tomorrow when a new variant is added.  See
      the McpTransportConfig::StreamableHttp regression (commit
      `f4830540`).
- [ ] **No new `git stash` left behind** by the change.  Run
      `git stash list` and ensure no `WIP on main: <random-sha>`
      stash names appear.  The audit 2026-06-02 found two stale
      stashes (`pre-main-fast-forward-cargo-20260524-012551` and
      `WIP on main: c59093ee Fix claude provider formatting`).
- [ ] **No new `git update-ref` outside `scripts/purge_zcode_refs.sh`.**
      That script is the only sanctioned way to remove refs.
- [ ] **No new tracked binary files in `.gitignore` locations.**
      `.codex-logs/`, `apps/remote-code-gui/screenshots/`, and
      `docs/screenshots/legacy-2026-06-01/` are ignored; if a file
      is in there and tracked, run `git rm --cached`.

## Build / CI

- [ ] **No new `dtolnay/rust-toolchain@master` floating ref.**
      Use `@v1` (the action is a thin wrapper) or pin the SHA.
- [ ] **No new `cargo build --workspace` in a sub-job.**  Use
      `python scripts/cargo_workspace_slice.py` so the job runs
      one of `claude / codex / roo / apps-shared` and stays under
      the 60-min timeout.
- [ ] **No new dependency without bumping `Cargo.lock`.**  Run
      `cargo build` and commit the resulting `Cargo.lock` diff.
- [ ] **No new `deny.toml` allow entry without a risk note + review
      date.**  The file is read by the `cargo-deny` CI job.

## Documentation

- [ ] **No new public API without a doc comment.**  `cargo doc`
      must build without `missing_docs` warnings.
- [ ] **No new script in `scripts/` without a `--help` flag** and a
      one-paragraph usage block at the top of the file.
- [ ] **No new commit message containing GBK / `?` placeholders.**
      The global `i18n.commitencoding = utf-8` config catches this
      in fresh clones, but old workstations may still produce
      mojibake.  Check the rendered log on GitHub before merging.

## Post-merge

- [ ] **`git status` is clean** after `git pull --rebase`.
- [ ] **`git log --all --format='%an <%ae>'` does not show**
      `ZCode Checkpoint <checkpoint@zcode.local>` (run
      `bash scripts/purge_zcode_refs.sh` if it does).
- [ ] **CHANGELOG.md has an entry under `[Unreleased]`** if the
      change is user-visible.
