# Claude Code 深度对比分析报告

**日期**: 2026-04-14  
**分析对象**: `.research/claude-code-rev` (Restored Claude Code Source) vs `remote-code-rust`  
**方法**: 逐模块代码级静态分析  
**前置文档**: `plans/claude-cli-parity-program.md` (架构级方案)

---

## 1. 工具集对比（Tool-by-Tool）

### 1.1 Claude Code 完整工具清单（50+ 工具）

| # | Claude Code 工具 | 工具名 | remote-code 对应 | 差距说明 |
|---|---|---|---|---|
| 1 | BashTool | `Bash` | `bash_command` | ✅ 基本对等，缺少 sandbox 集成 |
| 2 | FileReadTool | `Read` | `read_file` | ✅ 对等 |
| 3 | FileWriteTool | `Write` | `write_file` | ✅ 对等，缺少 pre-read 校验 |
| 4 | FileEditTool | `Edit` | `apply_diff` | ⚠️ 缺少 sed 解析器 |
| 5 | GlobTool | `Glob` | `glob` | ✅ 对等 |
| 6 | GrepTool | `Grep` | `grep` | ✅ 对等 |
| 7 | AgentTool | `Agent` | `agent` | ⚠️ 缺少 fork subagent |
| 8 | TodoWriteTool | `TodoWrite` | ❌ 缺失 | **需要实现** |
| 9 | TaskCreateTool | `TaskCreate` | `task_create` | ✅ 基本对等 |
| 10 | TaskGetTool | `TaskGet` | `task_get` | ✅ 基本对等 |
| 11 | TaskListTool | `TaskList` | `task_list` | ✅ 基本对等 |
| 12 | TaskUpdateTool | `TaskUpdate` | `task_update` | ✅ 基本对等 |
| 13 | TaskOutputTool | `TaskOutput` | `task_output` | ✅ 基本对等 |
| 14 | TaskStopTool | `TaskStop` | `task_stop` | ✅ 基本对等 |
| 15 | AskUserQuestionTool | `AskUserQuestion` | ❌ 缺失 | **需要实现** |
| 16 | SkillTool | `Skill` | ❌ 缺失 | **需要实现** |
| 17 | SendMessageTool | `SendMessage` | ❌ 缺失 | **需要实现** (多 agent 通信) |
| 18 | MCPTool | `mcp_tool` | `mcp_tool` | ✅ 基本对等 |
| 19 | ListMcpResourcesTool | `ListMcpResources` | ❌ 缺失 | 低优先级 |
| 20 | ReadMcpResourceTool | `ReadMcpResource` | ❌ 缺失 | 低优先级 |
| 21 | McpAuthTool | `McpAuth` | ❌ 缺失 | 低优先级 |
| 22 | WebSearchTool | `WebSearch` | `web_search` | ✅ 基本对等 |
| 23 | WebFetchTool | `WebFetch` | `web_fetch` | ✅ 基本对等 |
| 24 | WebBrowserTool | `WebBrowser` | ❌ 缺失 | P1 增强 |
| 25 | NotebookEditTool | `NotebookEdit` | ❌ 缺失 | P2 延后 |
| 26 | PowerShellTool | `PowerShell` | (bash_command 覆盖) | 可选 |
| 27 | EnterPlanModeTool | `EnterPlanMode` | ❌ 缺失 | **需要实现** |
| 28 | ExitPlanModeTool | `ExitPlanMode` | ❌ 缺失 | **需要实现** |
| 29 | EnterWorktreeTool | `EnterWorktree` | ❌ 缺失 | P1 增强 |
| 30 | ExitWorktreeTool | `ExitWorktree` | ❌ 缺失 | P1 增强 |
| 31 | BriefTool | `Brief` | ❌ 缺失 | P2 (KAIROS) |
| 32 | DiscoverSkillsTool | `DiscoverSkills` | ❌ 缺失 | P1 增强 |
| 33 | ToolSearchTool | `ToolSearch` | ❌ 缺失 | P1 增强 |
| 34 | SleepTool | `Sleep` | ❌ 缺失 | 低优先级 |
| 35 | SnipTool | `Snip` | ❌ 缺失 | P1 (compaction 相关) |
| 36 | MonitorTool | `Monitor` | ❌ 缺失 | P2 |
| 37 | ReviewArtifactTool | `ReviewArtifact` | ❌ 缺失 | P1 |
| 38 | ScheduleCronTool | `ScheduleCron` | ❌ 缺失 | P2 |
| 39 | SyntheticOutputTool | `SyntheticOutput` | ❌ 缺失 | P1 (JSON schema 输出) |
| 40 | SendUserFileTool | `SendUserFile` | ❌ 缺失 | P2 |
| 41 | LSPTool | `LSP` | `lsp` | ✅ 骨架存在 |
| 42 | TungstenTool | `Tungsten` | ❌ 缺失 | P2 (虚拟终端) |
| 43 | RemoteTriggerTool | `RemoteTrigger` | ❌ 缺失 | P2 |
| 44 | VerifyPlanExecutionTool | `VerifyPlanExecution` | ❌ 缺失 | P1 |
| 45 | WorkflowTool | `Workflow` | ❌ 缺失 | P2 |
| 46 | TeamCreateTool | `TeamCreate` | ❌ 缺失 | P2 |
| 47 | TeamDeleteTool | `TeamDelete` | ❌ 缺失 | P2 |
| 48 | TerminalCaptureTool | `TerminalCapture` | ❌ 缺失 | P2 |
| 49 | REPLTool | `REPL` | ❌ 缺失 | P2 |

