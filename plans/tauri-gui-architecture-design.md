# Remote Code: GUI 架构与设计方案 (Tauri + React)

## 1. 核心技术栈选型
基于现有 `remote-code-rust` 的全异步、多 Crate 架构，我们采用以下技术栈构建桌面客户端：

* **核心框架**: **Tauri (v2)**
  * 利用现有的 Rust 后端 (`rc-core`, `rc-agents`, `rc-ui-bridge` 等)。
  * 提供原生的跨平台桌面体验，极低的内存占用和极小的打包体积。
* **前端框架**: **React 18/19 (Vite) + TypeScript**
* **UI 组件与样式**:
  * **Tailwind CSS**: 原子化 CSS 工作流，便于快速迭代精细化的布局。
  * **shadcn/ui + Radix UI**: 提供无头 (Headless) 且可高度定制的现代化组件（如下拉菜单、极简阴影卡片、弹窗等），以完美复刻类似 Codex/Cursor 的半透明和深色/浅色主题。
* **状态管理**:
  * **Zustand**: 管理复杂的智能体状态、流式对话数据和任务树。
* **Markdown 与代码渲染**:
  * **React Markdown + Shiki**: 提供极快的语法高亮。
  * **差异渲染 (Diff Viewer)**: 定制化的并排/内联代码 Diff 组件。

---

## 2. 核心交互特性 (User Experience)

### 2.1 复杂任务的折叠与流式展示
现代 AI IDE 的核心在于“过程透明化”。系统不能仅仅返回一个最终的答案，而是需要展示整个推导与执行链：
* **任务树 (Task Tracker)**: 
  * 当用户下达复杂指令时，`dev-planner` 等智能体输出的执行计划会被渲染为带有 Checkbox 的任务列表。
  * 用户可以清晰看到进度（如 “共 5 个任务，已完成 2 个”）。
* **思考与动作流 (Agent Trace)**:
  * 终端执行的命令 (`$ python script.py`) 会以暗色代码块的形式内联展示。
  * 智能体的内部思考过程 (Thought Process) 可以折叠隐藏，保持界面清爽，开发者在需要 Review 逻辑时点击展开。
* **代码修改与 Diff 预览**:
  * 在文件被实际修改前或修改后，渲染一个卡片展示该文件的 Diff 统计（如绿色的 `+1177`, 红色的 `-1`）。
  * 点击该片段即可展开一个内联的 Diff UI，开发者可以逐行审核变更，并支持一键 `Approve` (接受) 或 `Reject` (拒绝)。

### 2.2 多智能体的协作交互 (@ Tag Agent)
* **智能体调度器 (Agent Router)**: 在聊天输入框中键入 `@` 时，激活弹出菜单（类似 GitHub Copilot 的 `@workspace` 或 `@terminal`），展示可用的专精智能体。
  * `@designer`: 负责 UI 设计与前端代码生成。
  * `@bug-analyzer`: 负责深度的错误排查和调用栈分析。
  * `@code-reviewer`: 负责静态分析和安全审计。
* **可见的协作状态**: 
  * 当一个智能体呼叫另一个智能体时（如 Planner 将任务委托给 Coder），UI 上会明确显示这种“接力”过程，如 `Planner 正在等待 Coder 的结果...`。

---

## 3. 界面布局规划 (Layout Structure)

### 区域 A：左侧导航活动栏 (Sidebar)
* **顶网区 (Top)**: `新建线程` (新建会话), `全局搜索`, `技能 (Skills)`, `插件 (Plugins)`。
* **线程列表 (Threads List)**: 按项目文件夹分组显示历史对话及时间戳。
* **底网区 (Bottom)**: 全局设置、提示词编辑入口。

### 区域 B：顶部状态与控制栏 (Top Header)
* **左侧**: 当前会话标题。
* **右侧工具流**: 全局暂停/继续按钮、VS Code / IDE 外部编辑器跳转按钮、全局代码修改统计条。

### 区域 C：核心流式对话区 (Main Chat / Trace Area)
* 瀑布流式的交互区域，交替出现：
  * 用户的 Prompt 卡片。
  * 智能体的 Task List 卡片。
  * 终端命令的执行状态 (Loading, Success, Failed)。
  * 折叠的代码 Diff 面板。

### 区域 D：底部输入与上下文栏 (Input & Context Footer)
* **输入框**: 支持自动扩展高度的富文本框，支持文件拖拽附加上下文。
* **智能体配置器**: 底部悬浮条显示当前主导的 Agent（如 `CodexManager`）及所用底层大模型引擎。
* **环境状态条 (Status Bar)**: 
  * 文件权限指示器 (`完全访问权限` vs `沙盒隔离`)。
  * 提取并显示当前的 Git 分支状态。

---

## 4. 与 Rust 后端 (remote-code-rust) 的架构对接

Tauri 提供了一个 `IPC (Inter-Process Communication)` 桥梁。我们将充分利用现有的 Crate：

1. **`rc-ui-bridge` (前端通信层)**: 
   * 将作为 Tauri 的 `#[tauri::command]` 暴露点。
   * 提供 API 如 `send_prompt(thread_id, prompt, agent_tags)`、`approve_diff(diff_id)`。
   * **SSE / Tauri Events**: 后端执行是长时间运行的，不能阻塞前端。`rc-core` 或 `rc-session` 产生的事件通过 `rc-event-bus` (mpsc channel/broadcast) 路由到 `rc-ui-bridge`，转为 Tauri 的 `Window::emit` 推送给前端（如 `TaskProgressUpdated`, `AgentThoughtEmitted`, `NewDiffAvailable`）。
2. **`rc-agents` (多智能体层)**: 
   * 响应前端通过 `@` 标签发来的定向请求，或者由默认 Router 自动分发任务。
3. **`rc-permissions` (安全控制层)**:
   * 支持拦截敏感操作文件（修改配置、运行高危脚本），以阻塞状态推送到前端，等待用户在 UI 上点击 `接受` 后方可放行。

---

## 5. 第一阶段实施路径 (Phase 1 Roadmap)

1. **初始化工程**: 在 `apps/` 目录下创建一个新的 `remote-code-gui` Tauri 工程骨架（结合 React + Vite）。
2. **连接桥梁**: 打通前端 `invoke` 与后端的第一个测试联调（例如发送 "Hello" 并接收模型返回）。
3. **实现核心 UI**: 搭建基础的 Sidebar 布局与底部 Markdown 输入框。
4. **建立事件流 (Event Stream)**: 实现后端状态到前端 Zustand store 的单向数据流推送。 
5. **Diff 渲染与 Agent UI**: 逐步封装折叠面板、代码高亮、和基于 `@` 的智能体选择器组件。
