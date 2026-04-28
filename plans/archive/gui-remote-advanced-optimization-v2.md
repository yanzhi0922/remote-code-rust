# Remote Code: GUI 与 Remote 控制端进阶架构与体验优化方案 v2

> 本文档自 2026-04-14 起直接取代并删除旧版 v1 草案，作为 GUI 与 Remote 进阶优化的唯一正式实施稿。

---

## 0. 先说结论

- 这次优化**不是**“把桌面 GUI 和 Remote Web 立刻抽成一套完全相同的页面”。
- 正确路线是：**共享领域模型与展示组件，保留本地桌面壳和远程 Web 壳的差异**，先把数据流和视图模型统一，再逐步提高组件复用率。
- 当前最真实的问题不是“完全没有统一抽象”，而是：
  - 本地 GUI 与远程 Web 已经各自长出了可用能力，但两套状态与渲染模型开始分叉。
  - [apps/remote-code-gui/src/remote/RemoteApp.tsx](../apps/remote-code-gui/src/remote/RemoteApp.tsx) 已经承担过多职责，文件体量和耦合度过高。
  - 远程移动端已经可用，但交互仍偏“桌面缩放版”，还没有形成专门的 mobile shell。
  - 发布链路已经证明是产品风险点，PWA 缓存和静态资源部署不能再被视为边角问题。

### 0.1 文档定位

- 这不是产品愿景稿，也不是设计概念稿。
- 这是一份**按当前仓库现实、可逐阶段编码、可逐阶段验收、可按 commit 拆分推进**的工程实施稿。
- 所有阶段都必须满足：
  - 主分支始终可编译
  - 远程线上能力不回退
  - 本地桌面端不因抽象重构而失去已有功能

### 0.2 当前最重要的工程判断

1. 优先拆 `RemoteApp`，而不是先重做桌面端。
2. 优先统一 ViewModel，而不是强行统一原始事件。
3. 优先共享中层组件，而不是先追求“整个页面共用”。
4. 优先把发布门禁写进主计划，而不是等 UI 重构结束后再补。

---

## 1. 当前代码真实状态

### 1.1 入口结构

- [apps/remote-code-gui/src/App.tsx](../apps/remote-code-gui/src/App.tsx) 当前按运行环境二选一：
  - 本地桌面模式：`Layout + ChatArea + ChatInput + PermissionModal`
  - 远程模式：直接渲染 `RemoteApp`
- 这意味着当前并不是“同一棵 React 树注入不同 Provider”，而是**两套壳层直接分叉**。

### 1.2 本地桌面 GUI 真实状态

- 本地 GUI 已经明确建立在 `Zustand + Tauri 事件桥` 之上。
- [apps/remote-code-gui/src/stores/useAppStore.ts](../apps/remote-code-gui/src/stores/useAppStore.ts) 负责：
  - Tauri 初始化
  - 会话加载与切换
  - 对话状态
  - 工具进度与结果
  - 子任务树与 batch 进度
  - 上下文使用情况
  - 权限请求与响应
  - Provider / Settings / Projects
- 这不是一个“待搭建”桌面基础，而是一个**已经投入使用的完整本地状态层**。

### 1.3 远程 Web/PWA 真实状态

- [apps/remote-code-gui/src/remote/RemoteApp.tsx](../apps/remote-code-gui/src/remote/RemoteApp.tsx) 当前已经是完整远程客户端，不是 demo：
  - 健康检查
  - bootstrap claim
  - pairing accept
  - session 列表
  - timeline 加载
  - approvals / artifacts
  - follow-up / interrupt
  - WebSocket 实时事件流
  - runner 离线禁控
  - 当前会话持久化恢复
- 当前主要问题不是“功能缺失”，而是**一个 1700+ 行的超级组件同时承担 transport、状态机、业务规则和 UI 组装**。

### 1.4 远程传输真实状态

- 当前远程端不是 SSE，也不是 EventSource。
- 真实实现是：
  - REST 拉取列表与快照
  - `WebSocket + after cursor` 订阅实时事件
- 证据：
  - [apps/remote-code-gui/src/remote/api.ts](../apps/remote-code-gui/src/remote/api.ts) 的 `buildSessionEventsStreamUrl()` 会构造 `ws://` / `wss://`
  - [crates/rc-control-plane/src/handlers.rs](../crates/rc-control-plane/src/handlers.rs) 使用 `WebSocketUpgrade`
