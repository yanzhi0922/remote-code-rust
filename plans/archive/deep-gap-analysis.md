# 深度 Gap 分析报告

> 基于 `.research/claude-code-rev/src/` (Claude Code TypeScript 参考) 与 `crates/` (remote-code Rust 实现) 的逐代码级对比。
> 生成时间: 2026-04-17

---

## 1. Query Engine (查询引擎)

### 1.1 Claude Code 实现

#### 关键文件
- [`query.ts`](/.research/claude-code-rev/src/query.ts) (1730 行) — 核心查询循环
- [`QueryEngine.ts`](/.research/claude-code-rev/src/QueryEngine.ts) (1296 行) — 查询引擎状态机

#### 核心函数/类/接口

**`query()` 函数** (`query.ts:219-239`)
- 类型: `AsyncGenerator<StreamEvent | RequestStartEvent | Message | TombstoneMessage | ToolUseSummaryMessage, Terminal>`
- 入口: 接收 `QueryParams`，yield 流式事件，返回 `Terminal` 结果

**`queryLoop()` 函数** (`query.ts:241-1729`)
- 核心无限循环: `while (true)` 结构
- 状态管理: `State` 类型 (`query.ts:204-217`) 携带跨迭代可变状态
- 包含 9 个 continue 站点，每个通过 `state = { ... }` 更新状态

**`QueryEngine` 类** (`QueryEngine.ts:184`)
- 拥有完整的会话生命周期: `mutableMessages`, `abortController`, `permissionDenials`, `totalUsage`
- `submitMessage()` (`QueryEngine.ts:209`): AsyncGenerator，处理用户输入 → 斜杠命令 → API 调用 → 工具执行循环
- 集成: `processUserInput()` → `query()` → `recordTranscript()` → SDK 消息标准化

**`QueryParams` 类型** (`query.ts:181-199`)
```typescript
type QueryParams = {
  messages, systemPrompt, userContext, systemContext,
  canUseTool, toolUseContext, fallbackModel, querySource,
  maxOutputTokensOverride, maxTurns, skipCacheWrite,
  taskBudget?: { total: number }, deps?: QueryDeps
}
```

#### 核心逻辑流程

1. **消息预处理管线** (每轮迭代):
   - `applyToolResultBudget()` — 工具结果大小预算 (`query.ts:379`)
   - `snipCompactIfNeeded()` — 历史裁剪 (`query.ts:401-410`)
   - `microCompact()` — 微压缩 (`query.ts:414`)
   - `contextCollapse.applyCollapsesIfNeeded()` — 上下文折叠 (`query.ts:440`)
   - `autocompact()` — 自动压缩 (`query.ts:454`)

2. **API 调用** (`query.ts:659`):
   - `deps.callModel()` — 流式调用 LLM
   - 支持 fallback model 切换 (`query.ts:894-950`)
   - 流式工具执行: `StreamingToolExecutor` (`query.ts:562-568`)

3. **错误恢复**:
   - Prompt-too-long 恢复: context collapse drain → reactive compact (`query.ts:1085-1183`)
   - Max-output-tokens 恢复: 8k→64k 升级 + 多轮续写 (`query.ts:1188-1256`)
   - 模型 fallback: `FallbackTriggeredError` 处理 (`query.ts:894`)

4. **工具执行后处理**:
   - 工具使用摘要生成 (Haiku 异步) (`query.ts:1411-1482`)
   - 附件消息注入 (memory prefetch, skill discovery) (`query.ts:1580-1628`)
   - 队列命令消费 (`query.ts:1566-1643`)
   - Max turns 检查 (`query.ts:1705`)

5. **Stop hooks** (`query.ts:1267-1306`):
   - `handleStopHooks()` — 拦截/阻止继续
   - 支持 blocking errors 重试循环

### 1.2 remote-code 现状

#### 已实现的功能

**`QueryEngine`** (`crates/claude/rc-query-engine/src/engine.rs:112`)
- `EngineState` 携带: `turn`, `messages`, `usage`, `budget_tracker`, `state_machine`, `current_chain`, `failure_tracker`, `model_switcher`, `stop_hook_manager`, `structured_output`, `tool_summarizer`
- `submit_message()` (`engine.rs:164`): 接收用户输入，执行查询循环

**`StateMachine`** (`crates/claude/rc-query-engine/src/state_machine.rs`)
- `EnginePhase` 枚举: `Idle → Initializing → BuildingPrompt → CallingProvider → ProcessingResponse → ExecutingTools → Compacting → Finalizing → Failed/Cancelled`
- 验证转换合法性，记录转换历史

**`run_query_loop()`** (`crates/claude/rc-query-engine/src/query_loop.rs:29`)
- 基本循环: 预算评估 → 压缩 → API 调用 → 工具执行
- 支持流式观察者 (`StreamingCallbacks`)
- 基本压缩集成 (`maybe_compact_conversation`)

