# Claude Code → Rust 全面复刻方案（执行优化版）

**日期**: 2026-04-15  
**目标**: 将 `.research/claude-code-rev` (2,048 文件, ~200,000 行 TS/TSX) **全面**复刻为 Rust，并以 `remote-code-rust` 为宿主完成 full-scope parity  
**原则**: 
1. 复刻行为、能力边界与架构分层，保留 remote-code-rust 的多 provider 优势
2. **TUI 也复刻**（React/Ink → ratatui）
3. **所有 prompt、命令、工具、bridge、skills 都在范围内**，关键文案逐段对齐原版
4. 不考虑工作量大小，追求完整性和正确性
5. 所有改造以 `main` 为唯一长期交付线，通过 feature flag、compat shim、里程碑门禁推进，不走长期并行分支

**状态**: 深度调研完成，差距分析完成，执行方案已优化；Phase 1 / Phase 2 基础骨架已开始落地到 `main`
**核心结论**: **必须以新内核重构为主，但不能做无差别推倒重写。** 当前 Rust 代码结构与 Claude Code 之间存在根本性架构差距；正确做法是以 full-scope parity 为目标，在 `main` 上通过抽取、替换、兼容过渡、默认切换完成整仓升级。

### 0.0 当前执行进展（2026-04-15）

本轮已将计划从“研究态”推进到“可编译、可测试的主干骨架”：

- 已在 [`rc-core`](../crates/rc-core/src/lib.rs) 落地 v2 基础类型层：`SessionId` / `AgentId`、`Message` 联合类型、`AppState`、`ToolPermissionContext`、`FileHistoryState`、`UsageAccumulator`、`CostTracker`、扩展 hook 响应类型。
- 已在 [`rc-engine-events`](../crates/rc-engine-events/src/lib.rs) 落地 Phase 1 事件层：保留现有 `RuntimeEventDetail` 兼容面，同时新增 `EngineEvent`、`ContentBlockType`、`ContentBlockDelta`、`EventStream`。
- 已新增 [`rc-transcript`](../crates/rc-transcript/src/lib.rs) crate，完成 `TranscriptEntry` / `CompactBoundary` / `TranscriptStorage` 的 JSONL 持久化与 round-trip 测试。
- 已为 [`rc-session`](../crates/rc-session/src/lib.rs) 接入 transcript V2 兼容层：新增 `append_transcript_entry()`、`load_transcript_v2()`、`transcript_storage()`，保持现有 `StoredEvent` 主路径不破坏。
- 已新增 [`rc-query-engine`](../crates/rc-query-engine/src/lib.rs) compat 内核，先抽出“回合推进 + budget + tool loop + legacy backend seam”，并通过最小闭环测试。
- 已为 [`rc-query-engine`](../crates/rc-query-engine/src/lib.rs) 补齐 host observer / checkpoint seam：新增 `QueryObserver`、`QueryObserverEvent`、`QueryCheckpoint`，query loop 现在会显式发出消息追加、budget 评估/超限、context compaction、assistant commit、tool batch checkpoint create/clear 等生命周期事件。
- 已为 [`rc-query-engine`](../crates/rc-query-engine/src/lib.rs) 补齐默认关闭的 provider streaming seam：新增 `ProviderInvocationMode::{Buffered, Streaming}`；opt-in 后 query loop 会走 `complete_with_streaming_observer(...)`，并向宿主发出 `StreamingTextDelta`、`StreamingToolCallStarted`、`StreamingToolCallDelta`、`StreamingUsageUpdated`。
- 已继续把 [`apps/remote-code/src/conversation.rs`](../apps/remote-code/src/conversation.rs) 迁移到 app 层 compat adapter：新增 [`query_engine_compat.rs`](../apps/remote-code/src/query_engine_compat.rs)，当前 `run_prompt()` 默认已切到 compat path；当存在 `event_sink` 时，compat adapter 会将 `rc-query-engine` 切到 `ProviderInvocationMode::Streaming`，并把 streaming observer 事件翻译回 `PromptStreamEvent`，同时继续由 app 层承担 transcript、named events、resume boundary、hook/tool side effects 映射。
- 当前 cutover 策略已进一步推进：`conversation.rs` 默认走 `rc-query-engine` compat adapter，legacy prompt loop 仅作为 `REMOTE_CODE_FORCE_LEGACY_PROMPT_LOOP` 回退开关保留；[`apps/remote-code/src/headless.rs`](../apps/remote-code/src/headless.rs) 已通过同一 `run_prompt()` 进入 compat path，并继续保留 `ChannelPermissionBroker` 审批链路与 `stream-json` 事件发射。
- 当前验证结果：`cargo test -p rc-query-engine`、`cargo test -p remote-code`、`cargo test --workspace` 均通过；使用 MiniMax anthropic-compatible provider 实测 `--print` 路径返回 `OK`，`--output-format stream-json --include-partial-messages` 路径成功发出 `message_delta` / `message_committed` 并返回预期结果。
- 当前 tranche 已继续补上几项直接影响官方行为拟合度的 hardening：headless error result 复用 compat 落盘元数据；approval 响应后显式回到 `running`；compat error 路径保留最近一次 streaming usage；`permission_denials` 带上 `tool_input`；provider streaming 在工具活动已开始后不再自动 fallback 到 non-streaming 重跑。
- 当前 tranche 也已补上安全的 request continuity 基础设施：[`rc-config`](../crates/rc-config/src/lib.rs) 会生成可扩展的 `request_metadata`；[`rc-provider`](../crates/rc-provider/src/lib.rs) / [`streaming.rs`](../crates/rc-provider/src/streaming.rs) 会在 Anthropic-compatible 请求中写入 `metadata.user_id`，并从 OpenAI / Anthropic 的 buffered 与 streaming 响应中提取 `request_id`；[`apps/remote-code/src/query_engine_compat.rs`](../apps/remote-code/src/query_engine_compat.rs) 会把该 `request_id` 持久化到 `assistant_turn` / `result` / `model_usage`。
- 相关回归测试已补齐：provider 侧新增 `metadata.user_id` 编码与 `request_id` 解析测试；compat 侧新增 `mock-request-id` 持久化断言；当前 `cargo test -p rc-provider`、`cargo test -p remote-code`、`cargo test --workspace` 全部通过。
- 当前 tranche 也已继续把启动期 settings/auth/source-policy 往主干推进：[`rc-config`](../crates/rc-config/src/settings_layers.rs) 在未显式传 `--settings` 时会自动发现 `legacy-import -> profile -> workspace -> local` 四层 runtime settings，并将其装配进 runtime config；显式 `--settings` 仍保持权威优先级并关闭自动发现；`--setting-sources` 也已可把 startup discovery 收窄到 `user/project/local` 指定范围。更关键的是，这个 source policy 现在不再只停留在 settings 层：[`apps/remote-code/src/hooks.rs`](../apps/remote-code/src/hooks.rs)、[`apps/remote-code/src/runtime_hooks.rs`](../apps/remote-code/src/runtime_hooks.rs)、[`apps/remote-code/src/conversation.rs`](../apps/remote-code/src/conversation.rs)、[`apps/remote-code/src/mcp_cli.rs`](../apps/remote-code/src/mcp_cli.rs) 都已接入统一 gating，因此 runtime hooks、doctor/headless 的 hook 统计、runtime extensions，以及 `mcp list/get/call/serve` 的隐式配置发现都会 obey 同一套 `allowed_setting_sources`。本轮又继续把剩余启动期可见面往同一语义收口：`skills_cli`、TUI `/skills`、TUI `/mcp`、TUI `/plugins`、GUI doctor、GUI MCP list 已补齐对应 gating，不再绕过 `config.allowed_setting_sources` 直接统计 user/project 侧 skills、plugins、managed MCP、plugin MCP 或对应 scope 的 MCP 列表；同时插件可见性也已进一步分层：默认 runtime discovery 会跳过带 `.remote-code-disabled` marker 的插件，避免 disabled 插件继续参与 runtime hooks / runtime extensions / skills / plugin MCP 等启动期 surface，但 `/plugins`、TUI `/plugins`、GUI doctor 等管理面仍保留 disabled 插件可见性，并把 disabled 与 enabled 统计分离；其中 `plugins --connect` / `plugins inspect` 对 disabled 插件会显式跳过 runtime inspection，`plugins invoke` 则直接拒绝执行。共享 runtime status / UI snapshot / doctor runtime 也已开始显式暴露 `allowed_setting_sources`，`--show-setting-sources`、CLI doctor text 与 TUI `/status` 现在也会把 `settings_files` 一并展示，便于解释某些 surface 为什么被 source policy 或 explicit settings 模式隐藏。[`apps/remote-code/src/headless.rs`](../apps/remote-code/src/headless.rs) 与 runtime status 现也会直接复用解析后的 `auth_source` / `setting_sources`，同时 provider-aware env auth/source 识别已扩展到 MiniMax / GLM / 腾讯 / 百炼等现有路径。这表明启动期 settings/auth source 分层已经前进一步，但还不等价于官方 startup parity 已完成：plugin cache / external plugin fetch、MCP preconnect、完整 source precedence matrix，以及 disabled plugin 是否参与 cache/materialization/preconnect 仍待继续收口。

这意味着 Phase 1 的契约冻结与 Phase 2 的最小可运行引擎都已进入主干骨架阶段；当前主线重点已经从“搭骨架”进入“默认 compat + parity hardening”：

- 稳定 `conversation.rs -> query_engine_compat.rs -> rc-query-engine` 的默认主路径，并保留 env-based legacy escape hatch。
- 把 observer/checkpoint 与 streaming observer 事件完整翻译回 `SessionStore`、`PromptStreamEvent`、named events、resume state，同时把 startup source-policy 从“settings 层可配置”继续推进到完整启动矩阵：重点收口 plugin cache、external plugin fetch、MCP preconnect、完整 source precedence matrix，以及 disabled plugin 在 startup cache/materialization/management surfaces 间的参与边界，并继续把 status / doctor / GUI bridge 的可观测性与官方行为矩阵对齐。
- 继续补齐 `headless` / remote 所需的 runtime event fidelity 与 parity hardening，而不是再等待 provider streaming seam 落地。
- 在默认 compat 路径稳定后，再推进 live usage 流、更多 host outcome/runtime 粒度、`previousRequestId` continuity、以及启动期插件/MCP/cache 预热矩阵与 legacy shim 收缩。

最新研究已把“parity hardening”的验收口径进一步收紧：

- 目标不再是“Anthropic SDK 兼容即可”，而是尽量复刻官方 Claude Code 的真实运行行为；后续 provider、启动链路、协议输出与 prompt/system 组织方式，均需同时参考本机官方 CLI 实测与 `.research/claude-code-rev` 的源码证据。
- 对本机官方 `claude` CLI `2.1.39` 的本地显式代理观察显示，官方启动阶段会先发生 hooks 决策、插件缓存、外部插件 `git clone`、MCP 预连接/建连等真实活动；因此 Rust 侧不能把启动抽象成单一模型请求，而要把 hooks、插件/MCP/cache 预热、disabled plugin 管理语义，以及 source precedence matrix 一并纳入 parity 范围。
- 同一观察也确认 `--setting-sources local` 下存在无 auth 的纯本地启动路径；这意味着“本地配置/插件发现/MCP 预连接”与“远端鉴权/模型调用”在官方实现里是可分离的，Rust 路线也应保持这一分层，避免错误耦合。
- `.research/claude-code-rev` 已证明官方关键语义包括动态 beta/header 组合、标准 `metadata` / request continuity、streaming usage/`stop_reason` 最终化、谨慎的 streaming -> non-streaming fallback、以及 rich result/protocol 字段；这些都必须进入下一轮 compat cutover 的硬性清单，而不是留作后续细节优化。
- 以上结论来自“官方 CLI 代理实测 + 逆向源码对照”的行为边界，不等价于我们已经完成完整报文级复刻；当前只是把最影响正确性与风控特征的几处缺口先收口到主干。

---

## 0. 深度差距分析与重构决策

### 0.1 核心结论

| 维度 | 当前 Rust | Claude Code | 差距等级 |
|------|----------|-------------|---------|
| 查询引擎 | 单主循环 + 已接入 streaming callbacks / session store / context manager ([`conversation.rs`](apps/remote-code/src/conversation.rs) 1,288 行) | 状态机 + AsyncGenerator ([`QueryEngine.ts`](.research/claude-code-rev/src/QueryEngine.ts) 1,296 行 + [`query.ts`](.research/claude-code-rev/src/query.ts) 1,730 行 = 3,026 行) | 🔴 **根本性** |
| API 客户端 | 已有 `complete()` + `complete_streaming_with_callbacks()` + fallback ([`rc-provider`](crates/rc-provider/src/lib.rs), [`streaming.rs`](crates/rc-provider/src/streaming.rs)) | 流式 streaming + cache + betas ([`claude.ts`](.research/claude-code-rev/src/services/api/claude.ts) 3,420 行) | 🔴 **根本性，但已有基础** |
| 上下文压缩 | 已有 compact/reactive/collapse/microcompact，多策略但未达到 Claude 级 cache-boundary / auto / snip ([`context.rs`](crates/rc-provider/src/context.rs)) | 5 种策略 (auto/micro/snip/reactive/collapse) | 🔴 **根本性** |
| 工具执行 | 已有 30+ built-in tools + runtime policy + tool search ([`rc-tools`](crates/rc-tools/src/)) | 流式并行执行 + 进度流 + 动态 prompt | 🔴 **根本性** |
| System Prompt | 硬编码字符串 ([`default_system_prompt()`](crates/rc-core/src/lib.rs:548)) | 动态构建 + 缓存断点 ([`prompts.ts`](.research/claude-code-rev/src/constants/prompts.ts) 915 行) | 🟡 **重大** |
| 权限系统 | 简单 PermissionBroker | 分类器 + 自动模式 + 拒绝追踪 (204 行 hook) | 🟡 **重大** |
| TUI | 已有 ratatui 交互 TUI / Vim / slash commands ([`rc-tui`](crates/rc-tui/src/lib.rs)) | 完整 React/Ink TUI (407 组件) | 🔴 **根本性** |
| Agent 系统 | 简单 SubAgent | Fork + Subagent + Built-in agents (14 文件) | 🟡 **重大** |
| MCP | 基础 stdio 连接 | 动态连接 + OAuth + Elicitation (25 文件) | 🟡 **重大** |
| 会话存储 | SQLite metadata + NDJSON transcript ([`rc-session`](crates/rc-session/src/lib.rs)) | transcript boundary + cache-aware state + migration set | 🟡 **重大** |
| 斜杠命令 / 应用面 | 已有 CLI/TUI/GUI/mobile/control-plane/runner 多入口 | 80+ 命令 + CLI/TUI/bridge/remote 深度整合 | 🟡 **重大** |

### 0.1A 现有资产更正（不降低复刻范围）

当前仓库不是“从零开始”，而是“已有一套可运行骨架，但还远未达到 Claude Code parity”：

- 已有 30+ 内建工具、runtime policy、权限分类与工具搜索基础，不是只有十几个工具。
- 已有 provider 流式回调与流式失败回退，不是纯非流式客户端。
- 已有多种上下文压缩策略，不是只有单一截断。
- 已有 ratatui TUI、slash commands、Vim 输入模式，不是只有 headless 空壳。
- 已有 SQLite + NDJSON 会话存储，不是简单 JSON 数组会话。
- 已有 `remote-code`、`remote-code-control-plane`、`remote-code-gui`、`remote-code-mobile`、`remote-code-runner` 多应用面，需要纳入统一升级路线。

这不改变 full clone 的工作量和范围；它只改变执行方法。正确方法应是 **replace-by-extraction / trunk-safe migration**，而不是把现有资产当成“全部作废”。

### 0.2 查询引擎差距（30+ 目标能力，当前仅部分覆盖）

当前 [`conversation.rs`](apps/remote-code/src/conversation.rs) 的 `run_prompt()` 核心仍是 turn loop，但它并非空白实现：已经接了 provider streaming callbacks、上下文管理和工具执行。真正的差距在于缺少统一状态机、事件语法、链路追踪、压缩编排和 Claude Code 风格的 engine lifecycle：

```
for turn_index in 0..max_turns {
    检查上下文溢出 → 简单截断
    调用 provider.complete() 或 complete_streaming()
    如果有 tool_calls → 顺序执行
    如果没有 tool_calls → 返回
}
```

而 Claude Code 的查询引擎是 **3,026 行的状态机 + AsyncGenerator**，包含以下当前 Rust 完全缺失的能力：

| # | 缺失能力 | Claude Code 实现 | 影响 |
|---|---------|-----------------|------|
| 1 | Snip Compact | `snipCompact.ts` - 按策略裁剪历史 | 长对话必需 |
| 2 | Micro Compact | `microCompact.ts` + cache editing | 缓存优化必需 |
| 3 | Context Collapse | `contextCollapse/` - 折叠上下文 | 大项目必需 |
| 4 | Auto Compact (LLM 摘要) | `autoCompact.ts` - 用 LLM 生成摘要 | 长对话必需 |
| 5 | Reactive Compact | `reactiveCompact.ts` - 响应式压缩 | 错误恢复必需 |
| 6 | Streaming Tool Execution | `StreamingToolExecutor` - 流式并行执行 | 实时反馈必需 |
| 7 | Tool Progress Streaming | `ToolProgressData` 流 | 用户体验必需 |
| 8 | Query Chain Tracking | chainId + depth 追踪 | 分析必需 |
| 9 | Thinking Config | enabled/disabled/adaptive | 模型交互必需 |
| 10 | Model Switching | runtime model + fallback | 灵活性必需 |
| 11 | Task Budget | token budget per task | 成本控制必需 |
| 12 | Structured Output | JSON schema enforcement | API 模式必需 |
| 13 | Skill Discovery | discoveredSkillNames 追踪 | 技能系统必需 |
| 14 | Memory Mechanics | MEMORY.md auto-load | 记忆系统必需 |
| 15 | Streaming Fallback | non-streaming on timeout | 稳定性必需 |
| 16 | Tombstone Handling | orphaned message cleanup | 流式回退必需 |
| 17 | Backfill Observable Input | tool input backfill | 工具精度必需 |
| 18 | Stop Hook Retry | `stopHooks.ts` | 流程控制必需 |
| 19 | Tool Result Summary | LLM-generated summary | 上下文管理必需 |
| 20 | Consecutive Failure Tracking | circuit breaker | 稳定性必需 |
| 21 | File History State | snapshot management | 文件安全必需 |
| 22 | Attribution State | commit attribution | Git 集成必需 |
| 23 | Advisor Model | server-side advisor | 代码审查必需 |
| 24 | Tool Search | deferred tool discovery | 大工具集必需 |
| 25 | Agent Definitions | activeAgents + allAgents | Agent 系统必需 |
| 26 | MCP Tool Integration | mcpTools in API call | MCP 集成必需 |
| 27 | Effort Value | low/medium/high | 交互控制必需 |
| 28 | Fast Mode | fast mode toggle | 速度优化必需 |
| 29 | Permission Denial Tracking | SDK reporting | 安全审计必需 |
| 30 | ProcessUserInputContext | 30+ 字段上下文 | 核心架构必需 |

### 0.3 API 客户端差距（20+ 目标能力，当前已有流式基础）

当前 [`rc-provider`](crates/rc-provider/src/lib.rs) 已有 `complete()`、`complete_streaming_with_callbacks()` 以及流式失败回退，但仍缺失：

| # | 缺失能力 | Claude Code 实现 |
|---|---------|-----------------|
| 1 | Prompt Caching | cache_control 断点管理 |
| 2 | Beta Headers | interleaved-thinking, output-128k 等 |
| 3 | Thinking Blocks | thinking + signature delta 解析 |
| 4 | Server Tool Use | server_tool_use blocks |
| 5 | Tool Search Integration | deferred tool schema |
| 6 | Advisor Integration | advisor model + beta |
| 7 | Effort Parameter | low/medium/high effort |
| 8 | Task Budget | token budget tracking |
| 9 | Streaming Fallback | non-streaming on timeout |
| 10 | Media Stripping | excess media removal |
| 11 | Usage Accumulation | cache_read/creation tokens |
| 12 | Model-specific Max Tokens | per-model output limits |
| 13 | Previous Request ID | request chain tracking |
| 14 | MCP Tools in API | mcpTools option |
| 15 | Agent Types | allowedAgentTypes |
| 16 | Non-interactive Session | isNonInteractiveSession |
| 17 | Fast Mode | fastMode option |
| 18 | Query Source | compact/session_memory/agent |
| 19 | Content Block Types | tool_use/text/thinking/signature |
| 20 | withRetry | 完整重试逻辑 |

