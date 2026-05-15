# Remote Code GUI

基于 Tauri v2 + React 19 的桌面 GUI 客户端，为 [Remote Code Rust](../../README.md) 提供图形化界面。

## 技术栈

- **后端**: Tauri v2 (Rust)
- **前端**: React 19 + TypeScript 5.8 + Vite 7
- **样式**: Tailwind CSS
- **状态管理**: Zustand v5
- **Markdown 渲染**: react-markdown + KaTeX + highlight.js + GFM
- **虚拟化**: @tanstack/react-virtual

## 功能

- 📁 **多项目管理** — 添加/移除项目文件夹，按项目分组显示会话
- 💬 **多会话支持** — 在项目下创建、切换、管理多个对话会话
- 🤖 **三 Agent 引擎** — Claude Code (QueryEngine) / OpenAI Codex (AppServer) / Roo Code (26 Provider backends)
- ⚙️ **多 Provider 管理** — 添加/编辑/删除/切换多个 LLM Provider（OpenAI、Anthropic、GLM 等）
- 📝 **Markdown 渲染** — 支持代码高亮、数学公式（KaTeX）、GFM 表格
- 🔧 **工具调用折叠** — 思维链、工具调用、代码编辑可折叠展开
- 🛡️ **权限管理** — 5 种权限模式，工具执行权限弹窗，Shift+Tab 快捷切换
- 🎨 **IDE 级布局** — ActivityBar、SplitPane、StatusBar、Command Palette
- 🔍 **集成工具面板** — Terminal (xterm.js)、Diff Viewer、Preview Pane
- 📜 **对话级 Checkpoint** — 时间轴展示、Review/Restore/Undo 操作
- 🔀 **Git 面板** — Changes/History/Branches 三标签，Cmd+Enter 提交
- 🎯 **专业化 Agent** — @agent-name 引用下拉选择，5 个内置 Agent
- 🌐 **PWA / 移动端** — Tauri v2 移动构建目标（iOS / Android）

## 产品形态

桌面端以独立 Windows GUI 应用为主入口。安装包使用 Tauri NSIS 生成，安装后可从桌面快捷方式或开始菜单打开 `Remote Code`，Release 构建不会弹出额外控制台窗口。

远程控制不再要求用户手工设置环境变量。打开 GUI 后进入 `设置 -> 远程控制`：

1. 填写控制平面 URL。
2. Runner ID 可留空，应用会生成稳定 ID 并保存到本机配置目录。
3. 保持“随 GUI 自动启动远程服务”开启后，下次点击桌面快捷方式打开应用会自动连接控制平面。
4. 设置电脑端和手机端一致的用户名与配对密码后，手机 App 可以通过控制平面控制这台电脑上的本地 Agent。

## 开发

```bash
# 安装前端依赖
cd apps/remote-code-gui
npm install

# 开发模式（需要 Rust 工具链）
npm run desktop:dev

# 仅构建前端
npm run build

# 构建 Windows 桌面安装包
npm run desktop:build

# 类型检查
npx tsc --noEmit

# 运行前端测试
npm run test
```

## 安全约束

前端 `VITE_*` 环境变量只能承载公开配置，例如控制平面 URL；不得承载服务端信任凭据、访问令牌或 API Key。需要认证的控制平面请求应通过登录/配对得到的设备 token 或 Tauri 后端代理路径完成，避免把长期凭据写入浏览器 bundle。

## 项目结构

```
apps/remote-code-gui/
├── src/                    # React 前端
│   ├── components/
│   │   ├── agent/          # Agent 选择器
│   │   ├── agents/         # 专业化 Agent（AgentCard, AgentPicker 等）
│   │   ├── chat/           # 聊天区域组件
│   │   ├── checkpoint/     # Checkpoint 时间轴
│   │   ├── diff/           # Diff 查看器
│   │   ├── git/            # Git 面板
│   │   ├── layout/         # 布局组件（Header, Layout, Sidebar）
│   │   ├── mcp/            # MCP 管理
│   │   ├── panes/          # 工具面板（Terminal, Preview, PaneHost）
│   │   ├── permissions/    # 权限弹窗与模式切换
│   │   ├── prompt-input/   # 输入区域
│   │   ├── settings/       # 设置面板
│   │   ├── skills/         # Skills 管理
│   │   ├── tasks/          # 任务管理
│   │   ├── teams/          # 团队管理
│   │   └── ui/             # 基础 UI 组件
│   ├── lib/
│   │   ├── tauri.ts        # Tauri IPC 封装
│   │   ├── types.ts        # TypeScript 类型定义
│   │   ├── runtime.ts      # 运行时工具
│   │   └── utils.ts        # 工具函数
│   └── stores/
│       ├── useAppStore.ts  # Zustand 全局状态
│       ├── useAgentStore.ts # Agent 状态
│       └── useCodexStore.ts # Codex 状态
├── src-tauri/              # Tauri Rust 后端
│   ├── src/
│   │   └── lib.rs          # 核心 Tauri 命令
│   ├── Cargo.toml
│   └── tauri.conf.json
├── plugins/                # Tauri 本地插件（network, share）
├── package.json
├── tailwind.config.ts
├── tsconfig.json
└── vite.config.ts
```

## 推荐 IDE

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