#### 缺失的功能（逐条列出）

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| Q1 | **消息预处理管线缺失**: 无 `applyToolResultBudget()`, 无 snip compact, 无 microcompact, 无 context collapse | `query.ts:379-447` | P0 |
| Q2 | **Reactive compact 缺失**: 无 prompt-too-long 恢复路径，无 reactive compact 触发 | `query.ts:1085-1183` | P0 |
| Q3 | **Max-output-tokens 恢复缺失**: 无 8k→64k 升级，无多轮续写机制 | `query.ts:1188-1256` | P0 |
| Q4 | **模型 fallback 缺失**: 无 `FallbackTriggeredError` 处理，无运行时模型切换 | `query.ts:894-950` | P0 |
| Q5 | **StreamingToolExecutor 集成缺失**: query_loop 中工具串行执行，无流式并行工具执行 | `query.ts:562-568, 838-862` | P0 |
| Q6 | **工具使用摘要缺失**: 无 Haiku 异步摘要生成 | `query.ts:1411-1482` | P1 |
| Q7 | **附件消息注入缺失**: 无 memory prefetch, 无 skill discovery attachment | `query.ts:1580-1628` | P1 |
| Q8 | **队列命令消费缺失**: 无 `getCommandsByMaxPriority()`, 无 slash command 过滤 | `query.ts:1566-1643` | P1 |
| Q9 | **Stop hooks 阻塞重试缺失**: 无 `handleStopHooks()` 完整实现 | `query.ts:1267-1306` | P1 |
| Q10 | **Token budget continuation 缺失**: 无 `checkTokenBudget()` + nudge message | `query.ts:1308-1355` | P1 |
| Q11 | **Task budget remaining 跟踪缺失**: 无跨压缩边界的 task_budget.remaining 计算 | `query.ts:508-515, 1138-1146` | P1 |
| Q12 | **Tombstone 消息缺失**: 无流式 fallback 时的孤立消息标记 | `query.ts:713-723` | P2 |
| Q13 | **Post-sampling hooks 缺失**: 无 `executePostSamplingHooks()` | `query.ts:1000-1009` | P2 |
| Q14 | **Dump prompts 缺失**: 无 `createDumpPromptsFetch()` 调试支持 | `query.ts:588-590` | P2 |
| Q15 | **Query chain tracking 不完整**: 有 `ChainManager` 但缺少 depth 增量逻辑 | `query.ts:347-363` | P2 |

### 1.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-query-engine/src/preprocessing.rs` — 消息预处理管线 (snip, microcompact, tool result budget)
- `crates/claude/rc-query-engine/src/reactive_compact.rs` — reactive compact 恢复逻辑
- `crates/claude/rc-query-engine/src/max_tokens_recovery.rs` — max-output-tokens 恢复
- `crates/claude/rc-query-engine/src/attachment_injector.rs` — 附件消息注入 (memory, skills, queued commands)
- `crates/claude/rc-query-engine/src/tool_summary.rs` — 已存在但需要集成到 query_loop

**需要修改的文件/函数:**
- `crates/claude/rc-query-engine/src/query_loop.rs:run_query_loop()` — 添加预处理管线、错误恢复、流式工具执行
- `crates/claude/rc-query-engine/src/engine.rs:EngineState` — 添加 `max_output_tokens_recovery_count`, `has_attempted_reactive_compact`, `pending_tool_use_summary` 字段
- `crates/claude/rc-query-engine/src/config.rs:QueryEngineConfig` — 添加 `fallback_model`, `task_budget`, streaming executor 配置

**预估工作量:** 3-4 周 (P0 项目)

---

## 2. Tool System (工具系统)

### 2.1 Claude Code 实现

#### 关键文件
- [`Tool.ts`](/.research/claude-code-rev/src/Tool.ts) (793 行) — Tool trait 定义
- [`toolExecution.ts`](/.research/claude-code-rev/src/services/tools/toolExecution.ts) (1746 行) — 工具执行管线
- [`StreamingToolExecutor.ts`](/.research/claude-code-rev/src/services/tools/StreamingToolExecutor.ts) — 流式工具执行器

#### 核心类型

**`Tool<Input, Output, P>` 类型** (`Tool.ts:362-695`)
```typescript
type Tool = {
  name: string
  aliases?: string[]
  searchHint?: string
  call(args, context, canUseTool, parentMessage, onProgress?): Promise<ToolResult<Output>>
  description(input, options): Promise<string>
  inputSchema: Input  // Zod schema
  inputJSONSchema?: ToolInputJSONSchema
  outputSchema?: z.ZodType
  isEnabled(): boolean
  isReadOnly(input): boolean
  isDestructive?(input): boolean
  isConcurrencySafe(input): boolean
  interruptBehavior?(): 'cancel' | 'block'
  isSearchOrReadCommand?(input): { isSearch, isRead, isList }
  validateInput?(input, context): Promise<ValidationResult>
  checkPermissions(input, context): Promise<PermissionResult>
  preparePermissionMatcher?(input): Promise<(pattern) => boolean>
  prompt(options): Promise<string>
  userFacingName(input): string
  toAutoClassifierInput(input): unknown
  mapToolResultToToolResultBlockParam(content, toolUseID): ToolResultBlockParam
  backfillObservableInput?(input): void
  shouldDefer?: boolean
  alwaysLoad?: boolean
  mcpInfo?: { serverName, toolName }
  maxResultSizeChars: number
  strict?: boolean
  // 10+ render methods (React-based)
}
```

**`ToolUseContext` 类型** (`Tool.ts:158-300`)
- 40+ 字段，包含: `options`, `abortController`, `readFileState`, `getAppState()`, `setAppState()`, `handleElicitation()`, `setToolJSX()`, `addNotification()`, `appendSystemMessage()`, `sendOSNotification()`, `nestedMemoryAttachmentTriggers`, `loadedNestedMemoryPaths`, `dynamicSkillDirTriggers`, `discoveredSkillNames`, `userModified`, `setInProgressToolUseIDs`, `setResponseLength`, `pushApiMetricsEntry`, `setStreamMode`, `onCompactProgress`, `setSDKStatus`, `openMessageSelector`, `updateFileHistoryState`, `updateAttributionState`, `setConversationId`, `agentId`, `agentType`, `requireCanUseTool`, `messages`, `fileReadingLimits`, `globLimits`, `toolDecisions`, `queryTracking`, `requestPrompt`, `toolUseId`, `criticalSystemReminder_EXPERIMENTAL`, `preserveToolUseResults`, `localDenialTracking`, `contentReplacementState`, `renderedSystemPrompt`

**`buildTool()` 函数** (`Tool.ts:783-792`)
- 填充默认值: `isEnabled→true`, `isConcurrencySafe→false`, `isReadOnly→false`, `isDestructive→false`, `checkPermissions→allow`, `toAutoClassifierInput→''`, `userFacingName→name`

#### 工具执行管线 (`toolExecution.ts`)

**`runToolUse()` 函数** (`toolExecution.ts:337-490`)
1. 查找工具 (含 alias 回退)
2. 检查 abort signal
3. 调用 `streamedCheckPermissionsAndCallTool()`