- 因此，**v2 不允许再把远程适配器写成 SSE 方案**，除非未来明确决定重构协议。

### 1.5 事件模型真实状态

- 当前本地桌面端已有一套稳定的 UI 事件与任务事件，不是文档中假设的 `PlanCreated / StepCompleted / StepFailed`。
- 当前真实事件来源包括：
  - `subtask_started`
  - `subtask_completed`
  - `batch_progress`
  - `task_snapshot`
  - `tool_started`
  - `approval_requested`
  - `artifact_created`
- 证据：
  - [crates/rc-ui-bridge/src/lib.rs](../crates/rc-ui-bridge/src/lib.rs)
  - [crates/rc-protocol/src/lib.rs](../crates/rc-protocol/src/lib.rs)
  - [apps/remote-code-gui/src/stores/useAppStore.ts](../apps/remote-code-gui/src/stores/useAppStore.ts)
- 所以 v2 必须基于**当前事件真实名称和字段**来设计任务树与时间线，而不是重新假设另一套事件语言。

### 1.6 共享组件真实状态

- 目前已经存在可复用的展示能力，例如：
  - `MarkdownRenderer`
  - `CollapsibleBlock`
  - 若干布局组件
- 但 [apps/remote-code-gui/src/components/chat/ChatArea.tsx](../apps/remote-code-gui/src/components/chat/ChatArea.tsx) 直接绑定本地 `ConversationEntry` 和 Zustand selector。
- 这说明当前适合共享的是**渲染原子和中层展示组件**，而不是直接声称“ChatArea 已可 100% 跨环境复用”。

### 1.7 依赖与 UI 基座真实状态

- Radix 不是计划中“待安装”的未来事项，而是已经部分接入：
  - `@radix-ui/react-dialog`
  - `@radix-ui/react-dropdown-menu`
  - `@radix-ui/react-scroll-area`
- 证据见 [apps/remote-code-gui/package.json](../apps/remote-code-gui/package.json)
- 但以下依赖当前并不存在：
  - `vaul`
  - `@tanstack/react-virtual`
- 因此 v2 不能把这两者写成既成事实，只能作为按需引入项。

### 1.8 发布链路真实状态

- 当前 Web/PWA 已经有 service worker、缓存版本和安装路径：
  - [apps/remote-code-gui/src/main.tsx](../apps/remote-code-gui/src/main.tsx)
  - [apps/remote-code-gui/public/sw.js](../apps/remote-code-gui/public/sw.js)
- 当前生产部署已经证明两个高风险事实：
  - 静态资源权限错误会直接导致白屏
  - PWA 缓存与入口资源版本不一致会引发线上问题
- 当前静态发布已经有专门脚本：
  - [deploy/tencent-cloud/deploy-remote-code-gui.sh](../deploy/tencent-cloud/deploy-remote-code-gui.sh)
- 因此 v2 必须把发布门禁写入主计划，不能只谈界面。

### 1.9 中文与白屏修复基线

- 当前远程端已经不是“只有英文”：
  - [apps/remote-code-gui/src/remote/i18n.ts](../apps/remote-code-gui/src/remote/i18n.ts) 已经落地 `zh-CN / en` 文案与自动语言探测
  - [apps/remote-code-gui/src/remote/RemoteApp.tsx](../apps/remote-code-gui/src/remote/RemoteApp.tsx) 已经实际接入 `resolveRemoteLocale()` 和 `getRemoteCopy()`
- 当前白屏问题也已经不应再被当成抽象风险，而是**已有防线、但仍需体系化固化**：
  - [apps/remote-code-gui/src/components/layout/AppErrorBoundary.tsx](../apps/remote-code-gui/src/components/layout/AppErrorBoundary.tsx) 已有运行时崩溃兜底与“清缓存后重载”能力
  - [apps/remote-code-gui/src/main.tsx](../apps/remote-code-gui/src/main.tsx) 已接入 service worker 注册与 controller 切换后自动刷新
- 当前仍存在一个必须写进 v2 的现实约束：
  - `main.tsx` 的 `SERVICE_WORKER_URL` 版本号
  - `public/sw.js` 的 `CACHE_NAME` 版本号
  - 现在是**两处手工维护**，这本身就是下一次缓存错配和白屏回归的风险源
