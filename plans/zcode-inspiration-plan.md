# ZCode 启发分析与重构建议

> 基于对 https://zcode.z.ai/ 的深度研究，结合 remote-code-rust 现有架构的差距分析和重构建议。
> 更新日期: 2026-05-02

---

## 一、ZCode 核心创新点总结

ZCode 是智谱 AI (Z.AI) 推出的 Agent-First 开发环境（ADE），其核心创新：

1. **Agent-First UI** — 整个界面围绕 Agent 对话构建，而非在编辑器上叠加 AI
2. **对话级版本控制** — 每条消息创建 checkpoint，支持 Review/Undo/Restore
3. **专业化 Agent 系统** — 内置 bug-analyzer、code-reviewer、dev-planner 等，通过 `@` 引用
4. **多模型无缝切换** — GLM/Claude/GPT/Kimi/DeepSeek/自定义，每个对话可选模型
5. **Skills 技能系统** — Markdown 格式的可复用 Agent 行为剧本
6. **插件市场** — 6 种插件类型（Agent/Command/MCP/LSP/Skill/Hook）
7. **权限模式快捷切换** — Shift+Tab 循环切换 4 种权限模式
8. **内置 Git 面板** — 侧边栏完整 Git 管理
9. **移动端远程控制** — QR 码流式传输桌面会话到手机
10. **输出风格系统** — 5 种内置风格 + 自定义

---

## 二、差距分析：remote-code-rust vs ZCode

| 特性 | remote-code-rust 现状 | ZCode 实现 | 差距等级 |
|------|----------------------|-----------|---------|
| **对话级版本控制** | Phase 19 计划中 | ✅ 完整实现 (Review/Undo/Restore) | 🔴 关键缺失 |
| **专业化 Agent** | 无内置专业化 Agent | ✅ 5 个内置 + 自定义 | 🔴 关键缺失 |
| **Agent @引用** | 无 | ✅ `@agent-name` 触发 | 🔴 关键缺失 |
| **权限模式快捷切换** | 需进入设置 | ✅ Shift+Tab 即时切换 | 🟡 体验差距 |
| **内置 Git 面板** | 无 GUI Git 管理 | ✅ 完整侧边栏 Git | 🟡 体验差距 |
| **输出风格** | 无 | ✅ 5 种内置 + 自定义 | 🟡 体验差距 |
| **Prompt 增强** | 无 | ✅ 发送前自动优化 | 🟡 体验差距 |
| **插件市场 UI** | 插件系统有，市场无 | ✅ 完整市场 UI | 🟡 体验差距 |
| **移动端 QR 连接** | PWA 方案 | ✅ QR 码流式传输 | 🟢 已有替代 |
| **多模型切换** | ✅ 三引擎 26 Provider | ✅ 多 Provider | 🟢 已有优势 |
| **Rust 性能** | ✅ ~50ms 启动 | Electron 较慢 | 🟢 已有优势 |
| **远程执行** | ✅ Runner + Control Plane | ✅ SSH + Docker | 🟢 已有优势 |
| **MCP 支持** | ✅ stdio/HTTP/WS | ✅ 内置 + 自定义 | 🟢 已有优势 |
| **Skills 系统** | ✅ SKILL.md + TOML | ✅ Markdown + YAML | 🟢 已有优势 |
| **工具数量** | ✅ 62 内置工具 | ~20 工具 | 🟢 已有优势 |

---

## 三、重构建议：六大核心改进

### 改进 1：对话级版本控制（Conversation Checkpoint）

**ZCode 方案**：每条消息创建一个 checkpoint，支持：
- **Review**：查看任意消息后的所有文件变更（多文件 diff）
- **Undo**：仅撤销上次交互的变更
- **Restore**：跳回到聊天历史中任意消息后的状态

**我们的实现方案**：

#### 1.1 Rust 后端：Checkpoint Service

