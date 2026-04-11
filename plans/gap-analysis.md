# Remote Code Rust — 全面差距分析报告

> 生成时间: 2026-04-11 02:38 CST
> 基于对 remote-code-rust、原始 remote-code (TypeScript)、claude-code-best、claw-code-parity、codex、shanraisshan/claude-code-best-practice、Rangizingo/cc-cache-fix 的综合研究

---

## 一、当前可用性评估

### 现在能用吗？

**部分可用，但不可用于生产。** 具体来说：

| 维度 | 状态 | 说明 |
|------|------|------|
| 编译 | ✅ 可编译 | 147 个测试全部通过，clippy 无警告 |
| CLI 框架 | ✅ 完整 | 4 个二进制目标，完整的 clap 命令树 |
| Provider 调用 | ⚠️ 基础可用 | 支持 OpenAI/Anthropic 同步+流式，但缺少重试/缓存优化 |
| 工具系统 | ⚠️ 最小可用 | 仅 7 个内置工具，缺少 40+ 关键工具 |
| 交互式 TUI | ❌ 不可用 | 仅 87 行占位代码，无法实际交互 |
| MCP 客户端 | ✅ 可用 | stdio/HTTP/WebSocket 三种传输 |
| 插件系统 | ✅ 可用 | JSON-RPC stdio 协议完整 |
| 会话管理 | ✅ 可用 | SQLite 持久化，支持导出/恢复 |
| Control Plane | ✅ 可用 | Runner/Session/Approval/Artifact/Event 全链路 |
| 权限系统 | ⚠️ 基础可用 | 5 种模式，但缺少细粒度规则匹配 |
| Hook 系统 | ✅ 可用 | 4 种事件类型，支持 profile/workspace/plugin 三级发现 |

**结论：** 项目可以作为 Control Plane + Runner 的远程执行基础设施使用，但作为终端用户的交互式编码代理（类似 Claude Code 的体验）尚不可用，主要瓶颈是 TUI 和工具数量。

---

## 二、工具系统差距分析

### 当前内置工具（7 个）

| 工具 | 协议名 | 权限 | 状态 |
|------|--------|------|------|
| `list_directory` | ListDirectory | Read | ✅ 完整 |
| `read_file` | ReadFile | Read | ✅ 完整 |
| `search_text` | SearchText | Read | ✅ 完整 |
| `write_file` | WriteFile | Edit | ✅ 完整 |
| `replace_in_file` | ReplaceInFile | Edit | ✅ 完整 |
| `edit_file` | EditFile | Edit | ✅ 完整 |
| `bash_command` | Bash | Bash | ✅ 完整 |

### 上游 Claude Code 内置工具（55+）

原始 remote-code 的 `src/tools/` 目录包含以下工具模块，按优先级分为三个层级：

#### P0 — 核心缺失工具（严重影响可用性）

| 工具 | 上游路径 | 说明 | 实现难度 |
|------|----------|------|----------|
| **GlobTool** | `GlobTool/` | Glob 模式文件搜索，`find` 的高效替代 | 低 |
| **GrepTool** | `GrepTool/` | 正则文件内容搜索（ripgrep 风格），比 search_text 更强大 | 中 |
| **AgentTool** | `AgentTool/` | 子代理生成，支持 Task 代理和自定义代理 | 高 |
| **WebSearchTool** | `WebSearchTool/` | 网络搜索 | 中 |
| **WebFetchTool** | `WebFetchTool/` | URL 内容获取 | 低 |
| **AskUserQuestionTool** | `AskUserQuestionTool/` | 向用户提问获取澄清 | 低 |
| **TodoWriteTool** | `TodoWriteTool/` | 任务追踪列表管理 | 低 |
| **ConfigTool** | `ConfigTool/` | 运行时配置修改 | 中 |

#### P1 — 重要增强工具

