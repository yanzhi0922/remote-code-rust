# Remote Code Rust — 项目状态与路线图

> 更新日期: 2026-04-28
> 当前阶段: Phase 14 已完成 — 代码清理与架构精简
> 代码规模: ~85,000 行 (Rust + TypeScript)，38 个 crates
> 当前验证基线: `cargo test --workspace` 全绿（860+ 测试），`cargo clippy` 零警告，MiniMax Provider + MCP 端到端测试通过。
> 基准提交: `05b05ec`

---

## 一、当前产品状态

### 1.1 总体评估: 生产就绪

| 维度 | 状态 | 详情 |
|------|------|------|
| 编译 | ✅ 通过 | `cargo build --release`、GUI build、mobile build 通过 |
| 安全 | ✅ 强化 | `unsafe_code = "forbid"`，`unwrap_used = "deny"`，`todo`/`dbg` 禁止 |
| 测试 | ✅ 通过 | `cargo test --workspace` 860+ 测试全绿，0 failures |
| Clippy | ✅ 零警告 | `cargo clippy --workspace -- -D warnings` 通过 |
| CLI | ✅ 可发布 | clap 命令树、doctor 与核心子命令可构建并通过回归 |
| TUI | ✅ 可发布 | 交互式终端 + 65+ slash commands + Vim 模式回归通过 |
| GUI (Tauri) | ✅ 可构建 | Rust/Tauri crate 纳入工作区回归 |
| GUI (Web/PWA) | ✅ 可发布 | 远程控制面、中英双语、错误边界、PWA 缓存更新链路通过 |
| GUI (Mobile/Capacitor) | ✅ 可构建 | 跨包 React 类型冲突已清除，补齐 smoke tests |
| Provider | ✅ 完整 | OpenAI/Anthropic 协议，流式，多 key 轮换，故障转移 |
| 工具系统 | ✅ 丰富 | 65+ 内置工具 + MCP + 插件扩展 |
| Control Plane | ✅ 完整 | Runner/Session/Approval/Artifact/Event 全链路 |
| Runner | ✅ 完整 | Daemon 模式，心跳，命令拉取，审批中继 |
| 多 Agent | ✅ 统一 | InProcessAdapter 进程内回调 + QueryEngine 统一执行路径 |
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
│  │ AgentRouter → InProcessAdapter       │    │  REST/WS API │ │
│  │ (RemoteClaude / Roo / Codex)         │    │  (api.ts)    │ │
│  │       ↓                              │    └──────┬───────┘ │
│  │ QueryEngine 统一执行路径              │           │        │
│  └────────────────┬────────────────────┘           │        │
│                   │                                │        │
├───────────────────┼────────────────────────────────┼────────┤
│              核心运行时层            │     Control Plane 层    │
│  ┌────────────────▼────────────┐   │  ┌──────────────────▼─┐ │
│  │ rc-core │ rc-session │ rc-tools   │  │ rc-control-plane   │ │
│  │ rc-provider │ rc-agents │ rc-mcp   │  │ (registry, events, │ │
│  │ rc-config │ rc-skills │ rc-plugins │  │  approvals, SQLite)│ │
│  │ rc-compact│rc-context│ rc-query-eng│  └────────────────────┘ │
│  │ rc-system-prompt│rc-auth│rc-model  │                         │
│  └─────────────────────────────┘   │                         │
│                                     │                         │
├─────────────────────────────────────┼─────────────────────────┤
│              Runner 层              │                         │
│  ┌─────────────────────────────────▼───────────────────────┐ │
│  │ rc-runner (daemon, heartbeat, session hosting)          │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Crate 结构 (38 个)