- 因此 v2 必须明确要求：
  - PWA 版本号单一来源化
  - 白屏回归测试固定化
  - 中文回归测试固定化

### 1.10 当前测试基线

- 远程端已经不是“零测试起步”：
  - [apps/remote-code-gui/src/remote/RemoteApp.test.tsx](../apps/remote-code-gui/src/remote/RemoteApp.test.tsx) 已覆盖：
    - 中文文案选择
    - 首次认证后不进入异常轮询
    - approvals / artifacts 展示与审批转发
    - follow-up 转发
    - interrupt 转发
    - owner runner 离线时禁控
    - active session 恢复与持久化
- 这意味着 v2 的正确策略不是“以后再补测试”，而是：
  - 把现有测试升级为新抽象的回归护栏
  - 拆 `RemoteApp` 时同步迁移测试，不允许先删后补

---

## 2. v2 要解决的真正问题

### 2.1 主要目标

1. 统一本地桌面端和远程 Web 端的**会话领域模型与视图模型**。
2. 把 `RemoteApp` 拆成可维护的 shell + feature 结构。
3. 让本地端和远程端逐步共享中层展示组件，而不是暴力共享整个页面。
4. 建立 mobile-first 的远程专用壳层。
5. 把 PWA 发布、缓存、移动端 smoke test 纳入正式交付流程。

### 2.2 非目标

- 不重写 control plane 协议。
- 不把 WebSocket 切换成 SSE。
- 不在第一阶段就移除本地 `Zustand`。
- 不在第一阶段做完整 diff viewer。
- 不在第一阶段做 dark mode / theme 系统。
- 不做“大爆炸式 UI 全量翻修”。

### 2.3 明确出范围

以下事项不属于本稿实施范围，除非后续另起专项：

- Relay 协议升级
- 原生 iOS / Android 壳
- Push 通知
- 云端 runner
- 文件树浏览器
- 完整终端仿真器
- 完整 diff 审查平台
- 多用户协作权限系统

### 2.4 当前必须正视的具体债务

1. [apps/remote-code-gui/src/remote/RemoteApp.tsx](../apps/remote-code-gui/src/remote/RemoteApp.tsx) 同时承担认证、拉数、WS 生命周期、事件归并、状态持久化和 UI 组装，已经超过单文件可维护上限。
2. 远程时间线的 `hydrate / append / dedupe / merge` 逻辑与页面渲染混写，导致后续共享 ViewModel 时无法小步迁移。
3. 本地与远程目前都能表达“消息 / 工具 / 审批 / 产物”，但视觉和数据归一层还没分离，后续复用很容易演变成复制粘贴。
4. PWA 缓存版本号当前分散在两个文件里，发布链路仍然依赖人工同步，不符合长期可维护要求。
5. 中文、白屏、防缓存事故已经有局部修复，但还没有被写成正式门禁；只要缺门禁，就等于问题还没真正解决。

---

## 3. 新的架构原则

### 3.1 原则一：统一的是 ViewModel，不是原始事件

- 本地桌面端当前拿到的是：
  - conversation snapshot
  - live tool progress
  - task snapshot
  - permission modal state
- 远程端当前拿到的是：
  - session timeline event log
  - approval list
  - artifact list
  - remote connection state
- 两边的数据形态并不相同，因此 v2 的统一层应该是：
  - `Transport -> Domain Snapshot -> SessionViewModel -> Shared Components`
- 不应该直接强迫“本地 conversation entry”和“远程 timeline event”一一对齐。

### 3.2 原则二：共享核心，保留壳层

- 本地桌面端继续保留：
  - `Layout`
  - `Header`
  - `Sidebar`
  - `PermissionModal`
- 远程 Web 端继续保留：
  - `AuthGate`
  - `RemoteShell`
  - `Mobile session list / drawer`
  - `Connection banner`
- 真正共享的是：
  - Timeline cards
  - Tool / approval / artifact cards
  - Composer action bar
  - Session header summary
  - Task tree blocks

### 3.3 原则三：先拆职责，再谈美学

- 先解决 `RemoteApp` 的职责过载。
- 先建立 transport 契约与 view model 正规层。
- 只有在这些基础完成后，才有资格引入：
  - bottom sheet 细节动效
  - virtualization
  - diff viewer
  - theme system

### 3.4 原则四：发布链路属于产品架构，不是运维附录

