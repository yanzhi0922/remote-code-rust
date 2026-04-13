# P6B: 子任务委派系统 — 详细实施计划

> 优先级：Phase 6 第一个实施
> 借鉴：Roo-Code Task.ts + hermes-agent delegate_tool.py
> 目标：实现超越竞品的子任务委派系统

---

## 一、当前状态分析

### 现有代码

1. **[`agent_tool()`](crates/rc-tools/src/agent.rs:8)** — 基础子代理
   - 创建子对话，最多5轮
   - 支持工具白名单过滤
   - 使用 `SubAgentCompletion` trait 调用 LLM
   - 限制：无深度控制、无并行、无凭证轮换、无进度回调

2. **[`SubAgentCompletion`](crates/rc-core/src/lib.rs:515)** — 子代理完成 trait
   - `async fn complete(&self, conversation: &[ConversationEntry]) -> Result<ProviderResponse>`
   - 在 TUI 层通过 [`TuiSubAgent`](crates/rc-tui/src/lib.rs:53) 实现

3. **[`ToolExecutionContext`](crates/rc-tools/src/lib.rs:93)** — 工具执行上下文
   - `cwd`, `timeout_ms`, `sub_agent: Option<Arc<dyn SubAgentCompletion>>`

4. **[`AgentScheduler`](crates/rc-agents/src/lib.rs:1)** — 多代理调度器
   - 已有 mailbox 系统、预算追踪、生命周期事件
   - 但与 agent_tool 是独立的，没有集成

### 需要新增的功能

| 功能 | hermes-agent | Roo-Code | 我们的目标 |
|------|-------------|----------|-----------|
| 深度限制 | MAX_DEPTH=2 | 任务栈 | ✅ 可配置深度 |
| 并行执行 | ThreadPoolExecutor | N/A | ✅ tokio JoinSet |
| 阻止工具 | 5个阻止 | 子任务限制 | ✅ 可配置列表 |
| 凭证轮换 | 凭证池 | N/A | ✅ 多 key 轮换 |
| 进度回调 | 回调中继 | 委派事件 | ✅ UiEvent |
| 任务栈 | N/A | LIFO 栈 | ✅ LIFO 栈 |

---

## 二、实施步骤

### P6B-1: 新增 TaskStack（rc-core）

**文件**: `crates/rc-core/src/task_stack.rs`（新建）

```rust
//! Task stack for managing nested subtask delegation.

/// Maximum nesting depth for subtask delegation.
pub const DEFAULT_MAX_DEPTH: u32 = 3;

/// A frame on the task stack representing a paused parent task.
#[derive(Debug, Clone)]
pub struct TaskFrame {
    /// Unique identifier for this task.
    pub task_id: String,
    /// Conversation history snapshot at pause time.
    pub conversation_snapshot: Vec<ConversationEntry>,
    /// Tool calls that were pending when paused.
    pub pending_tool_calls: Vec<ToolCall>,
    /// Current depth in the delegation hierarchy.
    pub depth: u32,
    /// Parent task ID (None for root task).
    pub parent_task_id: Option<String>,
    /// Current state of this frame.
    pub state: TaskFrameState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFrameState {
    /// Task is actively running.
    Running,
    /// Task is paused waiting for a child to complete.
    PausedForChild,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed,
}

/// LIFO stack of task frames for nested delegation.
pub struct TaskStack {
    frames: Vec<TaskFrame>,
    max_depth: u32,
}

impl TaskStack {
    pub fn new(max_depth: u32) -> Self { ... }
    
    /// Push a new task frame onto the stack.
    /// Returns error if max depth exceeded.
    pub fn push(&mut self, frame: TaskFrame) -> Result<()> { ... }
    
    /// Pop the top frame (child completed).
    pub fn pop(&mut self) -> Option<TaskFrame> { ... }
    
    /// Peek at the current top frame.
    pub fn current(&self) -> Option<&TaskFrame> { ... }
    
    /// Get current depth.
    pub fn depth(&self) -> u32 { ... }
    
    /// Check if delegation is allowed at current depth.
    pub fn can_delegate(&self) -> bool { ... }
    
    /// Pause current frame and prepare for child.
    pub fn pause_for_child(&mut self, pending_calls: Vec<ToolCall>) -> Result<()> { ... }
    
    /// Resume parent after child completes.
    pub fn resume_parent(&mut self) -> Result<TaskFrame> { ... }
}
```

**修改**: `crates/rc-core/src/lib.rs` — 添加 `pub mod task_stack;`

**测试**: `task_stack_push_pop`, `task_stack_max_depth`, `task_stack_pause_resume`

---

### P6B-2: DelegationEngine（rc-tools）

**文件**: `crates/rc-tools/src/delegate.rs`（新建）