### 0.4 重构决策（范围不减，执行方式优化）

**结论：必须以新内核重构为主，但采用抽取替换、兼容过渡、主干门禁，不做无差别推倒重写。**

| Crate | 决策 | 原因 |
|-------|------|------|
| [`rc-core`](crates/rc-core/src/lib.rs) | **新增 v2 类型层并渐进替换导出面** | 类型系统需要升级到 Message/SDKMessage/Tool trait 级别，但现有基础类型和兼容导出可复用 |
| [`rc-provider`](crates/rc-provider/src/lib.rs) | **深度重构，复用现有流式/回退资产** | 缺少缓存、thinking blocks 等 20+ 能力，但不应丢弃现有 streaming parser / callback / fallback 资产 |
| [`rc-tools`](crates/rc-tools/src/) | **深度重构并扩展** | 现有工具基座可复用，但需补齐 Claude 级 tool trait、prompt、tool search、runtime orchestration |
| [`conversation.rs`](apps/remote-code/src/conversation.rs) | **冻结旧循环，新增兼容适配层，最终退役** | 由 `rc-query-engine` 接管主路径，但要保留兼容适配直到 cutover 完成 |
| [`rc-mcp`](crates/rc-mcp/src/lib.rs) | **深度重构** | 现有 stdio 路径可保留为底层 transport，向上补齐动态连接、OAuth、Elicitation |
| [`rc-agents`](crates/rc-agents/src/lib.rs) | **深度重构** | 现有 SubAgent 是起点，但需扩展为 Fork/Built-in/Agent catalog |
| [`rc-tui`](crates/rc-tui/src/) | **保留现有 ratatui 骨架并分层替换** | 当前已有交互式 TUI；目标是补齐 Claude 级组件树、状态管理、快捷键和 bridge 行为 |
| [`rc-hooks`](apps/remote-code/src/hooks.rs) | **深度重构并复用现有接缝** | 生命周期远不完整，但现有 hook integration points 有价值 |
| [`rc-skills`](crates/rc-skills/src/lib.rs) | **深度重构并补齐 bundled skills** | 基础框架可保留，需扩成 Claude 级 discovery / bundled / MCP skill 路线 |
| [`rc-session`](crates/rc-session/src/lib.rs) | **保留增强** | 现有 SQLite + NDJSON 基础很好，应升级为 transcript V2 / boundary-aware store |
| [`rc-permissions`](crates/rc-permissions/src/) | **保留增强** | 基础可用，需要增强分类器/自动模式/拒绝追踪 |
| [`rc-config`](crates/rc-config/src/lib.rs) | **保留增强** | 基础可用，需要增加更多参数与迁移逻辑 |
| [`rc-event-bus`](crates/rc-event-bus/src/lib.rs) | **保留增强** | 基础可用，需要增加 EngineEvent / TUI / Hook / Bridge 统一事件流 |
| [`rc-protocol`](crates/rc-protocol/src/lib.rs) | **保留增强** | 基础可用，需要增加 SDKMessage / content blocks / transcript boundaries |
| [`rc-ui-bridge`](crates/rc-ui-bridge/src/lib.rs) | **保留增强并对齐 upstream bridge** | 现有桥接层是资产，需要扩成 Claude Code 级 bridge/* 能力 |

---

## 1. Claude Code 源码全景（完整盘点）

### 1.1 模块清单与规模

| 模块 | 文件数 | 代码行 | 功能 | 复刻优先级 |
|------|--------|--------|------|-----------|
| `utils/` | 577 | ~50,000 | 工具函数集合（认证、git、文件、diff、markdown 等） | **P0** |
| `components/` | 407 | ~40,000 | React/Ink TUI 组件（消息、权限、Agent、diff 等） | **P0** (TUI) |
| `commands/` | 215 | ~15,000 | 斜杠命令（/help, /compact, /model 等 80+ 命令） | **P0** |
| `tools/` | 199 | 47,282 | 50+ 工具实现（Bash, Read, Write, Edit, Agent 等） | **P0** |
| `services/` | 149 | 50,036 | API/MCP/compact/analytics/lsp 等服务层 | **P0** |
| `hooks/` | 105 | 17,932 | React hooks + 权限 hooks + UI hooks | **P0** |
| `ink/` | 100 | ~8,000 | Ink TUI 框架适配层（渲染、布局、终端管理） | **P0** (TUI) |
| `skills/` | 53 | 4,582 | 技能系统（bundled skills + MCP skills） | **P0** |
| `bridge/` | 33 | 11,767 | UI/远程桥接（REPL bridge + remote bridge） | **P1** |
| `constants/` | 22 | 2,363 | 常量/工具名/prompt 模板 | **P0** |
| `cli/` | 20 | 11,472 | CLI 命令处理 | **P0** |
| `types/` | 19 | 3,365 | TypeScript 类型定义 | **P0** |
| `entrypoints/` | 14 | 3,833 | 入口点（CLI/SDK/MCP/Sandbox） | **P0** |
| `tasks/` | 14 | 3,091 | 任务系统 | **P1** |
| `keybindings/` | 15 | ~2,000 | 快捷键绑定（解析、匹配、验证、模板） | **P1** |
| `memdir/` | 9 | 1,640 | 记忆管理（MEMORY.md 读写） | **P0** |
| `state/` | 6 | 1,144 | 全局状态管理（AppStateStore + selectors） | **P0** |
| `context/` | 9 | ~1,500 | React context providers | **P0** (TUI) |
| `query/` | 5 | 606 | 查询配置（tokenBudget/config/transitions） | **P0** |
| `migrations/` | 11 | ~1,500 | 数据迁移 | **P1** |
| `buddy/` | 6 | ~500 | 伙伴系统 | P3 |
| `vim/` | 5 | ~800 | Vim 模式 | **P1** |
| `native-ts/` | 4 | ~400 | 原生绑定 | P2 |
| `remote/` | 4 | ~600 | 远程操作 | **P1** |
| `screens/` | 3 | ~400 | 全屏视图 | **P1** |
| `server/` | 3 | ~500 | HTTP 服务器 | P2 |
| `assistant/` | 3 | ~400 | 助手功能 | P2 |
| `coordinator/` | 2 | 277 | 协调器模式 | P2 |
| `proactive/` | 2 | 52 | 主动模式 | P3 |
| `plugins/` | 2 | ~200 | 插件管理 | **P1** |
| `ssh/` | 2 | ~300 | SSH 支持 | **P1** |
| `upstreamproxy/` | 2 | ~200 | 上游代理 | P2 |
| 根级文件 | ~15 | ~5,000 | QueryEngine/query/Tool/main/Task 等 | **P0** |

### 1.2 核心文件清单（必须按能力 / 行为 / 边界对齐复刻）

| 文件 | 行数 | 功能 | Rust 目标 |
|------|------|------|-----------|
| `src/query.ts` | 1,730 | 核心查询循环 | `rc_query_engine::query_loop` |
| `src/QueryEngine.ts` | 1,296 | 查询引擎状态机 | `rc_query_engine::QueryEngine` |
| `src/Tool.ts` | 793 | Tool trait 定义 | `rc_tools::Tool` trait |
| `src/services/api/claude.ts` | 3,420 | Anthropic API 客户端 | `rc_provider::AnthropicClient` |
| `src/services/tools/toolExecution.ts` | 1,746 | 工具执行管线 | `rc_tools::ToolExecutor` |
| `src/services/compact/compact.ts` | 1,706 | 上下文压缩 | `rc_compact::CompactEngine` |
| `src/constants/prompts.ts` | 915 | System prompt 管理 | `rc_system_prompt::SystemPromptBuilder` |
| `src/state/AppStateStore.ts` | ~500 | 全局状态 store | `rc_tui::AppState` |
| `src/components/Message.tsx` | 627 | 消息渲染组件 | `rc_tui_components::Message` |
| `src/hooks/useCanUseTool.tsx` | 204 | 工具权限 hook | `rc_permissions::can_use_tool` |
| `src/tools/BashTool/prompt.ts` | 370 | Bash 工具 prompt | `rc_tool_prompts::bash` |
| `src/tools/AgentTool/prompt.ts` | 288 | Agent 工具 prompt | `rc_tool_prompts::agent` |
| `src/keybindings/defaultBindings.ts` | ~200 | 默认快捷键 | `rc_tui::keybindings` |

---

## 2. Rust 架构映射（完整版）

### 2.1 Crate 划分

```
remote-code-rust/
├── apps/
│   ├── remote-code/          # CLI 入口 (现有，增强)
│   ├── remote-code-gui/      # GUI (现有，保留)
│   ├── remote-code-runner/   # Runner (现有，保留)
│   └── remote-code-migrate/  # 迁移工具 (现有，保留)
│
├── crates/
│   │
│   │  ════════ 新建 crate ════════
│   │
│   ├── rc-query-engine/      # 🆕 查询引擎 (QueryEngine.ts + query.ts)
│   ├── rc-transcript/        # 🆕 会话记录 (sessionStorage + compact boundary)
│   ├── rc-engine-events/     # 🆕 统一事件 (SDKMessage + StreamEvent)
│   ├── rc-system-prompt/     # 🆕 System prompt (constants/prompts.ts)
│   ├── rc-compact/           # 🆕 上下文压缩 (services/compact/*)
│   ├── rc-tool-prompts/      # 🆕 工具 prompt (tools/*/prompt.ts)
│   ├── rc-tasks/             # 🆕 任务系统 (tasks/*)
│   ├── rc-memory/            # 🆕 记忆管理 (memdir/*)
│   ├── rc-analytics/         # 🆕 分析/遥测 (services/analytics/*)
│   ├── rc-context/           # 🆕 上下文管理 (contextCollapse + tokenBudget)
│   ├── rc-lsp/               # 🆕 LSP 客户端 (services/lsp/*)
│   ├── rc-commands/          # 🆕 斜杠命令 (commands/*)
│   ├── rc-output-styles/     # 🆕 输出风格 (outputStyles/*)
│   │
│   │  ════════ 现有 crate（增强）════════
│   │
│   ├── rc-core/              # 核心类型 (增强: 更多枚举和结构体)
│   ├── rc-provider/          # Provider 客户端 (增强: Anthropic 协议细节)
│   ├── rc-tools/             # 工具运行时 (大幅增强: 50+ 工具)
│   ├── rc-mcp/               # MCP 客户端 (大幅增强: 动态连接/OAuth)
│   ├── rc-permissions/       # 权限系统 (增强: 分类器/自动模式)
│   ├── rc-hooks/             # Hook 系统 (增强: 完整生命周期)
│   ├── rc-skills/            # 技能系统 (增强: bundled skills)
│   ├── rc-plugins/           # 插件系统 (增强: 动态加载)
│   ├── rc-session/           # 会话存储 (增强: transcript V2)
│   ├── rc-config/            # 配置管理 (增强: 更多参数)
│   ├── rc-agents/            # Agent 系统 (大幅增强: fork/built-in agents)
│   ├── rc-event-bus/         # 事件总线 (增强: EngineEvent)
│   ├── rc-protocol/          # 协议定义 (增强: SDKMessage)
│   ├── rc-control-plane/     # 控制面 (保留)
│   ├── rc-runner/            # Runner (保留)
│   ├── rc-telemetry/         # 遥测 (保留)
│   │
│   │  ════════ TUI 复刻 crate ════════
│   │
│   ├── rc-tui/               # TUI 框架 (大幅增强: 复刻 Claude Code TUI)
│   ├── rc-tui-components/    # 🆕 TUI 组件库 (对应 components/*)
│   ├── rc-tui-input/         # 🆕 TUI 输入处理 (对应 hooks/useTextInput 等)
│   └── rc-ui-bridge/         # UI 桥接 (增强: 对齐 bridge/*)
```

### 2.2 Crate 依赖图

```
                         ┌─────────────┐
                         │  remote-code │  (CLI 入口)
                         └──────┬──────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                  │
     ┌────────┴──────┐  ┌──────┴───────┐  ┌──────┴───────┐
     │ rc-tui        │  │ rc-commands  │  │ rc-query-    │
     │ (TUI 框架)    │  │ (斜杠命令)   │  │ engine       │
     └───────┬───────┘  └──────┬───────┘  └──────┬───────┘
             │                 │                  │
    ┌────────┴────────┐       │        ┌─────────┼──────────┐
    │                 │       │        │         │          │
┌───┴────┐  ┌────────┴──┐   │   ┌────┴───┐ ┌───┴────┐ ┌───┴──────┐
│rc-tui- │  │rc-tui-    │   │   │rc-     │ │rc-     │ │rc-engine │
│compo-  │  │input      │   │   │system- │ │compact │ │events    │
│nents   │  │(输入处理) │   │   │prompt  │ │(压缩)  │ │(事件)    │
└───┬────┘  └─────┬─────┘   │   └────┬───┘ └───┬────┘ └───┬──────┘
    │             │         │        │         │          │
    └──────┬──────┘         │        │    ┌────┴────┐     │
           │                │        │    │rc-      │     │
    ┌──────┴──────┐         │        │    │context  │     │
    │ rc-tui      │         │        │    │(上下文) │     │
    │ (框架增强)  │         │        │    └────┬────┘     │
    └──────┬──────┘         │        │         │          │
           │                │        │         │          │
    ┌──────┴────────────────┴────────┴─────────┴──────────┴──┐
    │                                                        │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
    │  │rc-core   │ │rc-tools  │ │rc-       │ │rc-       │  │
    │  │(核心类型)│ │(工具运行)│ │provider  │ │permissns │  │
    │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │
    │       │             │            │             │        │
    │  ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐  │
    │  │rc-tool-  │ │rc-mcp    │ │rc-session│ │rc-hooks  │  │
    │  │prompts   │ │(MCP)     │ │(会话)    │ │(Hook)    │  │
    │  └──────────┘ └────┬─────┘ └────┬─────┘ └──────────┘  │
    │                    │            │                       │
    │  ┌──────────┐ ┌────┴─────┐ ┌───┴──────┐ ┌──────────┐  │
    │  │rc-agents │ │rc-skills │ │rc-config │ │rc-       │  │
    │  │(Agent)   │ │(技能)    │ │(配置)    │ │protocol  │  │
    │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
    │                                                        │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
    │  │rc-memory │ │rc-tasks  │ │rc-       │ │rc-lsp    │  │
    │  │(记忆)    │ │(任务)    │ │analytics │ │(LSP)     │  │
    │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
    │                                                        │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐               │
    │  │rc-       │ │rc-event- │ │rc-ui-    │               │
    │  │transcript│ │bus       │ │bridge    │               │
    │  └──────────┘ └──────────┘ └──────────┘               │
    └────────────────────────────────────────────────────────┘