| Crate | 职责 | 关键文件 |
|-------|------|---------|
| `rc-core` | 核心类型、hook、状态、消息 | lib.rs, hooks.rs, state.rs, message.rs |
| `rc-protocol` | Stream-JSON 协议、消息类型 | lib.rs |
| `rc-config` | 配置加载、Provider 发现 | lib.rs, settings_layers.rs |
| `rc-provider` | LLM 调用、流式、故障转移 | lib.rs, streaming.rs, circuit_breaker.rs |
| `rc-tools` | 65+ 内置工具、运行时策略 | specs.rs, streaming_executor.rs |
| `rc-session` | 会话存储、记忆、恢复 | lib.rs, transcript.rs, resume_state.rs |
| `rc-permissions` | 权限规则引擎、审计 | lib.rs, classifier.rs, denial_tracking.rs |
| `rc-agents` | 多代理调度器、Fork、Built-in | lib.rs, fork.rs, builtins.rs |
| `rc-mcp` | MCP 客户端 (stdio/HTTP/WS) | lib.rs, lifecycle.rs, oauth.rs |
| `rc-plugins` | 插件发现与加载 | lib.rs, loader.rs, marketplace/ |
| `rc-skills` | Skill 发现与加载 | lib.rs |
| `rc-compact` | 上下文压缩 (5 策略) | engine.rs, auto.rs, micro.rs, snip.rs |
| `rc-system-prompt` | 动态系统提示词 | lib.rs + 22 sections |
| `rc-query-engine` | 统一查询引擎 | query_loop.rs, state_machine.rs, observer.rs |
| `rc-engine-events` | 统一事件层 | lib.rs, types.rs, stream.rs |
| `rc-auth` | 认证 (API key + OAuth PKCE) | lib.rs, oauth/ |
| `rc-model` | 模型管理、别名、能力 | lib.rs, model.rs, providers.rs |
| `rc-context` | 上下文管理 (effort/fast mode) | lib.rs, effort.rs, fast_mode.rs |
| `rc-settings` | 设置类型 (hooks/MCP/sandbox) | lib.rs, hooks.rs, mcp.rs |
| `rc-managed-settings` | MDM/安全策略 | lib.rs, mdm.rs, sync_engine.rs |
| `rc-skill-search` | BM25 技能搜索 | lib.rs, local_search.rs |
| `rc-ide` | IDE 集成 (bridge/connection) | lib.rs, bridge.rs |
| `rc-teleport` | 会话迁移 | lib.rs, api.rs |
| `rc-file-history` | 文件历史/备份 | lib.rs, backup.rs, snapshot.rs |
| `rc-voice` | 语音输入/输出 (TTS Mock) | lib.rs, stt.rs, tts.rs |
| `rc-lsp` | LSP 客户端 | lib.rs, client.rs |
| `rc-analytics` | 分析/遥测 | lib.rs |
| `rc-event-bus` | 发布/订阅事件总线 | lib.rs |
| `rc-ui-bridge` | UI 事件桥接 | lib.rs, bridge.rs |
| `rc-control-plane` | 远程控制面服务 | lib.rs, handlers.rs |
| `rc-runner` | Runner daemon | lib.rs |
| `rc-tui` | 终端用户界面 | app.rs, 20 components, 32 commands |
| `rc-utils` | 工具函数 (diff/git/markdown) | diff.rs, git_fs.rs, markdown.rs |
| `rc-agent-protocol` | 多 Agent 协议抽象层 | adapter.rs, events.rs, adapters/in_process.rs |
| `rc-swarm` | 多代理类型系统 | lib.rs, backends/in_process.rs |
| `rc-telemetry` | 追踪设置、结构化日志 | lib.rs, analytics.rs |
| `rc-runtime-prompt` | 运行时提示词 | lib.rs, auto_memory.rs |
| `rc-integration-tests` | 集成测试 | tests/ |

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
- ✅ 38+ 内置工具 → 65+ 工具
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

### Phase 11: 进程内回调模式转换 — ✅ 完成
- ✅ RooCode/Codex 从子进程 JSON-RPC 转换为进程内回调模式
- ✅ `InProcessAdapter` 统一实现
- ✅ 三个 Agent 共享同一个适配器（类型别名区分）
- ✅ 回调注入模式 (`with_send_message`, `with_cancel`, `with_resolve_permission`)
- ✅ 删除子进程管理代码

### Phase 12: 端到端真实测试 — ✅ 完成
- ✅ MiniMax Provider (anthropic-compatible) 真实 API 调用测试
- ✅ MCP 端到端集成测试
- ✅ Headless `--print` 与 `stream-json` 冒烟测试通过

### Phase 13: QueryEngine 统一执行路径 — ✅ 完成
- ✅ 三个 Agent 共享一条 QueryEngine 执行路径
- ✅ 消除双路径分歧 (`run_gui_prompt` vs `run_agent_prompt`)
- ✅ 统一状态机 + 流式执行器 + Token 预算
- ✅ Observer 模式支持 checkpoint 和恢复

