# Release Checklist

This is the short public checklist. The authoritative release gate remains
[docs/requirements.md](docs/requirements.md), especially sections 14 and 17.

## Required Before Publishing

- Run `powershell -ExecutionPolicy Bypass -File scripts\verify-release.ps1 -IncludeAudit -IncludeGitleaks`.
- For Windows desktop releases, also run `scripts\verify-release.ps1 -IncludeDesktopBundle -IncludeAudit -IncludeGitleaks`.
- Fill a redacted evidence report from [docs/release-acceptance-evidence.md](docs/release-acceptance-evidence.md).
- Verify CLI, TUI, desktop GUI, runner, control plane, Mobile/PWA, approvals, artifacts, and all declared transports.
- Verify MiniMax and KuaiKAT provider matrix records, plus applicable DeepSeek records.
- Verify MiniMax, context7, sequentialthinking, memory, and puppeteer MCP startup, discovery, one real call, failure messaging, and redacted logs.
- Confirm relay hosts run only the control plane and contain no source tree, workspace, runner, agent, provider key, or MCP key.
- Confirm docs, logs, screenshots, recordings, release reports, archives, and Git-tracked files contain no real secrets.

## GitHub Release Notes Template

```markdown
## Status

- Release type:
- Production-ready: yes/no
- Commit:

## Supported Platforms

- Windows desktop:
- Linux relay:
- Web/PWA:
- Mobile:

## Checks Run

- Base gates:
- CI:
- Desktop bundle:
- Provider matrix:
- MCP matrix:
- Remote E2E:

## Known Limitations

-

## Artifacts

- Windows NSIS installer:
- Linux relay package:
- Workspace tool archives:
- Web/PWA assets:
- SHA256SUMS.txt:

## Secret Hygiene

- Gitleaks result:
- Credential rotation notes:
- Report/log/screenshot redaction:
```