- service worker 缓存策略
- 静态资源原子替换
- 目录/文件权限
- mobile browser smoke
- reload / resume 验证

这些都必须纳入 v2 正式范围。

### 3.5 原则五：所有抽象都必须能映射回现有文件

- 任何新增层都必须回答三个问题：
  - 它替代当前哪个具体文件中的哪段职责
  - 它的输入输出是否能由现有代码真实提供
  - 它是否减少而不是增加理解成本
- 如果答不清楚，就不引入该抽象。

---

## 4. v2 目标架构

### 4.1 新的前端分层

```text
App
├─ LocalDesktopShell
│  ├─ LocalSessionTransport (wrap Zustand/Tauri)
│  └─ SharedSessionWorkspace
└─ RemoteWebShell
   ├─ RemoteAuthGate
   ├─ RemoteSessionTransport (REST + WS + after cursor)
   └─ SharedSessionWorkspace
```

### 4.2 推荐目录结构

```text
apps/remote-code-gui/src/
  session/
    contracts.ts
    transport.ts
    view-model.ts
    normalize/
      fromLocal.ts
      fromRemote.ts
    hooks/
      useSessionWorkspace.ts
    components/
      SessionHeader.tsx
      SessionTimeline.tsx
      SessionComposer.tsx
      ApprovalQueue.tsx
      ArtifactLibrary.tsx
      TaskTreePanel.tsx
      ConnectionBanner.tsx
  local/
    LocalSessionTransport.ts
  remote/
    RemoteAuthGate.tsx
    RemoteSessionTransport.ts
    RemoteShell.tsx
    RemoteMobileSheets.tsx
```

> 说明：这里只是建议结构。重点不是目录名，而是把 transport、normalize、shared components 从 `RemoteApp.tsx` 和 `useAppStore.ts` 的巨型职责里剥出来。

### 4.3 新的核心契约

#### `SessionTransport`

负责：
- 获取 session list
- 读取 session snapshot
- 读取 approvals / artifacts
- 提交 prompt / interrupt / approval decision
- 建立实时订阅

不负责：
- 直接拼接 UI 文案
- 直接决定 banner 展示
- 直接渲染组件

#### `SessionViewModel`

负责把不同来源的数据整理成统一可渲染结构，例如：
- `sessionHeader`
- `timelineItems`
- `composerState`
- `approvalState`
- `artifactState`
- `taskTreeState`
- `connectionState`

#### `SharedSessionWorkspace`

只消费 `SessionViewModel`，不关心当前是本地还是远程。

### 4.4 建议接口草案

> 以下不是最终代码签名，但必须保持这个粒度和职责边界。

```ts
export interface SessionTransport {
  listSessions(): Promise<SessionSummaryVm[]>;
  loadSessionBundle(sessionId: string): Promise<SessionBundleVm>;
  subscribeSession(
    sessionId: string,
    afterCursor: number | null,
    callbacks: SessionSubscriptionCallbacks,
  ): SessionSubscriptionHandle;
  sendPrompt(sessionId: string, content: string): Promise<CommandAckVm>;
  interrupt(sessionId: string): Promise<CommandAckVm>;
  resolveApproval(approvalId: string, decision: ApprovalDecisionVm, note?: string): Promise<void>;
}
```

```ts
export interface SessionBundleVm {
  session: SessionDetailVm;
  timeline: TimelineItemVm[];
  approvals: ApprovalItemVm[];
  artifacts: ArtifactItemVm[];
  taskTree: TaskNodeVm[];
  latestCursor: number | null;
}
```

### 4.5 当前真实数据源到目标 VM 的映射

| 来源 | 当前文件 | 目标 VM |
|---|---|---|
| 本地会话对话 | `useAppStore.ts` / `tauri.ts` | `TimelineItemVm[]` |
| 本地工具进度 | `useAppStore.ts` | `TimelineItemVm[]` / `ComposerVm` |
| 本地任务树 | `useAppStore.ts` / `rc-ui-bridge` | `TaskNodeVm[]` |
| 远程时间线 | `RemoteApp.tsx` / `remote/types.ts` | `TimelineItemVm[]` |
| 远程审批列表 | `RemoteApp.tsx` | `ApprovalItemVm[]` |
| 远程产物列表 | `RemoteApp.tsx` | `ArtifactItemVm[]` |
| 远程连接状态 | `RemoteApp.tsx` | `SessionConnectionVm` |

