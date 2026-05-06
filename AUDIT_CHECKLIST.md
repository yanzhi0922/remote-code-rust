# Remote-Code-Rust 全面无死角审计清单

> 生成日期: 2026-05-04
> 代码库规模: 5,079 .rs 文件 / 1,934 .ts/.tsx 文件 / 388 前端测试文件
> 核心文件: lib.rs (7,264行), RemoteApp.tsx (1,381行), useAppStore.ts (1,037行)

---

## 目录

- [1. 安全性 (Security)](#1-安全性-security)
- [2. 错误处理 (Error Handling)](#2-错误处理-error-handling)
- [3. 类型安全 (Type Safety)](#3-类型安全-type-safety)
- [4. 测试覆盖 (Test Coverage)](#4-测试覆盖-test-coverage)
- [5. 性能 (Performance)](#5-性能-performance)
- [6. 无障碍访问 (Accessibility)](#6-无障碍访问-accessibility)
- [7. 国际化 (i18n)](#7-国际化-i18n)
- [8. 架构与重构 (Architecture)](#8-架构与重构-architecture)
- [9. 日志与可观测性 (Observability)](#9-日志与可观测性-observability)
- [10. 配置与环境变量 (Configuration)](#10-配置与环境变量-configuration)
- [11. 前端质量 (Frontend Quality)](#11-前端质量-frontend-quality)
- [12. Rust 代码质量 (Rust Code Quality)](#12-rust-代码质量-rust-code-quality)
- [13. 依赖与构建 (Dependencies & Build)](#13-依赖与构建-dependencies--build)
- [14. 文档与注释 (Documentation)](#14-文档与注释-documentation)
- [15. 已完成项 (Completed)](#15-已完成项-completed)

---

## 1. 安全性 (Security)

### 1.1 生产环境中的 `unwrap()` / `panic!` (SEVERITY: HIGH)

- [x] **S-01**: `session_id.as_ref().unwrap()` 已消除 — 替换为直接绑定值
- [x] **S-02**: Runner中4处 `panic!()` — 全部在 `#[cfg(test)]` 模块中，非生产代码
- [ ] **S-03**: 全面审查 `unwrap()` 调用 (全库 3,366 处) — 非测试代码中的 `.unwrap()` 都应替换为 `.ok_or_else()`, `.unwrap_or_default()`, 或 `?` 操作符
- [ ] **S-04**: workspace已配置 `unwrap_used = "warn"` — adapters/runner/GUI 已确认零警告，可安全升级为 `deny`（codex/* 子crate除外）

### 1.2 密钥与敏感信息 (SEVERITY: HIGH)

- [ ] **S-05**: `mobile_secure_store_set` 将密钥以明文JSON存储 — 需要平台密钥链集成 (iOS Keychain / Android Keystore)
- [ ] **S-06**: `api_key: Option<String>` 在多个组件间传递 — 审查日志是否可能泄露API key (tracing不应打印包含key的字段)
- [ ] **S-07**: `REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN` 环境变量 — 确认不在日志或错误消息中暴露
- [ ] **S-08**: 前端 `VITE_REMOTE_CONTROL_PLANE_TOKEN` — Vite环境变量会打包进客户端，确认这个token不应暴露给客户端

### 1.3 路径遍历与SSRF (SEVERITY: MEDIUM)

- [x] **S-09**: `mobile.rs` 的 `validate_download_name()` — 已修复路径遍历 (拒绝 `..`, `/`, `\`, 绝对路径)
- [x] **S-10**: `mobile.rs` 的 `validate_download_url()` — 已修复SSRF (拒绝私有IP范围, 仅允许http/https)
- [ ] **S-11**: 审查其他Tauri命令是否也有类似的路径遍历风险 — **已审计**。高危: MCP命令的 `project_path` 未验证是否为已管理项目（可写入任意目录的 `.mcp.json`）。建议: 在 `mcp_config_path_for_scope` 中添加项目验证（参考 `create_session` 的 `path_identity` 检查模式）。中危: `codex_write_config_value/batch` 的 `file_path`、`codex_skills_config_write` 的 `skill_id`、`codex_upload_feedback` 的 `extra_log_files`。低危: Codex FS 命令已有沙箱保护。
- [ ] **S-12**: Runner的文件操作是否也做了路径校验 — runner直接处理用户工作目录外的路径时

### 1.4 网络安全 (SEVERITY: MEDIUM)

- [ ] **S-13**: WebSocket/SSE连接未验证TLS证书 — WebSocket使用条件式 `ws:/wss:` 取决于baseUrl，无强制TLS。本地开发可接受，远程连接需确保baseUrl始终为https
- [ ] **S-14**: CORS配置 — 确认 `remote-code-runner` 和 `control-plane` 的CORS策略是否足够严格
- [ ] **S-15**: Agent二进制校验和验证 — Claude/Codex/Roo的可执行文件是否在启动前验证完整性
- [ ] **S-16**: MCP服务器连接安全性 — MCP SSE/HTTP transport是否验证服务器证书

### 1.5 `unsafe` 代码块 (SEVERITY: LOW)

- [ ] **S-17**: 全库 `unsafe` 代码审查 — 主要集中在 `codex/arg0` 和 `codex/cli/debug_sandbox` (libc FFI)
- [x] **S-18**: adapters/ 和 gui/ 中无 `unsafe` 代码 — 已确认零 `unsafe` 出现

---

## 2. 错误处理 (Error Handling)

### 2.1 Rust 错误处理

- [x] **E-01**: Roo adapter `std::sync::Mutex` poisoning — 已修复，使用 `e.into_inner()` 恢复
- [x] **E-02**: Codex adapter `session_id.as_ref().unwrap()` — 已在S-01中一并修复
- [x] **E-03**: Runner中4处 `panic!` — 全部在 `#[cfg(test)]` 模块中，非生产风险，测试panic是标准做法
- [x] **E-04**: `rc-agent-protocol/src/events.rs` 中7处测试panic — 全部在 `#[cfg(test)]` 模块，非生产代码，可接受
- [ ] **E-05**: 统一 adapter 错误类型 — 三个adapter各自返回不同的错误类型(`String`, `anyhow::Error`)，考虑统一为 `AdapterError` enum

### 2.2 前端错误处理

- [x] **E-06**: RemoteApp.tsx 重复错误处理模式 — 已提取为 `reportAsyncError` 统一处理
- [x] **E-07**: AppErrorBoundary 错误边界 — 已添加，生产环境剥离stack trace
- [x] **E-08**: `as any` 类型绕过 — 已通过边界类型缩窄器消除 (7处已修复)
- [x] **E-09**: Tauri invoke 失败统一处理 — 审查确认模式一致，修复1处遗漏的 `.catch()`
- [x] **E-10**: React Suspense fallback — `LazyMarkdownRenderer` 已添加 skeleton pulse 动画 fallback

### 2.3 危险操作防护

- [x] **E-11**: `std::env::set_var` 使用 — adapters/ 和 gui/ 中零使用。codex/arg0 中的使用已由 `unsafe_code = "warn"` 覆盖
- [x] **E-12**: `catch_unwind` 在 Roo adapter — 正确使用 `AssertUnwindSafe` 包装 agent loop，panic 后转为 error 事件而非进程崩溃，模式安全

---

## 3. 类型安全 (Type Safety)

### 3.1 TypeScript 严格模式

- [x] **T-01**: `useAppStore.ts` 中7处 `as any` — 已全部消除
- [x] **T-02**: `TerminalPane.tsx:110` — 已通过 `declare global { interface HTMLElement { __terminal?: TerminalHandle } }` 替代 `as any`
- [ ] **T-03**: `CodexJsonValue` 透传类型 — 全库 ~30个函数返回 `Promise<CodexJsonValue>` (宽泛联合类型)，考虑定义具体的响应类型
- [ ] **T-04**: `tauri.ts` 中大量 `invoke<CodexJsonValue>` — 类型安全边界应在这一层缩窄
- [x] **T-05**: `extractGoalFromResponse` 类型 — `Record<string, unknown>` 输入已正确处理

### 3.2 Rust 类型安全

- [ ] **T-06**: `serde_json::Value` 在 adapter 间的传递 — 考虑为 MCP 配置定义具体的 struct 类型
- [ ] **T-07**: `AgentAdapter::send_message` 返回 `anyhow::Result` — 考虑定义具体错误类型而非泛型错误
- [ ] **T-08**: Protocol 类型对齐 — Rust `PermissionResolutionRequest` 与 TypeScript 类型的字段是否完全对齐

### 3.3 跨语言类型对齐

- [ ] **T-09**: Tauri 命令参数类型 — Rust `#[tauri::command]` 参数类型与前端 `invoke<>()` 泛型是否完全匹配
- [ ] **T-10**: 事件 payload 类型 — `app.emit(event, payload)` 的 payload 类型是否与前端 `listen()` 回调类型对齐

---

## 4. 测试覆盖 (Test Coverage)

### 4.1 Adapter 测试 (CRITICAL GAP)

- [x] **TC-01**: `rc-roo-adapter` — 已添加33个单元测试 (event_mapper全覆盖)
- [x] **TC-02**: `rc-claude-adapter` — 已添加17个单元测试 (event_mapper全覆盖)
- [x] **TC-03**: `rc-codex-adapter` — 有基本的 event_mapper 测试和 lib 测试
- [x] **TC-04**: 为 `event_mapper.rs` (Claude) 添加单元测试 — 17个测试覆盖每种 `QueryObserverEvent` 变体
- [x] **TC-05**: 为 `event_mapper.rs` (Codex) 添加单元测试 — 21个测试覆盖Lagged/Disconnected/streaming/item lifecycle/turn completion/error/context usage/permission/thread_item_tool_call
- [ ] **TC-06**: 为 `RooInProcessAdapter::load_mcp_servers()` 添加集成测试
- [ ] **TC-07**: 为 `RooInProcessAdapter::resolve_permission()` 添加单元测试 — mutex poisoning恢复逻辑

### 4.2 GUI Backend 测试

- [x] **TC-08**: `lib.rs` — 已添加4个 `recv_with_liveness_check` 单元测试
- [ ] **TC-09**: `forward_agent_events()` 泛型事件转发 — 需要测试每种事件类型的处理
- [ ] **TC-10**: `build_mcp_server_entries()` MCP配置构建 — 需要单元测试
- [x] **TC-11**: `roo_mcp_server_overrides()` / `codex_mcp_server_overrides()` — 7个测试覆盖Codex/Roo格式差异 (http_headers vs headers, type字段, transport类型映射)

### 4.3 前端测试

- [x] **TC-12**: 前端测试 — 388个测试文件, 3,400+测试用例, 3,500+断言 — 覆盖充分
- [ ] **TC-13**: `useAppStore.ts` 的 `handleGoalCommand` — 复杂状态逻辑缺少单元测试
- [ ] **TC-14**: `RemoteApp.tsx` 的 WebSocket 订阅 — SSE连接/断开/重连场景缺少测试

### 4.4 Runner 测试

- [ ] **TC-15**: `remote-code-runner` 集成测试 — 仅有 `doctor.rs`，核心会话管理缺少测试
- [ ] **TC-16**: Runner crash recovery — 会话状态持久化和恢复的端到端测试

---

## 5. 性能 (Performance)

### 5.1 内存与渲染

- [x] **P-01**: 远程 Timeline 虚拟化 — 已添加 `react-virtuoso` 的 `Virtuoso` 组件
- [x] **P-02**: 桌面 ChatArea 虚拟化 — 已使用 `@tanstack/react-virtual`
- [ ] **P-03**: `useDeferredValue(events)` 在 RemoteApp.tsx — 确认 `useDeferredValue` 与 Virtuoso 配合是否产生双重延迟
- [ ] **P-04**: `appendRemoteTimelineEvent()` — 每次事件都创建新数组，考虑 `Immer` 或 `mutative` 优化
- [ ] **P-05**: 会话列表 `sessions.find()` 每次渲染都线性扫描 — 大量会话时应使用 `Map<sessionId, Session>`

### 5.2 Rust 性能

- [ ] **P-06**: `.clone()` 调用审查 (全库 26,639 处) — 重点关注热路径:
  - `rc-roo-adapter/src/lib.rs` — session_id, tool_name 的重复 clone
  - `rc-claude-adapter/src/lib.rs` — event mapping 中的 string clone
  - `lib.rs` (GUI) — Arc clone 在事件循环中
- [ ] **P-07**: `serde_json::Value` 的深拷贝 — MCP配置传递时大量 clone，考虑 `Arc<serde_json::Value>`
- [ ] **P-08**: `conversation_history: Arc<Mutex<Vec<ApiMessage>>>` — Roo adapter 的历史消息每次都 clone 整个Vec
- [ ] **P-09**: LTO thin + codegen-units 1 已配置 — release构建优化已启用

### 5.3 网络性能

- [ ] **P-10**: MCP服务器连接等待 — `tokio::time::sleep(2s * count)` 阻塞性等待，考虑异步健康检查
- [ ] **P-11**: SSE事件处理 — 事件到达后 `scheduleSessionsRefresh()` 有 350ms debounce，确认是否足够
- [ ] **P-12**: WebSocket重连策略 — `transport.ts` 的重连退避策略是否合理

---

## 6. 无障碍访问 (Accessibility)

- [x] **A-01**: `role="status"` 添加到加载指示器 — RemoteShell, RemoteApp, ChatArea, SwipeableMessage, App.tsx
- [x] **A-02**: `role="alert"` 添加到错误消息 — RemoteShell, RemoteApp, ChatArea, Messages, RemoteAuthGate, AppErrorBoundary
- [ ] **A-03**: 键盘导航审查 — Tab/Shift+Tab 顺序是否逻辑合理
- [ ] **A-04**: 焦点管理 — 模态框打开/关闭后焦点是否返回正确位置
- [ ] **A-05**: 颜色对比度 — 自定义颜色方案是否满足 WCAG 2.1 AA 标准 (4.5:1 文本对比度)
- [ ] **A-06**: 屏幕阅读器 — `aria-label` 是否覆盖所有交互元素 (已有 textarea 的 aria-label)
- [ ] **A-07**: 动画减弱 — `prefers-reduced-motion` 媒体查询是否被尊重 (spin动画)
- [ ] **A-08**: 图片替代文本 — 所有图标 (lucide-react) 是否有 `aria-label` 或 `aria-hidden="true"`

---

## 7. 国际化 (i18n)

### 7.1 硬编码字符串 (SEVERITY: MEDIUM)

- [ ] **I-01**: `useAppStore.ts` — ~10个硬编码中文字符串:
  - `'等待子代理结果'`, `'请先选择项目文件夹'`, `'/goal 仅在 Codex agent 下可用'` 等
- [ ] **I-02**: `App.tsx` — ~5个硬编码中文字符串:
  - `'正在初始化...'`, `'请验证身份'`, `'初始化失败'`, `'网络已断开'` 等
- [ ] **I-03**: `ChatArea.tsx` — `'正在处理当前请求…'`
- [ ] **I-04**: `reconnectHelpers.ts` — `'空闲'`, `'重连中'`, `'已连接'`, `'重连失败'`
- [ ] **I-05**: 代码注释中的中文 — `types.ts` (`ThreadGoal 数据模型`), `useTypingAnimation.ts`, `messages/index.ts`
- [ ] **I-06**: 远程应用已有 i18n (`remote/i18n.ts`, 489行) — 桌面应用需统一使用同一框架

### 7.2 i18n 策略决策

- [ ] **I-07**: 选择 i18n 方案 — `react-intl`, `i18next`, 或自定义 hook
- [ ] **I-08**: 将 `remote/i18n.ts` 扩展为全局 i18n 系统
- [ ] **I-09**: 提取所有硬编码字符串到 locale 文件
- [ ] **I-10**: 考虑 RTL 布局支持

---

## 8. 架构与重构 (Architecture)

### 8.1 大文件拆分

- [ ] **A-01**: `lib.rs` (7,264行) — 拆分为模块:
  - `commands/` (Tauri commands)
  - `events.rs` (事件转发)
  - `mcp.rs` (MCP配置)
  - `session.rs` (会话管理)
- [ ] **A-02**: `RemoteApp.tsx` (1,381行) — 评估后暂缓分解 (状态耦合太紧)
- [ ] **A-03**: `useAppStore.ts` (1,037行) — 考虑将 goal 相关逻辑提取到 `useGoalStore.ts`
- [ ] **A-04**: `types.ts` (1,050行) — 按领域拆分: `codex-types.ts`, `session-types.ts`, `mcp-types.ts`
- [ ] **A-05**: `tauri.ts` (963行) — 按功能域拆分: `codex-invokes.ts`, `session-invokes.ts`, `mcp-invokes.ts`

### 8.2 代码复用

- [x] **A-06**: 事件转发循环 — 已提取 `forward_agent_events()` 泛型函数 (700行 → 200行)
- [x] **A-07**: MCP JSON转换 — 已提取 `build_mcp_server_entries()` 泛型函数
- [x] **A-08**: 错误处理 — 已统一 `reportAsyncError` / `emit_event` 辅助函数
- [ ] **A-09**: Codex adapter 事件映射 — 与 Claude adapter 有类似模式，考虑共享基础 mapper trait
- [ ] **A-10**: 三个adapter的 `start()` / `stop()` / `send_message()` — 生命周期管理模式相似，考虑提取基类/宏

### 8.3 协议对齐

- [x] **A-11**: MCP配置统一 — 三种agent共享GUI管理的MCP服务器
- [ ] **A-12**: 事件模型统一 — `CodexAppServerNotification` 仅Codex产生，`ContextOverflow` 无adapter产生，`Stopped` 仅Claude。`SubtaskProgress`/`SubtaskCompleted` 覆盖不一致。需要确认前端是否处理这些差异
- [ ] **A-13**: 权限模型统一 — Claude/Roo的权限流程差异较大，考虑统一 Permission Protocol
- [ ] **A-14**: 会话持久化 — Codex使用自己的线程模型，Claude/Roo使用 SessionStore — 需要统一

---

## 9. 日志与可观测性 (Observability)

### 9.1 结构化日志

- [x] **O-01**: Claude adapter `run_tool` / `on_event` — 已添加 `debug!` 日志
- [x] **O-02**: Roo adapter mutex recovery — 已添加 `warn!` 日志
- [x] **O-03**: GUI `app.emit()` — 已添加 `warn!` 日志 (之前是 `let _ =` 静默忽略)
- [x] **O-04**: Codex adapter 事件映射 — 已添加 `debug!` 日志 (lag/notification/request/turn-completed)
- [x] **O-05**: Runner 事件处理 — `info!`/`warn!`/`error!` 各级别使用充分，日志覆盖正常和降级路径
- [ ] **O-06**: 前端错误上报 — `console.error` 仅5处 (JSDoc示例)，考虑集成 Sentry

### 9.2 指标与追踪

- [ ] **O-07**: Token使用追踪 — `UsageInfo` 已收集但未暴露给用户
- [ ] **O-08**: API延迟追踪 — Provider调用耗时未记录
- [ ] **O-09**: MCP工具调用统计 — 哪些MCP工具被使用、成功率、耗时
- [ ] **O-10**: `opentelemetry` 依赖已在workspace中 — 但实际集成状态需确认

---

## 10. 配置与环境变量 (Configuration)

### 10.1 环境变量治理

- [ ] **C-01**: ~50+ 环境变量分散在代码库中 — 需要集中文档和管理
- [ ] **C-02**: `REMOTE_CODE_*` vs `CLAUDE_CODE_*` vs `CODEX_*` vs `ROO_CODE_*` — 命名不一致
- [ ] **C-03**: 环境变量验证 — 缺少启动时的环境变量完整性检查
- [ ] **C-04**: `dotenvy` 在workspace依赖中但几乎未使用 — 确认是否需要

### 10.2 配置文件管理

- [ ] **C-05**: `mcp.toml` / `.mcp.json` / `.roo/mcp.json` — 多种配置文件格式，文档化加载优先级
- [ ] **C-06**: 运行时配置热重载 — 配置变更是否需要重启应用
- [ ] **C-07**: 配置schema验证 — MCP配置文件缺少JSON Schema验证

---

## 11. 前端质量 (Frontend Quality)

### 11.1 React 最佳实践

- [x] **F-01**: Render-phase ref mutation — 已修复 (`activeSessionIdRef` 移入 `useEffect`)
- [x] **F-02**: `setTimeout` 内存泄漏 — 已在 `useSurveyState.tsx` 修复
- [x] **F-03**: Window event listener cleanup — 已在 `network.ts` 修复
- [ ] **F-04**: `useEffectEvent` 使用 — 仅React实验性API，确认是否在stable版本中
- [x] **F-05**: `useDeferredValue` + `Virtuoso` — 双重延迟对高频流式事件有益（防止主线程卡顿），非bug
- [ ] **F-06**: `startTransition` 使用审查 — 仅在 `RemoteApp.tsx` 的 `setEvents` 中使用

### 11.2 状态管理

- [ ] **F-07**: `useAppStore` (Zustand) — 1,037行的巨型store，考虑按领域拆分
- [x] **F-08**: `activeSessionIdRef` 与 `activeSessionId` state — 标准React模式：ref用于异步回调获取最新值，state用于渲染，无需简化
- [ ] **F-09**: `pendingGoalObjective` state — 仅在 `sendCommand` 中同步读写，理论上可降级为 ref 减少不必要渲染（低优先级风格改进）
- [ ] **F-10**: Provider config CRUD — `loadProviderConfigs` / `saveProviderConfig` / `deleteProviderConfig` 逻辑可提取到独立store

### 11.3 UI组件质量

- [ ] **F-11**: GitPanel — 7个 `// TODO: Call Tauri backend` 未实现
- [ ] **F-12**: CheckpointTimeline — `// TODO: Call Tauri backend checkpoint_list` 未实现
- [ ] **F-13**: 主题/暗色模式 — 部分组件使用硬编码颜色，部分使用CSS变量
- [ ] **F-14**: 响应式布局 — 移动端适配是否完整测试

---

## 12. Rust 代码质量 (Rust Code Quality)

### 12.1 Clippy 与Lint

- [x] **R-01**: Workspace lint 配置完善 — `unsafe_code = "warn"`, `dbg_macro = "deny"`, `todo = "deny"`, `unwrap_used = "warn"`
- [ ] **R-02**: `unwrap_used = "warn"` 仅警告未阻止 — 3,366处 `.unwrap()` 需要逐一审查
- [ ] **R-03**: `clippy::todo` 仅捕获 `todo!()` 宏，不捕获 `// TODO` 注释
- [ ] **R-04**: 运行 `cargo clippy --workspace` — 确认零警告

### 12.2 Async 安全

- [x] **R-05**: `tokio::sync::Mutex` vs `std::sync::Mutex` — Roo adapter正确划分：tokio Mutex用于异步字段，std Mutex用于OS线程同步字段，无死锁风险
- [x] **R-06**: `catch_unwind` + `std::thread::spawn` — panic正确隔离，已修复gap: panic后现在发送Error事件到channel
- [x] **R-07**: `blocking_send` — Claude adapter 中未使用；Roo adapter 正确用于OS线程向tokio channel发送，用法安全

### 12.3 API 设计

- [x] **R-08**: `AgentAdapter` trait `send_message` 用 `&mut self` — 有意设计：同adapter实例串行，不同session可并发，内部状态不支持重叠请求
- [ ] **R-09**: `PermissionDecision` 类型 — Roo用 `oneshot::Sender<bool>` 丢失AllowAll语义，Claude/Codex保留完整枚举。应统一
- [ ] **R-10**: `UnifiedAgentEvent` — 某些variant (如 `ContextCompacted`) 仅Claude产生，其他adapter如何处理

---

## 13. 依赖与构建 (Dependencies & Build)

### 13.1 依赖审计

- [ ] **D-01**: `cargo audit` — 未安装，需运行 `cargo install cargo-audit && cargo audit`
- [ ] **D-02**: `npm audit` — 镜像源不支持，需切换到官方源运行
- [ ] **D-03**: 过时依赖 — `react-virtuoso` 刚添加，确认版本最新
- [ ] **D-04**: `tungstenite` / `tokio-tungstenite` 使用 OpenAI fork — 确认patch版本是否跟踪上游安全更新
- [ ] **D-05**: `rusqlite` bundled — SQLite版本是否包含最新安全补丁

### 13.2 构建优化

- [x] **D-06**: LTO thin + codegen-units 1 — release构建已优化
- [ ] **D-07**: 增量编译 — workspace成员过多(~150 crates)可能导致增量编译失效
- [ ] **D-08**: 编译时间 — 首次编译时间可能超过10分钟，考虑 sccache
- [ ] **D-09**: 前端构建 — Tauri应用的前端bundle size未优化

---

## 14. 文档与注释 (Documentation)

### 14.1 代码文档

- [ ] **DOC-01**: `rc-agent-protocol` 公共API缺少 rustdoc — `AgentAdapter` trait 应有详细文档
- [ ] **DOC-02**: 三个adapter的架构文档 — 仅Roo adapter 有模块级文档
- [ ] **DOC-03**: `lib.rs` (7,264行) — 需要模块级架构文档解释各部分关系
- [ ] **DOC-04**: TypeScript 公共API — `useAppStore` 的action文档不完整

### 14.2 项目文档

- [x] **DOC-05**: `ARCHITECTURE.md` — 已存在
- [x] **DOC-06**: `ROADMAP.md` — 已存在
- [ ] **DOC-07**: 环境变量文档 — ~50个环境变量缺少集中文档
- [ ] **DOC-08**: 部署文档 — `deploy/` 目录存在但文档是否完整
- [ ] **DOC-09**: 贡献指南 — 缺少 `CONTRIBUTING.md`
- [ ] **DOC-10**: 代码风格指南 — Rust和TypeScript的编码规范未文档化

### 14.3 TODO 管理

- [ ] **DOC-11**: 全库 1,329 个 TODO/FIXME 标记 — 需要分类和优先级排序
- [ ] **DOC-12**: 前端 7个 GitPanel TODO — 明确是否计划实现或标记为 wontfix
- [ ] **DOC-13**: 适配器中仅1个 TODO (Claude `session rule for auto-approval`) — 优先级低

---

## 15. 已完成项 (Completed)

> 以下项目已在之前的审计中完成，标记为 `[x]` 表示已完成。

| 编号 | 项目 | 状态 |
|------|------|------|
| H7 | 提取700行重复事件转发为 `forward_agent_events()` 泛型函数 | ✅ 完成 |
| M1 | `app.emit()` 失败静默忽略 → 添加 `warn!` 日志 | ✅ 完成 |
| M4 | Prompt输入长度验证 (1MB限制) | ✅ 完成 |
| M5 | MCP JSON转换去重 → `build_mcp_server_entries()` | ✅ 完成 |
| M6 | Claude adapter `run_tool` / `on_event` 添加 `debug!` 日志 | ✅ 完成 |
| M7 | 双重 `ToolCallStarted` 事件 (Claude streaming variant) | ✅ 完成 |
| M8 | 7处 `as any` → `asGoalResponse()` / `asClearResponse()` | ✅ 完成 |
| A11Y | `role="status"` / `role="alert"` — 12处添加 | ✅ 完成 |
| MCP | Roo adapter MCP统一配置 (GUI → adapter) | ✅ 完成 |
| RP-4 | RemoteApp.tsx 8处重复错误处理 → `reportAsyncError` | ✅ 完成 |
| M3 | Runtime lock 审查 — 确认在streaming前释放 | ✅ 安全 |
| H8 | 远程Timeline虚拟化 (`react-virtuoso`) | ✅ 完成 |
| S-18 | adapters/gui 中无 `unsafe` 代码 | ✅ 确认 |
| S-09 | 路径遍历修复 (mobile.rs) | ✅ 完成 |
| S-10 | SSRF修复 (mobile.rs) | ✅ 完成 |
| H4 | AppErrorBoundary 全路径包裹 | ✅ 完成 |
| H5 | Render-phase ref mutation 修复 | ✅ 完成 |
| H9 | Mutex poisoning 恢复 (Roo adapter 3处) | ✅ 完成 |

---

## 优先级排序建议

### P0 — 立即处理 (安全/稳定性风险)
1. S-02: Runner中的 panic!
2. S-01: Codex adapter的 unwrap()
3. S-05: 明文密钥存储
4. TC-01/TC-02: Roo/Claude adapter 零测试

### P1 — 短期处理 (质量/可靠性)
5. TC-08: lib.rs 事件转发测试
6. T-03: CodexJsonValue 类型安全
7. S-14: CORS策略审查
8. A-01: lib.rs 大文件拆分

### P2 — 中期处理 (性能/体验)
9. P-06: clone() 热路径优化
10. I-01~I-06: i18n 字符串提取
11. P-10: MCP服务器异步连接
12. O-04~O-06: 日志补充

### P3 — 长期优化 (架构/文档)
13. A-09/A-10: Adapter代码复用
14. DOC-07: 环境变量文档
15. D-01/D-02: 依赖审计
16. A-03/A-04/A-05: 前端文件拆分