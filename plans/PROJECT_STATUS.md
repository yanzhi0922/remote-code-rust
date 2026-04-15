# Remote Code Rust — 项目状态与路线图

> 更新日期: 2026-04-15
> 当前阶段: Phase 4 — Remote Beta Hardening（并行推进 Claude parity compat / streaming hardening）
> 代码规模: ~53,000 行 (Rust + TypeScript)
> 当前验证基线: `cargo test -p remote-code` / `cargo test --workspace` 通过；MiniMax anthropic-compatible headless `--print` 与 `stream-json --include-partial-messages` 冒烟通过。GUI/mobile/clippy 维持上一轮稳定基线。

## Claude Parity Track

并行中的 Claude Code 全量复刻路线已从纯研究进入主干骨架阶段，并进入 app compat 接线阶段：

- 已新增 `rc-transcript`、`rc-query-engine`，并升级 `rc-engine-events` 为“运行时兼容事件 + EngineEvent/EventStream”双层结构。
- 已为 `rc-core` 补齐 v2 类型层（品牌 ID、Message 联合类型、AppState、Usage/Cost、扩展 hook 类型）。
- 已为 `rc-session` 接入 transcript V2 兼容读写 API，保持现有 session/NDJSON 主路径稳定。
- `rc-query-engine` 已补齐 host observer / checkpoint seam 与 provider streaming seam：`QueryObserver`、`QueryObserverEvent`、`QueryCheckpoint`、`ProviderInvocationMode` 已进入主干；query loop 现可向宿主发出 `StreamingTextDelta`、`StreamingToolCallStarted`、`StreamingToolCallDelta`、`StreamingUsageUpdated`。
- `apps/remote-code` 已继续推进到 app 层 compat adapter：`query_engine_compat.rs` 现已承接默认 prompt 主路径；当存在 `event_sink` 时，会显式启用 `rc-query-engine` 的 streaming provider mode，并把 streaming observer 事件翻译回 `PromptStreamEvent`。
- `headless` 已不再停留在独立 legacy streaming loop；它通过 `run_prompt()` 进入 compat path，同时保留 `ChannelPermissionBroker` + `LayeredPermissionBroker` 的审批链路与现有 `stream-json` 协议输出。legacy prompt loop 仅作为 `REMOTE_CODE_FORCE_LEGACY_PROMPT_LOOP` 回退开关保留。
- 当前 Rust 工作区回归已重新验证全绿：`cargo test -p remote-code`、`cargo test --workspace` 通过；MiniMax anthropic-compatible `--print` 与 `stream-json` 冒烟通过。
- 本轮已继续把 parity hardening 落到代码：headless error result 会复用 compat 落盘结果元数据；permission approval 响应后会显式重新发出 `session_state_changed: running`；compat error 路径会保留最新 streaming usage；`permission_denials` 现已补入 `tool_input`；provider streaming 在已观测到工具活动后会拒绝自动 fallback 到 non-streaming 重跑。
- 新一轮 provider / compat continuity 也已进入主干：`rc-config` 会为支持 Anthropic 协议的请求生成安全的 `request_metadata`（默认包含 `client` / `version` / `session_id`，并可叠加显式 env JSON）；`rc-provider` 会把它编码进 Anthropic-compatible `metadata.user_id`，并在 OpenAI / Anthropic 的 buffered 与 streaming 路径上采集服务端 `request_id`。
- `query_engine_compat` 与 `headless` 当前已不再把这条 request continuity 丢掉：`assistant_turn`、`result`、`model_usage` 与 compat 持久化元数据都已落下 `request_id`，并新增了对应回归测试，保证后续 rich result / 远端协议桥接不会再退化成“只有文本无请求链路”。
- 本轮已继续把启动期的 settings/auth/source-policy 往主干推进：`rc-config` 在未显式传 `--settings` 时会自动发现并按 `legacy-import -> profile -> workspace -> local` 顺序装配 runtime settings；显式 `--settings` 仍保持最高优先级并禁用自动发现；`--setting-sources` 也已可把启动期 discovery 收窄到 `user/project/local` 指定范围。更关键的是，这个 source policy 现在不再只停留在 settings 装配层：`hooks` / `runtime_hooks` / runtime extensions / MCP discovery 均已接入同一套 gating，而面向用户的可见面也已同步收口到同一语义，包括 `skills_cli`、TUI `/skills`、TUI `/mcp`、TUI `/plugins`、GUI doctor、GUI MCP list。`doctor`、`headless init`、`mcp list/get/call/serve` 以及上述 GUI/TUI 可见面也会复用解析后的 `auth_source` / `setting_sources` / `settings_files`；共享 runtime status / UI snapshot / doctor runtime 现在也会显式暴露 `allowed_setting_sources`。本轮又进一步把启动期配置与插件管理的两个硬缺口收口到主干：一是 `first_run_wizard` 不再写出 loader 不接受的扁平 JSON，而是会按当前 active settings target 写出与 `rc-config` 兼容的嵌套 provider schema，并尊重 explicit `--settings` / `allowed_setting_sources` 的当前语义；二是插件 disabled marker 的语义继续收口成“三层边界”：默认 runtime/plugin discovery 会跳过带 `.remote-code-disabled` marker 的插件，避免 disabled 插件继续参与 runtime hooks / runtime extensions / skills / plugin MCP 等启动期 surface；`/plugins`、TUI `/plugins`、GUI doctor 等管理面仍保留 disabled 插件可见性，并将 disabled 与 enabled 统计分离；`plugins --connect` / `plugins inspect` 对 disabled 插件只会返回 skipped，不再启动 runtime，而 `plugins invoke` 会直接拒绝执行。此外，`--show-setting-sources`、CLI doctor text、TUI `/status`、GUI OperationsTab 现在都会把 `settings_files` 与 disabled plugin 统计解释得更明确，便于排查 explicit `--settings`、source-policy 和 management/runtime surface 的交互。同时 provider-aware env auth/source 识别已补齐到 MiniMax / GLM / 腾讯 / 百炼等路径。这说明启动期 settings/auth layering 与 source-policy parity 已明显前进一步，但仍不代表官方启动期 parity 已完成：插件缓存、外部插件拉取、MCP 预连接、完整 source precedence matrix，以及 disabled 插件在 startup cache/materialization/preconnect 中的参与边界仍待继续收口。