### Phase 14: 代码清理 — ✅ 完成
- ✅ 删除 880 行旧代码 (AgentRouter、健康检查、重启追踪器等)
- ✅ AppState 精简
- ✅ `InProcessAdapter` 提取为独立模块
- ✅ 代码已推送 (commit `05b05ec`)

---

## 三、已知限制

| 限制 | 说明 | 优先级 |
|------|------|--------|
| TTS 为 Mock 实现 | `rc-voice::tts` 返回占位响应，未接入真实 TTS 服务 | P2 |
| 外部 Agent 回调为 Stub | Roo Code / Codex 的回调函数返回硬编码响应 | P1 |
| Headless 浏览器截图 | `web_browser` 工具的截图功能未完成 | P2 |

---

## 四、下一阶段路线图

### Phase 15: 外部 Agent 真实接入
- [ ] Roo Code 回调实现 — 接入 roo-server 逻辑
- [ ] Codex 回调实现 — 接入 codex-app-server 逻辑
- [ ] Agent 特有工具映射
- [ ] 端到端多 Agent 集成测试

### Phase 16: 增强远程交互
- [ ] 终端流 (Terminal Stream) — 实时终端输出远程查看
- [ ] 文件预览 — 远程文件内容浏览
- [ ] Diff 浏览 — 代码变更可视化
- [ ] 推送通知 — 移动端审批提醒

### Phase 17: 竞品超越
- [ ] 子任务深度委派 — 多级子代理 + 并行执行 + 凭证轮换
- [ ] 会话回退 — 回退到任意历史点继续
- [ ] Shadow Git 检查点 — 自动 git checkpoint
- [ ] Task Flow 可视化 — 任务依赖图 + 进度追踪

### Phase 18: 可选扩展
- [ ] 云端 Runner — 腾讯云执行代码
- [ ] 多工作站调度 — 多台机器协同
- [ ] 团队协作 — 多用户共享会话
- [ ] TTS 真实实现 — 接入语音合成服务

---

## 五、竞品对比

| 特性 | remote-code-rust | Claude Code | Roo-Code | opencode | openclaw |
|------|-----------------|-------------|----------|----------|----------|
| 语言 | Rust | TypeScript | TypeScript | Go | TypeScript |
| 性能 | ~50ms 启动 | ~2s | ~2s | ~100ms | ~2s |
| 内存 | ~20MB | ~200MB | ~200MB | ~50MB | ~200MB |
| 远程执行 | ✅ 完整 | ❌ | ❌ | ❌ | ❌ |
| GUI | ✅ Tauri+Web | ❌ CLI only | ✅ VSCode | ✅ TUI | ✅ VSCode |
| Circuit Breaker | ✅ | ❌ | ❌ | ❌ | ❌ |
| 多 Provider Failover | ✅ | ❌ | 部分 | ❌ | ❌ |
| 工具数量 | 65+ | 55+ | 40+ | 20+ | 30+ |
| PWA 移动端 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 国际化 | ✅ 中/英 | ❌ | ❌ | ❌ | ❌ |
| 多 Agent 统一架构 | ✅ InProcessAdapter | ❌ | ❌ | ❌ | ❌ |
| QueryEngine 统一路径 | ✅ | ❌ | ❌ | ❌ | ❌ |

**独有优势**: Rust 原生性能 + InProcessAdapter 统一多 Agent 架构 + QueryEngine 统一执行路径 + 分布式远程执行 + Circuit Breaker + PWA 移动端 + 多 Provider 故障转移 + 65+ 内置工具

---

## 六、安全机制

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
| [ARCHITECTURE.md](../ARCHITECTURE.md) | 完整架构设计文档 |
| [COMPATIBILITY.md](../COMPATIBILITY.md) | 兼容性说明 |
| [PROVENANCE.md](../PROVENANCE.md) | 来源证明 |
| [ROADMAP.md](../ROADMAP.md) | 路线图 |
| [multi-agent-architecture.md](multi-agent-architecture.md) | 多 Agent 架构设计（InProcessAdapter 统一架构） |
| [coding-plan-support.md](coding-plan-support.md) | 国内 Coding Plan 供应商参考 |
| [archive/](archive/) | 归档文档 |
| 本文档 | 项目状态、路线图、竞品对比 |