```

### 2.3 TUI 复刻架构

Claude Code 的 TUI 使用 **React + Ink**（React for CLI）。我们需要用 **ratatui** 复刻。

#### React/Ink → ratatui 映射

| Claude Code (React/Ink) | Rust (ratatui) | 说明 |
|--------------------------|-----------------|------|
| `<Box>` | `ratatui::layout::Rect` + `Block` | 布局容器 |
| `<Text>` | `ratatui::text::Text` / `Span` / `Line` | 文本渲染 |
| `<TextInput>` | `rc_tui_input::TextInput` (自定义) | 输入框 |
| `useInput()` | `crossterm::Event::Key` handler | 键盘事件 |
| `useApp()` | `rc_tui::App` struct | 应用实例 |
| `useState()` | `rc_tui::state::State` | 组件状态 |
| `useEffect()` | 生命周期方法 / tokio spawn | 副作用 |
| `useContext()` | 函数参数传递 (Rust 无 context) | 上下文 |
| `<Static>` | 缓冲区渲染 | 静态内容 |
| `<FocusContext>` | 自定义 focus 管理 | 焦点管理 |
| `render()` | `ratatui::Frame` 渲染 | 渲染函数 |
| `<Newline>` | `Line::from("")` | 换行 |
| `<Spacer>` | `Layout::split()` 空间分配 | 间距 |
| `useStdout()` | `crossterm::stdout()` | 标准输出 |

#### TUI 组件映射（完整 407 文件）

**核心消息组件** (`components/messages/`):

| Claude Code 组件 | Rust 组件 | 说明 |
|------------------|-----------|------|
| `AssistantTextMessage.tsx` | `messages::AssistantTextMessage` | 助手文本消息 |
| `AssistantThinkingMessage.tsx` | `messages::AssistantThinkingMessage` | 思考消息 |
| `AssistantRedactedThinkingMessage.tsx` | `messages::AssistantRedactedThinking` | 已编辑思考 |
| `AssistantToolUseMessage.tsx` | `messages::AssistantToolUseMessage` | 工具使用消息 |
| `UserTextMessage.tsx` | `messages::UserTextMessage` | 用户文本 |
| `UserImageMessage.tsx` | `messages::UserImageMessage` | 用户图片 |
| `UserToolResultMessage/` (7 文件) | `messages::UserToolResultMessage` | 工具结果（成功/失败/拒绝/取消） |
| `AttachmentMessage.tsx` | `messages::AttachmentMessage` | 附件消息 |
| `CompactBoundaryMessage.tsx` | `messages::CompactBoundaryMessage` | 压缩边界 |
| `SystemTextMessage.tsx` | `messages::SystemTextMessage` | 系统消息 |
| `SystemAPIErrorMessage.tsx` | `messages::SystemAPIErrorMessage` | API 错误 |
| `GroupedToolUseContent.tsx` | `messages::GroupedToolUseContent` | 分组工具使用 |
| `CollapsedReadSearchContent.tsx` | `messages::CollapsedReadSearch` | 折叠搜索 |
| `HookProgressMessage.tsx` | `messages::HookProgressMessage` | Hook 进度 |
| `AdvisorMessage.tsx` | `messages::AdvisorMessage` | Advisor 消息 |
| `PlanApprovalMessage.tsx` | `messages::PlanApprovalMessage` | 计划审批 |
| `RateLimitMessage.tsx` | `messages::RateLimitMessage` | 速率限制 |
| `ShutdownMessage.tsx` | `messages::ShutdownMessage` | 关闭消息 |
| `SnipBoundaryMessage.tsx` | `messages::SnipBoundaryMessage` | Snip 边界 |
| `TaskAssignmentMessage.tsx` | `messages::TaskAssignmentMessage` | 任务分配 |
| `HighlightedThinkingText.tsx` | `messages::HighlightedThinkingText` | 高亮思考 |

**用户消息变体** (`components/messages/`):

| Claude Code 组件 | Rust 组件 |
|------------------|-----------|
| `UserBashInputMessage.tsx` | `messages::UserBashInput` |
| `UserBashOutputMessage.tsx` | `messages::UserBashOutput` |
| `UserChannelMessage.tsx` | `messages::UserChannel` |
| `UserCommandMessage.tsx` | `messages::UserCommand` |
| `UserCrossSessionMessage.tsx` | `messages::UserCrossSession` |
| `UserForkBoilerplateMessage.tsx` | `messages::UserForkBoilerplate` |
| `UserGitHubWebhookMessage.tsx` | `messages::UserGitHubWebhook` |
| `UserMemoryInputMessage.tsx` | `messages::UserMemoryInput` |
| `UserPlanMessage.tsx` | `messages::UserPlan` |
| `UserPromptMessage.tsx` | `messages::UserPrompt` |
| `UserResourceUpdateMessage.tsx` | `messages::UserResourceUpdate` |
| `UserTeammateMessage.tsx` | `messages::UserTeammate` |
| `UserAgentNotificationMessage.tsx` | `messages::UserAgentNotification` |
| `UserLocalCommandOutputMessage.tsx` | `messages::UserLocalCommandOutput` |

**权限组件** (`components/permissions/`):

| Claude Code 组件 | Rust 组件 |
|------------------|-----------|
| `PermissionRequest.tsx` | `permissions::PermissionRequest` |
| `PermissionDialog.tsx` | `permissions::PermissionDialog` |
| `PermissionPrompt.tsx` | `permissions::PermissionPrompt` |
| `PermissionExplanation.tsx` | `permissions::PermissionExplanation` |
| `SandboxPermissionRequest.tsx` | `permissions::SandboxPermission` |
| `FallbackPermissionRequest.tsx` | `permissions::FallbackPermission` |
| `BashPermissionRequest/` (2 文件) | `permissions::BashPermission` |
| `FileEditPermissionRequest/` (1 文件) | `permissions::FileEditPermission` |
| `FileWritePermissionRequest/` (2 文件) | `permissions::FileWritePermission` |
| `FilesystemPermissionRequest.tsx` | `permissions::FilesystemPermission` |
| `NotebookEditPermissionRequest/` (2 文件) | `permissions::NotebookEditPermission` |
| `PowerShellPermissionRequest/` (2 文件) | `permissions::PowerShellPermission` |
| `WebFetchPermissionRequest.tsx` | `permissions::WebFetchPermission` |
| `SkillPermissionRequest.tsx` | `permissions::SkillPermission` |
| `MonitorPermissionRequest/` (1 文件) | `permissions::MonitorPermission` |
| `ReviewArtifactPermissionRequest.tsx` | `permissions::ReviewArtifactPermission` |
| `EnterPlanModePermissionRequest.tsx` | `permissions::PlanModePermission` |
| `ExitPlanModePermissionRequest.tsx` | `permissions::ExitPlanModePermission` |
| `AskUserQuestionPermissionRequest/` (5 文件) | `permissions::AskUserPermission` |
| `ComputerUseApproval/` (1 文件) | `permissions::ComputerUseApproval` |
| `rules/` (7 文件) | `permissions::Rules` |
| `shellPermissionHelpers.tsx` | `permissions::ShellHelpers` |
| `useShellPermissionFeedback.ts` | `permissions::ShellFeedback` |

**Agent 组件** (`components/agents/`):

| Claude Code 组件 | Rust 组件 |
|------------------|-----------|
| `AgentsList.tsx` | `agents::AgentsList` |
| `AgentsMenu.tsx` | `agents::AgentsMenu` |
| `AgentDetail.tsx` | `agents::AgentDetail` |
| `AgentEditor.tsx` | `agents::AgentEditor` |
| `AgentNavigationFooter.tsx` | `agents::AgentNavigationFooter` |
| `ModelSelector.tsx` | `agents::ModelSelector` |
| `ToolSelector.tsx` | `agents::ToolSelector` |
| `ColorPicker.tsx` | `agents::ColorPicker` |
| `validateAgent.ts` | `agents::validate` |
| `generateAgent.ts` | `agents::generate` |
| `new-agent-creation/` (11 文件) | `agents::CreateWizard` |

**其他关键组件**:

| Claude Code 组件 | Rust 组件 | 说明 |
|------------------|-----------|------|
| `App.tsx` | `rc_tui_components::App` | 顶层容器 |
| `Message.tsx` | `rc_tui_components::Message` | 消息路由（627 行） |
| `Messages.tsx` | `rc_tui_components::MessageList` | 消息列表 |
| `TextInput.tsx` | `rc_tui_components::TextInput` | 输入框 |
| `VimTextInput.tsx` | `rc_tui_components::VimInput` | Vim 输入 |
| `BaseTextInput.tsx` | `rc_tui_components::BaseTextInput` | 基础输入 |
| `Markdown.tsx` | `rc_tui_components::Markdown` | Markdown 渲染 |
| `MarkdownTable.tsx` | `rc_tui_components::MarkdownTable` | 表格渲染 |
| `Spinner.tsx` | `rc_tui_components::Spinner` | 加载动画 |
| `StatusLine.tsx` | `rc_tui_components::StatusLine` | 状态栏 |
| `Stats.tsx` | `rc_tui_components::Stats` | 统计信息 |
| `TokenWarning.tsx` | `rc_tui_components::TokenWarning` | Token 警告 |
| `CompactSummary.tsx` | `rc_tui_components::CompactSummary` | 压缩摘要 |
| `ContextVisualization.tsx` | `rc_tui_components::ContextViz` | 上下文可视化 |
| `ModelPicker.tsx` | `rc_tui_components::ModelPicker` | 模型选择器 |
| `ProviderPicker.tsx` | `rc_tui_components::ProviderPicker` | Provider 选择器 |
| `ThemePicker.tsx` | `rc_tui_components::ThemePicker` | 主题选择器 |
| `LanguagePicker.tsx` | `rc_tui_components::LanguagePicker` | 语言选择器 |
| `OutputStylePicker.tsx` | `rc_tui_components::OutputStylePicker` | 输出风格选择器 |
| `EffortIndicator.ts` | `rc_tui_components::EffortIndicator` | 努力程度指示器 |
| `MemoryUsageIndicator.tsx` | `rc_tui_components::MemoryIndicator` | 记忆使用指示器 |
| `FileEditToolDiff.tsx` | `rc_tui_components::FileEditDiff` | 文件编辑 diff |
| `StructuredDiff.tsx` | `rc_tui_components::StructuredDiff` | 结构化 diff |
| `StructuredDiffList.tsx` | `rc_tui_components::StructuredDiffList` | diff 列表 |
| `HighlightedCode.tsx` | `rc_tui_components::HighlightedCode` | 代码高亮 |
| `VirtualMessageList.tsx` | `rc_tui_components::VirtualList` | 虚拟滚动列表 |
| `MCPServerApprovalDialog.tsx` | `rc_tui_components::McpApprovalDialog` | MCP 审批对话框 |
| `CostThresholdDialog.tsx` | `rc_tui_components::CostDialog` | 费用对话框 |
| `BypassPermissionsModeDialog.tsx` | `rc_tui_components::BypassDialog` | 绕过权限对话框 |
| `AutoModeOptInDialog.tsx` | `rc_tui_components::AutoModeDialog` | 自动模式对话框 |
| `Feedback.tsx` | `rc_tui_components::Feedback` | 反馈组件 |
| `Onboarding.tsx` | `rc_tui_components::Onboarding` | 引导流程 |
| `TaskListV2.tsx` | `rc_tui_components::TaskList` | 任务列表 |
| `SearchBox.tsx` | `rc_tui_components::SearchBox` | 搜索框 |
| `GlobalSearchDialog.tsx` | `rc_tui_components::GlobalSearch` | 全局搜索 |
| `HistorySearchDialog.tsx` | `rc_tui_components::HistorySearch` | 历史搜索 |
| `ExportDialog.tsx` | `rc_tui_components::ExportDialog` | 导出对话框 |
| `ExitFlow.tsx` | `rc_tui_components::ExitFlow` | 退出流程 |
| `DiagnosticsDisplay.tsx` | `rc_tui_components::Diagnostics` | 诊断显示 |
| `AutoUpdater.tsx` | `rc_tui_components::AutoUpdater` | 自动更新 |
| `IdeStatusIndicator.tsx` | `rc_tui_components::IdeStatus` | IDE 状态 |
| `DevBar.tsx` | `rc_tui_components::DevBar` | 开发工具栏 |
| `FullscreenLayout.tsx` | `rc_tui_components::FullscreenLayout` | 全屏布局 |
| `ConfigurableShortcutHint.tsx` | `rc_tui_components::ShortcutHint` | 快捷键提示 |
| `ClickableImageRef.tsx` | `rc_tui_components::ImageRef` | 可点击图片 |
| `FilePathLink.tsx` | `rc_tui_components::FilePathLink` | 文件路径链接 |
| `PrBadge.tsx` | `rc_tui_components::PrBadge` | PR 徽章 |
| `FastIcon.tsx` | `rc_tui_components::FastIcon` | 图标 |
| `ThinkingToggle.tsx` | `rc_tui_components::ThinkingToggle` | 思考开关 |
| `LogSelector.tsx` | `rc_tui_components::LogSelector` | 日志选择器 |
| `ContextSuggestions.tsx` | `rc_tui_components::ContextSuggestions` | 上下文建议 |
| `CtrlOToExpand.tsx` | `rc_tui_components::CtrlOExpand` | Ctrl+O 展开 |
| `PressEnterToContinue.tsx` | `rc_tui_components::PressEnter` | 按回车继续 |
| `SessionBackgroundHint.tsx` | `rc_tui_components::SessionBgHint` | 会话后台提示 |
| `SessionPreview.tsx` | `rc_tui_components::SessionPreview` | 会话预览 |
| `ResumeTask.tsx` | `rc_tui_components::ResumeTask` | 恢复任务 |
| `TeleportError.tsx` | `rc_tui_components::TeleportError` | Teleport 错误 |
| `TeleportProgress.tsx` | `rc_tui_components::TeleportProgress` | Teleport 进度 |
| `ValidationErrorsList.tsx` | `rc_tui_components::ValidationErrors` | 验证错误列表 |
| `StatusNotices.tsx` | `rc_tui_components::StatusNotices` | 状态通知 |
| `TagTabs.tsx` | `rc_tui_components::TagTabs` | 标签页 |
| `RemoteCallout.tsx` | `rc_tui_components::RemoteCallout` | 远程提示 |
| `EffortCallout.tsx` | `rc_tui_components::EffortCallout` | 努力程度提示 |
| `AntModelSwitchCallout.tsx` | `rc_tui_components::ModelSwitchCallout` | 模型切换提示 |
| `AwsAuthStatusBox.tsx` | `rc_tui_components::AwsAuthStatus` | AWS 认证状态 |
| `SentryErrorBoundary.ts` | `rc_tui_components::ErrorBoundary` | 错误边界 |
| `OffscreenFreeze.tsx` | `rc_tui_components::OffscreenFreeze` | 离屏冻结 |

#### TUI Hook 映射（105 文件）

| Claude Code Hook | Rust 对应 | 说明 |
|------------------|-----------|------|
| `useCanUseTool.tsx` | `rc_permissions::can_use_tool()` | 工具权限检查 (204 行) |
| `useTextInput.ts` | `rc_tui_input::TextInputState` | 文本输入状态管理 |
| `useVimInput.ts` | `rc_tui_input::VimInputState` | Vim 模式输入 |
| `useGlobalKeybindings.tsx` | `rc_tui::keybindings::GlobalBindings` | 全局快捷键 |
| `useCommandKeybindings.tsx` | `rc_tui::keybindings::CommandBindings` | 命令快捷键 |
| `useTerminalSize.ts` | `rc_tui::terminal::size()` | 终端尺寸 |
| `useVirtualScroll.ts` | `rc_tui::scroll::VirtualScroll` | 虚拟滚动 |
| `useHistorySearch.ts` | `rc_tui::search::HistorySearch` | 历史搜索 |
| `useMergedTools.ts` | `rc_tools::merge_tools()` | 工具合并 |
| `useMergedCommands.ts` | `rc_commands::merge_commands()` | 命令合并 |
| `useSettings.ts` | `rc_config::RuntimeConfig` | 设置管理 |
| `useBackgroundTaskNavigation.ts` | `rc_tui::navigation::BackgroundNav` | 后台任务导航 |
| `useTurnDiffs.ts` | `rc_tui::diffs::TurnDiffs` | Turn diff 管理 |
| `usePromptSuggestion.ts` | `rc_tui::suggestion::PromptSuggestion` | 提示建议 |
| `useTypeahead.tsx` | `rc_tui::typeahead::Typeahead` | 自动补全 |
| `useElapsedTime.ts` | `rc_tui::timer::ElapsedTime` | 计时器 |
| `useCancelRequest.ts` | `rc_tui::cancel::CancelRequest` | 取消请求 |
| `useCommandQueue.ts` | `rc_tui::queue::CommandQueue` | 命令队列 |
| `useSessionBackgrounding.ts` | `rc_tui::session::Backgrounding` | 会话后台 |
| `useIdeConnectionStatus.ts` | `rc_tui::ide::ConnectionStatus` | IDE 连接状态 |
| `useIDEIntegration.tsx` | `rc_tui::ide::Integration` | IDE 集成 |
| `useScheduledTasks.ts` | `rc_tui::tasks::ScheduledTasks` | 定时任务 |
| `useTasksV2.ts` | `rc_tui::tasks::TasksV2` | 任务管理 |
| `useTaskListWatcher.ts` | `rc_tui::tasks::TaskListWatcher` | 任务列表监控 |
| `useMemoryUsage.ts` | `rc_tui::memory::MemoryUsage` | 记忆使用 |
| `useUpdateNotification.ts` | `rc_tui::update::UpdateNotification` | 更新通知 |
| `useVoice.ts` / `useVoiceEnabled.ts` / `useVoiceIntegration.tsx` | `rc_tui::voice::*` | 语音相关 |
| `useClipboardImageHint.ts` | `rc_tui::clipboard::ImageHint` | 剪贴板图片 |
| `useCopyOnSelect.ts` | `rc_tui::clipboard::CopyOnSelect` | 选择复制 |
| `usePasteHandler.ts` | `rc_tui_input::PasteHandler` | 粘贴处理 |
| `useArrowKeyHistory.tsx` | `rc_tui_input::ArrowKeyHistory` | 方向键历史 |
| `useAssistantHistory.ts` | `rc_tui::history::AssistantHistory` | 助手历史 |
| `useDiffData.ts` | `rc_tui::diffs::DiffData` | Diff 数据 |
| `useDiffInIDE.ts` | `rc_tui::ide::DiffInIDE` | IDE diff |
| `useDoublePress.ts` | `rc_tui::input::DoublePress` | 双击检测 |
| `useDynamicConfig.ts` | `rc_config::DynamicConfig` | 动态配置 |
| `useExitOnCtrlCD.ts` | `rc_tui::input::ExitOnCtrlCD` | Ctrl+C/D 退出 |
| `useInputBuffer.ts` | `rc_tui_input::InputBuffer` | 输入缓冲 |
| `useManagePlugins.ts` | `rc_plugins::ManagePlugins` | 插件管理 |
| `useMergedClients.ts` | `rc_mcp::MergedClients` | MCP 客户端合并 |
| `useMinDisplayTime.ts` | `rc_tui::display::MinDisplayTime` | 最小显示时间 |
| `useNotifyAfterTimeout.ts` | `rc_tui::notification::AfterTimeout` | 超时通知 |
| `usePrStatus.ts` | `rc_tui::git::PrStatus` | PR 状态 |
| `useQueueProcessor.ts` | `rc_tui::queue::QueueProcessor` | 队列处理 |
| `useRemoteSession.ts` | `rc_tui::remote::RemoteSession` | 远程会话 |
| `useSearchInput.ts` | `rc_tui::search::SearchInput` | 搜索输入 |
| `useSettingsChange.ts` | `rc_config::SettingsChange` | 设置变更 |
| `useSkillsChange.ts` | `rc_skills::SkillsChange` | 技能变更 |
| `useSSHSession.ts` | `rc_tui::ssh::SSHSession` | SSH 会话 |
| `useSwarmInitialization.ts` | `rc_tui::swarm::SwarmInit` | Swarm 初始化 |
| `useSwarmPermissionPoller.ts` | `rc_tui::swarm::PermissionPoller` | Swarm 权限轮询 |
| `useTeammateViewAutoExit.ts` | `rc_tui::teammate::AutoExit` | Teammate 自动退出 |
| `useTeleportResume.tsx` | `rc_tui::teleport::Resume` | Teleport 恢复 |
| `useTimeout.ts` | `rc_tui::timer::Timeout` | 超时处理 |
| `useLogMessages.ts` | `rc_tui::logging::LogMessages` | 日志消息 |
| `useLspPluginRecommendation.tsx` | `rc_tui::lsp::PluginRecommendation` | LSP 插件推荐 |
| `useApiKeyVerification.ts` | `rc_tui::auth::ApiKeyVerification` | API Key 验证 |
| `useChromeExtensionNotification.tsx` | `rc_tui::chrome::ExtensionNotification` | Chrome 扩展通知 |
| `useClaudeCodeHintRecommendation.tsx` | `rc_tui::hints::HintRecommendation` | 提示推荐 |
| `useIdeAtMentioned.ts` | `rc_tui::ide::AtMentioned` | IDE @提及 |
| `useIdeLogging.ts` | `rc_tui::ide::Logging` | IDE 日志 |
| `useIdeSelection.ts` | `rc_tui::ide::Selection` | IDE 选择 |
| `useInboxPoller.ts` | `rc_tui::inbox::InboxPoller` | 收件箱轮询 |
| `useMailboxBridge.ts` | `rc_tui::mailbox::MailboxBridge` | 邮箱桥接 |
| `useMainLoopModel.ts` | `rc_tui::model::MainLoopModel` | 主循环模型 |
| `useOfficialMarketplaceNotification.tsx` | `rc_tui::marketplace::Notification` | 市场通知 |
| `usePluginRecommendationBase.tsx` | `rc_tui::plugins::RecommendationBase` | 插件推荐基础 |
| `usePromptsFromClaudeInChrome.tsx` | `rc_tui::chrome::Prompts` | Chrome 提示 |
| `useReplBridge.tsx` | `rc_tui::bridge::ReplBridge` | REPL 桥接 |
| `useDirectConnect.ts` | `rc_tui::ide::DirectConnect` | IDE 直连 |
| `useBlink.ts` | `rc_tui::display::Blink` | 光标闪烁 |
| `useAwaySummary.ts` | `rc_tui::summary::AwaySummary` | 离开摘要 |
| `useDeferredHookMessages.ts` | `rc_tui::hooks::DeferredMessages` | 延迟 Hook 消息 |
| `useFileHistorySnapshotInit.ts` | `rc_tui::file::HistorySnapshotInit` | 文件历史快照 |
| `useIssueFlagBanner.ts` | `rc_tui::banner::IssueFlag` | Issue 标志横幅 |
| `useSkillImprovementSurvey.ts` | `rc_tui::skills::ImprovementSurvey` | 技能改进调查 |
| `useAfterFirstRender.ts` | `rc_tui::lifecycle::AfterFirstRender` | 首次渲染后 |
| `useSessionBackgrounding.ts` | `rc_tui::session::Backgrounding` | 会话后台化 |

---

## 3. System Prompt 完整复刻方案

### 3.1 Claude Code System Prompt 结构（915 行代码管理）

所有 prompt 必须 **按 Claude Code 原版的结构、分段、缓存边界、关键约束复刻**；静态关键文案逐段对齐，动态部分允许按 Rust 运行时与产品形态做等价适配，但不得丢失行为约束。

```
System Prompt 组成:
├── Static Prefix (可缓存, cache_scope: global)
│   ├── Intro Section
│   │   └── "You are an interactive agent that helps users with software
│   │        engineering tasks. Use the instructions below and the tools
│   │        available to you to assist the user."
│   │   └── CYBER_RISK_INSTRUCTION (网络安全警告)
│   │   └── "IMPORTANT: You must NEVER generate or guess URLs..."
│   │
│   ├── System Section (6 条规则)
│   │   ├── 所有文本输出直接显示给用户，支持 GFM markdown
│   │   ├── 工具在用户选择的权限模式下执行
│   │   ├── <system-reminder> 标签说明
│   │   ├── 外部数据 prompt injection 警告
│   │   ├── Hooks 说明
│   │   └── 自动压缩说明
│   │
│   ├── Doing Tasks Section (12+ 条指南)
│   │   ├── 软件工程任务为主
│   │   ├── "You are highly capable"
│   │   ├── 先读再改
│   │   ├── 不创建不必要的文件
│   │   ├── 不给时间估计
│   │   ├── 失败时先诊断再换策略
│   │   ├── 安全注意事项 (OWASP top 10)
│   │   ├── 代码风格指南 (不添加不必要功能/重构/注释)
│   │   │   ├── 不添加不必要的错误处理/验证
│   │   │   ├── 不创建一次性操作的辅助函数
│   │   │   ├── 默认不写注释（ant 用户）
│   │   │   ├── 验证后再报告完成（ant 用户）
│   │   │   └── 如实报告结果（ant 用户）
│   │   ├── 避免向后兼容性 hack
│   │   └── 用户帮助信息 (/help)
│   │
│   ├── Actions Section (行为谨慎性)
│   │   └── "Carefully consider the reversibility and blast radius of actions"
│   │   └── 风险操作列表 (删除/force push/发送消息/上传内容等)
│   │   └── "measure twice, cut once"
│   │
│   ├── Using Your Tools Section
│   │   ├── 专用工具优先于 Bash
│   │   │   ├── Read 代替 cat/head/tail
│   │   │   ├── Edit 代替 sed/awk
│   │   │   ├── Write 代替 cat heredoc
│   │   │   ├── Glob 代替 find/ls
│   │   │   └── Grep 代替 grep/rg
│   │   ├── TodoWrite/TaskCreate 任务管理
│   │   └── 并行工具调用指南
│   │
│   ├── Output Efficiency Section
│   │   └── "Go straight to the point. Be extra concise."
│   │
│   └── Tone and Style Section
│       ├── 不使用 emoji（除非用户要求）
│       ├── 简洁回复
│       ├── file_path:line_number 引用格式
│       └── GitHub issue/PR 链接格式
│
├── DYNAMIC_BOUNDARY (缓存分割点)
│
└── Dynamic Suffix (不可缓存)
    ├── Session Guidance
    │   ├── AskUserQuestion 使用指南
    │   ├── ! 前缀 shell 命令说明
    │   ├── Agent 工具使用指南
    │   │   └── fork 模式: "Calling Agent without subagent_type creates a fork"
    │   │   └── 非 fork 模式: "Use Agent tool with specialized agents"
    │   ├── Explore agent 使用指南
    │   ├── Skill 工具使用指南
    │   ├── DiscoverSkills 指南
    │   └── Verification agent 使用指南
    ├── Memory (MEMORY.md 内容)
    ├── Env Info
    │   ├── OS 类型/版本
    │   ├── Shell 类型
    │   ├── CWD
    │   ├── 日期
    │   └── Git 状态
    ├── MCP Instructions
    │   └── 各 MCP server 的使用说明
    ├── Language (语言偏好)
    ├── Output Style (输出风格配置)
    ├── Scratchpad (草稿板说明)
    ├── FunctionResultClearing (模型特定)
    ├── SummarizeToolResults
    ├── Brief Section
    └── Proactive Section