**`checkPermissionsAndCallTool()` 函数** (`toolExecution.ts:599-`)
1. Zod schema 验证 (`inputSchema.safeParse`)
2. `buildSchemaNotSentHint()` — deferred tool schema 提示
3. `validateInput()` — 工具自定义验证
4. Bash speculative classifier check
5. `backfillObservableInput()` — 输入字段回填
6. `runPreToolUseHooks()` — Pre-tool-use hooks
7. `canUseTool()` — 权限检查
8. `resolveHookPermissionDecision()` — hook 权限决策合并
9. `tool.call()` — 实际工具调用
10. `processToolResultBlock()` — 结果处理 (持久化大结果)
11. `runPostToolUseHooks()` / `runPostToolUseFailureHooks()` — Post hooks

### 2.2 remote-code 现状

#### 已实现的功能

**`ToolSpec` 结构** (`crates/claude/rc-tools/src/specs.rs`)
- 40+ 内置工具规范: `list_directory`, `read_file`, `search_text`, `write_file`, `replace_in_file`, `edit_file`, `bash_command`, `glob`, `grep`, `web_fetch`, `ask_user`, `todo_write`, `task_create`, `task_output`, `task_stop`, `agent`, `send_message`, `skill`, `discover_skills`, `notebook_edit`, `web_search`, `computer_use`, `review_artifact`, `plan_mode_enter/exit`, `mcp_*`, `lsp_*`, `memory_read/write`, `git_*`, `worktree_*`, `send_user_file`, `delegate`, `workflow`, `team_*`, `system`

**`ToolRunner` trait** (`crates/claude/rc-tools/src/streaming_executor.rs:71`)
```rust
pub trait ToolRunner: Send + Sync + 'static {
    fn run(&self, tool_call_id: &str, name: &str, input: &Value, progress: &ProgressStream) -> JoinHandle<ToolExecutionResult>;
}
```

**`StreamingToolExecutor`** (`crates/claude/rc-tools/src/streaming_executor.rs:160`)
- 并发控制: `max_concurrency`, `timeout`, `max_result_bytes`
- 工具状态跟踪: `Queued → Executing → Completed → Yielded`
- 结果按序输出

**`ToolOrchestrator`** (`crates/claude/rc-tools/src/tool_orchestration.rs:189`)
- 依赖分析: 文件路径读写依赖 (`analyse_dependencies`)
- 批次分区: 并发安全工具分组 (`partition_tool_calls`)
- 调度策略: `Auto`, `SerialOnly`, `ForceParallel`

**`ToolHookManager`** (`crates/claude/rc-tools/src/tool_hooks.rs`)
- Pre/Post tool use hooks

#### 缺失的功能

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| T1 | **Tool trait 丰富度不足**: 缺少 `aliases`, `searchHint`, `validateInput`, `checkPermissions`, `preparePermissionMatcher`, `backfillObservableInput`, `interruptBehavior`, `isSearchOrReadCommand`, `shouldDefer`, `alwaysLoad`, `maxResultSizeChars`, `strict` | `Tool.ts:362-695` | P0 |
| T2 | **ToolUseContext 极度简化**: Rust 版仅有基本字段，缺少 40+ 上下文字段 (notifications, memory triggers, skill discovery, file history, attribution, denial tracking, content replacement, etc.) | `Tool.ts:158-300` | P0 |
| T3 | **工具执行管线不完整**: 无 Zod schema 验证, 无 `validateInput`, 无 `backfillObservableInput`, 无 speculative classifier check | `toolExecution.ts:599-789` | P0 |
| T4 | **Permission 检查管线缺失**: 无 `canUseTool()` 回调集成, 无 `resolveHookPermissionDecision()` | `toolExecution.ts:790-900` (估计) | P0 |
| T5 | **Tool result 持久化缺失**: 无 `processToolResultBlock()` 大结果写入文件 | `toolExecution.ts:processToolResultBlock` | P1 |
| T6 | **Deferred tool / ToolSearch 缺失**: 无 `shouldDefer`, 无 `buildSchemaNotSentHint()`, 无 ToolSearch 集成 | `toolExecution.ts:578-597` | P1 |
| T7 | **MCP 工具执行细节缺失**: 无 `mcpInfo`, 无 `mcpServerType` 提取, 无 MCP auth error 处理 | `toolExecution.ts:283-335` | P1 |
| T8 | **buildTool() 默认值填充缺失**: 无集中式默认值管理 | `Tool.ts:757-792` | P2 |
| T9 | **工具进度报告不完整**: 有 `ProgressStream` 但缺少 typed progress data (BashProgress, MCPProgress, etc.) | `Tool.ts:307-319` | P2 |

### 2.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-tools/src/tool_trait.rs` — 完整的 Tool trait (含 aliases, validation, permissions, deferred 等方法)
- `crates/claude/rc-tools/src/tool_use_context.rs` — 完整的 ToolUseContext 构建
- `crates/claude/rc-tools/src/execution_pipeline.rs` — 完整的执行管线 (validate → backfill → pre-hooks → permission → call → post-hooks → result processing)
- `crates/claude/rc-tools/src/tool_result_storage.rs` — 大结果持久化

**需要修改的文件/函数:**
- `crates/claude/rc-tools/src/specs.rs` — 添加 `aliases`, `search_hint`, `is_concurrency_safe` (per-input), `max_result_size_chars`, `should_defer` 字段
- `crates/claude/rc-tools/src/streaming_executor.rs` — 集成 execution pipeline
- `crates/claude/rc-tools/src/tool_orchestration.rs` — 集成 permission check 和 hooks

**预估工作量:** 2-3 周

---

## 3. API Client (API 客户端)

### 3.1 Claude Code 实现

#### 关键文件
- [`claude.ts`](/.research/claude-code-rev/src/services/api/claude.ts) (3420 行) — Anthropic API 客户端

#### 核心函数

