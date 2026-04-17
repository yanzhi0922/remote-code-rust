# Remote Code Rust — 项目状态与路线图

> 更新日期: 2026-04-17
> 当前阶段: Phase 4 — Claude Parity Hardening（已完成核心骨架 + 测试验证基线）
> 代码规模: ~85,000 行 (Rust + TypeScript)，30+ crates
> 当前验证基线: `cargo test --workspace` 全绿（3,811+ 测试），MiniMax anthropic-compatible headless `--print` 与 `stream-json` 冒烟通过。

## Claude Parity Track — 当前覆盖度

基于 `plans/claude-code-rust-full-clone-plan.md` §21 全量差距清单的覆盖状态：

| 维度 | 覆盖度 | 详情 |
|------|--------|------|
| 工具系统 | **~95%** | 65+ 内置工具（含 Phase 9 扩展），覆盖 §21.1 全部 8 个"缺失工具" |
| 系统提示词 | **~90%** | 22 个 section 文件，覆盖 §21.2 全部 23 个段落 |
| 斜杠命令 | **~80%** | 65+ TUI slash commands，覆盖 §21.3 大部分命令 |
| 查询引擎 | **~75%** | `rc-query-engine` 状态机 + observer/checkpoint + streaming seam |
| API 客户端 | **~70%** | 流式 + fallback + circuit breaker + credential pool + request continuity |
| 上下文压缩 | **~85%** | 5 种策略（auto/micro/snip/reactive/collapse）+ session memory |
| Agent 系统 | **~80%** | Fork + Built-in 6 agents + Coordinator/Worker + Scheduler |
| MCP | **~70%** | stdio/HTTP/WS + OAuth + Elicitation + lifecycle + reconnect |
| 权限系统 | **~90%** | 5 模式 + 分类器 + 自动模式 + 拒绝追踪 + shadowed detection |
| Hook 系统 | **~85%** | 27 个 hook 事件 + 4 种 hook 类型（bash/prompt/agent/http） |
| 会话存储 | **~85%** | SQLite + NDJSON + Transcript V2 + resume state |
| 插件系统 | **~75%** | 发现/加载/管理/市场 + autoupdate + blocklist + LSP/MCP 集成 |
| TUI | **~70%** | ratatui 交互式终端 + 20 组件 + 32 命令模块 + Vim 模式 |
| 设置管理 | **~80%** | 多层 settings + source policy + managed settings + MDM |

### 已填补的 §21 关键 Gap

| §21 编号 | 原始 Gap | 当前状态 |
|----------|---------|---------|
| §21.1.1 #1 TaskOutputTool | 完全缺失 | ✅ `task_get` + `task_output` |
| §21.1.1 #2 TeamDeleteTool | 完全缺失 | ✅ `team_delete` + `team_list` |
| §21.1.1 #3 DiscoverSkillsTool | 完全缺失 | ✅ `discover_skills` (BM25 + rc-skill-search) |
| §21.1.1 #4 SendUserFileTool | 完全缺失 | ✅ `send_user_file` |
| §21.1.1 #5 ReviewArtifactTool | 完全缺失 | ✅ `review_artifact` |
| §21.1.1 #6 BriefTool | 完全缺失 | ✅ `brief` |
| §21.1.1 #7 VerifyPlanExecutionTool | 完全缺失 | ✅ `verify_plan` |
| §21.1.1 #8 ConfigTool | 部分覆盖 | ✅ `config_read` + `/config` slash command |
| §21.2 系统提示词 | 15% 覆盖 | ✅ 22 section 文件（~90% 覆盖） |
| §21.3 斜杠命令 | 35% 覆盖 | ✅ 65+ commands（~80% 覆盖） |
| §21.4.1 Coordinator/Worker | 完全缺失 | ✅ `rc-agents::coordinator` + `worker` |
| §21.4.2 Fork Subagent | 完全缺失 | ✅ `rc-agents::fork` (715 行) |
| §21.4.6 Dynamic Beta Headers | 完全缺失 | ✅ `rc-provider::beta_headers` |
| §21.4.7 Attribution Header | 完全缺失 | ✅ `rc-provider::attribution` |
| §21.4.14 Fast Mode / Effort | 完全缺失 | ✅ `rc-context::fast_mode` + `effort` |
| §21.4.15 Output Styles | 完全缺失 | ✅ `rc-tui::output_styles` + plugins load |
| §21.4.16 IDE Integration | 完全缺失 | ✅ `rc-ide` (bridge + connection + messaging) |
| §21.4.18 Plugin Autoupdate | 完全缺失 | ✅ `rc-plugins::autoupdate` |
| §21.5 Hook 事件 | 部分覆盖 | ✅ 27 个事件（覆盖 Claude Code 54 种的 ~50%） |
| §21.6 Provider/API | 部分覆盖 | ✅ thinking blocks + cache headers + effort params + server tool use |