```

### 3.2 Rust 实现

```rust
// rc-system-prompt/src/lib.rs

pub struct SystemPromptBuilder {
    sections: Vec<PromptSection>,
    cache_boundary_index: usize,
}

pub enum PromptSection {
    // Static (cacheable)
    Intro(OutputStyleConfig),
    System,
    DoingTasks { is_ant_user: bool },
    Actions,
    UsingYourTools { enabled_tools: HashSet<String>, has_embedded_search: bool },
    OutputEfficiency { is_ant_user: bool },
    ToneAndStyle { is_ant_user: bool },
    
    // Boundary
    DynamicBoundary,
    
    // Dynamic (not cacheable)
    SessionGuidance {
        has_ask_user: bool,
        has_agent: bool,
        has_skills: bool,
        skill_commands: Vec<SkillCommand>,
        is_fork_enabled: bool,
        is_explore_enabled: bool,
    },
    Memory { content: String },
    EnvInfo {
        os_type: String,
        os_version: String,
        cwd: PathBuf,
        date: String,
        is_git: bool,
        additional_dirs: Vec<PathBuf>,
    },
    McpInstructions { clients: Vec<McpInstruction> },
    Language { preference: String },
    OutputStyle { config: OutputStyleConfig },
    Scratchpad,
    FunctionResultClearing { model: String },
    SummarizeToolResults,
}

impl SystemPromptBuilder {
    /// 构建完整的 system prompt
    pub fn build(&self) -> Vec<String> { ... }
    
    /// 获取可缓存的静态前缀
    pub fn static_prefix(&self) -> Vec<String> { ... }
    
    /// 获取动态后缀
    pub fn dynamic_suffix(&self) -> Vec<String> { ... }
    
    /// 计算 cache control points
    pub fn cache_control_points(&self) -> Vec<CacheControl> { ... }
}
```

---

## 4. 工具 Prompt 完整复刻清单

所有工具 prompt 都必须 **按 Claude Code 原版的结构、关键文案、行为限制和安全边界对齐复刻**；目标不是机械搬运，而是可审计的高保真等价实现。以下列出每个工具 prompt 的完整内容要点。

### 4.1 BashTool Prompt (370 行 TS)

**来源**: `src/tools/BashTool/prompt.ts` → `getSimplePrompt()`

```
必须包含的完整内容:
1. 工具描述:
   "Executes a given bash command and returns its output."
   "The working directory persists between commands, but shell state does not."

2. 工具偏好指南:
   - File search: Use Glob (NOT find or ls)
   - Content search: Use Grep (NOT grep or rg)
   - Read files: Use Read (NOT cat/head/tail)
   - Edit files: Use Edit (NOT sed/awk)
   - Write files: Use Write (NOT echo >/cat <<EOF)
   - Communication: Output text directly (NOT echo/printf)

3. Instructions:
   - 创建文件前先用 ls 验证父目录
   - 路径含空格时用双引号
   - 尽量用绝对路径，避免 cd
   - 超时设置 (默认 2 分钟，最大 10 分钟)
   - run_in_background 使用说明
   - 多命令执行规则 (&& 顺序, ; 不关心失败, 不用换行)
   - Git 命令规则 (不 amend, 不 destructive, 不 skip hooks)
   - 避免 sleep (用 Monitor/background 代替)

4. 沙箱部分 (getSimpleSandboxSection):
   - 文件系统读写限制
   - 网络限制
   - $TMPDIR 使用说明
   - dangerouslyDisableSandbox 使用规则

5. Git Commit 指南 (getCommitAndPRInstructions):
   - Git Safety Protocol (6 条规则)
   - Commit 创建流程 (4 步)
   - PR 创建流程 (3 步)
   - HEREDOC 格式示例
```

### 4.2 FileWriteTool Prompt

**来源**: `src/tools/FileWriteTool/prompt.ts` → `getWriteToolDescription()`

```
必须包含:
1. "Writes a file to the local filesystem."
2. Pre-read 要求: "If this is an existing file, you MUST use Read first"
3. "Prefer the Edit tool for modifying existing files"
4. "NEVER create documentation files (*.md) unless explicitly requested"
5. "Only use emojis if the user explicitly requests it"
```

### 4.3 FileEditTool Prompt

**来源**: `src/tools/FileEditTool/prompt.ts`

```
必须包含:
1. "Performs exact string replacements in files"
2. old_string/new_string 匹配规则
3. "Use Read first to understand the file's content"
4. 多次编辑的并行调用指南
5. old_string 必须唯一匹配
```

### 4.4 AgentTool Prompt (288 行 TS)

**来源**: `src/tools/AgentTool/prompt.ts` → `getPrompt()`

```
必须包含:
1. Agent 列表格式:
   "- type: whenToUse (Tools: ...)"

2. When to fork 部分 (forkEnabled 时):
   - Fork yourself when intermediate tool output isn't worth keeping
   - Research: fork open-ended questions
   - Implementation: prefer to fork multi-edit work
   - Forks are cheap because they share prompt cache
   - Don't peek: don't Read the output_file
   - Don't race: never fabricate fork results
   - Writing a fork prompt: directive style

3. Writing the prompt 部分:
   - Be specific about scope
   - Don't re-explain background
   - Include file paths, line ranges, error messages

4. Parallel agents 部分:
   - Launch independent agents in parallel
   - Don't duplicate work

5. Agent 输出处理:
   - output_file 路径说明
   - 完成通知机制