---

## 5. 实施前置约束

### 5.1 不可破坏的已有能力

- 本地桌面端：
  - 会话切换
  - 消息发送
  - 工具进度显示
  - 子任务显示
  - 权限弹窗
- 远程 Web/PWA：
  - 配对与 token 认证
  - session 列表
  - 中文 / 英文
  - follow-up / interrupt
  - approvals / artifacts
  - runner 离线禁控
  - active session 恢复
  - PWA 正常加载，不白屏

### 5.2 新代码必须遵守的实施规则

1. 每个阶段都允许只新增文件和薄包装，不强制立刻删除旧代码。
2. 删除旧实现只能发生在新实现已被真实接管之后。
3. 每个阶段结束都要补对应测试，不能把测试拖到最后。
4. 每次远程 Web 相关改动上线前，都必须做一次真实浏览器 smoke。

---

## 6. 分阶段实施稿

### 6.0 立即执行批次

> 这一段不是路线图摘要，而是“从当前代码直接开工时的第一批提交顺序”。

#### Batch 0：先补共享契约，不拆页面

1. 新增 `src/session/contracts.ts`
2. 新增 `src/session/transport.ts`
3. 新增 `src/session/normalize/fromRemote.ts`
4. 新增 `src/session/normalize/fromLocal.ts`
5. 新增 normalize 单测

**禁止事项**

- 不改现有 REST/WS 协议
- 不改现有页面装配
- 不把 `RemoteApp.tsx` 里的 JSX 和 normalize 重构混在一个提交里

#### Batch 1：把远程事件整形从页面剥离

1. 把 `hydrateTimeline / appendTimelineEvent / sameMessageStream / sameToolProgress` 从 `RemoteApp.tsx` 抽走
2. 新增远程 bundle -> VM 的纯函数层
3. 先让 `RemoteApp.tsx` 调新纯函数，不着急重排 UI 结构

**验收要求**

- 现有 `RemoteApp.test.tsx` 全部继续通过
- 页面行为不变
- 新纯函数至少有去重、message delta 合并、tool progress 合并测试

#### Batch 2：把认证和 transport 拆开

1. 抽 `RemoteAuthGate`
2. 抽 `RemoteSessionTransport`
3. 抽 active session persistence
4. `RemoteApp.tsx` 降为壳层入口

**验收要求**

- 中文仍然正常
- 配对和 token 保存不回退
- 刷新恢复 active session 不回退
- runner 离线禁控不回退

#### Batch 3：把 PWA 版本和白屏门禁制度化

1. 收敛 `main.tsx` 与 `sw.js` 的版本号来源
2. 为 `AppErrorBoundary` 和 service worker 版本联动补测试或最少补 smoke 脚本
3. 把部署脚本和发布清单明确绑定到同一套回归步骤

**验收要求**

- 升级静态资源后不会因旧缓存导致入口白屏
- 出现运行时异常时能从 UI 看到恢复入口，而不是纯白页
- 中文浏览器和英文浏览器都能完成一次完整打开流程

### Phase A：建立统一契约，不动现有交互语义

#### 目标

- 新增共享领域类型和 view model 正规层。
- 不改变本地桌面端现有行为。
- 不改变远程端线上协议。

#### 必做项

1. 新建 `session/contracts.ts`
   - 定义共享结构：
     - `SessionSummaryVm`
     - `SessionConnectionVm`
     - `TimelineItemVm`
     - `ApprovalItemVm`
     - `ArtifactItemVm`
     - `ComposerVm`
     - `TaskNodeVm`

2. 新建 `session/transport.ts`
   - 定义 `SessionTransport` 接口：
     - `listSessions`
     - `loadSessionBundle`
     - `subscribeSession`
     - `sendPrompt`
     - `interrupt`
     - `resolveApproval`

3. 新建 `session/normalize/fromRemote.ts`
   - 仅把当前远程真实数据结构转换成共享 VM
   - 必须基于现有：
     - `RemoteSessionRecord`
     - `RemoteTimelineEvent`
     - `RemoteApprovalRecord`
     - `RemoteArtifactRecord`

4. 新建 `session/normalize/fromLocal.ts`
   - 仅把当前本地真实数据结构转换成共享 VM
   - 必须基于现有：
     - `ConversationEntry`
     - `ToolProgressInfo`
     - `ToolResultInfo`
     - `SessionSubtask`
     - `PermissionRequestInfo`

