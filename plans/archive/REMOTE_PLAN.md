# remote-code-rust 远程控制 v1 最终方案

## Summary
- 最优路线不是去兼容 Paseo 的 daemon/protocol，也不是单独起一个新产品重写；而是以现有 `remote-code-rust` 远程骨架为基础，做一套 clean-room 的自有远程栈。
- 架构锁定为：本地 `remote-code` 运行时 + 本地 daemon/runner + 腾讯云控制面 + 自建 relay + `PWA/Web` 优先；手机原生壳和更重的远程能力后置。
- 腾讯云服务器在 v1 只承担“控制面 + 中继”职责，不承载代码执行。你的 4C/4G/3Mbps 机器足够支撑 v1 的文本流、审批流、事件回放和小文件中继，不适合做云端 runner。
- 正式上线方案锁定为“域名 + HTTPS + 大陆合规备案”。公网 IP 证书只作为开发/联调兜底，不作为正式产品默认路径。
- 安全边界锁定为：服务器只保存元数据和密文，不保存代码内容、diff、工具参数、文件正文的明文。由于服务器在大陆地域，内容端到端加密是 v1 必做项，不是加分项。

## Implementation Changes
### 1. 本地执行面：把 runner 做成真正的 daemon
- 将现有 runner 从“会话/审批占位簿记”升级为真实的本地常驻进程，负责拉起和监管 headless `remote-code` 子进程。
- Windows 作为一等宿主平台处理，不能依赖 POSIX-only 语义；子进程创建、终止、重连、清理都按跨平台 supervisor 设计。
- 以现有 headless 协议为唯一执行内核，统一把运行时输出映射成规范远程事件：`message_delta`、`message_committed`、`approval_requested`、`approval_resolved`、`tool_started`、`tool_progress`、`tool_finished`、`artifact_available`、`session_state_changed`、`runtime_error`。
- daemon 只接受这几类命令：`start_session`、`send_prompt`、`interrupt`、`approval_decision`、`artifact_manifest`、`artifact_fetch`。v1 不做离线命令排队；daemon 离线时，控制面直接失败返回，不做隐式补发。

### 2. 控制面：从 CRUD 骨架升级为持久化远程控制核心
- 保留现有 control plane 角色，但职责改为“权威元数据服务 + 密文事件日志 + 订阅分发 + 配对/鉴权 + 审批审计 + relay 票据签发”。
- 存储锁定为 PostgreSQL，v1 不引入 Redis。单机单区部署，依靠数据库持久化和进程内 fan-out 即可。
- 核心数据模型锁定为：`devices`、`pairing_offers`、`sessions`、`session_members`、`event_log`、`approvals`、`artifacts`、`relay_tickets`、`audit_records`。
- 每个 session 使用单调递增 `cursor` 作为唯一事件顺序源；所有客户端都基于 `after_cursor` 回放和重连，不使用时间戳去猜顺序。
- 命令幂等性锁定为 `client_command_id`；重复提交必须返回同一逻辑结果，不能重复执行。

### 3. 安全与配对：自建协议，端到端加密
- 明确不复用 Paseo 代码，不 fork，不做协议级兼容，避免 AGPL 污染和后续协议耦合。
- 身份模型锁定为“设备身份优先”，不是第三方 SSO 优先。v1 不接 GitHub/OIDC，不引入外部身份依赖。
- 服务端部署时生成一次性 `owner bootstrap secret`。第一台本地 daemon 用它完成所有者绑定；之后新增手机/PWA 设备，只能通过已受信任的 daemon 二次配对。
- 每台 daemon 和每个 PWA 安装实例都拥有独立长期设备密钥。配对流程固定为：daemon 生成短时 QR offer，手机扫码后双方显示同一组六词校验短语，人工确认后建立设备信任。
- 内容加密范围锁定为：用户 prompt、assistant 输出、审批请求/决策、工具事件负载、artifact 元数据。控制面只看到会话 ID、设备 ID、cursor、时间戳和密文包。
- 抗重放是硬要求：每个 session/channel 使用严格单调计数器，relay ticket 带短时效和单次使用限制，重复包和过期票据一律 fail-closed。

### 4. Relay：无信任中继，v1 先服务文件/二进制通道
- relay 单独作为“看不懂内容的转发器”，不承担权限判定和业务状态机。
- v1 文本控制流以控制面 WS 为主，保证事件持久化、回放和审计稳定；relay 在 v1 先承担 artifact 拉取和未来大流量通道预留，不把正确性建立在 relay 上。
- relay 帧类型固定为：`open`、`data`、`ack`、`close`、`error`。所有业务负载都是 opaque ciphertext。
- Phase 2 再把 terminal、多路实时流、文件预览等更重通道迁到 relay，不在 v1 把问题一次做大。