```

### 4.5 其他工具 Prompt

| 工具 | Prompt 要点 | 对应 TS 文件 |
|------|------------|-------------|
| `Read` | 文件读取, offset/limit 说明, line number 格式 | `FileReadTool/prompt.ts` |
| `Glob` | 文件搜索, glob 模式说明, 返回格式 | `GlobTool/prompt.ts` |
| `Grep` | 内容搜索, 正则表达式说明, 输出格式 | `GrepTool/prompt.ts` |
| `TodoWrite` | 任务规划, 进度跟踪, markdown checklist 格式 | `TodoWriteTool/prompt.ts` |
| `AskUserQuestion` | 向用户提问, 选项格式, 多选支持 | `AskUserQuestionTool/prompt.ts` |
| `Skill` | 技能调用, 参数传递, SKILL.md 格式 | `SkillTool/prompt.ts` |
| `WebSearch` | 网络搜索, 结果格式 | `WebSearchTool/prompt.ts` |
| `WebFetch` | 网页获取, 内容提取 | `WebFetchTool/prompt.ts` |
| `MCPTool` | MCP 工具调用, schema 说明 | `MCPTool/prompt.ts` |
| `TaskCreate` | 创建后台任务, 预算设置 | `TaskCreateTool/prompt.ts` |
| `TaskGet` | 获取任务状态 | `TaskGetTool/prompt.ts` |
| `TaskList` | 列出任务 | `TaskListTool/prompt.ts` |
| `TaskUpdate` | 更新任务状态 | `TaskUpdateTool/prompt.ts` |
| `TaskOutput` | 获取任务输出 | `TaskOutputTool/prompt.ts` |
| `TaskStop` | 停止任务 | `TaskStopTool/prompt.ts` |
| `EnterPlanMode` | 进入计划模式 | `EnterPlanModeTool/prompt.ts` |
| `ExitPlanMode` | 退出计划模式 | `ExitPlanModeTool/prompt.ts` |
| `ToolSearch` | 工具搜索 | `ToolSearchTool/prompt.ts` |
| `SyntheticOutput` | 结构化输出 | `SyntheticOutputTool/prompt.ts` |
| `LSP` | LSP 操作 | `LSPTool/prompt.ts` |
| `NotebookEdit` | Notebook 编辑 | `NotebookEditTool/prompt.ts` |
| `PowerShell` | PowerShell 命令 | `PowerShellTool/prompt.ts` |
| `WebBrowser` | 浏览器操作 | `WebBrowserTool/prompt.ts` |
| `Snip` | 上下文裁剪 | `SnipTool/prompt.ts` |
| `Monitor` | 监控后台进程 | `MonitorTool/prompt.ts` |
| `Brief` | 简报生成 | `BriefTool/prompt.ts` |
| `DiscoverSkills` | 技能发现 | `DiscoverSkillsTool/prompt.ts` |
| `Workflow` | 工作流执行 | `WorkflowTool/prompt.ts` |
| `SendMessage` | Agent 间通信 | `SendMessageTool/prompt.ts` |
| `Sleep` | 延迟等待 | `SleepTool/prompt.ts` |
| `Config` | 配置管理 | `ConfigTool/prompt.ts` |
| `ScheduleCron` | 定时任务 | `ScheduleCronTool/prompt.ts` |
| `ReviewArtifact` | 审查产物 | `ReviewArtifactTool/prompt.ts` |
| `VerifyPlanExecution` | 验证计划执行 | `VerifyPlanExecutionTool/prompt.ts` |
| `TeamCreate` | 创建团队 | `TeamCreateTool/prompt.ts` |
| `TeamDelete` | 删除团队 | `TeamDeleteTool/prompt.ts` |
| `TerminalCapture` | 终端捕获 | `TerminalCaptureTool/prompt.ts` |
| `REPL` | REPL 执行 | `REPLTool/prompt.ts` |
| `Tungsten` | Tungsten 操作 | `TungstenTool/prompt.ts` |
| `RemoteTrigger` | 远程触发 | `RemoteTriggerTool/prompt.ts` |
| `SendUserFile` | 发送用户文件 | `SendUserFileTool/prompt.ts` |
| `ListMcpResources` | 列出 MCP 资源 | `ListMcpResourcesTool/prompt.ts` |
| `ReadMcpResource` | 读取 MCP 资源 | `ReadMcpResourceTool/prompt.ts` |
| `McpAuth` | MCP 认证 | `McpAuthTool/prompt.ts` |
| `EnterWorktree` | 进入 Worktree | `EnterWorktreeTool/prompt.ts` |
| `ExitWorktree` | 退出 Worktree | `ExitWorktreeTool/prompt.ts` |
| `OverflowTest` | 溢出测试 | `OverflowTestTool/prompt.ts` |

---

## 5. 斜杠命令完整复刻清单

### 5.1 核心命令（P0）

| 命令 | 对应目录 | 功能 |
|------|---------|------|
| `/help` | `commands/help/` | 帮助信息 |
| `/compact` | `commands/compact/` | 手动压缩上下文 |
| `/clear` | `commands/clear/` | 清除对话/缓存 |
| `/config` | `commands/config/` | 配置管理 |
| `/model` | `commands/model/` | 模型选择 |
| `/permissions` | `commands/permissions/` | 权限管理 |
| `/resume` | `commands/resume/` | 恢复会话 |
| `/status` | `commands/status/` | 状态查看 |
| `/doctor` | `commands/doctor/` | 诊断检查 |
| `/session` | `commands/session/` | 会话管理 |
| `/cost` | `commands/cost/` | 费用查看 |
| `/usage` | `commands/usage/` | 使用统计 |
| `/context` | `commands/context/` | 上下文管理 |
| `/memory` | `commands/memory/` | 记忆管理 |
| `/mcp` | `commands/mcp/` | MCP 管理 |
| `/skills` | `commands/skills/` | 技能管理 |
| `/tasks` | `commands/tasks/` | 任务管理 |
| `/version` | `commands/version.ts` | 版本信息 |
| `/commit` | `commands/commit.ts` | Git 提交 |
| `/review` | `commands/review.ts` | 代码审查 |
| `/init` | `commands/init.ts` | 项目初始化 |

### 5.2 增强命令（P1）

| 命令 | 对应目录 | 功能 |
|------|---------|------|
| `/diff` | `commands/diff/` | 查看变更 |
| `/export` | `commands/export/` | 导出会话 |
| `/files` | `commands/files/` | 文件管理 |
| `/hooks` | `commands/hooks/` | Hook 管理 |
| `/login` / `/logout` | `commands/login/`, `commands/logout/` | 认证 |
| `/provider` | `commands/provider/` | Provider 切换 |
| `/theme` | `commands/theme/` | 主题切换 |
| `/color` | `commands/color/` | 颜色设置 |
| `/effort` | `commands/effort/` | 努力程度设置 |
| `/fast` | `commands/fast/` | 快速模式 |
| `/vim` | `commands/vim/` | Vim 模式 |
| `/plan` | `commands/plan/` | 计划模式 |
| `/stats` | `commands/stats/` | 统计信息 |
| `/summary` | `commands/summary/` | 会话摘要 |
| `/tag` | `commands/tag/` | 标签管理 |
| `/thinkback` | `commands/thinkback/` | 回顾思考 |
| `/upgrade` | `commands/upgrade/` | 升级 |
| `/voice` | `commands/voice/` | 语音模式 |
| `/issue` | `commands/issue/` | 问题报告 |
| `/share` | `commands/share/` | 分享会话 |
| `/feedback` | `commands/feedback/` | 反馈 |
| `/agents` | `commands/agents/` | Agent 管理 |
| `/install` | `commands/install.tsx` | 安装 |
| `/desktop` | `commands/desktop/` | 桌面端 |
| `/ide` | `commands/ide/` | IDE 集成 |
| `/remote-env` | `commands/remote-env/` | 远程环境 |
| `/sandbox-toggle` | `commands/sandbox-toggle/` | 沙箱切换 |
| `/rewind` | `commands/rewind/` | 回退操作 |
| `/rename` | `commands/rename/` | 重命名会话 |
| `/branch` | `commands/branch/` | 分支管理 |
| `/pr_comments` | `commands/pr_comments/` | PR 评论 |
| `/output-style` | `commands/output-style/` | 输出风格 |
| `/privacy-settings` | `commands/privacy-settings/` | 隐私设置 |
| `/release-notes` | `commands/release-notes/` | 发布说明 |
| `/onboarding` | `commands/onboarding/` | 引导流程 |
| `/passes` | `commands/passes/` | Passes 管理 |
| `/keybindings` | `commands/keybindings/` | 快捷键管理 |
| `/add-dir` | `commands/add-dir/` | 添加工作目录 |
| `/break-cache` | `commands/break-cache/` | 打破缓存 |
| `/reload-plugins` | `commands/reload-plugins/` | 重新加载插件 |
| `/thinkback-play` | `commands/thinkback-play/` | 回放思考 |
| `/reset-limits` | `commands/reset-limits/` | 重置限制 |
| `/mock-limits` | `commands/mock-limits/` | 模拟限制 |
| `/rate-limit-options` | `commands/rate-limit-options/` | 速率限制选项 |
| `/extra-usage` | `commands/extra-usage/` | 额外使用量 |
| `/perf-issue` | `commands/perf-issue/` | 性能问题 |
| `/debug-tool-call` | `commands/debug-tool-call/` | 调试工具调用 |
| `/heapdump` | `commands/btw/` | 堆转储 |
| `/chrome` | `commands/chrome/` | Chrome 集成 |
| `/insights` | `commands/insights.ts` | 洞察 |
| `/security-review` | `commands/security-review.ts` | 安全审查 |
| `/ultraplan` | `commands/ultraplan.tsx` | 超级计划 |
| `/statusline` | `commands/statusline.tsx` | 状态栏配置 |
| `/commit-push-pr` | `commands/commit-push-pr.ts` | 提交推送 PR |
| `/agents-platform` | `commands/agents-platform/` | Agent 平台 |
| `/backfill-sessions` | `commands/backfill-sessions/` | 回填会话 |
| `/teleport` | `commands/teleport/` | Teleport |
| `/terminalSetup` | `commands/terminalSetup/` | 终端设置 |
| `/mobile` | `commands/mobile/` | 移动端 |
| `/plugin` | `commands/plugin/` | 插件管理 |
| `/ant-trace` | `commands/ant-trace/` | 追踪 |
| `/autofix-pr` | `commands/autofix-pr/` | 自动修复 PR |
| `/bughunter` | `commands/bughunter/` (via btw) | Bug 猎手 |
| `/good-claude` | `commands/good-claude/` | 好的 Claude |
| `/install-github-app` | `commands/install-github-app/` | 安装 GitHub App |
| `/install-slack-app` | `commands/install-slack-app/` | 安装 Slack App |
| `/oauth-refresh` | `commands/oauth-refresh/` | OAuth 刷新 |
| `/remote-setup` | `commands/remote-setup/` | 远程设置 |
| `/stickers` | `commands/stickers/` | 贴纸 |
| `/ctx_viz` | `commands/ctx_viz/` | 上下文可视化 |
| `/copy` | `commands/copy/` | 复制 |
| `/exit` | `commands/exit/` | 退出 |
| `/bridge` | `commands/bridge/` | 桥接 |
| `/btw` | `commands/btw/` | BTW |
| `/dream` | `commands/btw/` (via dream) | Dream |
| `/init-verifiers` | `commands/init-verifiers.ts` | 初始化验证器 |
| `/brief` | `commands/brief.ts` | 简报 |
| `/simplify` | via skills | 简化代码 |
| `/verify` | via skills | 验证 |
| `/claude-api` | via skills | Claude API 文档 |
| `/skillify` | via skills | 技能化 |
| `/remember` | via skills | 记忆 |
| `/stuck` | via skills | 卡住时 |
| `/loop` | via skills | 循环 |
| `/scheduleRemoteAgents` | via skills | 调度远程 Agent |
| `/runSkillGenerator` | via skills | 运行技能生成器 |
| `/updateConfig` | via skills | 更新配置 |
| `/debug` | via skills | 调试 |
| `/loremIpsum` | via skills | Lorem Ipsum |
| `/keybindings` | via skills | 快捷键 |
| `/hunter` | via skills | 猎手 |
| `/claudeInChrome` | via skills | Chrome 集成 |
| `/batch` | via skills | 批处理 |
| `/simplify` | via skills | 简化 |
| `/verifyContent` | via skills | 验证内容 |

---

## 6. 服务层架构

### 6.1 API 服务 (`services/api/`)

对应 `src/services/api/claude.ts` (3,420 行)：

```
rc-provider/src/
├── lib.rs                    # ProviderClient (增强)
├── anthropic.rs              # 🆕 Anthropic API 完整实现
│   ├── stream_messages()     # 流式消息 API
│   ├── stream_tools()        # 流式工具 API
│   ├── query_haiku()         # Haiku 快速查询
│   ├── query_with_model()    # 指定模型查询
│   ├── add_cache_breakpoints() # 缓存断点
│   ├── build_system_prompt_blocks() # System prompt 块
│   ├── strip_excess_media()  # 媒体裁剪
│   ├── accumulate_usage()    # 使用量累积
│   ├── cleanup_stream()      # 流清理
│   ├── update_usage()        # 使用量更新
│   ├── adjust_params_for_non_streaming() # 非流式参数调整
│   └── get_max_output_tokens_for_model() # 模型最大输出
├── openai.rs                 # 🆕 OpenAI API 增强
├── bedrock.rs                # Bedrock (现有，增强)
├── vertex.rs                 # Vertex (现有，增强)
├── streaming.rs              # 流式处理 (增强)
├── context.rs                # 上下文管理 (增强)
├── circuit_breaker.rs        # 熔断器 (现有)
├── cost.rs                   # 费用计算 (增强)
├── credential_pool.rs        # 凭证池 (现有)
├── failover.rs               # 故障转移 (现有)
├── model_info.rs             # 模型信息 (增强)
├── sigv4.rs                  # AWS 签名 (现有)
└── retry.rs                  # 🆕 重试逻辑 (withRetry)
```

### 6.2 MCP 服务 (`services/mcp/`)

对应 `src/services/mcp/*` (25 文件)：

```
rc-mcp/src/
├── lib.rs                  # (增强)
├── client.rs               # MCP 客户端 (增强)
├── connection_manager.rs   # 🆕 MCPConnectionManager
├── auth.rs                 # 🆕 OAuth 认证 (auth.ts)
├── elicitation.rs          # 🆕 Elicitation handler
├── channel_allowlist.rs    # 🆕 Channel allowlist
├── channel_permissions.rs  # 🆕 Channel permissions
├── channel_notification.rs # 🆕 Channel notification
├── env_expansion.rs        # 🆕 环境变量展开
├── headers_helper.rs       # 🆕 Headers helper
├── normalization.rs        # 🆕 MCP 规范化
├── official_registry.rs    # 🆕 官方注册表
├── oauth_port.rs           # 🆕 OAuth 端口
├── in_process_transport.rs # 🆕 进程内传输
├── sdk_control_transport.rs # 🆕 SDK 控制传输
├── vscode_sdk.rs           # 🆕 VSCode SDK MCP
├── xaa.rs                  # 🆕 XAA 认证
├── xaa_idp.rs              # 🆕 XAA IDP 登录
├── claudeai.rs             # 🆕 Claude.ai MCP
├── config.rs               # (增强)
├── types.rs                # (增强)
├── utils.rs                # (增强)
├── string_utils.rs         # 🆕 MCP 字符串工具
└── manage_connections.rs   # 🆕 useManageMCPConnections
```

### 6.3 Compact 服务 (`services/compact/`)

对应 `src/services/compact/*` (14 文件)：

```
rc-compact/src/
├── lib.rs              # CompactEngine + CompactStrategy trait
├── auto.rs             # autoCompact.ts - 自动压缩
├── micro.rs            # microCompact.ts - 微压缩
├── reactive.rs         # reactiveCompact.ts - 响应式压缩
├── snip.rs             # snipCompact.ts - Snip 压缩
├── snip_projection.rs  # 🆕 snipProjection.ts - Snip 投影
├── session_memory.rs   # sessionMemoryCompact.ts - 会话记忆压缩
├── api_micro.rs        # 🆕 apiMicrocompact.ts - API 微压缩
├── boundary.rs         # Compact boundary 管理
├── grouping.rs         # grouping.ts (消息分组)
├── prompt.rs           # compact prompt.ts (压缩提示词)
├── warning.rs          # compactWarningHook.ts + compactWarningState.ts
├── cleanup.rs          # postCompactCleanup.ts
├── config.rs           # cachedMCConfig.ts + timeBasedMCConfig.ts
└── attachments.rs      # 🆕 压缩后附件处理
```

### 6.4 Analytics 服务 (`services/analytics/`)

对应 `src/services/analytics/*` (10 文件，~2,000 行)：

```
rc-analytics/src/
├── lib.rs                  # 分析入口 + 公共 API
├── growthbook.rs           # 🆕 Feature flags (对应 growthbook.ts 1,156 行)
│   ├── GrowthBookClient    # GrowthBook SDK 封装
│   ├── get_feature_value() # 特性开关读取 (CACHED_MAY_BE_STALE)
│   ├── check_feature_gate() # 特性门控检查
│   ├── user_attributes    # 用户属性 (id/sessionId/deviceID/platform 等)
│   ├── experiment_tracking # 实验曝光追踪
│   └── security_gate      # 安全门控 (re-initialization 保护)
├── first_party_logger.rs   # 🆕 第一方事件日志 (对应 firstPartyEventLogger.ts)
├── first_party_exporter.rs # 🆕 第一方事件导出 (对应 firstPartyEventLoggingExporter.ts)
├── datadog.rs              # 🆕 Datadog APM (对应 datadog.ts)
├── metadata.rs             # 🆕 分析元数据 (对应 metadata.ts)
├── sink.rs                 # 🆕 事件接收器 (对应 sink.ts)
├── sink_killswitch.rs      # 🆕 接收器开关 (对应 sinkKillswitch.ts)
├── otel.rs                 # 🆕 OpenTelemetry 追踪
├── usage.rs                # 🆕 使用量追踪 (token 计数/缓存命中率)
├── cost.rs                 # 🆕 费用追踪 (按模型/会话/工具)
├── performance.rs          # 🆕 性能指标 (TTFB/流延迟/工具执行时间)
└── session.rs              # 🆕 会话分析 (会话长度/工具使用频率)
```

**注意**: GrowthBook 是 Claude Code 的 A/B 测试和特性开关平台。在 Rust 实现中，我们需要：
- 实现一个轻量的特性开关系统（可用配置文件替代远程 GrowthBook）
- 保留 `get_feature_value()` API 接口，但后端可切换为本地配置
- 第一方事件日志可对接 OpenTelemetry

### 6.5 LSP 服务 (`services/lsp/`)

对应 `src/services/lsp/*` (8 文件，~1,500 行)：

```
rc-lsp/src/
├── lib.rs                  # LSP 入口
├── client.rs               # 🆕 LSPClient (对应 LSPClient.ts 448 行)
│   ├── create_lsp_client() # 工厂函数，创建 LSP 客户端
│   ├── start()             # 启动 LSP server 进程
│   ├── initialize()        # 发送 initialize 请求
│   ├── send_request()      # 通用请求发送
│   ├── send_notification() # 通用通知发送
│   ├── on_notification()   # 注册通知处理器
│   ├── on_request()        # 注册请求处理器
│   └── stop()              # 停止 server
├── server_manager.rs       # 🆕 LSPServerManager (对应 LSPServerManager.ts 421 行)
│   ├── initialize()        # 加载所有配置的 LSP server
│   ├── shutdown()          # 关闭所有 server
│   ├── get_server_for_file() # 按文件扩展名路由
│   ├── ensure_server_started() # 懒启动
│   ├── open_file()         # textDocument/didOpen
│   ├── change_file()       # textDocument/didChange
│   ├── save_file()         # textDocument/didSave
│   └── close_file()        # textDocument/didClose
├── server_instance.rs      # 🆕 LSPServerInstance (对应 LSPServerInstance.ts)
├── diagnostic_registry.rs  # 🆕 LSPDiagnosticRegistry (对应 387 行)
│   ├── register_pending()  # 注册待处理诊断
│   ├── check_diagnostics() # 检查待处理诊断
│   ├── get_attachments()   # 转换为 Attachment[]
│   └── deduplication       # LRU 缓存去重 (MAX_DELIVERED_FILES=500)
├── config.rs               # 🆕 LSP 配置加载 (对应 config.ts)
├── types.rs                # 🆕 LSP 类型定义 (ScopedLspServerConfig 等)
├── transport.rs            # 🆕 LSP 传输层 (stdio JSON-RPC)
└── passive_feedback.rs     # 🆕 被动反馈 (对应 passiveFeedback.ts)
```

**关键依赖**: `lsp-types` crate (0.95+) 提供 LSP 协议类型，`tokio::process` 管理 LSP server 子进程。

### 6.6 其他服务

| 服务 | 对应目录 | Rust crate |
|------|---------|-----------|
| `services/tools/` | toolExecution.ts (1,746 行) | `rc-tools` (增强) |
| `services/contextCollapse/` | 上下文折叠 | `rc-context` (新建) |
| `services/extractMemories/` | 记忆提取 | `rc-memory` (新建) |
| `services/SessionMemory/` | 会话记忆 | `rc-memory` (新建) |
| `services/PromptSuggestion/` | 提示建议 | `rc-tui::suggestion` |
| `services/settingsSync/` | 设置同步 | `rc-config` (增强) |
| `services/oauth/` | OAuth 认证 | `rc-provider` (增强) |
| `services/policyLimits/` | 策略限制 | `rc-context` (新建) |
| `services/teamMemorySync/` | 团队记忆同步 | `rc-memory` (新建) |
| `services/tips/` | 提示 | `rc-tui::tips` |
| `services/toolUseSummary/` | 工具使用摘要 | `rc-analytics` (新建) |
| `services/remoteManagedSettings/` | 远程托管设置 | `rc-config` (增强) |
| `services/skillSearch/` | 技能搜索 | `rc-skills` (增强) |
| `services/plugins/` | 插件服务 | `rc-plugins` (增强) |
| `services/MagicDocs/` | Magic Docs | P2 |
| `services/autoDream/` | Auto Dream | P2 |
| `services/AgentSummary/` | Agent 摘要 | `rc-agents` (增强) |

---

## 7. 状态管理架构

### 7.1 AppState (`state/` 6 文件)

对应 `src/state/AppStateStore.ts` + `store.ts` + `selectors.ts`：

```rust
// rc-core/src/state.rs

/// 全局应用状态 (对应 AppStateStore.ts)
pub struct AppState {
    // 消息
    pub messages: Vec<Message>,
    
    // 工具权限上下文
    pub tool_permission_context: ToolPermissionContext,
    
    // 文件历史
    pub file_history: FileHistoryState,
    
    // 归属信息
    pub attribution: AttributionState,
    
    // 文件状态缓存
    pub file_state_cache: FileStateCache,
    
    // 进行中的工具调用
    pub in_progress_tool_use_ids: HashSet<String>,
    
    // 响应长度
    pub response_length: Option<usize>,
    
    // 推测状态
    pub speculation: SpeculationState,
    
    // 完成边界
    pub completion_boundary: Option<CompletionBoundary>,
    
    // 后台任务
    pub background_tasks: Vec<BackgroundTaskState>,
    
    // Agent 状态
    pub agent_states: HashMap<String, AgentState>,
    
    // 压缩状态
    pub compact_state: CompactState,
    
    // 费用追踪
    pub cost_tracker: CostTracker,
    
    // 使用量
    pub usage: UsageSummary,
}

/// ToolPermissionContext (对应 Tool.ts:123-148)
pub struct ToolPermissionContext {
    pub permission_mode: PermissionMode,
    pub cwd: PathBuf,
    pub available_tools: HashSet<String>,
    pub denied_tools: HashSet<String>,
    pub auto_approved_patterns: Vec<String>,
    pub recent_denials: Vec<DenialRecord>,
}

/// FileHistoryState
pub struct FileHistoryState {
    pub snapshots: HashMap<String, FileSnapshot>,
    pub max_snapshots: usize,
}

/// 消息类型 (对应 Message.tsx:82-354)
pub enum Message {
    Attachment(AttachmentMessage),
    Assistant(AssistantMessage),
    User(UserMessage),
    System(SystemMessage),
    GroupedToolUse(GroupedToolUseMessage),
    CollapsedReadSearch(CollapsedReadSearchMessage),
}

/// 助手消息内容块 (对应 AssistantMessageBlock:483-589)
pub enum AssistantContentBlock {
    ToolUse { id: String, name: String, input: Value },
    Text { text: String },
    RedactedThinking { data: String },
    Thinking { text: String, signature: String },
    AdvisorToolResult { content: String },
}
```

### 7.2 Selectors (`state/selectors.ts`)

```rust
/// 状态选择器 (对应 selectors.ts)
impl AppState {
    pub fn get_messages(&self) -> &[Message] { &self.messages }
    pub fn get_last_assistant_message(&self) -> Option<&AssistantMessage> { ... }
    pub fn get_tool_permission_context(&self) -> &ToolPermissionContext { ... }
    pub fn is_tool_in_progress(&self, tool_use_id: &str) -> bool { ... }
    pub fn get_background_tasks(&self) -> &[BackgroundTaskState] { ... }
    pub fn get_total_cost(&self) -> f64 { ... }
    pub fn get_usage(&self) -> &UsageSummary { ... }
}
```

---

## 8. 快捷键系统

### 8.1 Keybinding 架构 (`keybindings/` 15 文件)

```
rc-tui/src/keybindings/
├── mod.rs              # 快捷键入口
├── default_bindings.rs # defaultBindings.ts → 默认快捷键
├── context.rs          # KeybindingContext.tsx → 快捷键上下文
├── provider.rs         # KeybindingProviderSetup.tsx → 快捷键提供者
├── load_user.rs        # loadUserBindings.ts → 加载用户自定义
├── match.rs            # match.ts → 快捷键匹配
├── parser.rs           # parser.ts → 快捷键解析
├── reserved.rs         # reservedShortcuts.ts → 保留快捷键
├── resolver.rs         # resolver.ts → 快捷键解析器
├── schema.rs           # schema.ts → 快捷键 schema
├── format.rs           # shortcutFormat.ts → 快捷键格式
├── template.rs         # template.ts → 快捷键模板
├── types.rs            # types.ts → 快捷键类型
├── use_binding.rs      # useKeybinding.ts → 使用快捷键
├── use_display.rs      # useShortcutDisplay.ts → 快捷键显示
└── validate.rs         # validate.ts → 快捷键验证
```

### 8.2 默认快捷键映射

| 快捷键 | 功能 | Claude Code 绑定 |
|--------|------|-----------------|
| `Escape` | 取消/中断 | 取消当前操作 |
| `Ctrl+C` | 中断 | 中断当前流 |
| `Ctrl+D` | 退出 | 退出 CLI |
| `Enter` | 发送 | 发送消息 |
| `Shift+Enter` | 换行 | 新行 |
| `Up/Down` | 历史 | 浏览历史 |
| `Ctrl+O` | 展开 | 展开工具输出 |
| `Tab` | 补全 | 自动补全 |
| `Ctrl+L` | 清屏 | 清除屏幕 |
| `/` | 命令 | 斜杠命令 |

---

## 9. Bridge 系统架构

### 9.1 Bridge 模块 (`bridge/` 33 文件)

```
rc-ui-bridge/src/
├── lib.rs                    # Bridge 入口
├── api.rs                    # 🆕 bridgeApi.ts → Bridge API
├── config.rs                 # 🆕 bridgeConfig.ts → Bridge 配置
├── debug.rs                  # 🆕 bridgeDebug.ts → Bridge 调试
├── enabled.rs                # 🆕 bridgeEnabled.ts → Bridge 启用检测
├── main.rs                   # 🆕 bridgeMain.ts → Bridge 主逻辑
├── messaging.rs              # 🆕 bridgeMessaging.ts → Bridge 消息
├── permission_callbacks.rs   # 🆕 bridgePermissionCallbacks.ts → 权限回调
├── pointer.rs                # 🆕 bridgePointer.ts → Bridge 指针
├── status.rs                 # 🆕 bridgeStatusUtil.ts → 状态工具
├── ui.rs                     # 🆕 bridgeUI.ts → Bridge UI
├── capacity_wake.rs          # 🆕 capacityWake.ts → 容量唤醒
├── code_session_api.rs       # 🆕 codeSessionApi.ts → Code Session API
├── create_session.rs         # 🆕 createSession.ts → 创建会话
├── debug_utils.rs            # 🆕 debugUtils.ts → 调试工具
├── env_less_config.rs        # 🆕 envLessBridgeConfig.ts → 无环境配置
├── flush_gate.rs             # 🆕 flushGate.ts → 刷新门控
├── inbound_attachments.rs    # 🆕 inboundAttachments.ts → 入站附件
├── inbound_messages.rs       # 🆕 inboundMessages.ts → 入站消息
├── init_repl.rs              # 🆕 initReplBridge.ts → 初始化 REPL Bridge
├── jwt.rs                    # 🆕 jwtUtils.ts → JWT 工具
├── peer_sessions.rs          # 🆕 peerSessions.ts → 对等会话
├── poll_config.rs            # 🆕 pollConfig.ts + pollConfigDefaults.ts → 轮询配置
├── remote_core.rs            # 🆕 remoteBridgeCore.ts → 远程 Bridge 核心
├── repl.rs                   # 🆕 replBridge.ts → REPL Bridge
├── repl_handle.rs            # 🆕 replBridgeHandle.ts → REPL Bridge 句柄
├── repl_transport.rs         # 🆕 replBridgeTransport.ts → REPL Bridge 传输
├── session_id_compat.rs      # 🆕 sessionIdCompat.ts → 会话 ID 兼容
├── session_runner.rs         # 🆕 sessionRunner.ts → 会话运行器
├── trusted_device.rs         # 🆕 trustedDevice.ts → 受信设备
├── types.rs                  # 🆕 types.ts → Bridge 类型
├── webhook_sanitizer.rs      # 🆕 webhookSanitizer.ts → Webhook 清理
└── work_secret.rs            # 🆕 workSecret.ts → 工作密钥
```

---

## 10. Skills 系统

### 10.1 Bundled Skills (`skills/bundled/` 53 文件)

```
rc-skills/src/
├── lib.rs                    # (增强)
├── bundled.rs                # 🆕 bundledSkills.ts → 内置技能注册
├── load_dir.rs               # 🆕 loadSkillsDir.ts → 加载技能目录
├── mcp_builders.rs           # 🆕 mcpSkillBuilders.ts → MCP 技能构建器
├── mcp_skills.rs             # 🆕 mcpSkills.ts → MCP 技能
├── search.rs                 # 🆕 技能搜索
└── bundled/
    ├── batch.rs              # batch.ts → 批处理
    ├── claude_api.rs         # claude-api/ → Claude API 文档
    ├── claude_in_chrome.rs   # claudeInChrome.ts → Chrome 集成
    ├── debug.rs              # debug.ts → 调试
    ├── dream.rs              # dream.ts → Dream
    ├── hunter.rs             # hunter.ts → 猎手
    ├── keybindings.rs        # keybindings.ts → 快捷键
    ├── loop.rs               # loop.ts → 循环
    ├── lorem_ipsum.rs        # loremIpsum.ts → Lorem Ipsum
    ├── remember.rs           # remember.ts → 记忆
    ├── run_generator.rs      # runSkillGenerator.ts → 运行技能生成器
    ├── schedule_agents.rs    # scheduleRemoteAgents.ts → 调度远程 Agent
    ├── simplify.rs           # simplify.ts → 简化代码
    ├── skillify.rs           # skillify.ts → 技能化
    ├── stuck.rs              # stuck.ts → 卡住时
    ├── update_config.rs      # updateConfig.ts → 更新配置
    ├── verify.rs             # verify/ → 验证
    └── verify_content.rs     # verifyContent.ts → 验证内容
```

---

## 11. 分阶段实施计划（8 个 Phase）

### Phase 1: 核心类型 + 事件系统 + 状态管理（3-4 周）

**目标**: 建立所有核心类型定义、统一事件模型和状态管理

**产出文件**:
- `crates/rc-engine-events/src/lib.rs` - 统一事件类型
- `crates/rc-engine-events/src/types.rs` - EngineEvent 枚举
- `crates/rc-transcript/src/lib.rs` - Transcript 结构
- `crates/rc-transcript/src/entry.rs` - TranscriptEntry
- `crates/rc-transcript/src/boundary.rs` - CompactBoundary
- `crates/rc-transcript/src/storage.rs` - 持久化
- `crates/rc-core/src/state.rs` - AppState

**关键类型**: EngineEvent, Transcript, AppState, Message, AssistantContentBlock

---

### Phase 2: Query Engine V2（4-5 周）

**目标**: 实现完整的查询引擎状态机

**对应文件**:
- `src/QueryEngine.ts` (1,296 行) → `engine.rs`
- `src/query.ts` (1,730 行) → `query_loop.rs`
- `src/query/config.ts` → `config.rs`
- `src/query/tokenBudget.ts` → `token_budget.rs`
- `src/query/transitions.ts` → `transitions.rs`
- `src/query/stopHooks.ts` → `stop_hooks.rs`

**关键类型**: QueryEngine, EngineState (11 种状态), QueryEngineConfig

---

### Phase 3: 工具运行时 V2 + 所有 Prompt（5-6 周）

**目标**: 实现所有 50+ 工具，每个工具的 prompt、schema、runtime 行为与 Claude Code 对齐

**核心工具 (P0, 20 个)**:
Bash, Read, Write, Edit, Glob, Grep, Agent, TodoWrite,
TaskCreate/Get/List/Update/Output/Stop, MCPTool, WebSearch, WebFetch,
AskUserQuestion, Skill, SendMessage

**增强工具 (P1, 30+ 个)**:
EnterPlanMode, ExitPlanMode, ToolSearch, SyntheticOutput, Sleep,
LSP, NotebookEdit, EnterWorktree, ExitWorktree, PowerShell,
ListMcpResources, ReadMcpResource, McpAuth, WebBrowser, Snip,
Monitor, ReviewArtifact, VerifyPlanExecution, ScheduleCron, Workflow,
TeamCreate, TeamDelete, TerminalCapture, REPL, Tungsten, RemoteTrigger,
SendUserFile, Brief, DiscoverSkills, OverflowTest, Config

**产出**: rc-tool-prompts crate (50+ prompt 文件)

---

### Phase 4: System Prompt + Compaction + Context（4-5 周）

**目标**: 实现 Claude Code 级别的 system prompt 管理和上下文压缩

**对应**: constants/prompts.ts (915 行) + services/compact/* (14 文件)

**产出**: rc-system-prompt, rc-compact, rc-context 三个 crate

---

### Phase 5: TUI 复刻（5-6 周）

**目标**: 用 ratatui 完整复刻 Claude Code 的 TUI

**产出**:
- rc-tui (框架层 + 快捷键 + 命令)
- rc-tui-components (100+ 组件，对应 407 TSX 文件)
- rc-tui-input (输入处理)

---

### Phase 6: MCP + Agent + Hook 增强（4-5 周）

**目标**: 实现 Claude Code 级别的 MCP 连接管理、Agent 系统和 Hook 生命周期

**产出**: rc-mcp (25 文件), rc-agents (14 文件), rc-hooks (105 hooks)

---

### Phase 7: CLI Surface + Skills + Memory + Commands（3-4 周）

**目标**: 补齐 CLI 入口参数、技能系统、记忆管理和 80+ 斜杠命令

**产出**: rc-commands (80+ 命令), rc-skills (bundled skills), rc-memory

---

### Phase 8: 集成测试 + 端到端验证（3-4 周）

**目标**: 全面测试所有模块的集成

**测试矩阵**: 单元/集成/TUI/压力/兼容性/恢复/并发/Prompt

---

## 12. 实施时间线

| 阶段 | 内容 | 预估时间 | 累计 |
|------|------|---------|------|
| **Phase 1** | 核心类型 + 事件 + 状态 | 3-4 周 | 4 周 |
| **Phase 2** | Query Engine V2 | 4-5 周 | 9 周 |
| **Phase 3** | 工具运行时 + Prompt (50+) | 5-6 周 | 15 周 |
| **Phase 4** | System Prompt + Compaction | 4-5 周 | 20 周 |
| **Phase 5** | TUI 复刻 (ratatui) | 5-6 周 | 26 周 |
| **Phase 6** | MCP + Agent + Hook | 4-5 周 | 31 周 |
| **Phase 7** | CLI + Skills + Memory + Commands | 3-4 周 | 35 周 |
| **Phase 8** | 集成测试 + 验证 | 3-4 周 | 39 周 |

**总计**: 约 9-10 个月（1 人全职）或 5 个月（2 人团队）

说明：`main-only` 不等于串行开发。核心类型冻结后，可按 provider/query、tools/prompts、TUI/bridge、MCP/agents/hooks、verification 五条工作流并行推进。

### 12.1 并行工作流

| 工作流 | 范围 | 最早启动点 | 依赖 |
|------|------|-----------|------|
| A. Core / Transcript | `rc-engine-events`、`rc-transcript`、核心 ID / Message / Event 类型 | 立即 | 无 |
| B. Provider / Query / Compact | `rc-provider v2`、`rc-query-engine`、`rc-system-prompt`、`rc-compact` | Phase 1 类型冻结后 | A |
| C. Tools / Permissions / Commands | `rc-tools v2`、`rc-tool-prompts`、权限分类、命令面 | Phase 1 期间即可抽取 prompt 与 tool trait | A，部分依赖 B |
| D. TUI / State / Bridge | `rc-tui`、`rc-tui-components`、bridge/state/selectors/keybindings | EngineEvent / AppState 冻结后 | A，部分依赖 B |
| E. MCP / Agents / Hooks / Skills | `rc-mcp`、`rc-agents`、`rc-hooks`、`rc-skills`、memory | QueryEngine 接口稳定后 | A+B+C |
| F. Verification / Migration / Perf | `rc-migrate`、兼容层、基准测试、回归审计、parity ledger | 从第一天开始持续进行 | 贯穿全部 |

### 12.2 关键路径

1. 先冻结核心类型、事件模型、transcript 边界和 session 兼容层。
2. 以 shadow 模式接入 `rc-query-engine` 与 `rc-provider v2`，不要直接切断现有 `conversation.rs`。
3. 在 engine 新主链路稳定前，完成工具 runtime、prompt ledger、权限分类器与 command surface 的接线。
4. 在 cache boundary、compaction、system prompt 全部入位前，不切默认引擎。
5. TUI/bridge 以 EngineEvent 和 AppState 为契约独立推进，但默认切换要晚于 engine / tools / compact。
6. MCP、agent、hooks、skills 属于扩大能力面，不得阻塞核心引擎稳定，但必须在最终 cutover 前补齐。
7. 最终默认切换必须在完整测试矩阵、迁移路径、回滚路径都就位后进行。

### 12.3 Phase Exit Gates（不可协商）

| Phase | 合并 / 切换门禁 |
|------|----------------|
| Phase 1 | 核心类型冻结；所有 app 编译通过；transcript round-trip 与 session 兼容测试通过 |
| Phase 2 | `rc-query-engine` 可 shadow-run 真实 transcript；流式回调、fallback、turn-budget、failure tracking 测试通过 |
| Phase 3 | 50+ 工具全部进入统一 Tool trait；prompt ledger 完整；工具 schema / permission 分类稳定 |
| Phase 4 | system prompt 分段、cache boundary、5 种 compaction 策略全部可测；与 engine 集成后长对话稳定 |
| Phase 5 | TUI snapshot / 交互 / 性能 / 崩溃恢复测试通过；bridge 事件不丢失、不乱序 |
| Phase 6 | MCP OAuth / Elicitation、Fork Agent、Hook 生命周期全部跑通；关键失败路径可恢复 |
| Phase 7 | CLI surface、skills、memory、80+ 命令全量可用；多 provider 行为审计完成 |
| Phase 8 | 压力 / 并发 / 恢复 / 兼容性 / 回滚矩阵全绿，才允许默认切换与 compat 清理 |

### 12.4 `main` 合并准则

- `main` 是唯一长期交付线；不建立长期并行开发分支。
- 每次合并都必须保持 workspace 可编译，并通过对应模块的 smoke / integration tests。
- 新旧路径并存阶段必须通过 feature flag 或 compat shim 明确隔离，禁止隐式替换。
- 每个 cutover 提交都必须带回滚开关、迁移说明和 parity 缺口清单。

---

## 13. 后置模块（不阻塞核心 cutover，但仍保留在总范围内）

| 模块 | 处理方式 |
|------|------|
| `buddy/` (6 文件) | 后置到 P3/P4，不作为 CLI/TUI parity 的首批 cutover 门槛 |
| `proactive/` (2 文件) | 后置实现，但保留在总范围内 |
| `coordinator/` (2 文件) | 后置实现，但保留在总范围内 |
| `voice/` (1 文件) | 后置到 P3，纳入独立能力包 |
| `server/` (3 文件) | 后置到 remote/bridge 统一路线，不从总范围删除 |
| `native-ts/` (4 文件) | 不逐文件照搬，但对应能力必须映射到 Rust / FFI / 平台层 |
| `moreright/` (1 文件) | 后置审计，按实际产品价值决定最终归宿 |
| `outputStyles/` (1 文件) | 吸收进 `rc-output-styles`，能力保留 |
| `services/MagicDocs/` | 后置到 P2/P3，不阻塞核心 cutover |
| `services/autoDream/` | 后置到 P2/P3，不阻塞核心 cutover |
| `schemas/` | 允许由 Rust 运行时 / codegen 生成，但 schema 能力本身仍需对齐 |
| `jobs/` | 后置到任务系统统一重构阶段 |

**核心 cutover 前必须完成 P0/P1 全量复刻；上表模块不从总范围中删除，只调整实现时序与落地形态。**

---

## 14. 关键依赖版本

| Rust crate | 版本 | 用途 |
|-----------|------|------|
| `ratatui` | 0.29+ | TUI 框架 |
| `crossterm` | 0.28+ | 终端控制 |
| `tokio` | 1.x | 异步运行时 |
| `reqwest` | 0.12+ | HTTP 客户端 |
| `serde` / `serde_json` | 1.x | 序列化 |
| `anyhow` | 1.x | 错误处理 |
| `clap` | 4.x | CLI 参数解析 |
| `tree-sitter` | 0.24+ | 代码高亮 |
| `pulldown-cmark` | 0.11+ | Markdown 解析 |
| `syntect` | 5.x | 语法高亮 (备选) |
| `uuid` | 1.x | UUID 生成 |
| `chrono` | 0.4+ | 时间处理 |
| `tracing` | 0.1+ | 日志 |
| `async-trait` | 0.1+ | 异步 trait |
| `futures` | 0.3+ | Future 工具 |
| `tokio-stream` | 0.1+ | 流处理 |
| `parking_lot` | 0.12+ | 高性能锁 |
| `dashmap` | 6.x | 并发 HashMap |
| `tower` | 0.5+ | 中间件/重试 |
| `tokio-util` | 0.7+ | 编解码 |
| `lsp-types` | 0.95+ | LSP 协议类型 |
| `similar` | 2.x | Diff 算法 |
| `glob` | 0.3+ | Glob 模式匹配 |
| `regex` | 1.x | 正则表达式 |
| `git2` | 0.19+ | Git 操作 (备选 CLI) |
| `tarpc` | 0.35+ | RPC 框架 (备选) |
| `toml` | 0.8+ | TOML 解析 |
| `yaml-rust2` | 0.9+ | YAML 解析 |
| `base64` | 0.22+ | Base64 编解码 |
| `hmac-sha256` | 1.x | HMAC 签名 |
| `jsonwebtoken` | 9.x | JWT 处理 |
| `tokio-tungstenite` | 0.24+ | WebSocket |
| `rustls` | 0.23+ | TLS |
| `tempfile` | 3.x | 临时文件 |
| `lru` | 0.12+ | LRU 缓存 |

---

## 15. 迁移策略

### 15.1 `main-only` 主干迁移期

```
main (唯一长期交付线)
  │
  ├── feature flags / shadow paths
  │     ├── engine-v2-shadow
  │     ├── provider-v2-shadow
  │     ├── compact-v2-shadow
  │     └── tui-v2-shadow
  │
  ├── compatibility shims
  │     ├── conversation_compat
  │     ├── provider_compat
  │     └── transcript_compat
  │
  └── milestone tags
        ├── phase1-freeze
        ├── phase2-shadow-green
        ├── phase4-prompt-freeze
        └── phase8-cutover
```

**执行规则**：

- 不建立长期并行开发分支，所有新架构都以小步提交直接落在 `main`。
- 现有路径只允许做兼容修复、回归修复和适配层接线，不再演化出新的长期 feature 面。
- 默认切换只发生在对应 phase 的 exit gates 全部通过之后；每次切换都必须保留至少一个里程碑周期的回滚路径。

### 15.2 Crate 迁移顺序

```
阶段 0: 先冻结契约与审计索引（全部直接落在 main）
  ├── EngineEvent / Message / Transcript 边界冻结
  ├── parity ledger / prompt audit index 建立
  ├── transcript migration tests 建立
  └── rc-compat          (conversation/provider/transcript 兼容层)

阶段 1: 新建独立 crate（不影响现有默认路径）
  ├── rc-engine-events   (纯类型，零依赖)
  ├── rc-transcript      (纯类型 + 文件 I/O)
  ├── rc-system-prompt   (纯字符串构建)
  ├── rc-tool-prompts    (纯字符串常量)
  └── rc-compact         (依赖 rc-engine-events)

阶段 2: 并挂接 shadow pipeline（feature flag 控制）
  ├── rc-core v2         (新增 Message/Event 类型)
  ├── rc-provider v2     (新增流式 API)
  └── rc-query-engine    (新建，替代 conversation.rs)

阶段 3: 增强现有 crate（渐进式替换默认实现）
  ├── rc-tools v2        (新增 Tool trait + 动态 prompt)
  ├── rc-mcp v2          (新增 OAuth/Elicitation)
  ├── rc-agents v2       (新增 Fork/Built-in)
  └── rc-permissions v2  (新增分类器)

阶段 4: TUI / Bridge / CLI 切换
  ├── rc-tui v2          (ratatui 完整实现)
  ├── rc-tui-components  (新建)
  └── rc-tui-input       (新建)

阶段 5: 默认切换与兼容层清理
  ├── 默认启用 v2 flags
  ├── 冻结 v1 compatibility path
  ├── 固化 rc-migrate / transcript migrator
  └── 移除已无读路径的 shim
```

### 15.3 Feature Flag 策略

```toml
# Cargo.toml feature flags
[features]
default = ["stable-main"]
stable-main = ["v1-engine", "compat-layer"]

compat-layer = []         # 新旧协议 / transcript / provider 适配层
engine-v2-shadow = []     # 新 QueryEngine 状态机（shadow 运行）
provider-v2-shadow = []   # 新流式 API / cache / thinking
tools-v2-shadow = []      # 新 Tool trait / prompt runtime
compact-v2-shadow = []    # 新压缩引擎
tui-v2-shadow = []        # 新 ratatui TUI

cutover-core = ["engine-v2-shadow", "provider-v2-shadow", "tools-v2-shadow"]
cutover-ui = ["cutover-core", "compact-v2-shadow", "tui-v2-shadow"]
full-v2 = ["cutover-ui"]
```

说明：

- `shadow` flag 用于在 `main` 上双写、双跑、双校验，不改变默认用户路径。
- `cutover-*` 只有在对应 Phase exit gates 通过后才允许打开。
- `compat-layer` 至少保留一个里程碑周期，直到 transcript / config / commands / TUI 全部完成迁移审计。

### 15.4 数据迁移

| 数据 | 当前格式 | V2 格式 | 迁移策略 |
|------|---------|---------|---------|
| 会话记录 | SQLite metadata + NDJSON transcript | Transcript V2 (带 boundary / cache metadata) | `rc-migrate` 自动转换 + shadow reader 校验 |
| 配置文件 | TOML | TOML (增强字段) | 向后兼容 |
| MCP 配置 | JSON | JSON (增强字段) | 向后兼容 |
| 权限规则 | 文本 | 文本 (增强语法) | 向后兼容 |

---

## 16. Rust 核心类型签名与 Trait 设计

### 16.1 品牌 ID 类型

```rust
// rc-core/src/ids.rs
/// 会话 ID（品牌类型，防止与 AgentId 混淆）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

/// Agent ID（品牌类型）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(label: Option<&str>) -> Self {
        let hex = format!("{:016x}", rand::random::<u64>());
        match label {
            Some(l) => Self(format!("a{}-{hex}", l)),
            None => Self(format!("a{hex}")),
        }
    }
}
```

### 16.2 Message 类型系统

```rust
// rc-core/src/message.rs
/// 消息基础属性
pub struct MessageBase {
    pub uuid: Uuid,
    pub parent_uuid: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub is_meta: bool,
    pub is_virtual: bool,
    pub is_compact_summary: bool,
    pub origin: Option<MessageOrigin>,
}

/// 统一消息枚举（对应 types/message.ts 的 Message 联合类型）
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    Progress(ProgressMessage),
    System(SystemMessage),
    Attachment(AttachmentMessage),
    HookResult(HookResultMessage),
    ToolUseSummary(ToolUseSummaryMessage),
    Tombstone(TombstoneMessage),
    GroupedToolUse(GroupedToolUseMessage),
    CollapsedReadSearch(CollapsedReadSearchMessage),
}

/// 助手消息内容块（对应 AssistantMessageBlock:483-589）
pub enum AssistantContentBlock {
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Text {
        text: String,
    },
    RedactedThinking {
        data: String,
    },
    Thinking {
        text: String,
        signature: String,
    },
    AdvisorToolResult {
        content: String,
    },
}

/// 系统消息子类型（对应 types/message.ts:54-72）
pub enum SystemMessageSubtype {
    LocalCommand,
    BridgeStatus,
    TurnDuration,
    Thinking,
    MemorySaved,
    StopHookSummary,
    Informational,
    CompactBoundary,
    MicrocompactBoundary,
    PermissionRetry,
    ScheduledTaskFire,
    AwaySummary,
    AgentsKilled,
    ApiMetrics,
    ApiError { error: String },
    FileSnapshot,
}
```

### 16.3 Tool Trait（对应 Tool.ts:362-695）

```rust
// rc-tools/src/tool_trait.rs
use async_trait::async_trait;

/// 工具验证结果
pub enum ValidationResult {
    Valid,
    Invalid { message: String },
    RequiresUserAction { message: String },
}

/// 工具结果
pub struct ToolResult<T = serde_json::Value> {
    pub output: T,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

/// 工具进度数据
pub struct ToolProgress {
    pub kind: String,
    pub data: serde_json::Value,
}

/// 工具使用上下文（对应 Tool.ts:158-300）
pub struct ToolUseContext {
    pub session_id: SessionId,
    pub agent_id: Option<AgentId>,
    pub permission_mode: PermissionMode,
    pub tool_permission_context: ToolPermissionContext,
    pub abort_controller: AbortHandle,
    pub query_chain_tracking: Option<QueryChainTracking>,
    pub file_history: FileHistoryState,
    pub attribution: AttributionState,
    pub options: ToolUseContextOptions,
}

/// 核心工具 trait（对应 Tool.ts:362-695 的 Tool<T> 类型）
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述（动态，可包含运行时信息）
    async fn description(&self, context: &ToolUseContext) -> String;

    /// JSON Schema 输入定义
    fn input_schema(&self) -> serde_json::Value;

    /// 执行工具
    async fn call(
        &self,
        input: serde_json::Value,
        context: ToolUseContext,
    ) -> Result<ToolResult, ToolError>;

    /// 验证输入
    fn validate_input(&self, input: &serde_json::Value) -> ValidationResult {
        ValidationResult::Valid
    }

    /// 检查权限
    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::Allow
    }

    /// 动态 prompt（对应 Tool.ts:518-523 prompt()）
    async fn prompt(&self, options: PromptOptions) -> Option<String> {
        None
    }

    /// 是否为搜索/读取命令
    fn is_search_or_read_command(&self, input: &serde_json::Value) -> Option<SearchReadInfo> {
        None
    }

    /// 渲染工具使用消息
    fn render_tool_use_message(&self, input: &serde_json::Value) -> ToolUseRender {
        ToolUseRender::Default
    }

    /// 渲染工具结果消息
    fn render_tool_result_message(
        &self,
        result: &ToolResult,
    ) -> Option<ToolResultRender> {
        None
    }

    /// 渲染工具进度消息
    fn render_tool_use_progress_message(
        &self,
        progress: &ToolProgress,
    ) -> Option<ToolProgressRender> {
        None
    }

    /// 渲染工具使用拒绝消息
    fn render_tool_use_rejected_message(&self) -> Option<ToolRejectedRender> {
        None
    }

    /// 渲染工具错误消息
    fn render_tool_use_error_message(&self, error: &ToolError) -> Option<ToolErrorRender> {
        None
    }

    /// 渲染分组工具使用（对应 Tool.ts:678-694）
    fn render_grouped_tool_use(&self) -> Option<GroupedToolUseRender> {
        None
    }

    /// 将工具结果映射为 API 参数
    fn map_tool_result_to_block_param(
        &self,
        result: ToolResult,
    ) -> serde_json::Value;
}
```

### 16.4 QueryEngine（对应 QueryEngine.ts:184-1177）

```rust
// rc-query-engine/src/engine.rs

/// 查询引擎配置（对应 QueryEngine.ts:130-173）
pub struct QueryEngineConfig {
    pub session_id: SessionId,
    pub model: String,
    pub provider: Arc<dyn LlmProvider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub system_prompt_builder: Arc<SystemPromptBuilder>,
    pub compact_engine: Arc<CompactEngine>,
    pub permission_provider: Arc<dyn PermissionProvider>,
    pub event_sink: Arc<dyn EventSink>,
    pub max_turns: usize,
    pub thinking_config: ThinkingConfig,
    pub effort: EffortLevel,
    pub fast_mode: bool,
}

/// 查询引擎状态（对应 query.ts:204-217 State）
pub struct EngineState {
    pub turn: usize,
    pub messages: Vec<Message>,
    pub tool_use_blocks: Vec<ToolUseBlock>,
    pub tool_results: Vec<ToolResult>,
    pub usage: UsageAccumulator,
    pub budget_tracker: BudgetTracker,
    pub compact_state: CompactState,
    pub file_history: FileHistoryState,
    pub attribution: AttributionState,
    pub speculation: SpeculationState,
    pub completion_boundary: Option<CompletionBoundary>,
    pub background_tasks: Vec<BackgroundTaskState>,
    pub agent_states: HashMap<AgentId, AgentState>,
    pub cost_tracker: CostTracker,
    pub discovered_skill_names: HashSet<String>,
    pub consecutive_failures: usize,
    pub stream_idle_timer: Option<StreamIdleTimer>,
}

/// 查询引擎事件输出（对应 QueryEngine.ts submitMessage yield 的类型）
pub enum EngineOutput {
    /// 消息更新
    Message(Message),
    /// 流事件
    StreamEvent(StreamEvent),
    /// 附件
    Attachment(AttachmentMessage),
    /// 系统消息
    System(SystemMessage),
    /// 工具使用摘要
    ToolUseSummary(ToolUseSummaryMessage),
    /// 进度
    Progress(ProgressMessage),
}

/// 查询引擎（对应 QueryEngine.ts class）
pub struct QueryEngine {
    config: QueryEngineConfig,
    state: EngineState,
}

impl QueryEngine {
    /// 提交用户消息（对应 submitMessage AsyncGenerator）
    pub async fn submit_message(
        &mut self,
        user_input: Vec<Message>,
        context: ProcessUserInputContext,
    ) -> impl Stream<Item = Result<EngineOutput, EngineError>> {
        // ...
    }

    /// 中断当前查询
    pub async fn abort(&self) { ... }

    /// 获取当前状态快照
    pub fn snapshot(&self) -> &EngineState { ... }
}

/// 用户输入上下文（对应 QueryEngine.ts:209-408 ProcessUserInputContext 30+ 字段）
pub struct ProcessUserInputContext {
    pub session_id: SessionId,
    pub agent_id: Option<AgentId>,
    pub permission_mode: PermissionMode,
    pub tool_permission_context: ToolPermissionContext,
    pub system_prompt: Vec<String>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub messages: Vec<Message>,
    pub file_history: FileHistoryState,
    pub attribution: AttributionState,
    pub thinking_config: ThinkingConfig,
    pub effort: EffortLevel,
    pub fast_mode: bool,
    pub query_source: QuerySource,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub task_budget: Option<TaskBudget>,
    pub structured_output: Option<StructuredOutputConfig>,
    pub memory_content: Option<String>,
    pub mcp_instructions: Option<String>,
    pub discovered_skills: HashSet<String>,
    pub agent_definitions: Vec<AgentDefinition>,
    pub is_non_interactive: bool,
    pub query_chain: Option<QueryChainTracking>,
    pub abort_handle: AbortHandle,
    pub on_tool_permission_context_update: Option<Box<dyn Fn(ToolPermissionContext) + Send>>,
    pub on_file_history_update: Option<Box<dyn Fn(FileHistoryState) + Send>>,
    pub on_attribution_update: Option<Box<dyn Fn(AttributionState) + Send>>,
    pub on_state_update: Option<Box<dyn Fn(&EngineState) + Send>>,
}
```

### 16.5 CompactStrategy Trait

```rust
// rc-compact/src/strategy.rs

/// 压缩结果（对应 compact.ts:299-310 CompactionResult）
pub struct CompactionResult {
    pub summary: String,
    pub summary_tokens: usize,
    pub removed_messages: Vec<Message>,
    pub preserved_messages: Vec<Message>,
    pub boundary_message: Option<Message>,
    pub recompaction_info: Option<RecompactionInfo>,
}

/// 压缩策略 trait
#[async_trait]
pub trait CompactStrategy: Send + Sync {
    /// 策略名称
    fn name(&self) -> &str;

    /// 是否需要执行压缩
    fn should_compact(&self, context: &CompactContext) -> bool;

    /// 执行压缩
    async fn compact(
        &self,
        messages: Vec<Message>,
        context: CompactContext,
    ) -> Result<CompactionResult, CompactError>;
}

/// 5 种压缩策略实现
pub struct AutoCompact { /* LLM 生成摘要 */ }
pub struct MicroCompact { /* 缓存编辑微压缩 */ }
pub struct SnipCompact { /* 按策略裁剪历史 */ }
pub struct ReactiveCompact { /* 响应式压缩 */ }
pub struct ContextCollapse { /* 上下文折叠 */ }
```

### 16.6 PermissionProvider Trait

```rust
// rc-permissions/src/provider.rs

