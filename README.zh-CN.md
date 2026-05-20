# Remote Code Rust

[主 README](README.md) | [English](README.en.md) | [日本語](README.ja.md)

Remote Code Rust 是一个本地优先的 AI 编程工作台，用 Rust 统一 Claude-compatible、Codex、Roo 三类 coding agent，并提供 Tauri 桌面 GUI、本地 Runner、只做中继的 Control Plane，以及 Web/PWA 远程控制。

## 当前状态

本仓库处于 public preview 阶段。它适合开发、审计和受控 dogfood，但还不能直接当作正式生产版本发布。正式发布必须以 [docs/requirements.md](docs/requirements.md) 为验收基线。

发布前必须完整通过 Rust 分片检查、clippy、audit、GUI npm 检查、桌面安装包、Runner/Control Plane E2E、Mobile/PWA 配对、approval、artifact、QUIC、provider 矩阵、MCP 真实调用和密钥扫描。

## 产品边界

- Coding agent 在用户桌面或可信 Runner 上运行。
- Provider key 留在本机或可信 Runner。
- Workspace 文件留在本机或可信 Runner。
- 云端 relay 只做认证、配对、心跳、命令中继、事件流、审批、artifact 和 Web/PWA 静态资源。
- 云端 relay 不得运行 agent、workspace 工具、provider SDK loop、Cargo 或 `remote-code-runner`。
- 直连 Runner 是显式高级模式，必须使用独立 Runner API token。

## 主要模块

| 领域 | 位置 |
| --- | --- |
| CLI / TUI / headless runtime | `agents/claudecode` |
| 桌面 GUI 和 Web/PWA | `apps/remote-code-gui` |
| Control Plane / Relay | `apps/remote-code-control-plane` |
| 本地 Runner | `apps/remote-code-runner` |
| Agent 协议 | `crates/shared/rc-agent-protocol` |
| Claude adapter | `crates/adapters/rc-claude-adapter` |
| Codex adapter | `crates/adapters/rc-codex-adapter` |
| Roo adapter | `crates/adapters/rc-roo-adapter` |
| Claude runtime crates | `crates/claude` |
| Codex crates | `crates/codex` |
| Roo crates | `crates/roo` |

## 本地检查

```powershell
cargo fmt --all -- --check
git diff --check
cargo check --workspace -j 1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

候选发布版本必须执行 [docs/requirements.md](docs/requirements.md) 中的分片门禁、`scripts/verify-release.ps1`，并填写 [docs/release-acceptance-evidence.md](docs/release-acceptance-evidence.md) 的脱敏证据。

## 发布产物

`v*` tag 触发的正式 release 会构建 workspace 工具归档、relay-only Linux 包、Windows NSIS 安装包、Web/PWA 产物和 `SHA256SUMS.txt`。`main` 上的 `cloud-relay.yml` 只构建不带前端的 relay 包；完整 `release.yml` 会包含 Web/PWA 产物。

构建成功不等于可以发布。requirements 14/17 必须有完整、脱敏、可复核的验收报告。

## Provider 与 MCP 验收

主发布路径和补充测试 provider 必须分开记录。补充矩阵当前包含 MiniMax Token Plan、KuaiKAT Coding Plan，以及适用场景下的 DeepSeek。MCP 验收必须覆盖 MiniMax、context7、sequentialthinking、memory、puppeteer。

不要提交 provider key、MCP key、runner token、OAuth token、本地 settings、包含密钥的截图或带原始凭据的日志。

## 安全

部署或报告漏洞前请阅读 [SECURITY.md](SECURITY.md)。任何出现在聊天、日志、报告或 Git 历史中的凭据，都必须在公开使用前轮换。

## 许可

本仓库是 public source，默认不是 OSI 开源授权，也不授予再分发、托管 SaaS 或商业复用权利，除非另有书面许可。见 [LICENSE](LICENSE)。

`agents/` 和 `crates/codex/` 下的第三方源码镜像和测试 fixtures 保留其上游 notice，更新时不得删除来源说明。