### 1.2 关键差距总结

**P0 必须补齐的工具（6个）**:
1. `TodoWrite` - 任务规划与进度跟踪（Claude Code 的核心工具之一）
2. `AskUserQuestion` - 主动向用户提问（交互式场景关键）
3. `Skill` - 技能调用（/commit、/simplify 等技能的执行入口）
4. `EnterPlanMode` / `ExitPlanMode` - 计划模式切换
5. `SendMessage` - 多 agent 通信

**P1 高价值增强（8个）**:
1. `ToolSearch` - 工具搜索（帮助模型发现可用工具）
2. `DiscoverSkills` - 技能发现
3. `SyntheticOutput` - 结构化 JSON 输出
4. `VerifyPlanExecution` - 验证计划执行
5. `ReviewArtifact` - 审查产物
6. `Snip` - 上下文裁剪（compaction 相关）
7. `WebBrowser` - 浏览器自动化
8. `EnterWorktree` / `ExitWorktree` - 工作树切换

---

## 2. 核心架构对比

### 2.1 Query Engine

| 维度 | Claude Code (`QueryEngine.ts`) | remote-code (`conversation.rs`) |
|------|------|------|
| **代码量** | 1,296 行 | 1,287 行 |
| **状态模型** | 显式状态机 (class-based) | 隐式 loop (function-based) |
| **消息管理** | `mutableMessages` 数组 + `submitMessage()` generator | `Vec<ConversationEntry>` + `run_prompt()` loop |
| **权限系统** | `canUseTool` 回调 + denial tracking | `PermissionBroker` trait + decision |
| **上下文管理** | 多层：auto/reactive/micro/snip/proactive | 多策略：standard/reactive/micro/auto/sliding/priority/semantic |
| **Budget 控制** | `maxTurns` + `maxBudgetUsd` + `taskBudget` | `max_turns` only |
| **Compact 触发** | proactive + reactive + snip + micro | reactive only |
| **Resume** | `orphanedPermission` + session replay | `restore_session_context` |
| **Agent 支持** | 内置 agent 定义 + fork subagent | 基础 `SubAgentCompletion` trait |
| **Skill 支持** | 完整 skill 系统 + bundled skills | 基础骨架 |
| **Streaming** | `AsyncGenerator<SDKMessage>` | `PromptStreamEvent` enum |
| **错误恢复** | `categorizeRetryableAPIError` + retry graph | circuit breaker + retry |
| **Transcript** | 实时 flush + compact boundary | session store |

### 2.2 关键架构差距

#### 差距 1: Query Engine 状态机

Claude Code 的 `QueryEngine` 是一个 class，维护了完整的会话状态：
```typescript
class QueryEngine {
  private mutableMessages: Message[]
  private abortController: AbortController
  private permissionDenials: SDKPermissionDenial[]
  private totalUsage: NonNullableUsage
  private discoveredSkillNames: Set<string>
  private loadedNestedMemoryPaths: Set<string>
}
```

我们的 `conversation.rs` 是一个函数式 loop，状态分散在多个参数中。需要升级为显式状态机。

#### 差距 2: System Prompt 结构化

Claude Code 的 system prompt 是高度结构化的，分为多个 section：
- `getSimpleIntroSection` - 角色介绍
- `getSimpleSystemSection` - 系统规则
- `getSimpleDoingTasksSection` - 任务执行指南
- `getActionsSection` - 行为谨慎性指南
- `getUsingYourToolsSection` - 工具使用指南
- `getAgentToolSection` - Agent 工具指南
- `getOutputEfficiencySection` - 输出效率指南
- `getSimpleToneAndStyleSection` - 风格指南
- `getSessionSpecificGuidanceSection` - 会话特定指南

每个 section 都有 feature flag 控制，支持动态组合和缓存优化。