/// 权限行为（对应 types/permissions.ts:44）
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// 权限结果（对应 useCanUseTool.tsx 的决策）
pub enum PermissionResult {
    Allow,
    Deny { reason: String },
    Ask { prompt: String },
}

/// 权限规则来源（对应 types/permissions.ts:54-63）
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

/// 权限规则（对应 types/permissions.ts:67-79）
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub behavior: PermissionBehavior,
    pub tool_name: String,
    pub rule_content: Option<String>,
}

/// 权限提供者 trait
#[async_trait]
pub trait PermissionProvider: Send + Sync {
    /// 检查工具权限
    async fn check_permission(
        &self,
        tool: &str,
        input: &serde_json::Value,
        context: &PermissionContext,
    ) -> PermissionResult;

    /// 获取当前权限规则
    fn get_rules(&self) -> &[PermissionRule];

    /// 添加权限规则
    async fn add_rule(&mut self, rule: PermissionRule, destination: PermissionUpdateDestination);

    /// 获取拒绝记录（用于 denial tracking）
    fn get_recent_denials(&self) -> &[DenialRecord];
}
```

### 16.7 Event 类型系统

```rust
// rc-engine-events/src/types.rs

/// 统一引擎事件（对应 QueryEngine.ts yield 的所有消息类型）
#[derive(Debug, Clone, Serialize)]
pub enum EngineEvent {
    // 查询生命周期
    QueryStarted { session_id: SessionId },
    QueryCompleted { session_id: SessionId, duration_ms: u64 },
    QueryAborted { session_id: SessionId },