---

## 一、当前产品状态

### 1.1 总体评估: 已通过发布候选级基线验证

| 维度 | 状态 | 详情 |
|------|------|------|
| 编译 | ✅ 通过 | `cargo build --release`、GUI build、mobile build 通过 |
| 安全 | ✅ 强化 | `unsafe_code = "forbid"`，`unwrap_used = "deny"`，`todo`/`dbg` 禁止 |
| 测试 | ✅ 通过 | `cargo test --workspace` 3,811+ 测试全绿，0 failures |
| CLI | ✅ 可发布 | clap 命令树、doctor 与核心子命令可构建并通过回归 |
| TUI | ✅ 可发布 | 交互式终端 + 65+ slash commands + Vim 模式回归通过 |
| GUI (Tauri) | ✅ 可构建 | Rust/Tauri crate 纳入工作区回归 |
| GUI (Web/PWA) | ✅ 可发布 | 远程控制面、中英双语、错误边界、PWA 缓存更新链路通过 |
| GUI (Mobile/Capacitor) | ✅ 可构建 | 跨包 React 类型冲突已清除，补齐 smoke tests |
| Provider | ✅ 完整 | OpenAI/Anthropic 协议，流式，多 key 轮换，故障转移 |
| 工具系统 | ✅ 丰富 | 65+ 内置工具 + MCP + 插件扩展 |
| Control Plane | ✅ 完整 | Runner/Session/Approval/Artifact/Event 全链路 |
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
│  │ rc-compact│rc-context│ rc-query-eng │           │        │
│  │ rc-system-prompt│rc-auth│rc-model   │           │        │
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

### 1.3 Crate 结构 (30+)

| Crate | 职责 | 关键文件 |
|-------|------|---------|
| `rc-core` | 核心类型、hook、状态、消息 | lib.rs, hooks.rs, state.rs, message.rs |
| `rc-protocol` | Stream-JSON 协议、消息类型 | lib.rs (1098 行) |
| `rc-config` | 配置加载、Provider 发现 | lib.rs, settings_layers.rs |
| `rc-provider` | LLM 调用、流式、故障转移 | lib.rs, streaming.rs, circuit_breaker.rs |
| `rc-tools` | 65+ 内置工具、运行时策略 | specs.rs (1165 行), streaming_executor.rs |
| `rc-session` | 会话存储、记忆、恢复 | lib.rs, transcript.rs, resume_state.rs |
| `rc-permissions` | 权限规则引擎、审计 | lib.rs, classifier.rs, denial_tracking.rs |
| `rc-agents` | 多代理调度器、Fork、Built-in | lib.rs (1061 行), fork.rs, builtins.rs |
| `rc-mcp` | MCP 客户端 (stdio/HTTP/WS) | lib.rs, lifecycle.rs, oauth.rs |
| `rc-plugins` | 插件发现与加载 | lib.rs, loader.rs, marketplace/ |
| `rc-skills` | Skill 发现与加载 | lib.rs (498 行) |
| `rc-compact` | 上下文压缩 (5 策略) | engine.rs, auto.rs, micro.rs, snip.rs |
| `rc-system-prompt` | 动态系统提示词 | lib.rs (514 行) + 22 sections |
| `rc-query-engine` | 查询引擎状态机 | query_loop.rs, state_machine.rs, observer.rs |
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
| `rc-voice` | 语音输入/输出 | lib.rs, stt.rs, tts.rs |
| `rc-lsp` | LSP 客户端 | lib.rs, client.rs |
| `rc-analytics` | 分析/遥测 | lib.rs, sink.rs, event_logger.rs |
| `rc-event-bus` | 发布/订阅事件总线 | lib.rs |
| `rc-ui-bridge` | UI 事件桥接 | lib.rs (793 行), bridge.rs |
| `rc-control-plane` | 远程控制面服务 | lib.rs, handlers.rs |
| `rc-runner` | Runner daemon | lib.rs |
| `rc-tui` | 终端用户界面 | app.rs, 20 components, 32 commands |
| `rc-utils` | 工具函数 (diff/git/markdown) | diff.rs, git_fs.rs, markdown.rs |

