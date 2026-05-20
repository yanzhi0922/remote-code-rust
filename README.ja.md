# Remote Code Rust

[Root README](README.md) | [English](README.en.md) | [简体中文](README.zh-CN.md)

Remote Code Rust は、ローカル優先の AI コーディングワークスペースです。Claude-compatible、Codex、Roo の 3 種類の coding agent を Rust workspace に統合し、Tauri デスクトップアプリ、ローカル Runner、relay 専用 Control Plane、Web/PWA リモート操作を提供します。

## 現在の状態

このリポジトリは public preview です。開発、監査、制御された dogfood には使えますが、まだ本番投入可能なリリースではありません。本番リリースの基準は [docs/requirements.md](docs/requirements.md) です。

正式リリース前には、Rust の分割チェック、clippy、audit、GUI npm チェック、デスクトップバンドル、Runner/Control Plane E2E、Mobile/PWA ペアリング、approval、artifact、QUIC、provider matrix、MCP 実呼び出し、secret scan を通す必要があります。

## セキュリティ境界

- Coding agent はユーザーのデスクトップ、または信頼された Runner 上で動作します。
- Provider key はローカル、または信頼された Runner に保持します。
- Workspace ファイルはローカル、または信頼された Runner に保持します。
- Cloud relay は認証、ペアリング、heartbeat、command relay、event stream、approval、artifact、Web/PWA 静的ファイルのみを扱います。
- Cloud relay では agent、workspace tooling、provider SDK loop、Cargo、`remote-code-runner` を実行してはいけません。
- Direct runner access は明示的な advanced mode であり、独立した runner API token が必要です。

## 主な構成

| 領域 | パス |
| --- | --- |
| CLI / TUI / headless runtime | `agents/claudecode` |
| Desktop GUI / Web/PWA | `apps/remote-code-gui` |
| Control Plane / Relay | `apps/remote-code-control-plane` |
| Local Runner | `apps/remote-code-runner` |
| Agent protocol | `crates/shared/rc-agent-protocol` |
| Agent adapters | `crates/adapters` |
| Claude runtime crates | `crates/claude` |
| Codex crates | `crates/codex` |
| Roo crates | `crates/roo` |

## ローカルチェック

```powershell
cargo fmt --all -- --check
git diff --check
cargo check --workspace -j 1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

リリース候補では [docs/requirements.md](docs/requirements.md)、`scripts/verify-release.ps1`、および [docs/release-acceptance-evidence.md](docs/release-acceptance-evidence.md) の redacted evidence template を使用してください。

## Release Artifacts

`v*` タグで起動する正式 release は、workspace tool archives、relay-only Linux package、Windows NSIS installer、Web/PWA assets、`SHA256SUMS.txt` を生成します。`main` の `cloud-relay.yml` は frontend なしの relay package を生成し、完全な `release.yml` は Web/PWA assets を含みます。

Artifact が生成できただけでは公開リリース可能とは見なしません。requirements 14/17 の redacted evidence report が必要です。

## Secret 取り扱い

Provider key、MCP key、runner token、OAuth token、ローカル settings、secret を含むスクリーンショットやログを Git にコミットしないでください。チャット、ログ、レポート、Git history に出た credential は公開利用前に必ずローテーションしてください。

## License

Public source, proprietary by default. This is not OSI open source and does not grant redistribution, hosted SaaS, or commercial reuse rights without a separate written license. See [LICENSE](LICENSE).

Third-party source mirrors and fixtures under `agents/` and `crates/codex/` retain their upstream notices.
