# Claude Code vs remote-code-rust 深度差距分析

**日期**: 2026-04-14  
**结论**: **必须完全重构**。当前 Rust 代码结构与 Claude Code 存在根本性架构差距，无法通过增量修补达到同等能力。

---

## 1. 核心结论

| 维度 | 当前 Rust | Claude Code | 差距等级 |
|------|----------|-------------|---------|
| 查询引擎 | 单函数 for 循环 (1,288 行) | 状态机 + AsyncGenerator (3,026 行) | 🔴 **根本性** |
| API 客户端 | 非流式 complete() (1,657 行) | 流式 streaming + cache + betas (3,420 行) | 🔴 **根本性** |
| 上下文压缩 | 简单截断 (1 个策略) | 5 种策略 (auto/micro/snip/reactive/collapse) | 🔴 **根本性** |
| 工具执行 | 顺序执行 | 流式并行执行 + 进度流 | 🔴 **根本性** |
| System Prompt | 硬编码字符串 | 动态构建 + 缓存断点 (915 行) | 🟡 **重大** |
| 权限系统 | 简单 PermissionBroker | 分类器 + 自动模式 + 拒绝追踪 | 🟡 **重大** |
| TUI | 基础 headless 模式 | 完整 ratatui TUI (407 组件) | 🔴 **根本性** |
| Agent 系统 | 简单 SubAgent | Fork + Subagent + Built-in agents | 🟡 **重大** |
| MCP | 基础 stdio 连接 | 动态连接 + OAuth + Elicitation (25 文件) | 🟡 **重大** |
| 斜杠命令 | 12 个命令 | 80+ 命令 | 🟡 **重大** |

---

## 2. 逐模块差距分析

### 2.1 查询引擎：conversation.rs vs QueryEngine.ts + query.ts

**当前 Rust** (`apps/remote-code/src/conversation.rs`, 1,288 行):

```
run_prompt() {
    for turn_index in 0..max_turns {
        // 1. 检查上下文溢出 → 简单截断
        // 2. 调用 provider.complete() 或 complete_streaming()
        // 3. 如果有 tool_calls → 顺序执行
        // 4. 如果没有 tool_calls → 返回
    }
}
```

**Claude Code** (`QueryEngine.ts` 1,296 行 + `query.ts` 1,730 行 = 3,026 行):

```
QueryEngine.submitMessage() → AsyncGenerator<SDKMessage> {
    // 1. 构建 ProcessUserInputContext (30+ 字段)
    // 2. 动态构建 System Prompt (缓存断点)
    // 3. 注入 Memory Mechanics Prompt
    // 4. 注册 Structured Output Enforcement
    // 5. 处理 Orphaned Permission
    // 6. 管理 File History State
    // 7. 管理 Attribution State
    // 8. yield SDKMessage 事件流
}

query() {
    // 1. Snip Compact (feature gated)
    // 2. Micro-Compact (with cache editing)
    // 3. Context Collapse (feature gated)
    // 4. Auto-Compact (with summary generation via LLM)
    // 5. Token Blocking Limit Check
    // 6. Streaming API Call with fallback
    // 7. Streaming Tool Execution (parallel)
    // 8. Tool Use Block Tracking
    // 9. Permission Mode Integration
    // 10. Task Budget Tracking
    // 11. Query Chain Tracking
    // 12. Thinking Config Management
    // 13. Model Switching with Fallback
    // 14. Consecutive Failure Tracking
    // 15. Streaming Fallback with Tombstones
    // 16. Backfill Observable Input
    // 17. Stop Hook Retry
    // 18. Tool Result Summary
}
```

**缺失的关键能力**:

| # | 能力 | Claude Code | Rust 现状 |
|---|------|-----------|----------|
| 1 | 状态机 | 11 种 EngineState | 无状态机 |
| 2 | Snip Compact | snipCompact.ts | ❌ 无 |
| 3 | Micro Compact | microCompact.ts + cache editing | ❌ 无 |
| 4 | Context Collapse | contextCollapse/ | ❌ 无 |
| 5 | Auto Compact | autoCompact.ts (LLM 生成摘要) | ❌ 只有简单截断 |
| 6 | Reactive Compact | reactiveCompact.ts | ❌ 无 |
| 7 | Streaming Tool Execution | StreamingToolExecutor | ❌ 顺序执行 |
| 8 | Tool Progress Streaming | ToolProgressData 流 | ❌ 无 |
| 9 | Query Chain Tracking | chainId + depth | ❌ 无 |
| 10 | Thinking Config | enabled/disabled/adaptive | ❌ 无 |
| 11 | Model Switching | runtime model + fallback | ❌ 无 |
| 12 | Task Budget | token budget per task | ❌ 无 |
| 13 | Structured Output | JSON schema enforcement | ❌ 无 |
| 14 | Skill Discovery | discoveredSkillNames | ❌ 无 |
| 15 | Memory Mechanics | MEMORY.md auto-load | ❌ 无 |
| 16 | Streaming Fallback | non-streaming fallback | ❌ 无 |
| 17 | Tombstone Handling | orphaned message cleanup | ❌ 无 |
| 18 | Backfill Observable Input | tool input backfill | ❌ 无 |
| 19 | Stop Hook Retry | stopHooks.ts | ❌ 无 |
| 20 | Tool Result Summary | LLM-generated summary | ❌ 无 |
| 21 | Consecutive Failure Tracking | circuit breaker | ❌ 无 |
| 22 | Permission Denial Tracking | SDK reporting | 部分 |
| 23 | File History State | snapshot management | ❌ 无 |
| 24 | Attribution State | commit attribution | ❌ 无 |
| 25 | Advisor Model | server-side advisor | ❌ 无 |
| 26 | Tool Search | deferred tool discovery | ❌ 无 |
| 27 | Agent Definitions | activeAgents + allAgents | ❌ 无 |
| 28 | MCP Tool Integration | mcpTools in API call | ❌ 无 |
| 29 | Effort Value | low/medium/high | ❌ 无 |
| 30 | Fast Mode | fast mode toggle | ❌ 无 |

### 2.2 API 客户端：rc-provider vs services/api/claude.ts

**当前 Rust** (`crates/rc-provider/src/lib.rs`, 1,657 行):

- `complete()` → 非流式请求
- `complete_streaming_with_callbacks()` → 流式请求（简单回调）
- `complete_openai()` → OpenAI 格式
- `complete_anthropic()` → Anthropic 格式（非流式）
- `complete_bedrock()` → Bedrock SigV4
- `complete_vertex()` → Vertex AI

**Claude Code** (`services/api/claude.ts`, 3,420 行):

- `streamMessages()` → 完整流式 API（1,700+ 行核心逻辑）
- `queryModelWithoutStreaming()` → 非流式回退
- `queryHaiku()` → 快速查询
- `queryWithModel()` → 指定模型查询
- `addCacheBreakpoints()` → 缓存断点管理
- `buildSystemPromptBlocks()` → System prompt 块构建
- `stripExcessMediaItems()` → 媒体裁剪
- `accumulateUsage()` → 使用量累积
- `cleanupStream()` → 流清理
- `updateUsage()` → 使用量更新
- `getMaxOutputTokensForModel()` → 模型最大输出
- `configureEffortParams()` → effort 参数配置
- `configureTaskBudgetParams()` → task budget 参数
- `verifyApiKey()` → API Key 验证
- `getAPIMetadata()` → API 元数据
- `getPromptCachingEnabled()` → 缓存启用检测
- `getCacheControl()` → 缓存控制
- `should1hCacheTTL()` → 1h TTL 判断
- `adjustParamsForNonStreaming()` → 非流式参数调整
- `withRetry()` → 重试逻辑

**缺失的关键能力**:

| # | 能力 | Claude Code | Rust 现状 |
|---|------|-----------|----------|
| 1 | Prompt Caching | cache_control 断点 | ❌ 无 |
| 2 | Streaming with SSE | 完整 SSE 解析 | 部分（简单回调） |
| 3 | Beta Headers | interleaved-thinking, output-128k 等 | ❌ 无 |
| 4 | Thinking Blocks | thinking + signature delta | ❌ 无 |
| 5 | Server Tool Use | server_tool_use blocks | ❌ 无 |
| 6 | Tool Search Integration | deferred tool schema | ❌ 无 |
| 7 | Advisor Integration | advisor model + beta | ❌ 无 |
| 8 | Effort Parameter | low/medium/high effort | ❌ 无 |
| 9 | Task Budget | token budget tracking | ❌ 无 |
| 10 | Streaming Fallback | non-streaming on timeout | ❌ 无 |
| 11 | Media Stripping | excess media removal | ❌ 无 |
| 12 | Usage Accumulation | cache_read/creation tokens | 部分 |
| 13 | Model-specific Max Tokens | per-model output limits | ❌ 无 |
| 14 | Previous Request ID | request chain tracking | ❌ 无 |
| 15 | MCP Tools in API | mcpTools option | ❌ 无 |
| 16 | Agent Types | allowedAgentTypes | ❌ 无 |
| 17 | Non-interactive Session | isNonInteractiveSession | ❌ 无 |
| 18 | Fast Mode | fastMode option | ❌ 无 |
| 19 | Query Source | compact/session_memory/agent | ❌ 无 |
| 20 | Content Block Types | tool_use/text/thinking/signature | 部分 |

### 2.3 工具运行时：rc-tools vs tools/*

**当前 Rust** (`crates/rc-tools/src/`, ~3,000 行):