### 1.4 工具清单 (65+)

#### 文件操作
- `read_file`, `write_file`, `edit_file`, `replace_in_file` — 文件读写编辑
- `list_directory` — 目录列表
- `search_text` / `grep` — 内容搜索 (ripgrep 风格)
- `glob` — Glob 模式文件搜索

#### 执行环境
- `bash_command` — Shell 命令执行 (含沙箱)
- `powershell` — PowerShell 执行
- `repl` — REPL 交互
- `monitor` — 文件/进程监控
- `terminal_capture` — 终端输出捕获

#### 开发工具
- `agent` — 子代理委派
- `lsp` — LSP 集成 (定义/引用/悬停)
- `notebook_edit` — Jupyter Notebook 编辑
- `tool_search` — BM25 工具搜索
- `verify_plan` — 计划验证

#### 任务与计划
- `todo_write` — 任务列表管理
- `task_create/get/list/stop/update` — 后台任务系统
- `enter_plan_mode` / `exit_plan_mode` — 计划模式
- `workflow` — 工作流编排

#### 网络与搜索
- `web_search`, `web_fetch`, `web_browser` — 网络搜索与获取
- `discover_skills` — BM25 技能搜索

#### 系统与运维
- `mcp_call`, `mcp_auth`, `list_mcp_resources`, `read_mcp_resource` — MCP 工具
- `skill_discover`, `skill_execute` — Skill 系统
- `memory_read/write` — 记忆系统
- `schedule_cron` — 定时任务
- `enter/exit/list_worktree` — Git worktree 管理
- `suggest_pr` — PR 建议
- `snip` — 代码片段
- `brief` — 简报模式
- `ctx_inspect` — 上下文检查

#### 多代理与协作
- `team_create`, `team_delete`, `team_status`, `team_list` — 团队管理
- `send_message`, `broadcast_message`, `list_peers` — 消息系统
- `review_artifact` — Artifact 审查
- `send_user_file` — 文件发送

#### 系统集成
- `voice_input` — 语音输入
- `daemon` — 守护进程管理
- `remote_trigger` — 远程触发
- `synthetic_output` — 合成输出
- `overflow_test` — 溢出测试
- `tungsten` — WebSocket 工具
- `ask_user` — 用户交互
- `config_read` — 配置读取
- `sleep` — 延时等待

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
| Permission Bypass Killswitch | `bypass_killswitch.rs` 安全兜底 |
| Dangerous Pattern Detection | `dangerous_patterns.rs` 恶意命令检测 |
| Path Validation | `path_validation.rs` 工作区路径边界检查 |
| Shell Matching | `shell_matching.rs` 精确 shell 命令规则匹配 |

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
| Streaming Fallback | 流式超时自动降级到非流式 |
| Request Continuity | `request_id` + `previous_request_id` 链路追踪 |
| Failure Tracker | 连续失败追踪 + 自动 circuit break |

---

## 四、已完成阶段回顾