**`queryModelWithStreaming()`** (`claude.ts` — 估计位置)
- 构建完整 API 请求: system prompt blocks, messages, tools, thinking config, effort, beta headers
- 流式处理 SSE 事件: `message_start`, `content_block_start/delta/stop`, `message_delta`, `message_stop`
- 处理: thinking blocks, tool_use blocks, text blocks, connector text blocks
- 支持: prompt caching (1h scope), cache editing, fast mode, AFK mode, advisor model
- Beta headers: `REDACT_THINKING`, `PROMPT_CACHING_SCOPE`, `CONTEXT_MANAGEMENT`, `EFFORT`, `FAST_MODE`, `STRUCTURED_OUTPUTS`, `TASK_BUDGETS`, `AFK_MODE`, `CONTEXT_1M`, `ADVISOR`

**`buildSystemPromptBlocks()`** — 系统提示词构建
- 支持 global cache scope (static/dynamic boundary)
- Cache breakpoints 插入
- Tool schema 构建 (含 deferred tools, ToolSearch)

**`configureTaskBudgetParams()`** — task_budget API 参数配置

**关键特性:**
- `accumulateUsage()` / `updateUsage()` — 使用量跟踪
- `computeFingerprintFromMessages()` — 消息指纹
- Model-specific max tokens 配置
- Streaming fallback to non-streaming
- Rate limit handling and quota extraction
- MCP tool injection at API level
- Advisor model support (secondary model for guidance)

### 3.2 remote-code 现状

#### 已实现的功能

**`ApiClient`** (`crates/claude/rc-provider/src/api_client.rs:175`)
- `query_model_streaming()` (`api_client.rs:204`) — 流式查询
- `query_model_without_streaming()` (`api_client.rs:261`) — 非流式查询
- `query_haiku()` (`api_client.rs:332`) — Haiku 模型快捷查询
- `update_usage()` / `accumulate_usage()` — 使用量跟踪

**`ProviderClient`** (`crates/claude/rc-provider/src/streaming.rs`)
- `complete_streaming_with_callbacks()` — 支持 OpenAI 和 Anthropic 协议
- SSE 解析: text delta, tool call accumulation, usage tracking
- Streaming → non-streaming fallback (`streaming.rs:122-142`)
- `StreamingCallbacks`: `on_text_delta`, `on_tool_call_start`, `on_tool_call_delta`, `on_usage`

**其他已实现:**
- Beta headers (`crates/claude/rc-provider/src/beta_headers.rs`)
- Cache headers/breakpoints (`crates/claude/rc-provider/src/cache_headers.rs`)
- Effort params (`crates/claude/rc-provider/src/effort_params.rs`)
- Retry logic (`crates/claude/rc-provider/src/retry.rs`)
- Fingerprint (`crates/claude/rc-provider/src/fingerprint.rs`)
- Thinking blocks (`crates/claude/rc-provider/src/thinking_blocks.rs`)
- Circuit breaker (`crates/claude/rc-provider/src/circuit_breaker.rs`)
- Model failover (`crates/claude/rc-provider/src/failover.rs`)
- SigV4 signing (`crates/claude/rc-provider/src/sigv4.rs`)
- Cost calculation (`crates/claude/rc-provider/src/cost.rs`)
- Max tokens config (`crates/claude/rc-provider/src/max_tokens.rs`)

#### 缺失的功能

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| A1 | **Thinking block 完整处理缺失**: 有基础支持但缺少 redacted_thinking, thinking signature 验证, thinking block 保留规则 | `claude.ts` thinking 相关逻辑 | P0 |
| A2 | **Deferred tools / ToolSearch API 集成缺失**: 无 `defer_loading: true` 工具发送, 无 ToolSearch schema 注入 | `claude.ts` tool schema 构建 | P0 |
| A3 | **Task budget API 参数缺失**: 无 `output_config.task_budget` 发送 | `claude.ts` configureTaskBudgetParams | P1 |
| A4 | **Advisor model 缺失**: 无二级模型指导支持 | `claude.ts` advisor 相关 | P1 |
| A5 | **Prompt cache 1h scope 缺失**: 有基础 cache 但缺少 1h TTL 支持 | `claude.ts` promptCache1hAllowlist | P1 |
| A6 | **Fast mode / AFK mode headers 缺失**: 无 `FAST_MODE_BETA_HEADER`, `AFK_MODE_BETA_HEADER` | `claude.ts` beta headers | P2 |
| A7 | **Cache editing 缺失**: 无 `getCacheEditingHeaderLatched()` | `claude.ts` cache editing | P2 |
| A8 | **MCP tool injection at API level 缺失**: 无运行时 MCP 工具注入到 API 请求 | `claude.ts` mcpTools, hasPendingMcpServers | P1 |
| A9 | **Media validation/resize 缺失**: 无图片大小验证和调整 | `claude.ts` image handling | P2 |
| A10 | **Rate limit quota extraction 缺失**: 无 `extractQuotaStatusFromHeaders/Error` | `claude.ts` quota handling | P2 |

### 3.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-provider/src/deferred_tools.rs` — deferred tool schema 处理
- `crates/claude/rc-provider/src/advisor.rs` — advisor model 支持
- `crates/claude/rc-provider/src/task_budget.rs` — task_budget API 参数

**需要修改的文件/函数:**
- `crates/claude/rc-provider/src/streaming.rs` — 添加 thinking block 完整处理, deferred tools
- `crates/claude/rc-provider/src/beta_headers.rs` — 添加 fast mode, AFK mode, task budget beta headers
- `crates/claude/rc-provider/src/cache_headers.rs` — 添加 1h scope, cache editing 支持

**预估工作量:** 2 周

---

## 4. System Prompt (系统提示词)

### 4.1 Claude Code 实现

#### 关键文件
- [`prompts.ts`](/.research/claude-code-rev/src/constants/prompts.ts) (915 行) — 系统提示词构建
- [`systemPromptSections.ts`](/.research/claude-code-rev/src/constants/systemPromptSections.ts) — 分节管理

#### 核心结构