| 工具 | 上游路径 | 说明 | 实现难度 |
|------|----------|------|----------|
| **LSPTool** | `LSPTool/` | 语言服务器协议集成（定义跳转、引用查找） | 高 |
| **NotebookEditTool** | `NotebookEditTool/` | Jupyter notebook 编辑 | 中 |
| **TaskCreateTool** | `TaskCreateTool/` | 后台任务创建 | 中 |
| **TaskGetTool** | `TaskGetTool/` | 获取任务状态 | 低 |
| **TaskListTool** | `TaskListTool/` | 列出所有任务 | 低 |
| **TaskStopTool** | `TaskStopTool/` | 停止后台任务 | 低 |
| **TaskUpdateTool** | `TaskUpdateTool/` | 更新任务状态 | 低 |
| **SkillTool** | `SkillTool/` | 运行已安装的 Skill | 中 |
| **DiscoverSkillsTool** | `DiscoverSkillsTool/` | 发现可用 Skills | 低 |
| **ToolSearchTool** | `ToolSearchTool/` | BM25 工具搜索 | 中 |
| **SendMessageTool** | `SendMessageTool/` | 跨代理消息传递 | 中 |
| **EnterPlanModeTool** | `EnterPlanModeTool/` | 进入计划模式 | 低 |
| **ExitPlanModeTool** | `ExitPlanModeTool/` | 退出计划模式 | 低 |

#### P2 — 高级/专业工具

| 工具 | 上游路径 | 说明 |
|------|----------|------|
| TeamCreateTool | `TeamCreateTool/` | 创建多代理团队 |
| TeamDeleteTool | `TeamDeleteTool/` | 删除团队 |
| MonitorTool | `MonitorTool/` | 监控代理执行 |
| ScheduleCronTool | `ScheduleCronTool/` | 定时任务 |
| RemoteTriggerTool | `RemoteTriggerTool/` | 远程触发 |
| WorkflowTool | `WorkflowTool/` | 工作流编排 |
| PowerShellTool | `PowerShellTool/` | 原生 PowerShell 执行 |
| REPLTool | `REPLTool/` | REPL 交互 |
| TerminalCaptureTool | `TerminalCaptureTool/` | 终端截图 |
| SleepTool | `SleepTool/` | 延迟等待 |
| SnipTool | `SnipTool/` | 代码片段 |
| BriefTool | `BriefTool/` | 上下文摘要 |
| VerifyPlanExecutionTool | `VerifyPlanExecutionTool/` | 计划验证 |
| EnterWorktreeTool | `EnterWorktreeTool/` | Git worktree 切换 |
| ExitWorktreeTool | `ExitWorktreeTool/` | 退出 worktree |
| SuggestBackgroundPRTool | `SuggestBackgroundPRTool/` | 后台 PR 建议 |
| TungstenTool | `TungstenTool/` | 高级执行 |
| WebBrowserTool | `WebBrowserTool/` | 浏览器自动化 |
| McpAuthTool | `McpAuthTool/` | MCP 认证 |
| ListMcpResourcesTool | `ListMcpResourcesTool/` | MCP 资源列表 |
| ReadMcpResourceTool | `ReadMcpResourceTool/` | 读取 MCP 资源 |
| ListPeersTool | `ListPeersTool/` | 列出对等代理 |
| CtxInspectTool | `CtxInspectTool/` | 上下文检查 |

---

## 三、架构层面差距

### 3.1 TUI 系统 — 严重缺失

当前 [`rc-tui`](crates/rc-tui/src/lib.rs) 仅 87 行，是一个基本的 `io::stdin().read_line()` 循环。

**上游 Claude Code 的 TUI 特性：**
- React Ink 终端渲染框架
- 完整的交互式界面：输入框、工具调用展示、流式输出
- Vim 模式支持
- 快捷键系统
- 语音输入支持
- 模态对话框（权限确认、工具调用审批）
- 多面板布局
- 颜色/样式系统
- overlay 上下文

**建议方案：** 使用 `ratatui` + `crossterm` 构建异步 TUI，或集成 `rustyline` 作为最小可行方案。

### 3.2 上下文窗口管理 — 完全缺失