- `specs.rs` (1,041 行) → 工具 JSON Schema 定义
- `shell/mod.rs` (500 行) → Shell 执行
- `file_ops.rs` → 文件操作
- `search.rs` → 搜索
- `agent.rs` → Agent
- `delegate.rs` → 委托
- `command.rs` → 命令
- `git.rs` → Git 操作
- `mcp_tools.rs` → MCP 工具
- `misc.rs` → 杂项

**Claude Code** (`src/tools/`, 199 文件, 47,282 行):

50+ 独立工具目录，每个包含:
- `prompt.ts` → 动态 prompt 生成
- `ToolName.ts` → 工具实现
- `constants.ts` → 工具常量

**关键差距**:

| # | 差距 | 说明 |
|---|------|------|
| 1 | 工具数量 | Rust ~15 工具 vs Claude Code 50+ 工具 |
| 2 | 动态 Prompt | Rust 无 vs Claude Code 每个工具有独立 prompt.ts |
| 3 | 工具分类 | Rust 无 vs Claude Code 有 ToolCategory |
| 4 | 进度报告 | Rust 无 vs Claude Code 有 ToolProgressData |
| 5 | 文件历史快照 | Rust 无 vs Claude Code needs_file_snapshot |
| 6 | 并发安全 | Rust 无 vs Claude Code is_concurrency_safe |
| 7 | 只读标记 | Rust 无 vs Claude Code is_read_only |
| 8 | 工具搜索 | Rust 无 vs Claude Code ToolSearch |
| 9 | 流式工具执行 | Rust 无 vs Claude Code StreamingToolExecutor |
| 10 | 工具验证 | Rust 无 vs Claude Code validateInput |

### 2.4 核心类型：rc-core vs types/*

**当前 Rust** (`crates/rc-core/src/lib.rs`, 591 行):

- `PermissionMode` (6 变体)
- `ProviderProtocol` (4 变体)
- `ConversationRole` (5 变体)
- `ToolCall` (简单结构体)
- `ConversationEntry` (简单结构体)
- `ProviderResponse` (简单结构体)
- `ToolResult` (简单结构体)
- `default_system_prompt()` (硬编码)

**Claude Code** (`src/types/` 19 文件 + `src/Tool.ts` 793 行):

- `Message` 类型 (复杂联合类型)
- `SDKMessage` (流式消息)
- `Tool` trait (30+ 方法)
- `ToolUseContext` (20+ 字段)
- `ToolPermissionContext` (复杂结构)
- `ToolProgressData` (进度数据)
- `ToolResult` (多类型结果)
- `CompactProgressEvent` (压缩事件)
- `QueryChainTracking` (查询追踪)
- `ValidationResult` (验证结果)
- `ToolInputJSONSchema` (输入 Schema)
- `SetToolJSXFn` (TUI 设置)

### 2.5 权限系统：rc-permissions vs useCanUseTool.tsx

**当前 Rust** (`crates/rc-permissions/src/`, ~500 行):

- `PermissionBroker` trait → 简单 decide() 方法
- `PermissionMode` → 6 种模式
- `rule_parser.rs` → 规则解析
- `rules.rs` → 规则匹配

**Claude Code** (`hooks/useCanUseTool.tsx`, 204 行 + `services/tools/toolExecution.ts`, 1,746 行):

- `canUseTool()` → 复杂权限检查（ask/deny/allow）
- `streamedCheckPermissionsAndCallTool()` → 流式权限检查
- `checkPermissionsAndCallTool()` → 完整权限管线
- `classifyToolError()` → 错误分类
- `buildSchemaNotSentHint()` → Schema 缺失提示
- `PermissionDecision` → 多种决策类型
- `ToolPermissionContext` → 复杂上下文

---

## 3. 重构方案

### 3.1 核心决策：完全重构

**结论：必须完全重构 `conversation.rs`，不能增量修补。**

原因：
1. `conversation.rs` 是单函数 for 循环，不是状态机
2. 缺少 30+ 关键能力，无法通过修补添加
3. 压缩策略需要完全不同的消息处理管线
4. 流式工具执行需要完全不同的执行模型
5. System prompt 构建需要独立的 crate

### 3.2 重构策略