当前执行焦点：

- 稳定 `conversation.rs -> query_engine_compat.rs -> rc-query-engine` 的默认 compat 主路径，并继续保留可控的 legacy escape hatch。
- 用 observer/checkpoint 与 streaming observer seam 补全 app 宿主侧的落盘、事件转译、恢复边界与 headless runtime event fidelity。
- 在默认 compat 主路径已经切换完成的前提下，继续做 headless / remote 事件保真、live usage/runtime 级透传和 legacy shim 收缩，而不是再把 headless cutover 作为前置阻塞项。

最新研究结论已收敛为新的 parity 约束：

- 复刻目标必须从“Anthropic SDK 兼容”上调为“官方 Claude Code 真实行为等价”；后续 provider、prompt、协议与宿主启动链路的验收，都要以官方 CLI 与 `.research/claude-code-rev` 的组合证据为准，而不是只看接口可调用。
- 基于本机官方 `claude` CLI `2.1.39` 的本地显式代理实测，官方启动阶段会先触发插件缓存、外部插件 `git clone`、MCP 连接建立等真实网络活动；这意味着 remote-code 不能把启动期简化成单一模型请求，插件/MCP/缓存预热也要纳入 parity 范围。
- 同一实测也确认，在 `--setting-sources local` 下存在无 auth 的纯本地启动路径；这为后续拆分“本地配置加载/插件发现/MCP 预连接”和“远端鉴权/模型调用”提供了可验证基线，避免把所有启动行为错误绑死到联网登录态；同时也要求我们把 disabled plugin 的本地管理语义纳入同一启动矩阵，而不是只把它当成 UI 层的装饰字段。
- `.research/claude-code-rev` 已进一步坐实官方关键语义不止于请求体 schema：包括动态 beta/header 组合、标准 `metadata` / request continuity、streaming usage 与 `stop_reason` 的最终化、谨慎的 streaming -> non-streaming fallback，以及 rich result / protocol 字段完备性；这些都应成为 app compat、headless stream-json 与 provider adapter 的下一轮硬性对齐项。

下一步收口方向：

- 先把“启动阶段行为”继续纳入 parity ledger：虽然 settings/hook/MCP/extensions 的主启动链已经 obey `--setting-sources`，`skills_cli`、TUI `/skills`、TUI `/mcp`、TUI `/plugins`、GUI doctor、GUI MCP list，以及 runtime status / UI snapshot / doctor runtime 的 source policy 暴露都已并入同一语义，disabled marker 对默认 runtime discovery 与管理面的边界也已初步落地，`first_run_wizard` 的 schema/target 也已不再绕开 active settings 语义，但插件缓存、外部插件拉取、MCP 预连接、以及完整 settings/auth/plugin/MCP/disabled-state source precedence matrix 仍需形成可回归的行为矩阵。MCP 侧下一步更安全的切入点也已明确：先抽共享 startup/runtime MCP discovery plan，再进入真正的 preconnect，而不是直接堆一个 stdio-only startup shim。
- 再把“流式最终化语义”纳入默认 compat 主路径：live usage、`stop_reason`、fallback 原因与 richer runtime/control-plane 字段要能从 engine 透传到 headless / remote 协议输出，而不是只保证结果面 usage / request_id 已落盘。
- 上述约束目前属于新的验收边界而非“已全部完成”的事项：当前已补齐 error-side usage 保真、approval running-state、denial 结构、request metadata / request_id continuity，以及启动期 settings/auth source 分层基础设施；但 live usage 对外事件、dynamic headers/betas/previousRequestId、以及启动期插件/MCP 行为矩阵仍待继续收口。
- 风险判断同步上调：如果继续停留在 SDK 兼容层，MiniMax / GLM 等平台对“非官方工具行为特征”的风控会直接反噬产品可用性，因此官方行为拟合度已从“体验优化项”提升为“可用性与封禁风险控制项”。

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
