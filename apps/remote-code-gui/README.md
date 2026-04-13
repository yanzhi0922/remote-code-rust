# Remote Code GUI

基于 Tauri v2 + React 19 的桌面 GUI 客户端，为 [Remote Code Rust](../../README.md) 提供图形化界面。

## 技术栈

- **后端**: Tauri v2 (Rust)
- **前端**: React 19 + TypeScript + Vite
- **样式**: Tailwind CSS
- **状态管理**: Zustand
- **Markdown 渲染**: react-markdown + KaTeX + highlight.js

## 功能

- 📁 **多项目管理** — 添加/移除项目文件夹，按项目分组显示会话
- 💬 **多会话支持** — 在项目下创建、切换、管理多个对话会话
- ⚙️ **多 Provider 管理** — 添加/编辑/删除/切换多个 LLM Provider（OpenAI、Anthropic、GLM 等）
- 📝 **Markdown 渲染** — 支持代码高亮、数学公式（KaTeX）、GFM 表格
- 🔧 **工具调用折叠** — 思维链、工具调用、代码编辑可折叠展开
- 🛡️ **权限管理** — 5 种权限模式，工具执行权限弹窗
- 🎨 **暖色调 UI** — 统一的奶油色/米色设计语言

## 开发

```bash
# 安装前端依赖
cd apps/remote-code-gui
npm install

# 开发模式（需要 Rust 工具链）
npm run tauri dev

# 仅构建前端
npm run build

# 类型检查
npx tsc --noEmit
```

## 项目结构

```
apps/remote-code-gui/
├── src/                    # React 前端
│   ├── components/
│   │   ├── chat/           # 聊天区域组件
│   │   │   ├── ChatArea.tsx
│   │   │   ├── ChatInput.tsx
│   │   │   ├── CollapsibleBlock.tsx
│   │   │   ├── MarkdownRenderer.tsx
│   │   │   └── TaskTree.tsx
│   │   └── layout/         # 布局组件
│   │       ├── Header.tsx
│   │       ├── Layout.tsx
│   │       ├── PermissionModal.tsx
│   │       ├── SettingsPanel.tsx
│   │       └── Sidebar.tsx
│   ├── lib/
│   │   ├── tauri.ts        # Tauri IPC 封装
│   │   ├── types.ts        # TypeScript 类型定义
│   │   └── utils.ts        # 工具函数
│   └── stores/
│       └── useAppStore.ts  # Zustand 全局状态
├── src-tauri/              # Tauri Rust 后端
│   ├── src/
│   │   └── lib.rs          # 核心 Tauri 命令
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── tailwind.config.ts
├── tsconfig.json
└── vite.config.ts
```

## 推荐 IDE

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