    // 流事件
    StreamStarted { request_id: String },
    StreamMessageStart { model: String, usage: Usage },
    StreamContentBlockStart { index: usize, block_type: ContentBlockType },
    StreamContentBlockDelta { index: usize, delta: ContentBlockDelta },
    StreamContentBlockStop { index: usize },
    StreamMessageDelta { stop_reason: Option<String>, usage: Usage },
    StreamMessageStop,
    StreamError { error: String },

    // 工具事件
    ToolUseStarted { tool_use_id: String, tool_name: String, input: serde_json::Value },
    ToolUseProgress { tool_use_id: String, progress: ToolProgress },
    ToolUseCompleted { tool_use_id: String, result: ToolResult },
    ToolUseError { tool_use_id: String, error: ToolError },
    ToolUseRejected { tool_use_id: String, reason: String },

    // 压缩事件
    CompactStarted { strategy: String },
    CompactProgress { status: String },
    CompactCompleted { result: CompactionResult },

    // Agent 事件
    AgentStarted { agent_id: AgentId, agent_type: String },
    AgentCompleted { agent_id: AgentId },
    AgentFailed { agent_id: AgentId, error: String },

    // 状态事件
    StateUpdated { state_snapshot: EngineStateSnapshot },
    CostUpdated { total_cost_usd: f64 },
    UsageUpdated { usage: Usage },
}

/// 内容块类型（对应 claude.ts 流式解析）
#[derive(Debug, Clone, Serialize)]
pub enum ContentBlockType {
    ToolUse,
    ServerToolUse,
    Text,
    Thinking,
}

/// 内容块增量
#[derive(Debug, Clone, Serialize)]
pub enum ContentBlockDelta {
    InputJsonDelta { partial_json: String },
    TextDelta { text: String },
    SignatureDelta { signature: String },
    ThinkingDelta { thinking: String },
}
```

### 16.8 Hook 事件类型（对应 entrypoints/sdk/coreTypes.ts:25-53）

```rust
// rc-core/src/hooks.rs

/// Hook 事件类型（完整 26 种，对应 HOOK_EVENTS const）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Notification,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    PermissionRequest,
    PermissionDenied,
    Setup,
    TeammateIdle,
    TaskCreated,
    TaskCompleted,
    Elicitation,
    ElicitationResult,
    ConfigChange,
    WorktreeCreate,
    WorktreeRemove,
    InstructionsLoaded,
    CwdChanged,
    FileChanged,
}

/// Hook 响应（对应 types/hooks.ts syncHookResponseSchema）
pub struct HookResponse {
    pub continue_exec: bool,
    pub suppress_output: bool,
    pub stop_reason: Option<String>,
    pub decision: Option<HookDecision>,
    pub reason: Option<String>,
    pub system_message: Option<String>,
    pub hook_specific_output: Option<HookSpecificOutput>,
}

pub enum HookDecision {
    Approve,
    Block,
}

pub enum HookSpecificOutput {
    PreToolUse {
        permission_decision: Option<PermissionBehavior>,
        permission_decision_reason: Option<String>,
        updated_input: Option<serde_json::Value>,
        additional_context: Option<String>,
    },
    UserPromptSubmit {
        additional_context: Option<String>,
    },
    SessionStart {
        additional_context: Option<String>,
        initial_user_message: Option<String>,
        watch_paths: Option<Vec<String>>,
    },
    Setup {
        additional_context: Option<String>,
    },
    SubagentStart {
        additional_context: Option<String>,
    },
    // ... 其他事件类型的特定输出
}
```

---

## 17. Utils 模块完整映射（577 文件）

### 17.1 核心工具（P0，必须复刻）

| 分类 | Claude Code 文件 | 行数 | Rust 目标 | 说明 |
|------|-----------------|------|-----------|------|
| **Git** | `utils/git.ts` | 927 | `rc-tools::git` | Git 操作封装 |
| **Git** | `utils/gitDiff.ts` | ~200 | `rc_tools::git::diff` | Git diff 操作 |
| **Git** | `utils/gitSettings.ts` | ~100 | `rc_config::git` | Git 设置 |
| **Git** | `utils/ghPrStatus.ts` | ~150 | `rc_tools::git::pr` | GitHub PR 状态 |
| **Diff** | `utils/diff.ts` | 178 | `rc_tools::diff` | Diff 算法 |
| **文件** | `utils/file.ts` | 585 | `rc_tools::file_ops` | 文件操作 |
| **文件** | `utils/fileRead.ts` | ~300 | `rc_tools::file_ops::read` | 文件读取 |
| **文件** | `utils/fileReadCache.ts` | ~100 | `rc_tools::file_ops::cache` | 文件读取缓存 |
| **文件** | `utils/fsOperations.ts` | ~200 | `rc_tools::file_ops::fs` | 文件系统操作 |
| **文件** | `utils/fileHistory.ts` | ~200 | `rc_tools::file_ops::history` | 文件历史 |
| **文件** | `utils/fileStateCache.ts` | ~100 | `rc_tools::file_ops::state` | 文件状态缓存 |
| **上下文** | `utils/context.ts` | 228 | `rc_context::window` | 上下文窗口管理 |
| **上下文** | `utils/contextAnalysis.ts` | ~200 | `rc_context::analysis` | 上下文分析 |
| **上下文** | `utils/contextSuggestions.ts` | ~150 | `rc_context::suggestions` | 上下文建议 |
| **权限** | `utils/permissions/` (10+ 文件) | ~1,500 | `rc_permissions::*` | 权限系统工具 |
| **权限** | `utils/classifierApprovals.ts` | ~150 | `rc_permissions::classifier` | 分类器审批 |
| **权限** | `utils/autoModeDenials.ts` | ~100 | `rc_permissions::auto_mode` | 自动模式拒绝 |
| **配置** | `utils/config.ts` | ~500 | `rc_config::settings` | 配置管理 |
| **配置** | `utils/configConstants.ts` | ~100 | `rc_config::constants` | 配置常量 |
| **配置** | `utils/env.ts` | ~200 | `rc_config::env` | 环境变量 |
| **配置** | `utils/envUtils.ts` | ~100 | `rc_config::env_utils` | 环境变量工具 |
| **Auth** | `utils/auth.ts` | ~300 | `rc_provider::auth` | 认证 |
| **Auth** | `utils/authPortable.ts` | ~200 | `rc_provider::auth_portable` | 便携认证 |
| **Shell** | `utils/promptShellExecution.ts` | ~100 | `rc_tools::shell::prompt` | Shell 执行提示 |
| **路径** | `utils/path.ts` | ~200 | `rc_tools::path` | 路径工具 |
| **路径** | `utils/windowsPaths.ts` | ~100 | `rc_tools::path::windows` | Windows 路径 |
| **搜索** | `utils/glob.ts` | ~200 | `rc_tools::search::glob` | Glob 搜索 |
| **编辑** | `utils/readEditContext.ts` | ~200 | `rc_tools::file_ops::edit_ctx` | 读取编辑上下文 |
| **编辑** | `utils/readFileInRange.ts` | ~100 | `rc_tools::file_ops::range` | 范围读取 |

### 17.2 增强工具（P1）

| 分类 | Claude Code 文件 | 行数 | Rust 目标 | 说明 |
|------|-----------------|------|-----------|------|
| **Agent** | `utils/agentContext.ts` | ~150 | `rc_agents::context` | Agent 上下文 |
| **Agent** | `utils/agentId.ts` | ~50 | `rc_core::ids` | Agent ID |
| **Agent** | `utils/forkedAgent.ts` | ~200 | `rc_agents::fork` | Fork Agent |
| **Agent** | `utils/standaloneAgent.ts` | ~200 | `rc_agents::standalone` | 独立 Agent |
| **Agent** | `utils/agentSwarmsEnabled.ts` | ~50 | `rc_agents::swarm` | Swarm 启用 |
| **Advisor** | `utils/advisor.ts` | ~200 | `rc_query_engine::advisor` | Advisor 模型 |
| **附件** | `utils/attachments.ts` | ~200 | `rc_core::attachment` | 附件处理 |
| **附件** | `utils/imagePaste.ts` | ~100 | `rc_tui_input::image` | 图片粘贴 |
| **附件** | `utils/imageStore.ts` | ~100 | `rc_core::image` | 图片存储 |
| **附件** | `utils/imageResizer.ts` | ~100 | `rc_core::image::resize` | 图片缩放 |
| **附件** | `utils/imageValidation.ts` | ~50 | `rc_core::image::validate` | 图片验证 |
| **Markdown** | `utils/cliHighlight.ts` | ~100 | `rc_tui::highlight` | CLI 高亮 |
| **Markdown** | `utils/frontmatterParser.ts` | ~100 | `rc_tools::markdown` | Frontmatter 解析 |
| **压缩** | `utils/truncate.ts` | ~100 | `rc_compact::truncate` | 截断工具 |
| **压缩** | `utils/collapseReadSearch.ts` | ~150 | `rc_compact::collapse` | 折叠搜索 |
| **压缩** | `utils/collapseBackgroundBashNotifications.ts` | ~100 | `rc_compact::collapse_bash` | 折叠 Bash |
| **压缩** | `utils/collapseHookSummaries.ts` | ~100 | `rc_compact::collapse_hooks` | 折叠 Hook |
| **压缩** | `utils/collapseTeammateShutdowns.ts` | ~50 | `rc_compact::collapse_teammate` | 折叠 Teammate |
| **费用** | `utils/billing.ts` | ~200 | `rc_provider::billing` | 费用计算 |
| **费用** | `utils/extraUsage.ts` | ~100 | `rc_provider::extra_usage` | 额外使用量 |
| **任务** | `utils/cron.ts` | ~200 | `rc_tasks::cron` | Cron 任务 |
| **任务** | `utils/cronScheduler.ts` | ~300 | `rc_tasks::cron_scheduler` | Cron 调度器 |
| **任务** | `utils/cronTasks.ts` | ~200 | `rc_tasks::cron_tasks` | Cron 任务管理 |
| **任务** | `utils/activityManager.ts` | ~200 | `rc_tasks::activity` | 活动管理 |
| **IDE** | `utils/ide.ts` | ~300 | `rc_tui::ide` | IDE 集成 |
| **IDE** | `utils/editor.ts` | ~200 | `rc_tui::ide::editor` | 编辑器集成 |
| **IDE** | `utils/jetbrains.ts` | ~200 | `rc_tui::ide::jetbrains` | JetBrains |
| **格式化** | `utils/format.ts` | ~200 | `rc_tui::format` | 格式化工具 |
| **格式化** | `utils/formatBriefTimestamp.ts` | ~50 | `rc_tui::format::timestamp` | 时间戳格式 |
| **格式化** | `utils/treeify.ts` | ~100 | `rc_tui::format::tree` | 树形格式 |
| **格式化** | `utils/words.ts` | ~100 | `rc_tui::format::words` | 单词处理 |
| **Worktree** | `utils/worktree.ts` | ~200 | `rc_tools::worktree` | Worktree 操作 |
| **Worktree** | `utils/getWorktreePaths.ts` | ~100 | `rc_tools::worktree::paths` | Worktree 路径 |
| **会话** | `utils/conversationRecovery.ts` | ~200 | `rc_session::recovery` | 会话恢复 |
| **会话** | `utils/crossProjectResume.ts` | ~150 | `rc_session::cross_project` | 跨项目恢复 |
| **会话** | `utils/concurrentSessions.ts` | ~100 | `rc_session::concurrent` | 并发会话 |
| **记忆** | `utils/claudemd.ts` | ~200 | `rc_memory::claudemd` | CLAUDE.md 处理 |
| **加密** | `utils/crypto.ts` | ~100 | `rc_core::crypto` | 加密工具 |
| **加密** | `utils/hash.ts` | ~50 | `rc_core::crypto::hash` | 哈希工具 |
| **加密** | `utils/fingerprint.ts` | ~100 | `rc_core::crypto::fingerprint` | 指纹 |
| **网络** | `utils/http.ts` | ~200 | `rc_provider::http` | HTTP 工具 |
| **网络** | `utils/api.ts` | ~300 | `rc_provider::api` | API 工具 |
| **网络** | `utils/browser.ts` | ~200 | `rc_tools::web::browser` | 浏览器 |
| **网络** | `utils/apiPreconnect.ts` | ~100 | `rc_provider::preconnect` | API 预连接 |
| **性能** | `utils/startupProfiler.ts` | ~200 | `rc_telemetry::startup` | 启动性能 |
| **性能** | `utils/slowOperations.ts` | ~200 | `rc_telemetry::slow` | 慢操作追踪 |
| **性能** | `utils/fpsTracker.ts` | ~100 | `rc_telemetry::fps` | FPS 追踪 |
| **进程** | `utils/execFileNoThrow.ts` | ~100 | `rc_tools::shell::exec` | 文件执行 |
| **进程** | `utils/genericProcessUtils.ts` | ~100 | `rc_tools::shell::process` | 进程工具 |
| **进程** | `utils/gracefulShutdown.ts` | ~200 | `rc_core::shutdown` | 优雅关闭 |
| **进程** | `utils/cleanup.ts` | ~200 | `rc_core::cleanup` | 清理 |
| **进程** | `utils/cleanupRegistry.ts` | ~100 | `rc_core::cleanup::registry` | 清理注册 |

### 17.3 平台/环境工具

| 分类 | Claude Code 文件 | Rust 目标 | 说明 |
|------|-----------------|-----------|------|
| `utils/cwd.ts` | `rc_core::cwd` | 工作目录 |
| `utils/env.ts` | `rc_config::env` | 环境变量 |
| `utils/envDynamic.ts` | `rc_config::env_dynamic` | 动态环境变量 |
| `utils/envValidation.ts` | `rc_config::env_validation` | 环境验证 |
| `utils/which.ts` | `rc_tools::shell::which` | 可执行文件查找 |
| `utils/findExecutable.ts` | `rc_tools::shell::find` | 可执行文件搜索 |
| `utils/binaryCheck.ts` | `rc_tools::shell::binary` | 二进制检查 |
| `utils/xdg.ts` | `rc_config::xdg` | XDG 路径 |
| `utils/cachePaths.ts` | `rc_config::cache` | 缓存路径 |
| `utils/caCerts.ts` | `rc_provider::tls` | CA 证书 |
| `utils/caCertsConfig.ts` | `rc_provider::tls_config` | TLS 配置 |
| `utils/appleTerminalBackup.ts` | `rc_tui::terminal::macos` | macOS 终端 |
| `utils/iTermBackup.ts` | `rc_tui::terminal::iterm` | iTerm2 |
| `utils/intl.ts` | `rc_core::intl` | 国际化 |
| `utils/platform.ts` | `rc_core::platform` | 平台检测 |

---

## 18. Phase 详细文件级实现规格

### Phase 1: 核心类型 + 事件系统 + 状态管理（3-4 周）

#### 1.1 新建文件清单

```
crates/rc-engine-events/
├── Cargo.toml
├── src/
│   ├── lib.rs              # pub mod types, stream
│   ├── types.rs            # EngineEvent 枚举 (§16.7)
│   └── stream.rs           # EventStream (tokio broadcast channel)