```rust
//! Enhanced subtask delegation engine with parallel execution support.

/// Default tools blocked for child agents.
pub const DEFAULT_BLOCKED_TOOLS: &[&str] = &[
    "agent",       // No recursive delegation by default
    "send_message", // No inter-agent messaging in subtasks
];

/// Configuration for task delegation.
pub struct DelegationConfig {
    /// Maximum concurrent child agents.
    pub max_concurrent: usize,
    /// Maximum delegation depth.
    pub max_depth: u32,
    /// Tools blocked for child agents.
    pub blocked_tools: Vec<String>,
    /// Maximum turns per child agent.
    pub max_turns: u32,
    /// Timeout per child agent in seconds.
    pub timeout_secs: u64,
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            max_depth: DEFAULT_MAX_DEPTH,
            blocked_tools: DEFAULT_BLOCKED_TOOLS.iter().map(String::from).collect(),
            max_turns: 10,
            timeout_secs: 120,
        }
    }
}

/// Context passed to child agents.
pub struct DelegationContext<'a> {
    /// The task description.
    pub task: &'a str,
    /// Parent's working directory.
    pub cwd: &'a Path,
    /// Parent conversation for context extraction.
    pub parent_conversation: &'a [ConversationEntry],
    /// Current depth.
    pub depth: u32,
    /// Allowed tools (empty = all non-blocked).
    pub allowed_tools: &'a [String],
}

/// Result from a delegated task.
pub struct DelegationResult {
    /// Task description.
    pub task: String,
    /// Final text output from the child agent.
    pub output: String,
    /// Whether the task succeeded.
    pub success: bool,
    /// Number of turns used.
    pub turns_used: u32,
    /// Tool calls made by the child.
    pub tool_trace: Vec<ToolTraceEntry>,
}

pub struct ToolTraceEntry {
    pub tool_name: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// The delegation engine manages subtask execution.
pub struct DelegationEngine {
    config: DelegationConfig,
}

impl DelegationEngine {
    pub fn new(config: DelegationConfig) -> Self { ... }
    
    /// Delegate a single task to a child agent.
    pub async fn delegate_single(
        &self,
        context: DelegationContext<'_>,
        executor: &dyn SubAgentCompletion,
        tool_context: &ToolExecutionContext,
        progress_cb: Option<Box<dyn Fn(DelegationProgress) + Send>>,
    ) -> Result<DelegationResult> { ... }
    
    /// Delegate multiple tasks in parallel using JoinSet.
    pub async fn delegate_batch(
        &self,
        tasks: &[DelegationContext<'_>],
        executor: &dyn SubAgentCompletion,
        tool_context: &ToolExecutionContext,
        progress_cb: Option<Box<dyn Fn(DelegationProgress) + Send>>,
    ) -> Result<Vec<DelegationResult>> { ... }
    
    /// Build a focused system prompt for the child agent.
    fn build_child_system_prompt(
        &self,
        task: &str,
        depth: u32,
        cwd: &Path,
    ) -> String {
        format!(
            "You are a sub-agent at depth {depth}. \
             Complete the following task concisely:\n\n\
             ## Task\n{task}\n\n\
             ## Workspace\n{cwd}\n\n\
             ## Rules\n\
             - Focus only on the assigned task\n\
             - Return results concisely\n\
             - Do not delegate to other agents\n\
             - Depth remaining: {remaining}",
            depth = depth,
            task = task,
            cwd = cwd.display(),
            remaining = self.config.max_depth - depth,
        )
    }
    
    /// Filter blocked tools from allowed list.
    fn filter_tools(&self, requested: &[String]) -> Vec<String> { ... }
}
```

**修改**: `crates/rc-tools/src/agent.rs` — 重写 `agent_tool()` 使用 DelegationEngine

新的 `agent_tool()` 实现：
```rust
pub(crate) async fn agent_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    // 1. 解析输入（prompt, tools, mode=single/batch）
    // 2. 检查 TaskStack 深度
    // 3. 创建 DelegationEngine
    // 4. 调用 delegate_single() 或 delegate_batch()
    // 5. 返回结果
}
```

**修改**: `crates/rc-tools/src/specs.rs` — 更新 agent tool schema 添加新参数：
```json
{
    "name": "agent",
    "input_schema": {
        "type": "object",
        "properties": {
            "prompt": {"type": "string"},
            "tools": {"type": "array", "items": {"type": "string"}},
            "mode": {"type": "string", "enum": ["single", "batch"]},
            "tasks": {
                "type": "array",
                "items": {"type": "string"},
                "description": "For batch mode: list of task descriptions"
            }
        },
        "required": ["prompt"]
    }
}
```

**修改**: `crates/rc-tools/src/lib.rs` — 添加 `pub mod delegate;`

---

### P6B-3: CredentialPool（rc-provider）

**文件**: `crates/rc-provider/src/credential_pool.rs`（新建）

