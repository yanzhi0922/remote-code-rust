# Remote Code Rust — 项目状态与路线图

> 更新日期: 2026-05-16
> 当前阶段: 发布收敛与安全加固 — 核心门禁已恢复，安装包与真实设备链路仍需发布验收
> 代码规模: `cargo metadata --no-deps` 实测 231 个 Rust packages；Roo/Codex/Claude 均纳入当前 workspace 视角
> 当前验证基线: 以本地 `cargo check --workspace --all-targets` / `cargo clippy --workspace -- -D warnings` / `cargo audit --quiet` / `npm test` / `npm run build` 实测为准，不再使用旧的“全绿”静态结论；`tauri build --bundles nsis` 仍需在磁盘空间充足的发布机上完成。
> 基准分支: `main`

---

## 一、当前产品状态

### 1.1 总体评估: 发布候选收敛中

| 维度 | 状态 | 详情 |
|------|------|------|
| 编译 | 🟡 核心通过 | `cargo check --workspace --all-targets` 与 `npm run build` 已通过；NSIS 桌面安装包仍需独立发布机完成 |
| 安全 | 🟡 加固后待验收 | WebSocket 改用一次性 stream ticket，默认 relay-only，CI 增加 gitleaks；`cargo audit` 使用显式风险接受文件并仍跟踪上游 warning-only advisories |
| 测试 | 🟡 分批通过 | 前端测试与关键 Rust 包通过；Windows 全 workspace 测试可能受 PDB 写入/磁盘压力影响，需要分包或限并发重跑 |
| Clippy | ✅ 门禁恢复 | `cargo clippy --workspace -- -D warnings` 已恢复通过，Roo 来源代码采用 scoped quarantine |
| CLI | ✅ 可发布 | clap 命令树、doctor 与核心子命令可构建并通过回归 |
| TUI | ✅ 可发布 | 交互式终端 + Vim 模式回归通过 |
| GUI (Tauri) | ✅ 可构建 | Rust/Tauri crate 纳入工作区回归，Phase 2-5 GUI Redesign 完成 |
| GUI (Web/PWA) | ✅ 门禁通过 | 远程控制面、中英双语、错误边界、PWA 缓存更新链路通过 |
| GUI (Mobile/Tauri v2) | 🟡 预览可用 | Android 配置已整理；推送 token 获取仍依赖原生 FCM/APNs 插件，未拿到 token 时明确返回 unavailable |
| Provider | ✅ 完整 | OpenAI/Anthropic 协议，流式，多 key 轮换，故障转移 |
| 工具系统 | ✅ 丰富 | 62 内置工具 + MCP + 插件扩展 |
| Control Plane | 🟡 核心可用 | Runner/Session/Approval/Artifact/Event 全链路；远程流改为一次性 stream ticket |
| Runner | 🟡 本地执行 | Daemon、心跳和审批中继可用；生产目标要求桌面端首启自动上线仍需端到端打包验收 |
| 多 Agent | ✅ 三引擎独立适配器 | Claude (QueryEngine) + Codex (AppServer) + Roo (AgentLoop, 26 Provider backends) 三条独立 in-process 路径 |
| 国际化 | ✅ 完整 | Web GUI / mobile shell 支持中文界面 |

### 1.2 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                     用户界面层                                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │
│  │   CLI    │  │   TUI    │  │GUI(Tauri)│  │ GUI(Web/PWA) │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────┬───────┘ │
│       │              │              │                │        │
├───────┼──────────────┼──────────────┼────────────────┼────────┤
│       │    多 Agent 统一层          │                │        │
│  ┌────▼────────────────────────────▼────┐    ┌──────▼───────┐ │
│  │ send_prompt() → agent_type routing   │    │  REST/WS API │ │
│  │ Claude→QueryEngine  Codex→AppServer  │    │  (api.ts)    │ │
│  │ Roo→Provider+Dispatcher              │    └──────┬───────┘ │
│  └────────────────┬────────────────────┘           │        │
│                   │                                │        │
├───────────────────┼────────────────────────────────┼────────┤
│              核心运行时层            │     Control Plane 层    │
│  ┌────────────────▼────────────┐   │  ┌──────────────────▼─┐ │
│  │ claude-core │ claude-session │ claude-tools   │  │ claude-control-plane   │ │
│  │ claude-provider │ claude-agents │ claude-mcp   │  │ (registry, events, │ │
│  │ claude-config │ claude-skills │ claude-plugins │  │  approvals, SQLite)│ │
│  │ claude-compact│claude-context│ rc-query-eng│  └────────────────────┘ │
│  │ claude-system-prompt│claude-auth│claude-model  │                         │
│  └─────────────────────────────┘   │                         │
│                                     │                         │
├─────────────────────────────────────┼─────────────────────────┤
│              Runner 层              │                         │
│  ┌─────────────────────────────────▼───────────────────────┐ │
│  │ claude-runner (daemon, heartbeat, session hosting)          │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Crate 结构（当前 metadata 视角约 231 packages）