上游有完整的上下文管理策略：
- **Token 估算**：粗估算 + 精确计算
- **自动压缩**：当上下文接近阈值时自动摘要
- **工具输出截断**：大输出自动截断，保留在 JSONL 中但不发送给 API
- **延迟工具**：非必要工具延迟加载
- **缓存前缀优化**：稳定 cache key

**当前状态：** 无任何上下文管理。对话会无限增长直到 API 报错。

### 3.3 沙箱/安全执行 — 完全缺失

**上游 Claude Code：**
- macOS Seatbelt 策略
- Linux Landlock 沙箱
- Windows 沙箱限制
- 网络隔离模式

**上游 Codex (Rust)：**
- `codex-rs/sandboxing/` 完整的跨平台沙箱
- macOS: `sandbox-exec` + SBPL 策略
- Linux: seccomp + Landlock
- 网络代理路由
- 文件系统访问控制

**当前状态：** `bash_command` 工具直接执行，无任何沙箱保护。

### 3.4 缓存优化 — 完全缺失

**Rangizingo/cc-cache-fix 揭示的关键问题：**
- Anthropic API 的 cache 前缀在 resume 时会断裂
- `deferred_tools_delta` 导致缓存失效
- 5 分钟默认 TTL 太短
- sentinel 替换 bug 导致缓存 key 不稳定

**当前状态：** 无任何 API 缓存策略，每次请求都是全量发送。

### 3.5 子代理系统 — 基础框架存在但不可用

**上游 Claude Code AgentTool：**
- 5 种内置代理类型（general-purpose, explore 等）
- 自定义代理发现（`.claude/agents/` 目录）
- 代理群（swarm）并行执行
- tmux 会话隔离
- 邮箱消息传递
- 工具白名单限制
- 独立权限追踪

**当前 [`rc-agents`](crates/rc-agents/src/lib.rs)：**
- ✅ `AgentScheduler` 基础调度器
- ✅ 任务分配、邮箱、预算
- ❌ 无实际子代理执行（无 `AgentTool`）
- ❌ 无并行执行
- ❌ 无进程隔离
- ❌ 无工具白名单

### 3.6 记忆系统 — 完全缺失

**上游 CLAUDE.md 记忆系统：**
- 全局 `~/.claude/CLAUDE.md`
- 项目级 `.claude/CLAUDE.md`
- 自动读写工具
- Read/Write/Edit 工具自动启用

**当前状态：** 无任何持久化记忆机制。

---

## 四、与各项目的具体差距

### 4.1 vs 原始 remote-code (TypeScript)

| 维度 | 上游 | 我们 | 差距 |
|------|------|------|------|
| 源文件规模 | ~50 个子目录 | 15 个 crate | 架构更清晰但功能更少 |
| 内置工具 | 55+ | 7 | **差 48+ 工具** |
| TUI | React Ink 完整 UI | 87 行占位 | **完全缺失** |
| 子代理 | AgentTool + swarms | 基础调度器 | **不可用** |
| 上下文管理 | 压缩 + token 估算 | 无 | **完全缺失** |
| 沙箱 | 跨平台沙箱 | 无 | **完全缺失** |
| Provider | OpenAI + Anthropic + Bedrock + Vertex | OpenAI + Anthropic | 缺 Bedrock/Vertex |
| 缓存 | cache prefix + TTL | 无 | **完全缺失** |
| 成本追踪 | `cost-tracker.ts` | 无 | **完全缺失** |
| SSH | `ssh/` 目录 | 无 | 缺失 |
| Daemon | `daemon/` 目录 | 无 | 缺失 |
| 语音 | `voice/` 目录 | 无 | 缺失 |
| Vim 模式 | `vim/` 目录 | 无 | 缺失 |

### 4.2 vs claw-code-parity

claw-code-parity 的 PARITY.md 显示 40/40 工具规格对齐，但许多是 stub 实现。我们的差距类似但更极端——我们只实现了 7 个工具的实际执行逻辑。

**claw-code-parity 的优势：**
- 更完整的工具规格覆盖
- 明确的 stub 标记
- PARITY.md 跟踪每个工具的实现状态