```rust
// crates/claude/claude-checkpoint/src/lib.rs

pub struct CheckpointService {
    db: Pool<Sqlite>,
    workspace_root: PathBuf,
}

/// 每条用户消息对应一个 checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub message_id: String,        // 关联的用户消息 ID
    pub message_index: usize,      // 消息在对话中的序号
    pub created_at: DateTime<Utc>,
    pub file_snapshots: Vec<FileSnapshot>,
    pub summary: String,           // 本次变更摘要
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,              // 相对于 workspace root
    pub hash_before: Option<String>, // 变更前 SHA256
    pub hash_after: Option<String>,  // 变更后 SHA256
    pub content_before: Option<String>,
    pub content_after: Option<String>,
    pub operation: FileOperation,  // Created / Modified / Deleted
}

pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}

impl CheckpointService {
    /// 在 Agent 执行前创建快照
    pub async fn create_pre_snapshot(&self, session_id: &str, message_id: &str) -> Result<CheckpointId>;
    
    /// 在 Agent 执行后完成快照（对比差异）
    pub async fn finalize_snapshot(&self, checkpoint_id: &CheckpointId) -> Result<Checkpoint>;
    
    /// 撤销到指定 checkpoint
    pub async fn restore_to(&self, checkpoint_id: &CheckpointId) -> Result<RestoreResult>;
    
    /// 撤销最后一次交互
    pub async fn undo_last(&self, session_id: &str) -> Result<RestoreResult>;
    
    /// 获取 diff 视图
    pub async fn get_diff(&self, checkpoint_id: &CheckpointId) -> Result<Vec<FileDiff>>;
    
    /// 列出会话所有 checkpoint
    pub async fn list_checkpoints(&self, session_id: &str) -> Result<Vec<CheckpointSummary>>;
}
```

#### 1.2 集成到 QueryEngine

在 `claude-query-engine` 的执行循环中，每次用户发送消息时：
1. **Pre-tool-execution**: 扫描 workspace 文件哈希
2. **Post-tool-execution**: 再次扫描，计算差异，存储为 checkpoint
3. **GUI 事件**: 发送 `checkpoint_created` 事件到前端

#### 1.3 GUI 前端

- Chat 消息气泡上增加 "⏮ Undo" 和 "📋 Review Changes" 按钮
- InspectorPanel 中增加 Checkpoint Timeline 视图
- 点击任意 checkpoint 可预览 diff 或恢复

---

### 改进 2：专业化 Agent 系统（Specialized Agents）

**ZCode 方案**：内置 5 个专业化 Agent，通过 `@agent-name` 引用，每个有独立 prompt 和工具权限。

**我们的实现方案**：

#### 2.1 Agent 定义格式

```markdown
<!-- ~/.remote-code/agents/code-reviewer.md -->
---
name: code-reviewer
description: 代码审查专家。用于审查 PR、检测安全漏洞、性能问题和生产可靠性。
model: inherit          # 继承当前会话模型，或指定 "claude-sonnet-4.5"
tools: [read_file, search_files, list_files, read_file]  # 只读工具
max_turns: 10
output_style: structural
---

你是一位资深代码审查专家。在审查代码时，请按以下维度分析：

1. **安全性** — 注入、XSS、认证绕过、硬编码密钥
2. **性能** — N+1 查询、内存泄漏、不必要的计算
3. **可靠性** — 错误处理、边界条件、竞态条件
4. **可维护性** — 命名、复杂度、重复代码

报告格式：
- 🔴 Critical（必须修复）
- 🟡 Warning（建议修复）
- 🟢 Info（改进建议）
```

#### 2.2 Rust 后端：Agent Registry

