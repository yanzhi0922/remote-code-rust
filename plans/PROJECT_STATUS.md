# Remote Code Rust — 项目状态与路线图

> 更新日期: 2026-04-14
> 当前阶段: Phase 4 — Remote Beta Hardening
> 代码规模: ~53,000 行 (Rust + TypeScript)
> 当前验证基线: `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `apps/remote-code-gui npm test` / `apps/remote-code-mobile npm test` / GUI & mobile build 全部通过

## Claude Parity Track

并行中的 Claude Code 全量复刻路线已从纯研究进入主干骨架阶段：

- 已新增 `rc-transcript`、`rc-query-engine`，并升级 `rc-engine-events` 为“运行时兼容事件 + EngineEvent/EventStream”双层结构。
- 已为 `rc-core` 补齐 v2 类型层（品牌 ID、Message 联合类型、AppState、Usage/Cost、扩展 hook 类型）。
- 已为 `rc-session` 接入 transcript V2 兼容读写 API，保持现有 session/NDJSON 主路径稳定。
- 当前 Rust 工作区回归仍保持全绿：`cargo test --workspace` 通过。

---

## 一、当前产品状态

### 1.1 总体评估: 已通过发布候选级基线验证

| 维度 | 状态 | 详情 |
|------|------|------|
| 编译 | ✅ 通过 | `cargo build`、GUI build、mobile build 当前基线通过 |
| 安全 | ✅ 强化 | `unsafe_code = "forbid"`，`unwrap_used = "deny"`，控制面对公网暴露新增 fail-closed 校验 |
| 测试 | ✅ 通过 | `cargo test --workspace`、GUI 41 测试、mobile 4 测试全部通过 |
| CLI | ✅ 可发布 | clap 命令树、doctor 与核心子命令可构建并通过工作区回归 |
| TUI | ✅ 可发布 | 交互式终端与 slash commands 回归通过 |
| GUI (Tauri) | ✅ 可构建 | Rust/Tauri crate 纳入工作区回归 |
| GUI (Web/PWA) | ✅ 可发布 | 远程控制面、中英双语、错误边界、PWA 缓存更新链路通过 |
| GUI (Mobile/Capacitor) | ✅ 可构建 | 跨包 React 类型冲突已清除，补齐第一方 smoke tests |
| Provider | ✅ 完整 | OpenAI/Anthropic 协议，流式，多 key 轮换，故障转移 |
| 工具系统 | ✅ 丰富 | 30+ 内置工具 + MCP + 插件扩展 |
| Control Plane | ✅ 完整 | Runner/Session/Approval/Artifact/Event 全链路，公网配置安全检查已收紧 |
| Runner | ✅ 完整 | Daemon 模式，心跳，命令拉取，审批中继 |
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
│       │         核心运行时层         │                │        │
│  ┌────▼─────────────▼──────────────▼────┐    ┌──────▼───────┐ │
│  │ rc-core │ rc-session │ rc-tools     │    │  REST/WS API │ │
│  │ rc-agents│ rc-permissions│ rc-mcp   │    │  (api.ts)    │ │
│  │ rc-config│ rc-skills │ rc-plugins   │    └──────┬───────┘ │
│  └────────────────┬────────────────────┘           │        │
│                   │                                │        │
├───────────────────┼────────────────────────────────┼────────┤
│              Provider 层           │     Control Plane 层    │
│  ┌────────────────▼────────────┐   │  ┌──────────────────▼─┐ │
│  │ rc-provider                 │   │  │ rc-control-plane   │ │
│  │ (circuit breaker, failover, │   │  │ (registry, events, │ │
│  │  credential pool, streaming)│   │  │  approvals, SQLite)│ │
│  └─────────────────────────────┘   │  └────────────────────┘ │
│                                     │                         │
├─────────────────────────────────────┼─────────────────────────┤
│              Runner 层              │                         │
│  ┌─────────────────────────────────▼───────────────────────┐ │
│  │ rc-runner (daemon, heartbeat, session hosting)          │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Crate 结构 (15 个)

| Crate | 职责 | 测试数 |
|-------|------|--------|
| `rc-core` | 核心类型、任务栈、系统提示 | 28 |
| `rc-protocol` | 协议定义、消息类型 | 85 |
| `rc-config` | 配置加载、Provider 发现 | 13 |
| `rc-provider` | LLM 调用、流式、故障转移 | 14 |
| `rc-tools` | 30+ 内置工具 | 96 |
| `rc-session` | 会话存储、记忆、恢复 | 34 |
| `rc-permissions` | 权限规则引擎、审计 | 19 |
| `rc-agents` | 多代理调度器 | 5 |
| `rc-mcp` | MCP 客户端 (stdio/HTTP/WS) | 10 |
| `rc-plugins` | 插件发现与加载 | 12 |
| `rc-skills` | Skill 发现与加载 | 13 |
| `rc-telemetry` | 指标收集 | 2 |
| `rc-event-bus` | 发布/订阅事件总线 | 9 |
| `rc-ui-bridge` | UI 事件桥接 | 8 |
| `rc-control-plane` | 远程控制面服务 | 66 |
| `rc-runner` | Runner daemon | 12 |
| `rc-tui` | 终端用户界面 | 13 |

### 1.4 工具清单 (30+)

#### 文件操作
- `read_file`, `write_file`, `edit_file`, `replace_in_file` — 文件读写编辑
- `list_directory` — 目录列表
- `search_text` / `grep` — 内容搜索 (ripgrep 风格)
- `glob` — Glob 模式文件搜索

#### 执行环境
- `bash_command` — Shell 命令执行 (含沙箱)
- `powershell` — PowerShell 执行
- `sandbox` — 沙箱隔离执行

#### 开发工具
- `agent` — 子代理委派
- `delegate` — 任务委派
- `lsp` — LSP 集成 (定义/引用/悬停)
- `git` — Git 操作
- `notebook_edit` — Jupyter Notebook 编辑

#### 任务与计划
- `todo_write` — 任务列表管理
- `task_create/get/list/stop/update` — 后台任务系统
- `plan_mode_enter/exit` — 计划模式

#### 网络与搜索
- `web_search`, `web_fetch` — 网络搜索与获取
- `tool_search` — BM25 工具搜索

#### 系统与运维
- `mcp_*` — MCP 工具调用/资源读取
- `skill_discover` — Skill 发现
- `memory_read/write` — 记忆系统
- `cron_schedule` — 定时任务
- `workflow_create/status` — 工作流编排
- `enter/exit_worktree` — Git worktree 管理
- `suggest_pr` — PR 建议
- `snip` — 代码片段
- `repl` — REPL 交互
- `voice_input` — 语音输入
- `remote_trigger` — 远程触发
- `daemon_start/stop` — 守护进程管理
- `send_message` — 跨代理消息
- `synthetic_output` — 合成输出

---

## 二、安全机制

| 机制 | 实现 |
|------|------|
| `unsafe_code = "forbid"` | 全局禁止 unsafe 代码 |
| `unwrap_used = "deny"` | 生产代码禁止 unwrap |
| `todo!` / `dbg!` 禁止 | 防止调试代码进入生产 |
| OS Keychain | Tauri 桌面端 API key 存储在系统钥匙串 |
| SHA-256 哈希 | Bootstrap secret 不明文存储 |
| Bearer Token | 所有 `/v1/*` API 受认证保护 |
| 设备信任链 | Bootstrap → 配对 → 二次授权 |
| URL 敏感参数清理 | Web 端自动移除 URL 中的 token/secret |
| Mutex poison recovery | 所有 Mutex 使用 `into_inner()` 恢复，不 panic |
| Error Boundary | React 渲染崩溃捕获 |

---

## 三、稳定性机制

| 机制 | 实现 |
|------|------|
| Circuit Breaker | 三态断路器 (Closed → Open → HalfOpen) |
| Credential Pool | 多 API key 轮询 + 自动 failover |
| WebSocket 重连 | 1 秒自动重连 + sequence-based 断点续传 |
| SQLite 持久化 | Control plane 状态原子写入 |
| Timeline 广播 | broadcast::channel (buffer=256) 多订阅者 |
| 事件溯源 | 单调递增 sequence，所有客户端基于 `after` 回放 |
| Provider 超时 | 请求级别超时控制 |
| 会话恢复 | 会话状态持久化 + 重启后恢复 |

---

## 四、已完成阶段回顾

### Phase 1: 核心基础设施
- ✅ 15 crate 模块化架构
- ✅ Provider 调用 (OpenAI/Anthropic)
- ✅ 会话管理 (SQLite)
- ✅ 权限系统 (5 模式)
- ✅ MCP 客户端 (3 种传输)
- ✅ 插件系统

### Phase 2: 工具与交互
- ✅ 30+ 内置工具
- ✅ TUI 交互式终端 (14 slash 命令)
- ✅ 上下文管理 (8 种压缩)
- ✅ 成本追踪
- ✅ 记忆系统
- ✅ 多代理调度器

### Phase 3: 远程架构
- ✅ Control Plane (Runner/Session/Approval/Artifact/Event)
- ✅ Runner Daemon (心跳/命令拉取/审批中继)
- ✅ SQLite 持久化
- ✅ WebSocket 实时事件流
- ✅ Bootstrap 认证 + 设备配对

### Phase 4: GUI 与远程客户端
- ✅ Tauri v2 桌面客户端 (React + Zustand)
- ✅ Web/PWA 远程客户端 (中英双语)
- ✅ Service Worker 离线缓存
- ✅ 响应式移动端适配
- ✅ Artifact 下载
- ✅ 实时审批卡片
- ✅ 腾讯云部署方案

---

## 五、下一阶段路线图

### Phase 5: 增强远程交互
- [ ] 终端流 (Terminal Stream) — 实时终端输出远程查看
- [ ] 文件预览 — 远程文件内容浏览
- [ ] Diff 浏览 — 代码变更可视化
- [ ] 推送通知 — 移动端审批提醒
- [ ] 原生手机壳 — iOS/Android WebView wrapper

### Phase 6: 竞品超越
- [ ] 子任务深度委派 — 多级子代理 + 并行执行 + 凭证轮换
- [ ] 会话回退 — 回退到任意历史点继续
- [ ] Shadow Git 检查点 — 自动 git checkpoint
- [ ] LLM 上下文压缩 — 智能摘要替代规则压缩
- [ ] Task Flow 可视化 — 任务依赖图 + 进度追踪
- [ ] 安全沙箱增强 — macOS Seatbelt / Linux Landlock

### Phase 7: 可选扩展
- [ ] 云端 Runner — 腾讯云执行代码
- [ ] 多工作站调度 — 多台机器协同
- [ ] 团队协作 — 多用户共享会话
- [ ] External Agent 适配器 — 接入第三方 Agent

---

## 六、竞品对比

| 特性 | remote-code-rust | Claude Code | Roo-Code | opencode | openclaw |
|------|-----------------|-------------|----------|----------|----------|
| 语言 | Rust | TypeScript | TypeScript | Go | TypeScript |
| 性能 | ~50ms 启动 | ~2s | ~2s | ~100ms | ~2s |
| 内存 | ~20MB | ~200MB | ~200MB | ~50MB | ~200MB |
| 远程执行 | ✅ 完整 | ❌ | ❌ | ❌ | ❌ |
| GUI | ✅ Tauri+Web | ❌ CLI only | ✅ VSCode | ✅ TUI | ✅ VSCode |
| Circuit Breaker | ✅ | ❌ | ❌ | ❌ | ❌ |
| 多 Provider Failover | ✅ | ❌ | 部分 | ❌ | ❌ |
| 工具数量 | 30+ | 55+ | 40+ | 20+ | 30+ |
| PWA 移动端 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 国际化 | ✅ 中/英 | ❌ | ❌ | ❌ | ❌ |

**独有优势**: Rust 原生性能 + 分布式远程执行架构 + Circuit Breaker + PWA 移动端 + 多 Provider 故障转移

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
| [`REMOTE_PLAN.md`](REMOTE_PLAN.md) | 远程控制 v1 架构主计划 |
| [`coding-plan-support.md`](coding-plan-support.md) | 国内 Coding Plan 供应商参考 |
| [`tauri-gui-architecture-design.md`](tauri-gui-architecture-design.md) | GUI 架构设计方案 |
| [`gui-remote-advanced-optimization-v2.md`](gui-remote-advanced-optimization-v2.md) | GUI 与 Remote 进阶优化方案 v2（基于当前代码真实状态的正式实施稿，已取代并删除 v1） |
| 本文档 | 项目状态、路线图、竞品对比 |