| 分类 | 数量 | 说明 |
|------|------|------|
| Claude / Apps / Adapters | 多个 | claude-core, claude-provider, claude-session, claude-checkpoint, claude-git, rc-* adapters, GUI, Control Plane, Runner, Migrate |
| Codex | 多个 | protocol, core, exec, app-server 等当前 workspace 依赖图中的 package |
| Roo | 多个 | provider, task, tools, terminal, checkpoint, config 等来源代码当前纳入 clippy quarantine 管理 |

---

## 二、已完成阶段回顾

### Phase 0: 基础设施 — ✅ 完成
- ✅ Cargo workspace + `apps/` + `crates/` 结构
- ✅ CI (Windows + Linux)
- ✅ 核心文档 (ARCHITECTURE.md, COMPATIBILITY.md, PROVENANCE.md, ROADMAP.md)

### Phase 1: 核心运行时 — ✅ 完成
- ✅ CLI (`doctor`, `sessions`, `resume`, `export`)
- ✅ Headless `stream-json` 模式
- ✅ Provider 适配 (Anthropic + OpenAI)
- ✅ 会话持久化 (SQLite + NDJSON)
- ✅ 权限引擎 (5 模式 + 3 权限类)
- ✅ 7 个内置工具

### Phase 2: 高级本地运行时 — ✅ 完成
- ✅ MCP 客户端 (stdio/HTTP/WS)
- ✅ Skills 发现与索引
- ✅ 插件系统 (JSON-RPC 进程隔离)
- ✅ 多代理调度器 + 邮箱模型
- ✅ 38+ 内置工具 → 62 工具
- ✅ BM25 工具搜索 + 延迟加载
- ✅ 上下文压缩 (5 策略)
- ✅ 记忆系统 (RC.md)
- ✅ Vim 模式 TUI

### Phase 3: 远程平台 — ✅ 完成
- ✅ Runner (HTTP API + 心跳 + 注册)
- ✅ Control Plane (REST + WebSocket)
- ✅ 审批中继 + Artifact 管理
- ✅ 事件扇出 (broadcast channel)

### Phase 4: 超越 Parity — ✅ 完成 (348 tests)
- ✅ 确定性重放
- ✅ Provider 故障转移 + 路由策略
- ✅ 细粒度权限规则 (通配符)
- ✅ 成本追踪 + 遥测

### Phase 5: 竞品 Parity 增强 — ✅ 完成
- ✅ P0: 7 个阻断性修复 (SSE 解析器、增量渲染、模型信息 DB、压缩策略、错误分类、首次运行向导、Doctor 诊断)
- ✅ P1: 9 个重要改进 (BM25 搜索、延迟加载、Cache 优化、成本追踪、记忆系统、调度器、沙箱、自动压缩、流式回调)
- ✅ P2: 9 个增强功能 (workflow、cron、daemon、SSH 增强、REPL、PowerShell、monitor、remote trigger、PR suggester)
- ✅ P3: 6 个打磨功能 (Tab 补全、Ctrl+R 搜索、主题系统、语音输入、交叉编译 CI、SHA256 签名)

### Phase 6: 桌面 GUI — ✅ 完成
- ✅ Tauri v2 + React 19 + TypeScript + Vite + Tailwind CSS
- ✅ 多项目侧边栏 + 文件夹选择器
- ✅ Chat 界面 + Markdown 渲染 (KaTeX + 代码高亮 + GFM)
- ✅ 可折叠工具调用 + 思考块 + 子任务展开
- ✅ 多 Provider 管理 + 设置面板
- ✅ 权限弹窗 + 快速选择器
- ✅ 会话持久化 + CI 前端任务