### Phase 1: 核心基础设施
- ✅ 30+ crate 模块化架构
- ✅ Provider 调用 (OpenAI/Anthropic)
- ✅ 会话管理 (SQLite + NDJSON + Transcript V2)
- ✅ 权限系统 (5 模式 + 分类器 + 审计)
- ✅ MCP 客户端 (stdio/HTTP/WS + OAuth + Elicitation)
- ✅ 插件系统 (发现/加载/市场/autoupdate)

### Phase 2: 工具与交互
- ✅ 65+ 内置工具
- ✅ TUI 交互式终端 (65+ slash commands + Vim 模式)
- ✅ 上下文管理 (5 种压缩策略)
- ✅ 成本追踪
- ✅ 记忆系统
- ✅ 多代理调度器 (Fork + Built-in + Coordinator/Worker)

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

### Phase 5-38: Claude Parity Hardening
- ✅ 查询引擎状态机 (`rc-query-engine`)
- ✅ 动态系统提示词 (`rc-system-prompt` + 22 sections)
- ✅ 认证系统 (`rc-auth` + OAuth PKCE)
- ✅ 模型管理 (`rc-model` + aliases + capabilities)
- ✅ 设置管理 (`rc-settings` + `rc-managed-settings` + MDM)
- ✅ IDE 集成 (`rc-ide`)
- ✅ 技能搜索 (`rc-skill-search` + BM25)
- ✅ 会话迁移 (`rc-teleport`)
- ✅ 文件历史 (`rc-file-history`)
- ✅ 语音系统 (`rc-voice`)
- ✅ LSP 客户端 (`rc-lsp`)
- ✅ 分析/遥测 (`rc-analytics`)
- ✅ 3,811+ 测试全绿 (190 E2E + 3,621 unit)
- ✅ MiniMax anthropic-compatible provider 实测通过
- ✅ Permission bypass + Windows path canonicalization 修复
- ✅ Stream-JSON 协议完整实现

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
| 工具数量 | 65+ | 55+ | 40+ | 20+ | 30+ |
| PWA 移动端 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 国际化 | ✅ 中/英 | ❌ | ❌ | ❌ | ❌ |
| 系统提示词 Sections | 22 | 23 | ~10 | ~5 | ~8 |
| 斜杠命令 | 65+ | 80+ | ~20 | ~10 | ~15 |
| 上下文压缩策略 | 5 | 5 | 2 | 1 | 2 |
| Agent 类型 | 6 built-in + fork | 6 + fork | 1 | 0 | 1 |

**独有优势**: Rust 原生性能 + 分布式远程执行架构 + Circuit Breaker + PWA 移动端 + 多 Provider 故障转移 + 65+ 内置工具

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
| [`claude-code-rust-full-clone-plan.md`](claude-code-rust-full-clone-plan.md) | Claude Code 全量复刻方案 (4986 行) |
| [`comprehensive-test-plan-500.md`](comprehensive-test-plan-500.md) | 630 测试计划 (16 批次) |
| [`REMOTE_PLAN.md`](REMOTE_PLAN.md) | 远程控制 v1 架构主计划 |
| [`coding-plan-support.md`](coding-plan-support.md) | 国内 Coding Plan 供应商参考 |
| [`tauri-gui-architecture-design.md`](tauri-gui-architecture-design.md) | GUI 架构设计方案 |
| [`gui-remote-advanced-optimization-v2.md`](gui-remote-advanced-optimization-v2.md) | GUI 与 Remote 进阶优化方案 v2 |
| [`claude-code-deep-comparison.md`](claude-code-deep-comparison.md) | Claude Code 深度对比分析 |
| [`gap-analysis-and-restructure.md`](gap-analysis-and-restructure.md) | Gap 分析与重构方案 |
| [`cli-stress-test-report.md`](cli-stress-test-report.md) | CLI 压力测试报告 |
| [`mobile-app-research-report.md`](mobile-app-research-report.md) | 移动端研究报告 |
| 本文档 | 项目状态、路线图、竞品对比 |