```rust
//! Credential pool for rotating API keys across subtask executions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A single credential entry in the pool.
pub struct CredentialEntry {
    /// API key or token.
    pub api_key: String,
    /// Optional model override for this credential.
    pub model: Option<String>,
}

/// Round-robin credential pool for distributing requests.
pub struct CredentialPool {
    entries: Vec<CredentialEntry>,
    index: AtomicUsize,
}

impl CredentialPool {
    /// Create a pool from a list of API keys.
    pub fn from_keys(keys: Vec<String>) -> Self { ... }
    
    /// Create a single-entry pool (no rotation).
    pub fn single(api_key: String) -> Self { ... }
    
    /// Get the next credential using round-robin.
    pub fn next(&self) -> &CredentialEntry { ... }
    
    /// Number of credentials in the pool.
    pub fn len(&self) -> usize { ... }
    
    /// Check if pool is empty.
    pub fn is_empty(&self) -> bool { ... }
}
```

**修改**: `crates/rc-provider/src/lib.rs` — 添加 `pub mod credential_pool;`

---

### P6B-4: 进度回调（rc-ui-bridge）

**修改**: `crates/rc-ui-bridge/src/lib.rs` — 添加新的 UiEvent 变体

```rust
pub enum UiEvent {
    // ... existing variants ...
    
    /// A subtask delegation started.
    SubtaskStarted {
        task_id: String,
        description: String,
        depth: u32,
    },
    
    /// A subtask made progress.
    SubtaskProgress {
        task_id: String,
        turn: u32,
        max_turns: u32,
        summary: String,
    },
    
    /// A subtask completed.
    SubtaskCompleted {
        task_id: String,
        success: bool,
        output_preview: String,
        turns_used: u32,
    },
    
    /// Batch delegation progress update.
    BatchProgress {
        total: usize,
        completed: usize,
        running: usize,
    },
}
```

---

### P6B-5: TUI 子任务进度渲染（rc-tui）

**修改**: `crates/rc-tui/src/lib.rs` — 在状态栏显示子任务进度

在 TUI 底部状态栏添加子任务信息：
```
┌─ Subtasks: 2/3 complete ─── Running: "Fix tests in auth module" (turn 3/10) ─┐
```

---

### P6B-6: 集成测试

**文件**: `crates/rc-tools/src/lib.rs` tests section — 添加新测试

```rust
#[tokio::test]
async fn delegation_engine_single_task() {
    // Test single task delegation with mock provider
}

#[tokio::test]
async fn delegation_engine_batch_parallel() {
    // Test parallel batch delegation
}

#[tokio::test]
async fn delegation_engine_depth_limit() {
    // Test that depth limit is enforced
}

#[tokio::test]
async fn delegation_engine_blocked_tools() {
    // Test that blocked tools are filtered
}

#[tokio::test]
async fn task_stack_lifo_order() {
    // Test LIFO push/pop behavior
}

#[tokio::test]
async fn task_stack_max_depth() {
    // Test max depth enforcement
}

#[tokio::test]
async fn credential_pool_rotation() {
    // Test round-robin rotation
}
```

---

## 三、文件变更清单

| 操作 | 文件 | 说明 |
|------|------|------|
| **新建** | `crates/rc-core/src/task_stack.rs` | TaskStack + TaskFrame |
| **修改** | `crates/rc-core/src/lib.rs` | 添加 `pub mod task_stack;` |
| **新建** | `crates/rc-tools/src/delegate.rs` | DelegationEngine |
| **修改** | `crates/rc-tools/src/agent.rs` | 重写使用 DelegationEngine |
| **修改** | `crates/rc-tools/src/specs.rs` | 更新 agent tool schema |
| **修改** | `crates/rc-tools/src/lib.rs` | 添加 `pub mod delegate;` |
| **新建** | `crates/rc-provider/src/credential_pool.rs` | CredentialPool |
| **修改** | `crates/rc-provider/src/lib.rs` | 添加 `pub mod credential_pool;` |
| **修改** | `crates/rc-ui-bridge/src/lib.rs` | 添加 Subtask* UiEvent 变体 |

---

## 四、依赖关系

```
P6B-1 (TaskStack)     ← 无依赖，先做
P6B-2 (Delegation)    ← 依赖 P6B-1
P6B-3 (CredentialPool) ← 无依赖，可与 P6B-1 并行
P6B-4 (UiEvent)       ← 无依赖，可与 P6B-1 并行
P6B-5 (TUI 渲染)      ← 依赖 P6B-4
P6B-6 (集成测试)       ← 依赖 P6B-1~P6B-5
```

建议实施顺序：**P6B-1 + P6B-3 + P6B-4 并行 → P6B-2 → P6B-5 → P6B-6**

---

## 五、超越竞品的关键设计

1. **Rust tokio JoinSet** vs Python ThreadPoolExecutor — 零成本异步并行
2. **可配置深度** vs hermes-agent 硬编码 MAX_DEPTH=2
3. **批量模式** — hermes-agent 有但 Roo-Code 没有
4. **凭证轮换** — hermes-agent 有但 Roo-Code 没有
5. **进度回调** — 两者都有，我们的 UiEvent 更灵活
6. **工具追踪** — 记录每个子任务的工具使用，用于审计和优化