### Phase 7-8: 多 Agent 架构 — ✅ 完成
- ✅ `rc-agent-protocol` crate 创建
- ✅ `AgentAdapter` trait 定义
- ✅ `UnifiedAgentEvent` 统一事件模型
- ✅ `AgentRouter` 路由分发
- ✅ GUI 前端 Agent 选择器

### Phase 9-10: 全面审计 + 38 个问题修复 — ✅ 完成
- ✅ 全代码库审计 (残留占位符、硬编码密钥、unsafe、TypeScript any、生产 unwrap)
- ✅ 38 个问题修复 (P1 功能缺陷 + P2 代码质量 + P3 文档/测试)
- ✅ Clippy 警告从 15 处清零
- ✅ 生产代码 `console.log` 清除
- ✅ `#[allow(dead_code)]` 标注审查

### Phase 11: 进程内执行模式 — ✅ 完成
- ✅ RooCode/Codex 从子进程 JSON-RPC 转换为进程内执行
- ✅ 删除子进程管理代码、bridge_proto.rs、subprocess.rs
- ✅ 三个 Agent 各自拥有独立的适配器 crate

### Phase 12: 端到端真实测试 — ✅ 完成
- ✅ MiniMax Provider (anthropic-compatible) 真实 API 调用测试
- ✅ MCP 端到端集成测试
- ✅ Headless `--print` 与 `stream-json` 冒烟测试通过

### Phase 13: QueryEngine 执行路径 — ✅ 完成
- ✅ Claude Agent 使用 QueryEngine 统一执行路径
- ✅ 消除双路径分歧 (`run_gui_prompt` vs `run_agent_prompt`)
- ✅ 统一状态机 + 流式执行器 + Token 预算
- ✅ Observer 模式支持 checkpoint 和恢复

### Phase 14: 代码清理 — ✅ 完成
- ✅ 删除 880 行旧代码 (AgentRouter、健康检查、重启追踪器等)
- ✅ AppState 精简
- ✅ 代码已推送 (commit `05b05ec`)

### Phase 15: 三 Agent 独立适配器架构 — ✅ 完成
- ✅ 删除 bridge 残留 (subprocess.rs 730行 + bridge_proto.rs)
- ✅ `rc-codex-adapter` 从 `crates/claude/` 移至 `crates/adapters/`
- ✅ `rc-roo-adapter` 从 `crates/claude/` 移至 `crates/adapters/`
- ✅ 新建 `rc-claude-adapter` (ClaudeInProcessAdapter = QueryEngine)
- ✅ 三个适配器编译验证通过
- ✅ 代码已推送 (commit `7798a5f`)

### Phase 16: GUI Redesign — ✅ 完成
- ✅ Phase 1: Design System Foundation — CSS tokens, Tailwind config, ThemeProvider
- ✅ Phase 2: Layout Overhaul — ActivityBar, SplitPane, StatusBar, tab sidebar
- ✅ Phase 3: Chat Experience — streaming animation, inline diff, slash commands
- ✅ Phase 4: Integrated Tool Panes — Terminal (xterm.js), Diff, Preview, PaneHost
- ✅ Phase 5: Command Palette — keyboard shortcuts overlay

### Phase 17: ZCode 启发功能 — ✅ 完成
- ✅ `claude-checkpoint` crate — 对话级版本控制（SHA256 快照扫描、SQLite 存储、unified diff、恢复引擎）
- ✅ `claude-specialized-agents` crate — 专业化 Agent 系统（Markdown+YAML frontmatter 定义、3 层发现、5 个内置 Agent）
- ✅ `claude-git` crate — Git 操作封装（gix 分支解析 + CLI status/stage/commit/diff/log）
- 🧹 废弃 GUI 原型已清理 — `PermissionModeSwitch`、`GitPanel`、`CheckpointTimeline`、`AgentPicker` 未接入生产界面，已在废弃代码清理中删除
- ✅ 灵感分析文档 [zcode-inspiration-plan.md](zcode-inspiration-plan.md)
- ✅ 三个新 crate 编译验证通过

---

## 三、已知限制