```rust
// crates/claude/claude-agents/src/registry.rs

pub struct SpecializedAgentRegistry {
    agents: HashMap<String, SpecializedAgent>,
}

pub struct SpecializedAgent {
    pub name: String,
    pub description: String,
    pub model_override: Option<String>,
    pub allowed_tools: Vec<String>,
    pub max_turns: Option<u32>,
    pub output_style: Option<String>,
    pub system_prompt: String,
    pub scope: AgentScope,  // BuiltIn / User / Project
}

impl SpecializedAgentRegistry {
    /// 从 ~/.remote-code/agents/ 和 .remote-code/agents/ 加载
    pub async fn discover() -> Result<Self>;
    
    /// 解析 @agent-name 引用
    pub fn resolve(&self, name: &str) -> Option<&SpecializedAgent>;
    
    /// 列出所有可用 Agent
    pub fn list_available(&self) -> Vec<&SpecializedAgent>;
}
```

#### 2.3 内置 Agent 预设

| Agent | 用途 | 工具权限 |
|-------|------|---------|
| `@code-reviewer` | 代码审查 | 只读 |
| `@bug-analyzer` | Bug 分析 | 只读 + 终端 |
| `@dev-planner` | 需求拆解 | 只读 |
| `@architect` | 架构设计 | 只读 |
| `@test-writer` | 测试生成 | 读写 + 终端 |

#### 2.4 GUI 集成

- ChatInput 中输入 `@` 弹出 Agent 选择器
- 消息中显示 Agent 标签（类似 ZCode 的彩色标签）
- Agent 执行结果折叠显示，可展开

---

### 改进 3：权限模式快捷切换

**ZCode 方案**：Shift+Tab 在 4 种模式间循环切换。

**我们的实现方案**：

在 ChatInput 组件中增加权限模式快速切换：

```typescript
// apps/remote-code-gui/src/components/chat/PermissionModeSwitch.tsx

const PERMISSION_MODES = [
  { id: 'always-ask', label: '🔒 Always Ask', desc: '每个操作都需确认' },
  { id: 'accept-edits', label: '✏️ Accept Edits', desc: '自动编辑文件，命令需确认' },
  { id: 'plan-mode', label: '📋 Plan Mode', desc: '先制定计划再执行' },
  { id: 'bypass', label: '⚡ Bypass', desc: '全自动无确认（仅沙箱）' },
] as const;

// Shift+Tab 快捷键循环切换
// 当前模式显示在 ChatInput 右下角
```

---

### 改进 4：内置 Git 面板

**ZCode 方案**：侧边栏完整 Git 管理（修改文件列表、一键提交、分支切换、历史浏览）。

**我们的实现方案**：

#### 4.1 Rust 后端：Git Operations

```rust
// crates/claude/claude-git/src/lib.rs

pub struct GitService {
    repo: Repository,
}

impl GitService {
    pub fn status(&self) -> Result<GitStatus>;
    pub fn stage(&self, paths: &[&str]) -> Result<()>;
    pub fn unstage(&self, paths: &[&str]) -> Result<()>;
    pub fn commit(&self, message: &str) -> Result<Oid>;
    pub fn branches(&self) -> Result<Vec<BranchInfo>>;
    pub fn switch_branch(&self, name: &str) -> Result<()>;
    pub fn log(&self, max_count: usize) -> Result<Vec<CommitInfo>>;
    pub fn diff_staged(&self) -> Result<Vec<FileDiff>>;
    pub fn diff_working(&self) -> Result<Vec<FileDiff>>;
}
```

#### 4.2 GUI 前端

在 ActivityBar 的侧边栏中增加 Git Tab：
- 修改文件列表（M/U 标记）
- Diff 预览（点击文件查看变更）
- 提交输入框 + 一键提交
- 分支切换下拉
- 提交历史列表

---

### 改进 5：输出风格系统

**ZCode 方案**：5 种内置风格 + 自定义 Markdown 定义。

**我们的实现方案**：