#### 建议工作包

- A1：补 `contracts.ts`
- A2：补 `fromRemote.ts`
- A3：补 `fromLocal.ts`
- A4：补 `view-model.ts`
- A5：补测试，不改页面接线

#### 完成标准

- 两端都能在不改协议的前提下产出统一 `SessionViewModel`
- 还没有复用 UI，也算通过
- 本地和远程都至少各有一组 normalize 测试
- 现有 `RemoteApp.test.tsx` 继续通过，且不减少覆盖面

### Phase B：拆 RemoteApp，先把远程端做干净

#### 目标

- 把 [RemoteApp.tsx](../apps/remote-code-gui/src/remote/RemoteApp.tsx) 从“超级组件”拆成壳层 + feature 组件。

#### 必做项

1. 抽出 `RemoteAuthGate`
   - 负责：
     - bootstrap claim
     - pairing accept
     - manual token
   - 不再和 session timeline 混写在同一组件里

2. 抽出 `RemoteSessionTransport`
   - 负责：
     - REST 请求
     - WebSocket 生命周期
     - `after` cursor 重连
     - active session persistence

3. 抽出 `RemoteShell`
   - 负责：
     - 左侧 session list
     - 顶部 connection banner
     - mobile / desktop 容器布局

4. 保持功能不回退
   - runner 离线禁控
   - 未分配 runner 禁控
   - 中文 / 英文
   - approvals / artifacts
   - reload 后恢复 active session

#### 建议工作包

- B1：抽 `RemoteAuthGate`
- B2：抽 `RemoteSessionTransport`
- B3：抽 `RemoteShell`
- B4：把 active session persistence 收口到 transport
- B5：补拆分后的组件测试

#### 完成标准

- `RemoteApp.tsx` 降为薄入口
- 远程线上行为与当前一致
- 不允许因为拆文件引入新协议或新状态机
- 远程现有测试迁移后仍覆盖：
  - 中文 locale
  - active session restore
  - approvals / artifacts
  - follow-up / interrupt
  - runner 离线禁控

### Phase C：提取共享展示组件

#### 目标

- 开始让本地端和远程端真正共享“中层渲染”，而不是共享整个页面。

#### 先共享这些组件

1. `SessionHeader`
2. `ConnectionBanner`
3. `ApprovalQueue`
4. `ArtifactLibrary`
5. `TaskTreePanel`
6. `Timeline event cards`

#### 暂不直接共享这些

1. 当前本地 `Layout`
2. 当前远程 `AuthGate`
3. 本地 `Sidebar`
4. 远程移动端壳层导航

#### 对 `ChatArea` 的正确处理

- 不要直接把现有 [ChatArea.tsx](../apps/remote-code-gui/src/components/chat/ChatArea.tsx) 搬给远程端。
- 正确做法是：
  - 先抽 `TimelineMessageCard`
  - 再抽 `ToolCallCard`
  - 再抽 `ToolResultCard`
  - 再抽 `ThinkingBlock`
  - 最后用这些原子件重新组装本地与远程 timeline

#### 建议工作包

- C1：抽时间线卡片原子
- C2：抽审批与产物面板
- C3：抽共享 session header
- C4：本地端接共享时间线
- C5：远程端接共享时间线

#### 完成标准

- 本地和远程至少共享 3 个以上非原子组件
- `RemoteApp` 与 `ChatArea` 不再分别维护两套完全独立的 message/tool/approval 视觉语言

### Phase D：Remote 移动优先重塑

#### 目标

- 让远程端真正适合手机，而不是“桌面布局在窄屏上的缩水版”。

#### 必做项

1. 审批改为移动端底部抽屉
   - 首选基于**现有已安装的** `@radix-ui/react-dialog`
   - 暂不强依赖 `vaul`
   - 如果后续手势体验不够，再引入 `vaul`

2. Session 列表移动端抽屉化
   - 手机上默认聚焦当前会话
   - session list 通过顶部按钮展开

3. 强化 connection / replay 反馈
   - `idle / connecting / streaming / reconnecting / error`
   - reload / background resume 后的状态切换必须更明确

4. Artifact 进入独立移动面板
   - 不再要求用户在长时间线中寻找下载入口

#### 建议工作包

