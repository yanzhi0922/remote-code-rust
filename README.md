# Remote Code Rust

高性能 Rust 实现的 AI 编码代理，兼容 Claude Code / OpenAI Codex 协议。

## 特性

- 🦀 **纯 Rust 实现** — 内存安全、零成本抽象、高性能异步运行时
- 🤖 **多 Provider 支持** — OpenAI、Anthropic、GLM/ZhipuAI、Bedrock、Vertex AI
- 🔧 **38+ 内置工具** — 文件操作、代码搜索、Web 搜索、LSP、后台任务、代理系统
- 🧠 **智能上下文管理** — 自动 token 估算和上下文压缩
- 🔒 **细粒度权限系统** — 规则引擎 + 通配符匹配
- 🏗️ **分布式架构** — Control Plane + Runner + WebSocket 流
- 🔌 **MCP 协议** — stdio/HTTP/WebSocket 三种传输
- 📦 **插件系统** — JSON-RPC stdio 协议
- 🧩 **Skills 系统** — Markdown frontmatter 技能发现
- 🤝 **多代理系统** — AgentScheduler + 并行执行 + 邮箱消息
- 💾 **记忆系统** — RC.md 持久化记忆（全局/项目双作用域）
- 🛡️ **沙箱执行** — 跨平台命令沙箱
- 📊 **成本追踪** — 多模型 token 使用统计
- ⚡ **流式响应** — SSE 流式 + 工具执行回调
- 🔍 **BM25 工具搜索** — 智能工具发现
- 🎯 **延迟工具加载** — 急切/延迟分离，优化上下文窗口
- 📡 **SSH 模式** — 远程主机执行
- ⌨️ **Vim 模式** — Normal/Insert 键绑定

## 架构

5 个应用 + 15 个库 crate：

- `remote-code` — 主 CLI（交互式/无头/远程模式）
- `remote-code-gui` — 桌面 GUI（Tauri v2 + React 19）
- `remote-code-control-plane` — 控制平面服务器
- `remote-code-runner` — Runner 代理
- `remote-code-migrate` — 数据迁移工具

库 crate：rc-core, rc-config, rc-provider, rc-tools, rc-permissions, rc-session, rc-mcp, rc-plugins, rc-skills, rc-agents, rc-runner, rc-control-plane, rc-tui, rc-telemetry, rc-protocol

## 内置工具（38+）

### 文件操作
- `read_file`, `write_file`, `edit_file`, `replace_in_file`, `list_directory`

### 搜索
- `search_text`, `glob`, `grep`, `lsp`（简化 LSP）

### 执行
- `bash_command`（带沙箱支持）

### Web
- `web_search`, `web_fetch`, `web_browser`

### 代理系统
- `agent`, `send_message`, `team_create`, `team_status`

### 任务管理
- `task_create`, `task_get`, `task_list`, `task_stop`, `task_update`, `todo_write`

### 记忆
- `memory_read`, `memory_write`

### 其他
- `ask_user`, `config_read`, `sleep`, `snip`, `skill_discover`, `tool_search`, `verify_plan`, `terminal_capture`, `notebook_edit`, `enter_plan_mode`, `exit_plan_mode`

## 快速开始

```bash
# 设置 API 密钥
export GLM_API_KEY=your_key_here

# 编译
cargo build --release

# 交互式 TUI 模式
cargo run --bin remote-code -- tui

# 桌面 GUI 模式
cd apps/remote-code-gui && npm install && npm run tauri dev

# 无头模式
echo "请帮我分析这个项目" | cargo run --bin remote-code -- headless

# Doctor 检查
cargo run --bin remote-code -- doctor
```

## 许可证

MIT