我们的 system prompt 是一个简单的字符串模板 (`default_system_prompt`)，缺乏结构化和动态组合能力。

#### 差距 3: Compaction 生命周期

Claude Code 的 compaction 是一个完整生命周期：
```
proactive autocompact → microcompact → reactive compact → snip compact → compact boundary → post-compact restore
```

包含 14+ 个文件 (`src/services/compact/`)：
- `autoCompact.ts` - 自动压缩触发
- `microCompact.ts` - 微压缩
- `compact.ts` - 主压缩逻辑 (1,706 行)
- `snipCompact.ts` - 裁剪压缩
- `snipProjection.ts` - 裁剪投影
- `reactiveCompact.ts` - 响应式压缩
- `cachedMCConfig.ts` - 缓存配置
- `sessionMemoryCompact.ts` - 会话记忆压缩
- `compactWarningHook.ts` - 压缩警告钩子
- `postCompactCleanup.ts` - 压缩后清理

我们的 compaction 是一组压缩算法，但缺少完整的生命周期管理。

#### 差距 4: Tool Prompt 工程

Claude Code 的每个工具都有精心设计的 prompt。例如 `BashTool` 的 prompt 有 370 行，包含：
- Git 操作详细指南（commit、PR 创建流程）
- 安全协议（禁止 --no-verify、force push 等）
- 后台任务使用说明
- 命令分类（read-only/mutating/network/risky）
- 沙箱使用说明
- 路径验证说明

我们的工具 prompt 相对简单，缺少这些精细化的行为指导。

---

## 3. Services 层对比

### 3.1 MCP 服务

| 维度 | Claude Code | remote-code |
|------|------|------|
| 连接管理 | `MCPConnectionManager` (React) | `McpConfig` 静态加载 |
| 认证 | OAuth + elicitation handler | 无 |
| 权限 | channel allowlist + notification | 无 |
| 动态刷新 | turn 间刷新 | 仅启动时加载 |
| Transport | InProcess + SdkControl + stdio | stdio only |

### 3.2 API 服务

| 维度 | Claude Code | remote-code |
|------|------|------|
| Provider | Anthropic only (原生) | 多 provider (优势) |
| Rate Limit | 完整的 rate limit 管理 + mocking | circuit breaker only |
| Token 估算 | `tokenEstimation.ts` | 依赖 provider 报告 |
| Cost Tracking | `cost-tracker.ts` + `costHook.ts` | 基础 usage summary |
| 错误分类 | `categorizeRetryableAPIError` | `classify_provider_error` |

### 3.3 会话服务

| 维度 | Claude Code | remote-code |
|------|------|------|
| 存储 | NDJSON transcript | SQLite + NDJSON |
| 恢复 | `sessionDiscovery` + `sessionHistory` | `restore_session_context` |
| 压缩边界 | compact boundary record | 无 |
| 分叉 | fork lineage | 无 |

---

## 4. Bridge/通信层对比

### 4.1 Claude Code Bridge 架构

Claude Code 有一个完整的 bridge 层（40+ 文件）：
- `bridgeMain.ts` - 主入口
- `bridgeApi.ts` - API 定义
- `bridgeConfig.ts` - 配置管理
- `bridgeMessaging.ts` - 消息传递
- `bridgePermissionCallbacks.ts` - 权限回调
- `bridgeUI.ts` - UI 通信
- `remoteBridgeCore.ts` - 远程桥接核心
- `replBridge.ts` - REPL 桥接
- `sessionRunner.ts` - 会话运行器
- `jwtUtils.ts` - JWT 认证
- `peerSessions.ts` - 对等会话
- `trustedDevice.ts` - 设备信任

### 4.2 remote-code 对应

我们的 `headless.rs` + `claude-protocol` + `claude-control-plane` 提供了类似功能，但架构不同：
- headless 模式通过 `PromptStreamEvent` 输出
- remote 通过 control plane WebSocket 通信
- 缺少 bridge 式的双向实时通信模型

---

## 5. System Prompt 对比

### 5.1 Claude Code System Prompt 结构

Claude Code 的 system prompt 由以下 section 组成（约 915 行代码管理）：

```
1. Static Prefix (可缓存)
   ├── Intro Section (角色定义)
   ├── System Section (系统规则)
   ├── Doing Tasks Section (任务执行)
   ├── Actions Section (行为谨慎性)
   ├── Using Your Tools Section (工具使用)
   ├── Output Efficiency Section (输出效率)
   ├── Tone and Style Section (风格)
   └── DYNAMIC_BOUNDARY

2. Dynamic Sections (不可缓存)
   ├── Session Guidance (会话特定)
   ├── Memory (记忆)
   ├── Env Info (环境信息)
   ├── MCP Instructions (MCP 指令)
   ├── Language (语言偏好)
   ├── Output Style (输出风格)
   └── Scratchpad (草稿板)
```