- D1：移动端 session drawer
- D2：审批 bottom sheet
- D3：artifact panel mobile 化
- D4：connection banner mobile 精简
- D5：真实手机视口测试固化

#### 完成标准

- 390px 宽度下可单手完成：
  - 切会话
  - 看时间线
  - 发 follow-up
  - 审批
  - 下载 artifact
- 浏览器崩溃恢复和缓存恢复路径已验证，不再出现“只剩白屏但没有恢复动作”的状态

### Phase E：高级能力，只在前置结构稳定后推进

#### E1. 任务树升级

- 基于真实事件：
  - `subtask_started`
  - `subtask_completed`
  - `batch_progress`
  - `task_snapshot`
- 不是重新发明 `PlanCreated`
- 目标是：
  - 支持层级树
  - 展示 `running/completed/failed`
  - 展示 `turns_used`
  - 支持本地和远程统一展示

#### E2. Diff Viewer

- 只有在工具层能稳定提供结构化变更元数据时再做
- 否则只会变成“把字符串塞进一个大弹窗”
- 前置条件：
  - 明确 diff 数据契约
  - 明确 patch / file summary 来源

#### E3. Virtualization

- 只在出现明确性能证据后引入
- 当前项目尚未证明必须立即引入 `@tanstack/react-virtual`
- 进入条件：
  - 单会话 DOM 节点规模
  - 滚动卡顿数据
  - 手机低端机性能样本

#### E4. Theme / Dark Mode

- 放到结构稳定后
- 不允许在组件抽象没稳定前先做全局主题改造

---

## 7. 文件迁移表

| 当前文件 | 当前主要职责 | v2 目标去向 |
|---|---|---|
| `src/App.tsx` | 本地/远程模式分流 | 保留分流，仅改为装配新 shell |
| `src/stores/useAppStore.ts` | 本地桌面全部状态 | 保留，逐步降为 Local transport backing store |
| `src/components/chat/ChatArea.tsx` | 本地时间线渲染 | 逐步拆为共享 timeline 子组件 |
| `src/components/chat/ChatInput.tsx` | 本地输入框 | 后续可抽共享 composer 子组件 |
| `src/remote/RemoteApp.tsx` | 远程认证、transport、状态、UI 全包 | 拆为 `RemoteAuthGate + RemoteSessionTransport + RemoteShell` |
| `src/remote/api.ts` | 远程 REST/WS 原始请求 | 保留为低层 API，供 `RemoteSessionTransport` 调用 |
| `src/lib/runtime.ts` | 运行模式、token、session 持久化 | 保留，逐步只做运行时环境与存储 helpers |

---

## 8. 逐提交推进建议

> 目标是每一步都可单独 review、单独回滚、单独上线。

### Commit Slice 1

- 新增 `session/contracts.ts`
- 新增 `session/normalize/fromRemote.ts`
- 新增其单元测试

### Commit Slice 2

- 新增 `session/normalize/fromLocal.ts`
- 新增 `view-model.ts`
- 新增其单元测试

### Commit Slice 3

- 抽 `RemoteAuthGate`
- `RemoteApp` 仅保留装配逻辑
- 保持线上功能完全不变

### Commit Slice 4

- 抽 `RemoteSessionTransport`
- 接管 WebSocket / cursor / session persistence
- 做浏览器真实回归

### Commit Slice 5

- 抽共享 `ApprovalQueue` / `ArtifactLibrary`
- 远程端接入

### Commit Slice 6

- 抽共享时间线卡片
- 本地端接入
- 远程端接入

### Commit Slice 7

- 移动端专用 shell 和 bottom sheet
- 做手机视口回归

---

## 9. 完成定义

### Phase A Done

- 新契约和 normalize 层存在
- 不改页面行为
- 测试覆盖核心映射

### Phase B Done

- `RemoteApp` 不再直接处理所有业务
- transport 与 auth 已独立
- 浏览器回归通过

### Phase C Done

- 本地和远程共用中层时间线组件
- approvals / artifacts 视觉语义统一

### Phase D Done

- 手机端交互不再是桌面缩水版
- 单手可完成核心操作
- 无白屏、无 console error

### v2 First Milestone Done

- 本地和远程共享 ViewModel
- 远程壳层职责清晰
- 共享中层组件正式落地
- 发布门禁进入常规流程

---

## 10. 测试与交付门禁