**我们的优势：**
- Rust 性能
- 更好的模块化（15 crate 分离）
- 完整的 Control Plane + Runner 基础设施
- 完整的测试覆盖（147 个测试）

### 4.3 vs Codex (OpenAI, Rust)

Codex 是最接近我们的参考实现（同为 Rust）。

| 维度 | Codex | 我们 | 差距 |
|------|-------|------|------|
| 沙箱 | 完整跨平台 | 无 | **关键差距** |
| 工具注册 | BM25 搜索 + 动态发现 | 静态列表 | 缺动态发现 |
| SDK | Python/TypeScript SDK | 无 | 缺 SDK |
| MCP 集成 | App Server Protocol | 基础客户端 | 更完整 |
| 会话管理 | resume by ID/name | 基础 resume | 类似 |
| macOS Seatbelt | 完整 SBPL 策略 | 无 | **完全缺失** |
| Linux seccomp | 完整 | 无 | **完全缺失** |

### 4.4 vs claude-code-best 文档

claude-code-best 提供了最详细的架构文档，揭示了以下我们缺失的关键特性：

1. **`assembleToolPool()`** — 内置工具 + MCP 工具合并去重
2. **`buildTool()`** — 统一工具构建管道
3. **权限内容过滤器** — `Bash(prefix:git)`, `FileEdit(path:src/)` 细粒度规则
4. **`yoloClassifier`** — 自动模式下的智能权限决策
5. **`localDenialTracking`** — 子代理独立权限追踪
6. **延迟工具** — 非必要工具延迟加载以节省上下文
7. **流式工具执行** — 工具执行结果流式返回

---

## 五、代码质量问题

### 5.1 已修复的问题
- ✅ main.rs 从 5,651 行拆分到 1,491 行
- ✅ rc-control-plane 从 5,112 行拆分到 6 个模块
- ✅ Python mock 脚本 Windows 兼容性
- ✅ python_command() 健壮性
- ✅ Clippy 警告清零

### 5.2 仍存在的问题

| 问题 | 位置 | 严重性 |
|------|------|--------|
| `rc-tui` 仅占位代码 | `crates/rc-tui/src/lib.rs` | 🔴 严重 |
| `rc-protocol` crate 内容不明 | `crates/rc-protocol/src/lib.rs` | 🟡 需确认 |
| `rc-telemetry` 仅 16 行 | `crates/rc-telemetry/src/lib.rs` | 🟡 需增强 |
| 部分模块缺少集成测试 | 多个 crate | 🟡 中等 |
| 无基准测试 | 全项目 | 🟢 低 |
| 无模糊测试 | rc-tools/rc-mcp | 🟢 低 |

### 5.3 依赖风险

| 依赖 | 版本风险 | 说明 |
|------|----------|------|
| `reqwest` | 低 | 稳定维护 |
| `tokio` | 低 | 稳定维护 |
| `serde_json` | 低 | 极度稳定 |
| `rusqlite` | 中 | 需关注 bundled feature |
| `axum` | 低 | 活跃维护 |
| `tokio-tungstenite` | 低 | 稳定 |
| `ratatui` | 中 | API 演变中（如果采用） |

---

## 六、按优先级排列的改进路线

### Phase 1 — 达到最小可用产品 (MVP)

> 目标：用户可以在终端中与 AI 进行交互式对话，执行基本编码任务

1. **实现交互式 TUI** — 使用 `rustyline` 或 `ratatui`
2. **添加 GlobTool** — glob 文件搜索
3. **添加 GrepTool** — 正则搜索（可封装 `grep`/`ripgrep`）
4. **添加 AskUserQuestionTool** — 用户交互
5. **实现上下文窗口管理** — token 估算 + 自动压缩
6. **连接对话循环** — provider → tool_call → execute → provider 的完整循环

### Phase 2 — 达到日常可用

> 目标：开发者可以日常使用进行编码辅助