### 5.2 remote-code System Prompt

我们的 system prompt 由 `default_system_prompt()` 生成，是一个相对简单的字符串模板。

**关键差距**：
1. 缺少结构化 section 管理
2. 缺少动态/静态分离（影响 prompt caching）
3. 缺少工具使用指南的精细化
4. 缺少输出风格控制
5. 缺少会话特定指南注入

---

## 6. 压力测试发现的问题与 Claude Code 的对比

### 6.1 write_file 缺少 path 参数

**现象**: MiniMax M2.7 模型在调用 `write_file` 时多次遗漏 `path` 参数。

**Claude Code 的做法**:
- `FileWriteTool` 的 prompt 明确说明：`"Writes a file to the local filesystem. Usage: This tool will overwrite the existing file..."`
- 工具 schema 定义清晰，`file_path` 是 required 参数
- 如果模型遗漏参数，API 层会返回结构化错误

**我们的改进方向**:
1. 在 `write_file` 的 tool description 中更明确地说明参数格式
2. 在 system prompt 中注入操作系统信息（避免 Unix-only 命令）
3. 增强错误提示，包含正确的参数格式示例

### 6.2 模型使用 Unix 命令

**现象**: 模型在 Windows 上使用 `cat >` 命令。

**Claude Code 的做法**:
- System prompt 中包含 `osType()` / `osVersion()` / `osRelease()` 信息
- `getSimpleEnvInfoSection` 注入完整的操作系统和 shell 信息
- 工具 prompt 中根据 OS 提供不同的命令建议

**我们的改进方向**:
1. 在 system prompt 中注入 `{os_type} {os_version}` 和当前 shell 类型
2. 在 `bash_command` 的 tool description 中说明当前平台

---

## 7. 优先级排序建议

### 7.1 立即可做（1-2 天）

1. **增强 system prompt 结构化** - 分离静态/动态 section
2. **注入 OS 信息到 system prompt** - 解决跨平台问题
3. **增强 write_file 错误提示** - 包含参数格式示例
4. **实现 TodoWrite 工具** - 核心任务跟踪

### 7.2 短期目标（1-2 周）

1. **Query Engine V2 状态机** - 显式状态管理
2. **实现 AskUserQuestion 工具** - 交互式提问
3. **实现 Skill 工具** - 技能调用入口
4. **Budget 控制增强** - max_budget_usd + task_budget
5. **Tool prompt 精细化** - 参考 Claude Code 的工具 prompt 工程

### 7.3 中期目标（1-2 月）

1. **Compaction Lifecycle V2** - 完整压缩生命周期
2. **Transcript V2** - compact boundary + fork lineage
3. **Background Task Runtime** - 完整后台任务模型
4. **Agent Fork** - fork subagent 支持
5. **MCP 动态刷新** - turn 间 MCP 重新连接

---

## 8. 结论

### 8.1 当前状态评估

| 维度 | 完成度 | 说明 |
|------|--------|------|
| **基础架构** | 85% | CLI/TUI/Headless/Remote 入口完整 |
| **Provider 支持** | 120% | 比 Claude Code 支持更多 provider |
| **工具集** | 55% | 核心工具齐全，缺少约 20 个增强工具 |
| **Query Engine** | 40% | 基础 loop 可用，缺少状态机和生命周期 |
| **System Prompt** | 30% | 功能性可用，缺少结构化和精细化 |
| **Compaction** | 45% | 算法丰富，缺少生命周期管理 |
| **Session/Transcript** | 60% | 存储完整，缺少语义层 |
| **MCP** | 50% | 基础连接可用，缺少动态刷新和认证 |
| **Agent/SubAgent** | 30% | 骨架存在，缺少 fork 和完整生命周期 |

### 8.2 核心结论

1. **CLI 稳定性已验证** - 压力测试证明 CLI 可以稳定长时间运行
2. **最大差距在运行时状态机** - 不是工具数量，而是 Query Engine 的复杂度
3. **System Prompt 工程是被低估的竞争力** - Claude Code 在 prompt 上的投入远超预期
4. **多 Provider 是我们的核心优势** - 必须保留并继续增强
5. **Tool Prompt 精细化直接影响模型表现** - 这是提升模型兼容性的关键杠杆

### 8.3 与 parity-program.md 的关系

本报告是 `plans/claude-cli-parity-program.md` 的补充文档。parity-program 定义了架构方向和实施路线，本报告提供了代码级的具体差距分析和优先级建议。两份文档应结合阅读。