```
Phase 0: 保留可复用的 crate（不重构）
├── rc-config/        → 保留，增强
├── rc-session/       → 保留，增强
├── rc-permissions/   → 保留，增强
├── rc-event-bus/     → 保留，增强
├── rc-protocol/      → 保留，增强
├── rc-control-plane/ → 保留
├── rc-runner/        → 保留
├── rc-telemetry/     → 保留
└── rc-ui-bridge/     → 保留，增强

Phase 1: 完全重写的 crate
├── rc-core/          → 完全重写类型系统
├── rc-provider/      → 完全重写 API 客户端（流式 + 缓存）
├── rc-tools/         → 完全重写工具运行时（50+ 工具）
├── rc-mcp/           → 完全重写 MCP 客户端
├── rc-hooks/         → 完全重写 Hook 系统
├── rc-skills/        → 完全重写技能系统
├── rc-agents/        → 完全重写 Agent 系统
└── rc-tui/           → 完全重写 TUI

Phase 2: 新建的 crate
├── rc-query-engine/  → 新建查询引擎
├── rc-engine-events/ → 新建事件系统
├── rc-transcript/    → 新建会话记录
├── rc-system-prompt/ → 新建 System Prompt
├── rc-compact/       → 新建上下文压缩
├── rc-tool-prompts/  → 新建工具 Prompt
├── rc-tasks/         → 新建任务系统
├── rc-memory/        → 新建记忆管理
├── rc-context/       → 新建上下文管理
├── rc-commands/      → 新建斜杠命令
├── rc-tui-components/→ 新建 TUI 组件
├── rc-tui-input/     → 新建 TUI 输入
├── rc-analytics/     → 新建分析
├── rc-lsp/           → 新建 LSP
└── rc-output-styles/ → 新建输出风格

Phase 3: 重写的应用入口
└── apps/remote-code/ → 完全重写 CLI 入口
    ├── conversation.rs → 删除，由 rc-query-engine 替代
    ├── headless.rs     → 重写，对接新引擎
    ├── interactive.rs  → 重写，对接新 TUI
    └── main.rs         → 重写，对接新 CLI
```

### 3.3 重构优先级

**P0（必须首先完成）**:
1. `rc-core` 类型系统重写 → 所有其他 crate 依赖
2. `rc-engine-events` 事件系统 → 查询引擎依赖
3. `rc-query-engine` 查询引擎 → 替代 conversation.rs
4. `rc-provider` API 客户端重写 → 流式 + 缓存
5. `rc-system-prompt` System Prompt → 对话质量依赖

**P1（核心能力）**:
6. `rc-compact` 上下文压缩 → 长对话依赖
7. `rc-tool-prompts` 工具 Prompt → 工具质量依赖
8. `rc-tools` 工具运行时重写 → 50+ 工具
9. `rc-permissions` 权限增强 → 安全依赖
10. `rc-transcript` 会话记录 → 恢复依赖

**P2（增强能力）**:
11. `rc-tui` + `rc-tui-components` TUI → 用户体验
12. `rc-mcp` MCP 增强 → 工具生态
13. `rc-agents` Agent 增强 → 复杂任务
14. `rc-commands` 斜杠命令 → 交互体验
15. `rc-hooks` Hook 增强 → 扩展性

### 3.4 重构后的 CLI 能力对比

| 能力 | 重构前 | 重构后 | Claude Code |
|------|--------|--------|-------------|
| 查询引擎 | 简单循环 | 状态机 + AsyncGenerator | ✅ 一致 |
| 流式 API | 简单回调 | 完整 SSE + 缓存 | ✅ 一致 |
| 上下文压缩 | 简单截断 | 5 种策略 | ✅ 一致 |
| 工具数量 | ~15 | 50+ | ✅ 一致 |
| 工具 Prompt | 硬编码 | 动态生成 | ✅ 一致 |
| System Prompt | 硬编码 | 动态构建 + 缓存 | ✅ 一致 |
| TUI | headless | 完整 ratatui | ✅ 一致 |
| 斜杠命令 | 12 | 80+ | ✅ 一致 |
| Agent 系统 | 简单 SubAgent | Fork + Subagent | ✅ 一致 |
| MCP | 基础 | 完整 (25 文件) | ✅ 一致 |
| 权限 | 简单 | 分类器 + 自动模式 | ✅ 一致 |
| 多 Provider | ✅ 已有 | ✅ 保留 | ✅ 超越 |
| 性能 | Rust | Rust | ✅ 超越 |
| 类型安全 | Rust | Rust | ✅ 超越 |

---

## 4. 最终结论

**是的，必须完全重构代码结构。** 当前 Rust CLI 的核心运行时（`conversation.rs`）是一个简单的 for 循环，缺少 Claude Code 30+ 关键能力。这些能力不是"锦上添花"，而是 Claude Code 能力的核心基础：

1. **5 种压缩策略** → 长对话不崩溃的基础
2. **流式工具执行** → 实时反馈的基础
3. **动态 System Prompt** → 对话质量的基础
4. **查询状态机** → 复杂交互的基础
5. **50+ 工具 + 动态 Prompt** → 编码能力的基础

重构后的 CLI 将 **≥ Claude Code**：
- 所有 Claude Code 能力全部复刻
- 保留多 Provider 优势（Claude Code 只有 Anthropic）
- Rust 性能优势
- Rust 类型安全优势
