# Remote Code Rust

Remote Code Rust 是一个 Rust 原生的 AI 编码工作台，目标是 Windows 桌面端一键安装、点击桌面快捷方式打开 GUI 后直接使用，并且可以通过手机 App/PWA 安全远程控制同一台电脑上的本地 Agent。

语言：[English](README.en.md) | [简体中文](README.zh-CN.md) | [完整 README](README.md)

## 核心边界

- Codex / Roo / Claude 三个 coding agent 都在用户电脑本地运行。
- 云服务器只做配对、认证、心跳、命令轮询、事件中继、Web/PWA 静态资源和安装包下载。
- Provider Key、工作区文件、工具执行、Agent 进程不进入中继服务器。
- Web/PWA 默认 `relay_only`，直连 runner 需要显式高级开关。
- Runner API token 和控制平面 token 分离，不复用。

## 一键安装

Windows 桌面端最新 Release：

```powershell
iwr -UseB https://raw.githubusercontent.com/yanzhi0922/remote-code-rust/main/scripts/install-windows.ps1 | iex
```

静默安装：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1 -Silent
```

Ubuntu 22.04 中继服务器一行安装：

```bash
curl -fsSL https://raw.githubusercontent.com/yanzhi0922/remote-code-rust/main/deploy/install-relay.sh | sudo REMOTE_CODE_DOMAIN=remote-code.example.com REMOTE_CODE_ACME_EMAIL=admin@example.com bash
```

中继安装脚本会下载 GitHub Release 中的 Linux control-plane 和 Web/PWA 前端，绑定 `127.0.0.1`，生成 bootstrap secret，安装 systemd 服务，并避免在服务器上运行 runner 或 coding agent。

## 本地验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -j1 -- -D warnings
cargo test -p codex-app-server --test all -j1
cd apps\remote-code-gui
npm ci
npm test
npm run build
```

大规模构建后清理缓存：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\clean-build-caches.ps1 -Aggressive
```

## 当前状态

项目正在向可正式发布的 Windows 桌面版推进。发布前仍必须跑完 `scripts/verify-release.ps1`、`cargo audit`、npm audit、gitleaks、Windows NSIS 安装包构建和真实远控验收。Android/iOS 原生打包与应用商店级加固需要独立验收。

## 许可

本仓库可以公开查看源码，但除非另有书面授权，并不等同于开源授权。见 [LICENSE](LICENSE)。