**`getSystemPrompt()` 函数** — 构建完整系统提示词数组:
1. **静态部分** (可缓存):
   - `getSimpleIntroSection()` — 角色介绍
   - `getSimpleSystemSection()` — 系统规则
   - `getSimpleDoingTasksSection()` — 任务执行指南
   - `getActionsWithCareSection()` — 谨慎操作指南
   - `getUsingToolsSection()` — 工具使用指南
   - `getToneAndStyleSection()` — 语气风格
   - `getOutputEfficiencySection()` — 输出效率

2. **动态边界标记**: `SYSTEM_PROMPT_DYNAMIC_BOUNDARY`

3. **动态部分** (每会话):
   - `getSessionGuidanceSection()` — 会话指导
   - `getMemorySection()` — CLAUDE.md 记忆
   - `getEnvironmentInfoSection()` — 环境信息
   - `getLanguageSection()` — 语言偏好
   - `getOutputStyleSection()` — 输出风格
   - `getMcpInstructionsSection()` — MCP 指令
   - `getScratchpadSection()` — Scratchpad
   - `getFunctionResultClearingSection()` — 函数结果清理
   - `getTokenBudgetSection()` — Token 预算
   - `getHooksSection()` — Hooks 说明
   - `getSystemRemindersSection()` — 系统提醒
   - `getProactiveSection()` — 主动建议 (Kairos)
   - `getAgentToolSection()` — Agent 工具说明
   - `getToolSearchSection()` — ToolSearch 说明

**特性:**
- `systemPromptSection()` — 带 cache breakpoint 的 section 包装
- `DANGEROUS_uncachedSystemPromptSection()` — 不缓存的 section
- `resolveSystemPromptSections()` — 条件解析
- Feature-gated sections (PROACTIVE, KAIROS, EXPERIMENTAL_SKILL_SEARCH, etc.)

### 4.2 remote-code 现状

#### 已实现的功能

**`SystemPromptBuilder`** (`crates/claude/rc-system-prompt/src/lib.rs:134`)
- 完整的 section 架构: `SystemPromptSection` trait
- 静态 sections: `IntroSection`, `SystemSection`, `DoingTasksSection`, `ActionsSection`, `UsingToolsSection`, `ToneStyleSection`, `OutputEfficiencySection`
- 动态 sections: `SessionGuidanceSection`, `MemorySection`, `EnvInfoSection`, `LanguageSection`, `OutputStyleSection`, `McpInstructionsSection`, `ScratchpadSection`, `ToolResultSection`, `TokenBudgetSection`, `HooksSection`, `SystemRemindersSection`, `CoordinatorSection`, `ProactiveSection`
- `SectionCache` — section 缓存
- `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` — 动态边界标记
- `PromptContext` — 运行时上下文

#### 缺失的功能

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| S1 | **Agent tool section 缺失**: 无 Agent 工具说明 prompt | `prompts.ts` getAgentToolSection | P1 |
| S2 | **ToolSearch section 缺失**: 无 ToolSearch 工具说明 | `prompts.ts` getToolSearchSection | P1 |
| S3 | **Feature-gated conditional sections 不完整**: 缺少 PROACTIVE, KAIROS, EXPERIMENTAL_SKILL_SEARCH 等 feature gate | `prompts.ts` feature() gates | P2 |
| S4 | **Prompt 内容精度**: 各 section 的具体 prompt 文本需要与 Claude Code 逐字对比更新 | `prompts.ts` 各 section | P1 |
| S5 | **Memory section (CLAUDE.md) 加载缺失**: 有 MemorySection 结构但缺少实际的 CLAUDE.md 文件读取和注入 | `prompts.ts` getMemorySection | P1 |
| S6 | **Output style configurations 不完整**: 缺少具体的 output style 预设 | `prompts.ts` getOutputStyleSection | P2 |

### 4.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-system-prompt/src/sections/agent_tool.rs` — Agent 工具说明
- `crates/claude/rc-system-prompt/src/sections/tool_search.rs` — ToolSearch 说明

**需要修改的文件/函数:**
- `crates/claude/rc-system-prompt/src/lib.rs` — 添加新 sections, 添加 feature gate 支持
- 各 section 文件 — 逐字对比更新 prompt 内容

**预估工作量:** 1 周

---

## 5. Compact System (压缩系统)

### 5.1 Claude Code 实现

#### 关键文件
- [`compact.ts`](/.research/claude-code-rev/src/services/compact/compact.ts) (1706 行) — 核心压缩
- [`autoCompact.ts`](/.research/claude-code-rev/src/services/compact/autoCompact.ts) (352 行) — 自动压缩
- [`reactiveCompact.ts`](/.research/claude-code-rev/src/services/compact/reactiveCompact.ts) — 响应式压缩
- [`microCompact.ts`](/.research/claude-code-rev/src/services/compact/microCompact.ts) — 微压缩
- [`snipCompact.ts`](/.research/claude-code-rev/src/services/compact/snipCompact.ts) — 裁剪压缩
- [`contextCollapse/index.ts`](/.research/claude-code-rev/src/services/contextCollapse/index.ts) — 上下文折叠
- [`sessionMemoryCompact.ts`](/.research/claude-code-rev/src/services/compact/sessionMemoryCompact.ts) — 会话记忆压缩
- [`postCompactCleanup.ts`](/.research/claude-code-rev/src/services/compact/postCompactCleanup.ts) — 压缩后清理
- [`grouping.ts`](/.research/claude-code-rev/src/services/compact/grouping.ts) — 消息分组

#### 核心函数

**`compactConversation()`** (`compact.ts`)
- 完整压缩: 图片剥离 → 消息分组 → LLM 摘要 → 附件恢复 → hook 执行
- 支持: recompaction, partial compaction, PTL retry (最多 3 次)
- Pre/post compact hooks
- File state cache 序列化/恢复
- Session memory 注入
- Tool discovery 保留