| 限制 | 说明 | 优先级 |
|------|------|--------|
| TTS 为 Mock 实现 | `claude-voice::tts` 返回占位响应，未接入真实 TTS 服务 | P2 |
| Roo 权限系统部分接线 | `RooInProcessAdapter::resolve_permission()` 可用但未完全接入 GUI 交互弹窗 | P1 |
| Roo Token 估算粗糙 | 使用 `text.len() / 4` 而非 Roo 原生 tiktoken | P2 |
| Roo MCP 未接入 | 声明了 McpSupport 但未集成 McpServerConnection | P2 |
| `rama-*` 依赖为预发布 | 锁定 `0.3.0-alpha.4`，待迁移至稳定版 | P3 |
| RustSec accepted advisories | `hickory-proto` / `rsa` 上游暂无直接 fixed version，已在 `.cargo/audit.toml` 明确接受并要求后续复核 | P1 |
| 移动端系统推送 | 当前可注册控制平面 endpoint，但原生 FCM/APNs token 获取仍依赖平台插件；无 token 时返回 unavailable | P1 |
| 用户名/密码远控 | 仅接受服务端预配置 SHA-256 user-key hash；生产优先 bootstrap/pairing | P1 |

---

## 四、下一阶段路线图

### Phase 18: Roo Agent 深化集成
- [ ] Roo 权限系统 — 将 `resolve_permission()` 完全接入 GUI 交互式权限弹窗
- [ ] Roo Token 精确计算 — 使用 Roo 原生 tiktoken 替代粗略估算
- [ ] Roo MCP 集成 — 在 `send_message()` 中集成 `McpServerConnection`
- [ ] 端到端多 Agent 集成测试

### Phase 19: 增强远程交互
- [ ] 终端流 (Terminal Stream) — 实时终端输出远程查看
- [ ] 文件预览 — 远程文件内容浏览
- [ ] Diff 浏览 — 代码变更可视化
- [ ] 推送通知 — 移动端审批提醒

### Phase 20: 竞品超越
- [ ] 子任务深度委派 — 多级子代理 + 并行执行 + 凭证轮换
- [ ] 会话回退 — 回退到任意历史点继续（基于 claude-checkpoint）
- [ ] Shadow Git 检查点 — 自动 git checkpoint（基于 claude-git）
- [ ] Task Flow 可视化 — 任务依赖图 + 进度追踪

### Phase 21: 可选扩展
- [ ] 多工作站本地 Runner — 多台可信电脑协同，服务器仍只做中继
- [ ] 多工作站调度 — 多台机器协同
- [ ] 团队协作 — 多用户共享会话
- [ ] TTS 真实实现 — 接入语音合成服务

---

## 五、竞品对比

| 特性 | remote-code-rust | Claude Code | Roo-Code | ZCode (Z.AI) | opencode | openclaw |
|------|-----------------|-------------|----------|---------------|----------|----------|
| 语言 | Rust | TypeScript | TypeScript | TypeScript | Go | TypeScript |
| 性能 | ~50ms 启动 | ~2s | ~2s | ~2s | ~100ms | ~2s |
| 内存 | ~20MB | ~200MB | ~200MB | ~200MB | ~50MB | ~200MB |
| 远程执行 | ✅ 完整 | ❌ | ❌ | ❌ | ❌ | ❌ |
| GUI | ✅ Tauri+Web | ❌ CLI only | ✅ VSCode | ✅ Web ADE | ✅ TUI | ✅ VSCode |
| Circuit Breaker | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 多 Provider Failover | ✅ | ❌ | 部分 | ❌ | ❌ | ❌ |
| 工具数量 | 62 | 55+ | 40+ | 30+ | 20+ | 30+ |
| PWA 移动端 | ✅ | ❌ | ❌ | ✅ QR 扫码 | ❌ | ❌ |
| 国际化 | ✅ 中/英 | ❌ | ❌ | ✅ 中文优先 | ❌ | ❌ |
| 多 Agent 统一架构 | ✅ 三引擎独立适配器 (26 Provider) | ❌ | ❌ | ✅ @agent 提及 | ❌ | ❌ |
| QueryEngine 执行路径 | ✅ Claude Agent | ❌ | ❌ | ❌ | ❌ | ❌ |
| 对话级 Checkpoint | 🟡 后端 crate 保留，废弃 GUI 原型已删除 | ❌ | ❌ | ✅ Review/Undo/Restore | ❌ | ❌ |
| 专业化 Agent | ✅ claude-specialized-agents | ❌ | ❌ | ✅ Markdown+YAML 定义 | ❌ | ❌ |
| 内置 Git 面板 | 🟡 `claude-git` 保留，废弃 GitPanel 原型已删除 | ❌ | ❌ | ✅ 完整 Git 管理 | ❌ | ❌ |
| 权限模式快捷切换 | 🟡 废弃 GUI 原型已删除 | ❌ | ❌ | ✅ Shift+Tab 4 模式 | ❌ | ❌ |

