# Release Readiness Snapshot - 2026-05-22

This snapshot records the publication state for `main` at commit
`4433cc5eef0aeffb6a01068760cdc47c65ad6633`. It is not a release approval.
Formal publication remains blocked until GitHub CI is green and the final
manual sign-off package is complete.

## Current Gate Status

| Gate | Status | Evidence |
| --- | --- | --- |
| Local full release acceptance | PASS | `.release-evidence\20260521-225254\release-acceptance.md` |
| GitHub repository visibility | PUBLIC | GitHub repository settings, verified before this snapshot |
| GitHub `Cloud Relay Artifact` | PASS | Latest cloud relay artifact run reported green before this snapshot |
| GitHub `CI` workflow | FAIL | <https://github.com/yanzhi0922/remote-code-rust/actions/runs/26268885747> |
| Manual Sign-Off | PENDING | `.release-evidence\20260521-225254\release-acceptance.md` |
| Final GitHub tag release | BLOCKED | Do not tag or publish until CI and sign-off are closed |

## CI Blockers

The latest `CI` workflow run for `main` was created on
2026-05-22T04:44:09Z and completed as `failure` on
2026-05-22T05:48:23Z.

| Area | Blocking symptom | Owner lane |
| --- | --- | --- |
| Frontend (GUI) | `npm ci` fails because `package-lock.json` is missing transitive `@emnapi/*` lock entries | Frontend/Node CI |
| Rust test infrastructure | Ubuntu test slices receive an empty `RUST_TEST_THREADS` value | Rust CI infrastructure |
| Codex clippy/tests | `dbg!` calls remain in `landlock.rs`; helper binaries are unavailable in unit-test contexts | Codex testing/clippy |
| Claude clippy/tests | Claude clippy has an unused `tempdir`; platform test slices still fail in CI | Claude/Roo regression |

Do not generate a release tag from this commit while these are failing.

## Release Evidence Status

| Evidence item | Status | Notes |
| --- | --- | --- |
| Windows NSIS installer SHA256 | PENDING FINAL REBUILD | The latest evidence log shows `npm run desktop:build` produced `target\release\bundle\nsis\Remote Code_0.1.0_x64-setup.exe`, but the installer file is not present in the current local workspace. Rebuild after CI is green and record the hash from the final artifact. |
| Windows first launch / runner online / session creation | PENDING CAPTURE | Attach screenshot or recording to the final release evidence package. |
| Mobile/PWA target-device capture | PENDING CAPTURE | The automated Mobile/PWA acceptance log passed; final release still needs real target-device screenshot or recording. |
| Provider/MCP owner sign-off | PENDING OWNER | Provider and MCP logs are present and passing; final owner signature is not filled. |
| Relay host audit sign-off | PENDING OWNER | Relay host audit log reports 13 pass / 0 warning / 0 failure; final relay host owner signature is not filled. |
| Release engineer sign-off | PENDING CI | Release engineer should sign only after GitHub CI is green and final artifacts are rebuilt. |
| Tailscale optional path | N/A FOR THIS RELEASE CANDIDATE | Tailscale is not enabled for this candidate. Do not describe it as formally available without fresh tailnet E2E plus ACL/device-trust evidence. |

## Release Notes Draft

```markdown
## Status

- Release type: release candidate only; do not publish as GA
- Production-ready: no
- Commit: 4433cc5eef0aeffb6a01068760cdc47c65ad6633
- CI workflow: blocked, https://github.com/yanzhi0922/remote-code-rust/actions/runs/26268885747
- Evidence report: .release-evidence/20260521-225254/release-acceptance.md
- Manual sign-off: pending

## Supported Platforms

- Windows desktop: pending final NSIS rebuild, first-launch capture, and SHA256
- Linux relay: relay-only host audit passed, owner sign-off pending
- Web/PWA: automated pairing/prompt/approval/artifact flow passed, target-device capture pending
- Mobile: target-device capture pending
- Tailscale path: N/A for this release candidate

## Checks Run

- Base gates: local PASS
- CI: FAIL, publish blocked
- Desktop bundle: local build log PASS, final artifact hash pending
- Provider matrix: local PASS, owner sign-off pending
- MCP matrix: local PASS, owner sign-off pending
- Remote E2E: local PASS

## Known Limitations

- GitHub CI is not green.
- Final NSIS installer artifact and SHA256 must be regenerated after CI is green.
- Tailscale is not enabled for this candidate and must not be marketed as formally available.

## Artifacts

- Windows NSIS installer: pending final rebuild
- Linux relay package: pending final GitHub release artifact
- Workspace tool archives: pending final GitHub release artifact
- Web/PWA assets: pending final GitHub release artifact
- SHA256SUMS.txt: pending final GitHub release artifact

## Secret Hygiene

- Gitleaks result: local PASS and GitHub Secret Scan PASS
- Credential rotation notes: no real credentials should appear in release notes, logs, screenshots, or recordings
- Report/log/screenshot redaction: required before attaching public release artifacts
```