**`AutoCompactStrategy`** (`autoCompact.ts`)
- 阈值计算: `effectiveContextWindow - AUTOCOMPACT_BUFFER_TOKENS (13,000)`
- 警告/错误/阻塞阈值
- Circuit breaker: `MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES = 3`
- `AutoCompactTrackingState`: `compacted`, `turnCounter`, `turnId`, `consecutiveFailures`

**`ReactiveCompact`** (`reactiveCompact.ts`)
- 在 prompt-too-long 后触发
- 图片/PDF 剥离重试
- 与 context collapse 协同

**`MicroCompact`** (`microCompact.ts`)
- 缓存编辑: 替换旧的 tool_result 而不重新发送
- `apiMicrocompact()` — API 级别的上下文管理

**`SnipCompact`** (`snipCompact.ts`)
- 历史裁剪: 移除旧的 tool_result 内容
- `snipProjection` — 裁剪投影

**`ContextCollapse`** (`contextCollapse/index.ts`)
- 分层折叠: 保持粒度上下文
- `recoverFromOverflow()` — 溢出恢复
- 与 autocompact 协同

### 5.2 remote-code 现状

#### 已实现的功能

**`FullCompactStrategy` / `PartialCompactStrategy`** (`crates/claude/rc-compact/src/engine.rs`)
- `compact_conversation()` (`engine.rs:121`) — 完整压缩
- PTL retry (最多 3 次) (`engine.rs:149`)
- Summary provider trait (抽象 LLM 调用)
- Preserved segment (保留最近消息)

**`AutoCompactStrategy`** (`crates/claude/rc-compact/src/auto.rs`)
- `AutoCompactTrackingState` — 与 TS 版匹配
- `should_auto_compact()` — 阈值检查
- `calculate_token_warning_state()` — 警告状态计算
- Circuit breaker (`MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES = 3`)

**其他已实现:**
- `CompactStrategy` trait (`crates/claude/rc-compact/src/strategy.rs`)
- `CompactOptions`, `CompactionResult`, `PreservedSegment`
- `ProgressCallback` 支持
- Compact prompt 构建 (`crates/claude/rc-compact/src/prompt.rs`)
- Message grouping (`crates/claude/rc-compact/src/grouping.rs`)

#### 缺失的功能

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| C1 | **Reactive compact 缺失**: 无 prompt-too-long 后的响应式压缩 | `reactiveCompact.ts` | P0 |
| C2 | **MicroCompact 缺失**: 无缓存编辑，无 API 级别上下文管理 | `microCompact.ts`, `api_micro.rs` | P0 |
| C3 | **SnipCompact 缺失**: 无历史裁剪 | `snipCompact.ts` | P1 |
| C4 | **Context Collapse 缺失**: 无分层折叠，无溢出恢复 | `contextCollapse/index.ts` | P1 |
| C5 | **Session memory compact 缺失**: 无会话记忆压缩 | `sessionMemoryCompact.ts` | P1 |
| C6 | **Post-compact cleanup 缺失**: 无压缩后清理 (file restore, skill re-injection) | `postCompactCleanup.ts` | P1 |
| C7 | **Image stripping 缺失**: 压缩前无图片剥离 | `compact.ts:145` stripImagesFromMessages | P1 |
| C8 | **Pre/post compact hooks 缺失**: 无 hook 执行 | `compact.ts` executePreCompactHooks/executePostCompactHooks | P1 |
| C9 | **Forked agent compact 缺失**: 无 `runForkedAgent()` 集成 | `compact.ts` runForkedAgent | P2 |
| C10 | **Compact warning hook 缺失**: 无 `compactWarningHook` | `compactWarningHook.ts` | P2 |

### 5.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-compact/src/reactive.rs` — 已存在但需要实现 reactive compact 逻辑
- `crates/claude/rc-compact/src/micro.rs` — 已存在但需要实现 microcompact
- `crates/claude/rc-compact/src/snip.rs` — 已存在但需要实现 snip compact
- `crates/claude/rc-compact/src/context_collapse.rs` — 已存在但需要实现 context collapse
- `crates/claude/rc-compact/src/post_compact.rs` — 已存在但需要实现 post-compact cleanup

**需要修改的文件/函数:**
- `crates/claude/rc-compact/src/engine.rs` — 添加 image stripping, hooks, session memory
- `crates/claude/rc-compact/src/auto.rs` — 集成 reactive/micro/snip

**预估工作量:** 2-3 周

---

## 6. State Management (状态管理)

### 6.1 Claude Code 实现

#### 关键文件
- [`AppStateStore.ts`](/.research/claude-code-rev/src/state/AppStateStore.ts) (570 行) — 应用状态定义
- [`AppState.tsx`](/.research/claude-code-rev/src/state/AppState.tsx) — React 状态提供者
- [`store.ts`](/.research/claude-code-rev/src/state/store.ts) — 状态存储
- [`selectors.ts`](/.research/claude-code-rev/src/state/selectors.ts) — 状态选择器

#### 核心类型

