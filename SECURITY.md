# Security Policy

Remote Code executes coding agents, shell tools, and provider requests on user
machines. Treat every credential, pairing token, runner token, and workspace
path as sensitive.

## Supported Versions

Only the current `main` branch and the latest GitHub Release receive security
fixes.

## Reporting a Vulnerability

Do not open a public issue for secrets, auth bypasses, remote-code execution,
or relay compromise reports. Send a private report to the repository owner with:

- affected commit or release,
- reproduction steps,
- expected and actual behavior,
- logs with tokens and paths redacted.

## Security Requirements

- The cloud relay must run `remote-code-control-plane` only. Do not run
  `remote-code-runner`, Codex, Roo, Claude, provider SDK loops, or workspace
  tooling on the relay host.
- Desktop runners must use outbound relay mode by default.
- Direct runner access requires an explicit advanced opt-in and a separate
  runner API token.
- WebSocket long-lived access tokens in URL query strings are disabled by
  default.
- Self-signed TLS/QUIC endpoints require certificate fingerprint pinning.
- Secrets must not be committed. Run gitleaks before public releases.

## Local Secret Scan

```powershell
gitleaks detect --source . --redact --no-git
```

For full history checks before making a repository public:

```powershell
gitleaks detect --source . --redact
```