```markdown
<!-- ~/.remote-code/styles/coding-vibes.md -->
---
name: coding-vibes
description: 轻松编码风格，带 emoji 和口语化表达
---

你的回复风格：
- 使用 emoji 表达情绪 (🚀 ✨ 🔥 💡)
- 口语化但专业
- 代码示例简洁有力
- 遇到 bug 用幽默化解
```

在 `claude-system-prompt` 中注入当前风格的指令。

---

### 改进 6：Prompt 增强（Prompt Enhancement）

**ZCode 方案**：发送前自动优化 prompt。

**我们的实现方案**：

```rust
// crates/claude/claude-prompt-enhancer/src/lib.rs

pub struct PromptEnhancer {
    context: ProjectContext,  // 项目结构、语言、框架信息
}

impl PromptEnhancer {
    /// 增强用户 prompt
    /// - 补充项目上下文
    /// - 明确模糊需求
    /// - 添加相关文件引用
    pub async fn enhance(&self, raw_prompt: &str) -> Result<EnhancedPrompt>;
}
```

GUI 中在发送按钮旁增加 "✨ Enhance" 按钮，点击后显示增强后的 prompt 预览。

---

## 四、全新重构方案（如果选择全面重构）

如果选择全面重构，建议采用以下架构：

### 4.1 新架构：Agent-First Desktop Environment

```
┌──────────────────────────────────────────────────────────────────┐
│                        Remote Code Pro                           │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                    UI Layer (React + Tauri)                │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │  │
│  │  │  Agent   │ │  File    │ │   Git    │ │  Checkpoint  │  │  │
│  │  │  Chat    │ │  Tree    │ │  Panel   │ │  Timeline    │  │  │
│  │  │  (Main)  │ │          │ │          │ │              │  │  │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └──────┬───────┘  │  │
│  │       │             │            │              │          │  │
│  │  ┌────▼─────────────▼────────────▼──────────────▼───────┐  │  │
│  │  │              Inspector Panel                         │  │  │
│  │  │  Terminal │ Diff Viewer │ Preview │ Agent Output     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │              Agent Orchestration Layer                     │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  Agent Router                                        │  │  │
│  │  │  Claude Agent │ Codex Agent │ Roo Agent │ Custom    │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │  │
│  │  │ Checkpoint   │ │ Specialized  │ │ Prompt           │  │  │
│  │  │ Service      │ │ Agent Reg.   │ │ Enhancer         │  │  │
│  │  └──────────────┘ └──────────────┘ └──────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │              Core Runtime Layer (Rust)                     │  │
│  │  Provider │ Session │ Tools │ MCP │ Skills │ Plugins      │  │
│  │  Permissions │ Context │ Memory │ Git │ Telemetry         │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │              Remote Platform Layer                         │  │
│  │  Runner │ Control Plane │ WebSocket │ Mobile Bridge       │  │
│  └───────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 新增 Crate 结构

```
crates/
├── claude-checkpoint/        # NEW: 对话级版本控制
│   ├── src/lib.rs
│   ├── src/snapshot.rs       # 文件快照
│   ├── src/restore.rs        # 恢复逻辑
│   └── src/diff.rs           # Diff 计算
│
├── claude-specialized-agents/ # NEW: 专业化 Agent 系统
│   ├── src/lib.rs
│   ├── src/registry.rs       # Agent 注册表
│   ├── src/loader.rs         # Markdown Agent 加载
│   ├── src/builtin/          # 内置 Agent 定义
│   │   ├── code_reviewer.rs
│   │   ├── bug_analyzer.rs
│   │   ├── dev_planner.rs
│   │   ├── architect.rs
│   │   └── test_writer.rs
│   └── src/executor.rs       # Agent 执行器
│
├── claude-git/               # NEW: Git 操作封装
│   ├── src/lib.rs
│   ├── src/operations.rs
│   └── src/diff.rs
│
├── claude-prompt-enhancer/   # NEW: Prompt 增强
│   ├── src/lib.rs
│   └── src/context.rs
│
├── claude-output-styles/     # NEW: 输出风格系统
│   ├── src/lib.rs
│   ├── src/loader.rs
│   └── src/builtin/
│
└── rc-agent-protocol/    # UPGRADE: 增加 Specialized Agent 支持
    ├── src/lib.rs
    ├── src/agent_adapter.rs
    ├── src/specialized.rs    # NEW
    └── src/events.rs