**`AppState`** (`AppStateStore.ts:89-230+`)
```typescript
type AppState = {
  settings: SettingsJson
  verbose: boolean
  mainLoopModel: ModelSetting
  mainLoopModelForSession: ModelSetting
  statusLineText: string | undefined
  expandedView: 'none' | 'tasks' | 'teammates'
  isBriefOnly: boolean
  showTeammateMessagePreview?: boolean
  selectedIPAgentIndex: number
  coordinatorTaskIndex: number
  viewSelectionMode: 'none' | 'selecting-agent' | 'viewing-agent'
  footerSelection: FooterItem | null
  toolPermissionContext: ToolPermissionContext
  spinnerTip?: string
  agent: string | undefined
  kairosEnabled: boolean
  remoteSessionUrl: string | undefined
  remoteConnectionStatus: 'connecting' | 'connected' | 'reconnecting' | 'disconnected'
  remoteBackgroundTaskCount: number
  replBridgeEnabled: boolean
  replBridgeExplicit: boolean
  replBridgeOutboundOnly: boolean
  replBridgeConnected: boolean
  replBridgeSessionActive: boolean
  replBridgeReconnecting: boolean
  replBridgeConnectUrl: string | undefined
  replBridgeSessionUrl: string | undefined
  replBridgeEnvironmentId: string | undefined
  replBridgeSessionId: string | undefined
  replBridgeError: string | undefined
  replBridgeInitialName: string | undefined
  showRemoteCallout: boolean
  tasks: { [taskId: string]: TaskState }
  agentNameRegistry: Map<string, AgentId>
  foregroundedTaskId?: string
  viewingAgentTaskId?: string
  companionReaction?: string
  companionPetAt?: number
  mcp: {
    clients: MCPServerConnection[]
    tools: Tool[]
    commands: Command[]
    resources: Record<string, ServerResource[]>
    pluginReconnectKey: number
  }
  plugins: {
    enabled: LoadedPlugin[]
    disabled: LoadedPlugin[]
    commands: Command[]
    errors: PluginError[]
    installationStatus: { marketplaces: [...] }
  }
  fileHistory: FileHistoryState
  attribution: AttributionState
  todo: TodoList | null
  promptSuggestion: { text: string; promptId: string } | null
  speculation: SpeculationState
  sessionHooks: SessionHooksState
  denialTracking: DenialTrackingState
  fastMode: boolean
  effortValue: EffortValue | null
  advisorModel: string | undefined
  outputStyle: string | null
  outputStyleConfig: OutputStyleConfig | null
  // ... 更多
}
```

**`ToolPermissionContext`** (`Tool.ts:123-138`)
```typescript
type ToolPermissionContext = {
  mode: PermissionMode
  additionalWorkingDirectories: Map<string, AdditionalWorkingDirectory>
  alwaysAllowRules: ToolPermissionRulesBySource
  alwaysDenyRules: ToolPermissionRulesBySource
  alwaysAskRules: ToolPermissionRulesBySource
  isBypassPermissionsModeAvailable: boolean
  isAutoModeAvailable?: boolean
  strippedDangerousRules?: ToolPermissionRulesBySource
  shouldAvoidPermissionPrompts?: boolean
  awaitAutomatedChecksBeforeDialog?: boolean
  prePlanMode?: PermissionMode
}
```

### 6.2 remote-code 现状

#### 已实现的功能

**`AppState`** (`crates/claude/rc-core/src/state.rs:44-62`)
```rust
pub struct AppState {
    pub session_id: Option<SessionId>,
    pub active_agent_id: Option<AgentId>,
    pub permission_mode: PermissionMode,
    pub messages: Vec<Message>,
    pub discovered_skills: BTreeSet<String>,
    pub active_tools: BTreeSet<String>,
    pub model: Option<String>,
    pub queued_task_count: usize,
}
```

**`ToolPermissionContext`** (`crates/claude/rc-core/src/state.rs:11-20`)
```rust
pub struct ToolPermissionContext {
    pub allowlisted_tools: BTreeSet<String>,
    pub denylisted_tools: BTreeSet<String>,
    pub working_directory: Option<PathBuf>,
    pub extra: Value,
}
```

**`FileHistoryState`** (`crates/claude/rc-core/src/state.rs:23-41`)

#### 缺失的功能

| # | 缺失功能 | Claude Code 参考 | 优先级 |
|---|---------|-----------------|--------|
| ST1 | **AppState 极度简化**: 缺少 50+ 字段 (settings, verbose, model, toolPermissionContext, mcp, plugins, tasks, todo, speculation, session hooks, denial tracking, fast mode, effort, advisor, output style, bridge state, remote state, etc.) | `AppStateStore.ts:89-230` | P0 |
| ST2 | **ToolPermissionContext 极度简化**: 缺少 `additionalWorkingDirectories`, `alwaysAllowRules`, `alwaysDenyRules`, `alwaysAskRules`, `isBypassPermissionsModeAvailable`, `isAutoModeAvailable`, `strippedDangerousRules`, `shouldAvoidPermissionPrompts`, `awaitAutomatedChecksBeforeDialog`, `prePlanMode` | `Tool.ts:123-138` | P0 |
| ST3 | **无 React-like 响应式状态管理**: 无 subscribe/selector 模式 | `AppState.tsx` useSyncExternalStore | P1 |
| ST4 | **无 settings 集成**: 缺少 `SettingsJson` 管理 | `AppStateStore.ts:91` | P1 |
| ST5 | **无 MCP 状态管理**: 缺少 clients, tools, commands, resources | `AppStateStore.ts:173-184` | P1 |
| ST6 | **无 Plugin 状态管理**: 缺少 enabled/disabled plugins, errors | `AppStateStore.ts:185-200` | P2 |
| ST7 | **无 Task 状态管理**: 缺少 `TaskState` 字典 | `AppStateStore.ts:160` | P1 |
| ST8 | **无 Speculation 状态**: 缺少 speculation state machine | `AppStateStore.ts:58-77` | P2 |
| ST9 | **无 Denial tracking**: 缺少 `DenialTrackingState` | `AppStateStore.ts` denialTracking | P1 |
| ST10 | **无 Bridge/Remote 状态**: 缺少 15+ bridge 相关字段 | `AppStateStore.ts:134-157` | P2 |

### 6.3 具体修复建议

**需要新增的文件/函数:**
- `crates/claude/rc-core/src/tool_permission_context.rs` — 完整的 ToolPermissionContext (含 rules, modes, directories)
- `crates/claude/rc-core/src/app_state.rs` — 已存在但需要大幅扩展字段

**需要修改的文件/函数:**
- `crates/claude/rc-core/src/state.rs:AppState` — 添加 30+ 字段 (settings, mcp, plugins, tasks, todo, effort, fast_mode, etc.)
- `crates/claude/rc-core/src/state.rs:ToolPermissionContext` — 替换为完整的 permission rules 系统

**预估工作量:** 2 周

---

## 优先级排序

### P0 — 核心功能缺失 (阻塞基本运行)