### 5. 客户端：PWA/Web 优先，直接面向网络
- 客户端形态锁定为 PWA，目标是手机浏览器可用、可安装、可恢复，不再依赖 Tauri 本地 IPC。
- v1 功能范围锁定为：session 列表、时间线、实时 follow-up、审批卡片、interrupt、cursor 重连、artifact 列表和下载。
- UI 语义复用现有 GUI 的会话/审批/工具进度模型，但 transport 全面改为 control plane REST + WS。
- 原生 iOS/Android 壳、推送、语音、终端视图、文件树、diff 浏览全部延后到协议稳定后再做，不在 v1 混进主线。

### 6. 基础设施与上线方式
- 腾讯云机器运行四个组件：`reverse proxy`、`control-plane`、`relay`、`postgres`。单机部署即可，不做分布式。
- 对外仅开放 `80/443`；其余服务仅内网访问。
- 正式上线使用域名、TLS 和大陆合规备案。IP 直连 + 短周期 IP 证书只用于 bring-up 或短期内测，不作为稳定交付路径。
- 默认开启结构化日志、健康检查、基础指标、日志脱敏、每日数据库备份和 7 天保留。
- 带宽策略锁定为“文本优先、小文件优先”；artifact 下载默认限速，避免 3Mbps 上行被大文件拖垮。

## Public Interfaces
- 配对接口：
  - `POST /v1/pairing/offers`
  - `POST /v1/pairing/complete`
- 会话与控制接口：
  - `GET /v1/sessions`
  - `GET /v1/sessions/{id}`
  - `GET /v1/sessions/{id}/events?after=<cursor>`
  - `POST /v1/sessions/{id}/commands`
  - `POST /v1/sessions/{id}/approvals/{approval_id}`
  - `POST /v1/sessions/{id}/interrupt`
  - `GET /v1/sessions/{id}/artifacts`
- 实时订阅接口：
  - `GET /v1/ws`
  - 统一 envelope 结构固定为：`session_id`、`cursor`、`kind`、`ciphertext`、`sent_at`
- relay 接口：
  - `GET /v1/relay/connect`
  - 只接受控制面签发的短时票据
- 事件语义固定为：
  - `message_delta`
  - `message_committed`
  - `approval_requested`
  - `approval_resolved`
  - `tool_started`
  - `tool_progress`
  - `tool_finished`
  - `artifact_available`
  - `session_state_changed`
  - `runtime_error`

## Delivery Phases
1. Phase 1a：本地 daemon 成型
   - 完成 headless supervisor、事件规范化、命令入口、跨平台进程管理、cursor 模型
2. Phase 1b：控制面成型
   - 完成 PostgreSQL 持久化、REST/WS API、设备身份、配对、审计、事件回放
3. Phase 1c：PWA 成型
   - 完成 session 列表、时间线、follow-up、审批、interrupt、重连恢复、artifact 列表
4. Phase 1d：relay 上线
   - 完成票据、密文帧、artifact 通道、限速和抗重放校验
5. Phase 2：增强远程交互
   - terminal、多路流、文件预览、diff 浏览、原生手机壳
6. Phase 3：可选扩展
   - 云端 runner、多工作站调度、外部 agent/provider 适配器；如果未来要接 ACP，也只在 provider 边界做 adapter，不污染 remote runtime

## Test Plan
- 单元测试：
  - `claude-protocol` 到规范远程事件的映射
  - `client_command_id` 幂等性
  - `cursor` 回放与去重
  - 审批生命周期和 interrupt 语义
  - 密文 envelope 编解码、计数器和重放拒绝
- 集成测试：
  - Windows 宿主机 daemon 能稳定拉起/停止 headless runtime
  - 控制面重启后 session 元数据和历史事件不丢失
  - daemon 掉线后 session 正确进入不可交互状态，但历史仍可查看
  - artifact 票据过期、盗用、复用全部被拒绝
- 网络测试：
  - 200–500ms RTT 和 3Mbps 限速下的实时体验
  - WS 断线重连后无事件丢失、无重复提交
  - 手机 Wi‑Fi/蜂窝切换后的恢复行为
- 端到端验收：
  - 桌面启动 session，手机扫码接入，实时看到时间线
  - 手机发送 follow-up，本地机器执行，结果在手机上连续可见
  - 手机完成审批，本地 session 正常继续
  - 浏览器刷新或后台恢复后，从上次 `cursor` 精确续播
- 安全验收：
  - 服务端数据库和日志中不存在代码正文、diff、工具参数的明文
  - relay 无法解密 payload
  - 过期 pairing offer、重放 envelope、重复 approval、过期 relay ticket 全部 fail-closed

## Assumptions
- v1 是单用户自托管产品，不做多租户 SaaS，不做团队权限系统。
- 本地工作站是唯一执行宿主；腾讯云只做控制面和中继，不做代码执行。
- 不做 Paseo 协议兼容，不复用 Paseo 代码，只借鉴其产品思路。
- 服务器默认不可信，必须以“只存元数据和密文”为设计前提。
- 语音、推送通知、终端多路复用、工作区浏览、完整文件同步、云端 runner 都不在 v1 范围内。
- 你当前这台腾讯云服务器足够支撑 v1；只有当你要上云端执行、大文件频繁传输或终端流量明显增加时，才需要先升级带宽和内存。