**独有优势**: Rust 原生性能 + 三引擎独立 in-process 适配器架构 (Claude/Codex/Roo 26 Provider) + 分布式远程执行 + Circuit Breaker + PWA 移动端 + 多 Provider 故障转移 + 62 内置工具 + IDE 级 GUI (ActivityBar/Terminal/Diff/Preview/Command Palette) + ZCode 启发功能 (Checkpoint/专业化 Agent/Git 面板)

---

## 六、安全机制

| 机制 | 实现 |
|------|------|
| `unsafe_code = "allow"` | 平台 FFI 允许；新增 unsafe 需 code review，CI 继续执行 clippy/audit |
| `unwrap_used = "warn"` | 生产代码不建议 unwrap |
| `todo!` / `dbg!` / `unimplemented!` 禁止 | 防止调试/未实现代码进入生产 |
| OS Keychain | Tauri 桌面端 API key 存储在系统钥匙串 |
| SHA-256 哈希 | Bootstrap secret 不明文存储 |
| Bearer Token | 所有 `/v1/*` API 受认证保护 |
| 一次性 Stream Ticket | WebSocket 事件流不再把长期 token 放入 URL query |
| Relay-only 默认模式 | 手机/Web 默认只经控制平面中继，直连 runner 需要显式高级开关 |
| User-key Hash Allowlist | 派生 user-key 只接受 `REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES` 配置的 SHA-256 哈希 |
| 设备信任链 | Bootstrap → 配对 → 二次授权 |
| URL 敏感参数清理 | Web 端自动移除 URL 中的 token/secret |
| Mutex poison recovery | 所有 Mutex 使用 `into_inner()` 恢复，不 panic |
| Error Boundary | React 渲染崩溃捕获 |
| Permission Bypass Killswitch | `bypass_killswitch.rs` 安全兜底 |
| Dangerous Pattern Detection | `dangerous_patterns.rs` 恶意命令检测 |
| Path Validation | `path_validation.rs` 工作区路径边界检查 |

---

## 七、部署架构

```
┌──────────────────┐     ┌──────────────────────────────────┐
│   用户设备        │     │        腾讯云服务器               │
│                  │     │                                  │
│  ┌────────────┐  │     │  ┌────────────────────────────┐  │
│  │ Tauri GUI  │  │     │  │     Caddy (反代 + HTTPS)    │  │
│  │ (桌面端)    │  │     │  └──────────┬─────────────────┘  │
│  └────────────┘  │     │             │                     │
│                  │     │  ┌──────────▼─────────────────┐  │
│  ┌────────────┐  │     │  │   Control Plane (8787)      │  │
│  │ PWA/Web    │◄─┼─────┼─►│   - REST API                │  │
│  │ (手机/浏览器)│  │     │  │   - WebSocket 事件流        │  │
│  └────────────┘  │     │  │   - SQLite 持久化           │  │
│                  │     │  └──────────────────────────────┘  │
│  ┌────────────┐  │     │                                  │
│  │ Runner     │  │     │                                  │
│  │ (本地执行)  │◄─┼─────┼── 心跳 + 命令拉取 + 事件上报     │
│  └────────────┘  │     │                                  │
└──────────────────┘     └──────────────────────────────────┘
```

---

## 八、文档索引

| 文档 | 说明 |
|------|------|
| [ARCHITECTURE.md](../ARCHITECTURE.md) | 完整架构设计文档（英文） |
| [COMPATIBILITY.md](../COMPATIBILITY.md) | 兼容性说明（英文） |
| [PROVENANCE.md](../PROVENANCE.md) | 来源证明（英文） |
| [ROADMAP.md](../ROADMAP.md) | 路线图（英文） |
| [multi-agent-architecture.md](multi-agent-architecture.md) | 多 Agent 架构设计（已过时，仅供参考） |
| [coding-plan-support.md](coding-plan-support.md) | 国内 Coding Plan 供应商参考 |
| [gui-redesign-plan.md](gui-redesign-plan.md) | GUI 重设计计划（已完成） |
| [zcode-inspiration-plan.md](zcode-inspiration-plan.md) | ZCode 启发分析与实施计划（已完成） |
| [archive/](archive/) | 归档文档 |
| 本文档 | 项目状态、路线图、竞品对比 |