| # | 系统 | 缺失功能 | 工作量 |
|---|------|---------|--------|
| 1 | Query Engine | 消息预处理管线 (snip, microcompact, tool result budget) | 1 周 |
| 2 | Query Engine | Reactive compact (prompt-too-long 恢复) | 3 天 |
| 3 | Query Engine | Max-output-tokens 恢复 | 2 天 |
| 4 | Query Engine | 模型 fallback | 2 天 |
| 5 | Query Engine | StreamingToolExecutor 集成到 query_loop | 3 天 |
| 6 | Tool System | Tool trait 丰富度 (aliases, validation, permissions) | 1 周 |
| 7 | Tool System | ToolUseContext 完整化 | 3 天 |
| 8 | Tool System | 工具执行管线 (validate → hooks → permission → call → post-hooks) | 1 周 |
| 9 | API Client | Thinking block 完整处理 | 3 天 |
| 10 | API Client | Deferred tools / ToolSearch API 集成 | 3 天 |
| 11 | Compact | Reactive compact 实现 | 3 天 |
| 12 | Compact | MicroCompact 实现 | 3 天 |
| 13 | State | AppState 扩展 (30+ 字段) | 3 天 |
| 14 | State | ToolPermissionContext 完整化 | 3 天 |

**P0 总工作量: ~7 周**

### P1 — 重要功能缺失 (影响用户体验)

| # | 系统 | 缺失功能 | 工作量 |
|---|------|---------|--------|
| 15 | Query Engine | 工具使用摘要 (Haiku 异步) | 2 天 |
| 16 | Query Engine | 附件消息注入 (memory, skills) | 3 天 |
| 17 | Query Engine | 队列命令消费 | 2 天 |
| 18 | Query Engine | Stop hooks 阻塞重试 | 2 天 |
| 19 | Query Engine | Token budget continuation | 1 天 |
| 20 | Query Engine | Task budget remaining 跟踪 | 1 天 |
| 21 | Tool System | Tool result 持久化 (大结果写文件) | 2 天 |
| 22 | Tool System | Deferred tool / ToolSearch 集成 | 3 天 |
| 23 | Tool System | MCP 工具执行细节 | 2 天 |
| 24 | API Client | Task budget API 参数 | 1 天 |
| 25 | API Client | Advisor model | 2 天 |
| 26 | API Client | Prompt cache 1h scope | 1 天 |
| 27 | API Client | MCP tool injection at API level | 2 天 |
| 28 | System Prompt | Agent tool / ToolSearch sections | 1 天 |
| 29 | System Prompt | Prompt 内容精度更新 | 2 天 |
| 30 | System Prompt | Memory section (CLAUDE.md) 加载 | 1 天 |
| 31 | Compact | SnipCompact | 2 天 |
| 32 | Compact | Context Collapse | 3 天 |
| 33 | Compact | Session memory compact | 2 天 |
| 34 | Compact | Post-compact cleanup | 2 天 |
| 35 | Compact | Image stripping | 1 天 |
| 36 | Compact | Pre/post compact hooks | 2 天 |
| 37 | State | 响应式状态管理 (subscribe/selector) | 3 天 |
| 38 | State | Settings 集成 | 2 天 |
| 39 | State | MCP 状态管理 | 2 天 |
| 40 | State | Task 状态管理 | 2 天 |
| 41 | State | Denial tracking | 1 天 |

**P1 总工作量: ~6 周**

### P2 — 次要功能缺失 (可后续迭代)

| # | 系统 | 缺失功能 | 工作量 |
|---|------|---------|--------|
| 42 | Query Engine | Tombstone 消息 | 1 天 |
| 43 | Query Engine | Post-sampling hooks | 1 天 |
| 44 | Query Engine | Dump prompts 调试 | 1 天 |
| 45 | Query Engine | Query chain tracking 完善 | 1 天 |
| 46 | Tool System | buildTool() 默认值填充 | 0.5 天 |
| 47 | Tool System | Typed progress data | 1 天 |
| 48 | API Client | Fast mode / AFK mode headers | 1 天 |
| 49 | API Client | Cache editing | 2 天 |
| 50 | API Client | Media validation/resize | 2 天 |
| 51 | API Client | Rate limit quota extraction | 1 天 |
| 52 | System Prompt | Feature-gated conditional sections | 1 天 |
| 53 | System Prompt | Output style presets | 1 天 |
| 54 | Compact | Forked agent compact | 2 天 |
| 55 | Compact | Compact warning hook | 1 天 |
| 56 | State | Plugin 状态管理 | 2 天 |
| 57 | State | Speculation state | 2 天 |
| 58 | State | Bridge/Remote 状态 | 2 天 |

**P2 总工作量: ~4 周**

---

## 总结

| 优先级 | 缺失项数 | 预估工作量 |
|--------|---------|-----------|
| P0 | 14 项 | ~7 周 |
| P1 | 27 项 | ~6 周 |
| P2 | 17 项 | ~4 周 |
| **总计** | **58 项** | **~17 周** |

### 建议实施顺序

1. **Phase 1 (P0, 7 周)**: 先修复 Query Engine 核心循环 (预处理管线、错误恢复、流式工具执行)，然后完善 Tool System (trait 丰富度、执行管线)，最后修复 API Client (thinking blocks、deferred tools) 和 Compact (reactive、micro)
2. **Phase 2 (P1, 6 周)**: 完善 Query Engine 后处理 (摘要、附件、队列)，Tool System 高级功能 (持久化、MCP)，System Prompt 精度，Compact 高级策略，State 扩展
3. **Phase 3 (P2, 4 周)**: 补齐所有次要功能

### 关键风险

1. **Query Engine 重构风险最大**: `query_loop.rs` 需要大幅重构以支持预处理管线和错误恢复，建议先写集成测试
2. **Tool trait 变更影响面广**: 修改 `ToolSpec` 和 `ToolRunner` 会影响所有工具实现
3. **Compact 系统依赖 LLM**: reactive/micro compact 需要额外的 API 调用，需要确保 provider 层稳定