```

### 4.3 GUI 新增组件

```
apps/remote-code-gui/src/components/
├── checkpoint/
│   ├── CheckpointTimeline.tsx    # Checkpoint 时间线
│   ├── CheckpointDiff.tsx        # Diff 预览
│   └── CheckpointActions.tsx     # Undo/Restore 按钮
│
├── agents/
│   ├── AgentPicker.tsx           # @ Agent 选择器
│   ├── AgentTag.tsx              # Agent 标签显示
│   └── AgentResult.tsx           # Agent 结果折叠面板
│
├── git/
│   ├── GitPanel.tsx              # Git 侧边栏面板
│   ├── GitStatusList.tsx         # 修改文件列表
│   ├── GitCommitInput.tsx        # 提交输入
│   ├── GitBranchSelector.tsx     # 分支选择
│   └── GitHistory.tsx            # 提交历史
│
├── chat/
│   ├── PermissionModeSwitch.tsx  # 权限模式快捷切换
│   ├── PromptEnhanceButton.tsx   # Prompt 增强按钮
│   ├── OutputStyleSelector.tsx   # 输出风格选择
│   └── MessageCheckpoint.tsx     # 消息上的 checkpoint 操作
```

---

## 五、实施优先级

### Phase 17（立即开始）：核心差异化功能

| 任务 | 工作量 | 影响 | 优先级 |
|------|--------|------|--------|
| 对话级版本控制 (Checkpoint) | 3-5 天 | 🔴 关键 | P0 |
| 专业化 Agent 系统 | 2-3 天 | 🔴 关键 | P0 |
| 权限模式快捷切换 | 0.5 天 | 🟡 体验 | P1 |
| 内置 Git 面板 | 2-3 天 | 🟡 体验 | P1 |

### Phase 18：体验增强

| 任务 | 工作量 | 影响 | 优先级 |
|------|--------|------|--------|
| 输出风格系统 | 1-2 天 | 🟡 体验 | P2 |
| Prompt 增强 | 1-2 天 | 🟡 体验 | P2 |
| 插件市场 UI | 3-5 天 | 🟡 体验 | P2 |

---

## 六、我们的独特优势（不应丢失）

在借鉴 ZCode 的同时，必须保持我们的核心优势：

1. **Rust 原生性能** — ~50ms 启动 vs Electron 的秒级启动
2. **三引擎独立适配器** — Claude/Codex/Roo 三条独立 in-process 路径
3. **26 Provider 后端** — 通过 Roo 适配器支持 26 个 Provider
4. **分布式远程执行** — Runner + Control Plane 完整链路
5. **62 内置工具** — 远超 ZCode 的工具数量
6. **Circuit Breaker + 故障转移** — 生产级可靠性
7. **PWA 移动端** — Web 技术栈的移动端方案
8. **Tauri 轻量桌面** — 比 Electron 更轻量、更安全

---

## 七、总结

ZCode 的核心洞察是 **"Agent-First"** — 开发者不需要另一个编辑器，他们需要的是一个与 AI Agent 高效协作的环境。这与我们的方向一致。

我们不需要变成 ZCode 的复制品。我们应该：

1. **吸收** ZCode 的 UX 创新（Checkpoint、专业化 Agent、Git 面板）
2. **保持** Rust 性能优势和三引擎架构
3. **超越** 在远程执行、工具丰富度、Provider 覆盖上的领先地位

最终目标是打造一个 **Rust 原生的 Agent-First 开发环境**，兼具 ZCode 的用户体验和我们已有的技术深度。