### 10.1 单元测试

- `fromLocal` / `fromRemote` 的 normalize 测试
- `SessionTransport` mock 测试
- runner 离线 / 未分配 runner 的 composer 禁用测试
- active session 持久化恢复测试
- 中文 locale 选择测试

### 10.2 组件测试

- `RemoteAuthGate`
- `ApprovalQueue`
- `ArtifactLibrary`
- `SessionTimeline`
- `TaskTreePanel`

### 10.3 浏览器真实回归

每次远程 UI 改造后，至少执行：

1. 手机视口打开远程页
2. 中文文案检查
3. console error 必须为 0
4. 未分配 runner 会话禁控
5. 离线 runner 会话禁控
6. 刷新后保持当前选中会话
7. approvals / artifacts 仍可见
8. 当前会话切换后重载不会跳回第一条

### 10.4 发布门禁

远程 Web 改动上线前必须检查：

1. `sw.js` 缓存版本是否按需更新
2. `main.tsx` 的 service worker 注册版本是否一致
3. 静态部署必须使用原子替换
4. 静态目录权限必须满足：
   - directory `755`
   - file `644`
5. 至少完成一次生产或准生产浏览器 smoke
6. 必须核对实际构建输出 hash 是否已完整替换到静态目录

### 10.5 真实回归矩阵

#### 本地桌面端

1. 进入 GUI 后初始化成功
2. 会话切换正常
3. 工具进度和子任务可见
4. 权限弹窗仍能响应

#### 远程 Web/PWA

1. 英文浏览器首次打开正常
2. 中文浏览器首次打开正常
3. 配对成功后刷新仍能恢复认证态
4. active session 恢复正确
5. runner 离线时输入框和 interrupt 同时禁用
6. approvals / artifacts / timeline 同时可见

#### 白屏与缓存

1. 构建新版本后旧缓存不会卡死在旧入口
2. 运行时异常会落到 `AppErrorBoundary`
3. “清缓存并重载”后可以重新进入页面
4. service worker 更新后 controller 切换只触发一次刷新

#### 移动端视口

1. 390x844 视口下首屏无横向溢出
2. session list 可开合
3. composer 可输入中文
4. approvals 可操作
5. artifact 下载入口可点击

---

## 11. 风险与回滚策略

### 风险一：共享过度，导致本地和远程一起回归

- 应对：
  - 先共享中层组件
  - 壳层保持分离
  - 每一步都保留现有路径可回滚

### 风险二：为了抽象而抽象，最终两边都不好改

- 应对：
  - transport 契约保持最小化
  - 先围绕当前真实字段建模
  - 不对未来未实现协议做预抽象

### 风险三：移动端体验优化引入发布事故

- 应对：
  - 把 PWA 缓存、静态资源部署、真实浏览器测试列为主线门禁
  - 不把它们留在“运维注意事项”

### 风险四：抽象层引入过多双向依赖

- 应对：
  - `session/` 只允许依赖类型和纯函数
  - `remote/` 与 `local/` 只实现 transport，不吞并共享组件
  - 共享组件不得直接 import `tauri.ts` 或 `remote/api.ts`

---

## 12. 执行红线

以下行为在 v2 实施过程中禁止出现：

1. 为了统一而改写现有线上协议
2. 在没有真实性能数据前先引入虚拟化
3. 在没有结构化 diff 契约前先实现大型 diff viewer
4. 在 `RemoteApp` 尚未拆干净前同步重构本地所有布局
5. 让共享组件直接依赖某一端专属 store 或 API

---

## 13. 最终实施顺序

1. 建共享领域类型和 normalize 层
2. 拆 `RemoteApp`，把远程 transport / auth / shell 分开
3. 抽共享展示组件
4. 接回本地桌面端
5. 做移动端壳层优化
6. 在此之后再做任务树增强、diff viewer、virtualization、theme

---

## 14. 最终判断

这份 v2 的核心不是“更炫的 UI 蓝图”，而是三句话：

- **尊重当前真实代码，而不是假设一套并不存在的架构。**
- **先统一领域模型和展示层，再提高复用率。**
- **把移动端和发布链路当成产品主架构的一部分，而不是附属优化。**

如果后续严格按这份 v2 推进，GUI 与 Remote 的进阶优化可以做到“渐进替换、持续可运行、每一步都可验收”，而不是一次高风险重构。