crates/rc-transcript/
├── Cargo.toml
├── src/
│   ├── lib.rs              # pub mod entry, boundary, storage
│   ├── entry.rs            # TranscriptEntry (序列化/反序列化)
│   ├── boundary.rs         # CompactBoundary (压缩边界标记)
│   └── storage.rs          # 文件持久化 (JSONL 格式)

crates/rc-core/src/          # 增强
├── ids.rs                   # 🆕 SessionId, AgentId (§16.1)
├── message.rs               # 🆕 Message 类型系统 (§16.2)
├── state.rs                 # 🆕 AppState (§7.1)
├── hooks.rs                 # 🆕 HookEvent 26 种 (§16.8)
├── permission_types.rs      # 🆕 PermissionBehavior/Rule (§16.6)
├── usage.rs                 # 🆕 UsageAccumulator
├── cost.rs                  # 🆕 CostTracker
└── lib.rs                   # 重新导出
```

#### 1.2 关键实现细节

```rust
// rc-engine-events/src/stream.rs
pub struct EventStream {
    sender: broadcast::Sender<EngineEvent>,
}

impl EventStream {
    pub fn new(buffer: usize) -> Self { ... }
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> { ... }
    pub fn emit(&self, event: EngineEvent) { ... }
}

// rc-transcript/src/storage.rs
pub struct TranscriptStorage {
    path: PathBuf,
}

impl TranscriptStorage {
    pub async fn append(&self, entry: &TranscriptEntry) -> Result<()> { ... }
    pub async fn read_all(&self) -> Result<Vec<TranscriptEntry>> { ... }
    pub async fn read_range(&self, start: usize, end: usize) -> Result<Vec<TranscriptEntry>> { ... }
    pub async fn truncate_after(&self, index: usize) -> Result<()> { ... }
}
```

#### 1.3 Phase 1 验证标准

- [ ] `rc-engine-events` 编译通过，所有 EngineEvent 变体可序列化
- [ ] `rc-transcript` 可正确持久化和恢复 TranscriptEntry
- [ ] `rc-core` 新增类型与现有类型兼容
- [ ] 所有新增类型实现 `Serialize + Deserialize + Clone + Debug`
- [ ] 单元测试覆盖率 > 80%

---

### Phase 2: Query Engine V2（4-5 周）

#### 2.1 新建文件清单

```
crates/rc-query-engine/
├── Cargo.toml
├── src/
│   ├── lib.rs              # pub mod engine, query_loop, config, budget
│   ├── engine.rs           # QueryEngine struct (§16.4)
│   ├── query_loop.rs       # 核心查询循环 (对应 query.ts 1,730 行)
│   ├── config.rs           # QueryConfig (对应 query/config.ts)
│   ├── token_budget.rs     # BudgetTracker (对应 query/tokenBudget.ts)
│   ├── transitions.rs      # 状态转换逻辑
│   ├── stop_hooks.rs       # 停止 Hook (对应 query/stopHooks.ts)
│   ├── streaming_tool.rs   # 流式工具执行器
│   ├── model_switch.rs     # 模型切换与回退
│   ├── structured_output.rs # 结构化输出
│   └── fallback.rs         # 非流式回退逻辑
```

#### 2.2 核心查询循环伪代码

```rust
// rc-query-engine/src/query_loop.rs
pub async fn query_loop(
    engine: &mut QueryEngine,
    user_input: Vec<Message>,
    context: ProcessUserInputContext,
) -> Result<QueryResult, EngineError> {
    let mut state = engine.state.clone();

    // 1. 处理用户输入
    for msg in &user_input {
        state.messages.push(msg.clone());
        engine.emit(EngineEvent::Message(msg.clone())).await;
    }

    loop {
        // 2. 检查 token budget
        let budget_decision = check_token_budget(
            &mut state.budget_tracker,
            context.agent_id.as_ref(),
            context.task_budget,
            state.usage.total_tokens,
        );
        if let TokenBudgetDecision::Stop { .. } = budget_decision {
            break;
        }

        // 3. 检查上下文窗口 → 触发压缩
        if should_compact(&state) {
            let compact_result = engine.config.compact_engine
                .compact(state.messages.clone(), &state).await?;
            apply_compaction(&mut state, compact_result);
        }

        // 4. 构建 API 请求
        let system_prompt = engine.config.system_prompt_builder.build(&state);
        let tools = filter_tools(&engine.config.tools, &context);
        let messages = prepare_messages(&state.messages);

        // 5. 流式调用 LLM
        let mut stream = engine.config.provider.stream_messages(
            system_prompt, messages, tools, &context,
        ).await?;

        // 6. 处理流式响应
        let mut tool_use_blocks = vec![];
        while let Some(part) = stream.next().await {
            match part? {
                StreamPart::ContentBlockStart(block) => { ... }
                StreamPart::ContentBlockDelta(delta) => { ... }
                StreamPart::ContentBlockStop => { ... }
                StreamPart::MessageDelta(delta) => { ... }
            }
        }

        // 7. 如果没有工具调用，结束循环
        if tool_use_blocks.is_empty() {
            break;
        }

        // 8. 执行工具（流式并行）
        let tool_results = execute_tools_streaming(
            &tool_use_blocks, &engine.config.tools, &context,
        ).await?;

        // 9. 将工具结果添加到消息
        for result in tool_results {
            state.messages.push(result.into());
        }

        state.turn += 1;
        if state.turn >= engine.config.max_turns {
            break;
        }
    }

    Ok(QueryResult { state, messages: state.messages })
}
```

#### 2.3 Phase 2 验证标准

- [ ] QueryEngine 可完成基本的 user → assistant → tool → result 循环
- [ ] 流式响应正确解析所有 ContentBlockType
- [ ] Token budget 正确触发 continue/stop
- [ ] 自动压缩在上下文窗口满时触发
- [ ] 非流式回退在超时时正确工作
- [ ] 集成测试：使用 mock provider 完成完整查询循环

---

### Phase 3: 工具运行时 V2 + 所有 Prompt（5-6 周）

#### 3.1 文件清单

```
crates/rc-tools/src/
├── tool_trait.rs            # 🆕 Tool trait (§16.3)
├── tool_registry.rs         # 🆕 工具注册表
├── tool_executor.rs         # 🆕 工具执行器（流式并行）
├── tool_search.rs           # 🆕 工具搜索
├── specs.rs                 # (增强) 50+ 工具 schema
├── builtin/
│   ├── mod.rs
│   ├── bash.rs              # BashTool
│   ├── read.rs              # FileReadTool
│   ├── write.rs             # FileWriteTool
│   ├── edit.rs              # FileEditTool
│   ├── glob.rs              # GlobTool
│   ├── grep.rs              # GrepTool
│   ├── agent.rs             # AgentTool
│   ├── todo_write.rs        # TodoWriteTool
│   ├── ask_user.rs          # AskUserQuestionTool
│   ├── skill.rs             # SkillTool
│   ├── web_search.rs        # WebSearchTool
│   ├── web_fetch.rs         # WebFetchTool
│   ├── mcp_tool.rs          # MCPTool
│   ├── task_create.rs       # TaskCreateTool
│   ├── task_get.rs          # TaskGetTool
│   ├── task_list.rs         # TaskListTool
│   ├── task_update.rs       # TaskUpdateTool
│   ├── task_output.rs       # TaskOutputTool
│   ├── task_stop.rs         # TaskStopTool
│   ├── send_message.rs      # SendMessageTool
│   ├── plan_mode.rs         # EnterPlanMode/ExitPlanMode
│   ├── tool_search.rs       # ToolSearchTool
│   ├── synthetic_output.rs  # SyntheticOutputTool
│   ├── sleep.rs             # SleepTool
│   ├── notebook_edit.rs     # NotebookEditTool
│   ├── worktree.rs          # EnterWorktree/ExitWorktree
│   ├── powershell.rs        # PowerShellTool
│   ├── mcp_resources.rs     # ListMcpResources/ReadMcpResource
│   ├── mcp_auth.rs          # McpAuthTool
│   ├── web_browser.rs       # WebBrowserTool
│   ├── snip.rs              # SnipTool
│   ├── monitor.rs           # MonitorTool
│   ├── review_artifact.rs   # ReviewArtifactTool
│   ├── verify_plan.rs       # VerifyPlanExecutionTool
│   ├── schedule_cron.rs     # ScheduleCronTool
│   ├── workflow.rs          # WorkflowTool
│   ├── team.rs              # TeamCreate/TeamDelete
│   ├── terminal_capture.rs  # TerminalCaptureTool
│   ├── repl.rs              # REPLTool
│   ├── brief.rs             # BriefTool
│   ├── discover_skills.rs   # DiscoverSkillsTool
│   ├── config.rs            # ConfigTool
│   └── lsp.rs               # LSPTool

crates/rc-tool-prompts/src/
├── lib.rs                   # prompt 注册表
├── bash.rs                  # BashTool prompt (370 行 TS → Rust)
├── write.rs                 # FileWriteTool prompt
├── edit.rs                  # FileEditTool prompt
├── read.rs                  # FileReadTool prompt
├── glob.rs                  # GlobTool prompt
├── grep.rs                  # GrepTool prompt
├── agent.rs                 # AgentTool prompt (288 行 TS → Rust)
├── todo_write.rs            # TodoWriteTool prompt
├── ask_user.rs              # AskUserQuestionTool prompt
├── skill.rs                 # SkillTool prompt
├── web_search.rs            # WebSearchTool prompt
├── web_fetch.rs             # WebFetchTool prompt
├── mcp.rs                   # MCPTool prompt
├── task_create.rs           # TaskCreateTool prompt
├── ...                      # (每个工具一个文件，共 50+)
└── sandbox.rs               # 沙箱部分 prompt
```

#### 3.2 Phase 3 验证标准

- [ ] 50+ 工具全部实现 Tool trait
- [ ] 每个工具的 prompt 在结构、关键文案、行为约束和安全边界上与 Claude Code 对齐，差异有审计记录
- [ ] 工具执行器支持流式并行执行
- [ ] 工具搜索（deferred tool discovery）正常工作
- [ ] 所有工具 schema 与 Claude Code 一致
- [ ] 集成测试：每个工具至少一个 happy path 测试

---

### Phase 4: System Prompt + Compaction + Context（4-5 周）

#### 4.1 文件清单

```
crates/rc-system-prompt/src/
├── lib.rs                   # SystemPromptBuilder (§3.2)
├── sections/
│   ├── mod.rs
│   ├── intro.rs             # Intro Section
│   ├── system.rs            # System Section (6 条规则)
│   ├── doing_tasks.rs       # Doing Tasks Section (12+ 指南)
│   ├── actions.rs           # Actions Section
│   ├── using_tools.rs       # Using Your Tools Section
│   ├── output_efficiency.rs # Output Efficiency Section
│   ├── tone_style.rs        # Tone and Style Section
│   ├── session_guidance.rs  # Session Guidance Section
│   ├── memory.rs            # Memory Section
│   ├── env_info.rs          # Env Info Section
│   ├── mcp_instructions.rs  # MCP Instructions Section
│   ├── language.rs          # Language Section
│   ├── output_style.rs      # Output Style Section
│   └── scratchpad.rs        # Scratchpad Section
└── cache.rs                 # Cache control 断点管理

crates/rc-compact/src/
├── lib.rs                   # CompactEngine
├── strategy.rs              # CompactStrategy trait (§16.5)
├── auto.rs                  # AutoCompact (LLM 摘要)
├── micro.rs                 # MicroCompact (缓存编辑)
├── snip.rs                  # SnipCompact (策略裁剪)
├── reactive.rs              # ReactiveCompact (响应式)
├── context_collapse.rs      # ContextCollapse (折叠)
├── boundary.rs              # Compact boundary 管理
├── grouping.rs              # 消息分组
├── prompt.rs                # 压缩提示词
├── warning.rs               # 压缩警告
├── cleanup.rs               # 压缩后清理
├── config.rs                # 压缩配置
└── attachments.rs           # 压缩后附件处理

crates/rc-context/src/
├── lib.rs                   # 上下文管理入口
├── window.rs                # 上下文窗口 (对应 utils/context.ts)
├── analysis.rs              # 上下文分析
├── suggestions.rs           # 上下文建议
├── token_budget.rs          # Token 预算管理
└── policy_limits.rs         # 策略限制
```

#### 4.2 Phase 4 验证标准

- [ ] System prompt 的静态段落、cache boundary、动态拼接顺序与 Claude Code 对齐，关键文案逐段比对并记录差异
- [ ] 5 种压缩策略全部实现并可独立测试
- [ ] Auto compact 使用 LLM 生成摘要
- [ ] Micro compact 正确处理缓存编辑
- [ ] 上下文窗口管理正确处理 200k/1M 上下文
- [ ] 压缩后附件正确恢复

---

### Phase 5-8 验证标准（概要）

**Phase 5 (TUI)**:
- [ ] ratatui TUI 渲染与 Claude Code 视觉一致
- [ ] 100+ 组件全部实现
- [ ] 快捷键系统完整工作
- [ ] Vim 模式完整工作
- [ ] 虚拟滚动支持 10,000+ 消息

**Phase 6 (MCP + Agent + Hook)**:
- [ ] MCP 动态连接管理
- [ ] OAuth 认证流程
- [ ] Elicitation 处理
- [ ] Fork Agent 正确创建和管理
- [ ] 26 种 Hook 事件全部触发
- [ ] Hook 响应正确处理

**Phase 7 (CLI + Skills + Memory + Commands)**:
- [ ] 80+ 斜杠命令全部注册
- [ ] Bundled skills 全部可执行
- [ ] MEMORY.md 自动加载和保存
- [ ] CLI 参数与 Claude Code 一致

**Phase 8 (集成测试)**:
- [ ] Golden transcript 测试（与 Claude Code 输出对比）
- [ ] 故障注入测试（网络断开、API 错误、超时）
- [ ] 压力测试（10,000+ 消息长对话）
- [ ] 并发测试（多 Agent 同时运行）
- [ ] 恢复测试（崩溃后恢复会话）

---

## 19. 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| ratatui 性能不足（大量消息渲染） | 中 | 高 | 虚拟滚动 + 增量渲染 + 基准测试 |
| 现有流式实现与新 engine 接缝复杂 | 中 | 高 | 复用现有 streaming callback / fallback 资产，先做 shadow parser / adapter，再切主链路 |
| 5 种压缩策略实现难度 | 高 | 中 | 按优先级实现：auto > snip > micro > reactive > collapse |
| Tool trait 设计不够灵活 | 低 | 高 | 参考 Claude Code Tool.ts 的完整接口 |
| 多 provider 兼容性回退 | 中 | 中 | 保留现有 provider 抽象层 |
| Hook 系统与 TUI 事件冲突 | 中 | 中 | 统一事件总线，Hook 作为中间件 |
| 会话恢复格式不兼容 | 低 | 高 | `rc-migrate` 自动转换工具 |
| 长期并行分支导致主线漂移 | 高 | 高 | `main-only` + feature flags + compat shims + milestone tags |
| 把 parity 误做成机械搬运，导致架构扭曲 | 中 | 高 | 以行为 / 边界 / 契约审计为准，静态关键文案逐段比对，动态实现做等价映射 |
| 只做到 SDK 兼容而未复刻官方启动/协议语义，触发第三方平台风控或行为偏差 | 高 | 高 | 以官方 CLI 代理实测 + `.research/claude-code-rev` 双证据维护 parity ledger，优先对齐启动链路、动态 headers/betas、streaming usage/stop_reason 最终化与 fallback 策略 |

---

## 20. 最终检查清单

### 完成标准（所有项必须通过）

- [ ] **功能完整性**: 所有 Claude Code 功能全部实现，§13 的后置模块也已在最终产品阶段补齐
- [ ] **Prompt 一致性**: 所有工具 prompt 和 system prompt 的结构、关键文案、cache boundary、行为约束与 Claude Code 对齐，差异均可审计
- [ ] **流式体验**: 流式响应、流式工具执行、流式进度全部正常
- [ ] **压缩能力**: 5 种压缩策略全部工作，长对话不崩溃
- [ ] **TUI 体验**: ratatui TUI 与 Claude Code 视觉和交互一致
- [ ] **MCP 集成**: MCP 动态连接、OAuth、Elicitation 全部工作
- [ ] **Agent 系统**: Fork Agent、Built-in Agent 全部工作
- [ ] **权限系统**: 分类器、自动模式、拒绝追踪全部工作
- [ ] **Hook 系统**: 26 种 Hook 事件全部触发并正确处理
- [ ] **斜杠命令**: 80+ 命令全部注册并执行
- [ ] **Skills 系统**: Bundled skills 全部可执行
- [ ] **记忆系统**: MEMORY.md 自动加载/保存
- [ ] **多 Provider**: Anthropic/OpenAI/Bedrock/Vertex 全部工作
- [ ] **会话恢复**: 崩溃后可恢复会话
- [ ] **性能**: 10,000 消息长对话不卡顿
- [ ] **测试覆盖**: 单元测试 > 80%，集成测试覆盖所有关键路径