7. **添加 AgentTool** — 子代理生成
8. **添加 WebSearchTool + WebFetchTool**
9. **添加 TodoWriteTool**
10. **实现细粒度权限规则** — `Bash(prefix:git)` 风格匹配
11. **添加缓存优化** — Anthropic cache prefix 稳定化
12. **实现沙箱执行** — 至少 macOS Seatbelt + Linux 基础
13. **成本追踪** — token 使用量统计

### Phase 3 — 达到功能对齐

> 目标：与上游 Claude Code 功能基本对齐

14. **添加 LSPTool** — 语言服务器集成
15. **添加后台任务系统** — TaskCreate/Get/List/Stop/Update
16. **添加 NotebookEditTool**
17. **实现代理群** — 并行多代理执行
18. **添加记忆系统** — CLAUDE.md 读写
19. **添加 Provider 支持** — Bedrock + Vertex
20. **添加 SSH 模式**
21. **添加 Vim 模式**

### Phase 4 — 高级特性

22. **工具搜索** — BM25 动态发现
23. **延迟工具加载**
24. **流式工具执行**
25. **语音输入**
26. **Daemon 模式**
27. **SDK** — Python/TypeScript 绑定

---

## 七、功能差距总览图

```mermaid
graph LR
    subgraph 已实现
        A1[7 个基础工具]
        A2[OpenAI/Anthropic Provider]
        A3[MCP 客户端]
        A4[插件系统]
        A5[会话管理]
        A6[Control Plane]
        A7[Runner]
        A8[Hook 系统]
        A9[权限模式]
        A10[Skills 发现]
    end

    subgraph P0_缺失[P0 - 严重缺失]
        B1[交互式 TUI]
        B2[GlobTool]
        B3[GrepTool]
        B4[AgentTool]
        B5[上下文管理]
        B6[对话循环]
    end

    subgraph P1_缺失[P1 - 重要缺失]
        C1[Web 搜索/获取]
        C2[沙箱执行]
        C3[缓存优化]
        C4[细粒度权限]
        C5[成本追踪]
        C6[LSPTool]
        C7[后台任务]
    end

    subgraph P2_缺失[P2 - 增强缺失]
        D1[代理群]
        D2[记忆系统]
        D3[Notebook]
        D4[Bedrock/Vertex]
        D5[SSH 模式]
        D6[Vim 模式]
        D7[语音输入]
    end

    P0_缺失 -->|阻塞 MVP| 已实现
    P1_缺失 -->|限制日常使用| P0_缺失
    P2_缺失 -->|限制高级场景| P1_缺失
```

---

## 八、与外部项目的差异化定位

| 项目 | 语言 | 定位 | 我们的关系 |
|------|------|------|-----------|
| remote-code (TS) | TypeScript | 完整的 Claude Code 实现 | 上游参考 |
| claw-code-parity | TypeScript | 工具对齐追踪 | 规格参考 |
| codex | Rust | OpenAI 官方 CLI | 沙箱/架构参考 |
| claude-code-best | 文档 | 架构深度分析 | 知识参考 |
| shanraisshan | 文档 | 最佳实践 | 权限/Hook 参考 |
| cc-cache-fix | Python | 缓存 bug 修复 | 缓存策略参考 |
| **remote-code-rust** | **Rust** | **高性能 Rust 实现** | **我们** |

**我们的独特优势：**
1. Rust 性能和内存安全
2. 完整的 Control Plane + Runner 分布式架构（上游没有）
3. 清晰的 15 crate 模块化设计
4. 全面的测试覆盖
5. 编译时类型安全

---

## 九、结论

remote-code-rust 目前是一个**架构完整但功能稀疏**的项目。基础设施层（Provider、MCP、Plugin、Session、Control Plane、Runner）已经相当成熟，但面向终端用户的核心体验层（TUI、工具、上下文管理、沙箱）几乎为零。

**最关键的三个阻塞项：**
1. 交互式 TUI — 没有它用户无法使用
2. 对话循环 — provider→tool→provider 的完整循环需要在 TUI 中串联
3. 上下文窗口管理 — 没有它对话会在几轮后崩溃

建议按照 Phase 1 → Phase 2 → Phase 3 的顺序推进，每个 Phase 结束后进行可用性评估。
