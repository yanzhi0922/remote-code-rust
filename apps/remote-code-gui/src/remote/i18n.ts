import type {
  RemoteApprovalDecision,
  RemoteDaemonPresenceState,
  RemoteMessageRole,
  RemoteSessionState,
} from './types';

export type RemoteLocale = 'en' | 'zh-CN';
export type RemoteConnectionState = 'idle' | 'connecting' | 'open' | 'reconnecting' | 'error';

export interface RemoteCopy {
  remoteModeNotConfiguredTitle: string;
  remoteModeNotConfiguredDescription: string;
  contactingControlPlane: string;
  remoteShellEyebrow: string;
  remoteShellDescription: string;
  refreshSessions: string;
  loadingRemoteSessions: string;
  noSessionsTitle: string;
  noSessionsDescription: string;
  selectRemoteSession: string;
  pickSessionTitle: string;
  pickSessionDescription: string;
  loadingSessionTimeline: string;
  timelineEmptyTitle: string;
  timelineEmptyDescription: string;
  followUpControl: string;
  followUpPlaceholder: string;
  controlUnavailableUnassigned: string;
  controlUnavailableRunnerOffline: (runnerId: string, lastSeenLabel: string | null) => string;
  send: string;
  interrupt: string;
  interrupting: string;
  pendingApprovals: string;
  noPendingApprovals: string;
  artifacts: string;
  noArtifacts: string;
  authGateEyebrow: string;
  authGateTitle: string;
  authGateDescription: string;
  deviceNameLabel: string;
  deviceNamePlaceholder: string;
  bootstrapTitle: string;
  bootstrapDescription: string;
  bootstrapSecretLabel: string;
  claimOwnerDevice: string;
  acceptPairingTitle: string;
  acceptPairingDescription: string;
  offerIdPlaceholder: string;
  pairingSecretPlaceholder: string;
  acceptPairingAction: string;
  existingTokenTitle: string;
  existingTokenDescription: string;
  saveToken: string;
  clearSavedToken: string;
  controlPlaneEyebrow: string;
  ownerClaimedLabel: string;
  trustedDevicesLabel: string;
  availableRunnersLabel: string;
  activeSessionsLabel: string;
  bootstrapConfiguredLabel: string;
  browserTokenNotice: string;
  messageHeaders: Record<RemoteMessageRole, string>;
  sessionStateLabels: Record<RemoteSessionState, string>;
  approvalDecisionLabels: Record<RemoteApprovalDecision, string>;
  approvalStateLabels: Record<RemoteApprovalDecision | 'pending', string>;
  connectionLabels: Record<RemoteConnectionState, string>;
  daemonStates: Record<RemoteDaemonPresenceState, string>;
  eventEyebrows: {
    streaming: string;
    tool: string;
    approval: string;
    artifact: string;
    runtime: string;
    daemon: string;
    session: string;
    runner: string;
    subtask: string;
    batch: string;
    context: string;
  };
  renderResponse: string;
  responderLabel: string;
  statusBootstrapClaimSucceeded: string;
  statusPairingSucceeded: string;
  statusSavedAccessToken: string;
  statusClearedAccessToken: string;
  statusPromptForwarded: string;
  statusInterruptForwarded: string;
  statusArtifactDownloaded: (fileName: string) => string;
  statusApprovalDecision: (decision: string) => string;
  approvalWaiting: (title: string) => string;
  approvalResolved: (approvalId: string, state: string) => string;
  artifactCreated: (name: string, fileName: string, size: string) => string;
  artifactManifest: (count: number) => string;
  sessionCreated: (workspaceId: string) => string;
  sessionMoved: (previous: string, next: string) => string;
  runnerRegistered: (workspaceCount: number, leaseTtlSeconds: number) => string;
  runnerHeartbeat: (activeSessions: number, queuedSessions: number) => string;
  toolStarted: (toolCallId: string) => string;
  toolElapsed: (seconds: number) => string;
  toolRunning: string;
  toolFailedWithoutSummary: string;
  toolCompleted: string;
  daemonNow: (state: string) => string;
  runnerUnassigned: string;
  runnerOfflineLabel: string;
  yes: string;
  no: string;
  justNow: string;
  loading: string;
  errorBoundaryTitle: string;
  errorBoundaryDescription: string;
  errorBoundaryReload: string;
  errorBoundaryClearCache: string;
  errorBoundaryClearingCache: string;
  errorBoundaryDetails: string;
  openSessionDrawer: string;
  multiUserTitle: string;
  multiUserDescription: string;
  usernameLabel: string;
  usernamePlaceholder: string;
  passwordLabel: string;
  passwordPlaceholder: string;
  signInAction: string;
  statusSignInSucceeded: string;
  statusSignInFailed: string;
  signOutAction: string;
  passwordChangeWarning: string;
  strategyDirect: string;
  strategyRelay: string;
  strategyPolling: string;
  strategyHybrid: string;
  strategyQuic: string;
  latencyLabel: string;
  pushNotificationApprovalTitle: string;
  pushNotificationApprovalBody: (title: string) => string;
  pushNotificationSessionTitle: string;
  pushNotificationSessionBody: (sessionId: string) => string;
  shareArtifact: string;
  shareArtifactTitle: string;
  deepLinkPairingReceived: string;
  mobileNotificationsEnabled: string;
  mobileNotificationsDenied: string;
  mobileNotificationsUnavailable: string;
  mobileTabSessions: string;
  mobileTabTimeline: string;
  mobileTabApprovals: string;
  mobileTabConnect: string;
  mobileTabConnected: string;
  connectAction: string;
  disconnectAction: string;
  connectedTitle: string;
  notConnectedTitle: string;
  notConnectedDescription: string;
  enterServerUrlTitle: string;
  enterServerUrlDescription: string;
  scanQrAction: string;
  mobileAuthSubtitle: string;
  mobileExpandOptions: string;
  mobileCollapseOptions: string;
}

const ENGLISH_COPY: RemoteCopy = {
  remoteModeNotConfiguredTitle: 'Remote Mode Is Not Configured',
  remoteModeNotConfiguredDescription:
    'Open this UI from your control-plane domain, or pass `?mode=remote&control_plane_url=https://your-domain`.',
  contactingControlPlane: 'Contacting the control plane...',
  remoteShellEyebrow: 'Remote Shell',
  remoteShellDescription:
    'Timeline, approvals, artifact download, and follow-up control routed through your self-hosted control plane.',
  refreshSessions: 'Refresh Sessions',
  loadingRemoteSessions: 'Loading remote sessions...',
  noSessionsTitle: 'No Sessions Yet',
  noSessionsDescription:
    'Start a local session on your runner, then refresh here to attach from the browser.',
  selectRemoteSession: 'Select a remote session',
  pickSessionTitle: 'Pick A Session',
  pickSessionDescription:
    'The browser shell stays read-only until you attach to a session on the left.',
  loadingSessionTimeline: 'Loading session timeline...',
  timelineEmptyTitle: 'Timeline Is Empty',
  timelineEmptyDescription:
    'Once the local runner starts streaming events, message deltas, approvals, tools, and artifacts appear here.',
  followUpControl: 'Follow-up control for the current session',
  followUpPlaceholder:
    'Send a follow-up prompt to the local runner. Shift+Enter inserts a newline.',
  controlUnavailableUnassigned:
    'This session is not assigned to a local runner yet. Follow-up control unlocks after a runner claims it.',
  controlUnavailableRunnerOffline: (runnerId, lastSeenLabel) =>
    lastSeenLabel
      ? `Runner ${runnerId} is currently offline. Follow-up control is paused until it reconnects. Last heartbeat ${lastSeenLabel}.`
      : `Runner ${runnerId} is currently offline. Follow-up control is paused until it reconnects.`,
  send: 'Send',
  interrupt: 'Interrupt',
  interrupting: 'Interrupting...',
  pendingApprovals: 'Pending Approvals',
  noPendingApprovals: 'No pending approvals for the current session.',
  artifacts: 'Artifacts',
  noArtifacts: 'No artifacts have been published yet.',
  authGateEyebrow: 'Remote Access',
  authGateTitle: 'Authenticate This Device',
  authGateDescription:
    'The control plane is live, but this browser is not trusted yet. Claim the owner device first, or accept a short-lived pairing offer generated from a device that is already trusted.',
  deviceNameLabel: 'Device Name',
  deviceNamePlaceholder: 'My iPhone',
  bootstrapTitle: 'Bootstrap owner claim',
  bootstrapDescription:
    'Use the bootstrap secret from the server to mint the first trusted device token.',
  bootstrapSecretLabel: 'Bootstrap Secret',
  claimOwnerDevice: 'Claim Owner Device',
  acceptPairingTitle: 'Accept pairing offer',
  acceptPairingDescription:
    'Paste the offer id and pairing secret from a trusted device, or open the pairing URL directly on this phone.',
  offerIdPlaceholder: 'Offer ID',
  pairingSecretPlaceholder: 'Pairing secret',
  acceptPairingAction: 'Accept Pairing Offer',
  existingTokenTitle: 'Use an existing token',
  existingTokenDescription:
    'If you already minted a device token from the CLI, paste it here for this browser session.',
  saveToken: 'Save Token',
  clearSavedToken: 'Clear Saved Token',
  controlPlaneEyebrow: 'Control Plane',
  ownerClaimedLabel: 'Owner claimed',
  trustedDevicesLabel: 'Trusted devices',
  availableRunnersLabel: 'Available runners',
  activeSessionsLabel: 'Active sessions',
  bootstrapConfiguredLabel: 'Bootstrap configured',
  browserTokenNotice:
    'The browser keeps device tokens in session storage only. Session content still stays on your local machine; the control plane only brokers access to the runner.',
  messageHeaders: {
    assistant: 'Assistant',
    user: 'User',
    system: 'System',
  },
  sessionStateLabels: {
    pending: 'Pending',
    assigned: 'Assigned',
    running: 'Running',
    waiting_approval: 'Waiting Approval',
    completed: 'Completed',
    failed: 'Failed',
    cancelled: 'Cancelled',
  },
  approvalDecisionLabels: {
    approved: 'Approve',
    denied: 'Deny',
    cancelled: 'Cancel',
  },
  approvalStateLabels: {
    pending: 'Pending',
    approved: 'Approved',
    denied: 'Denied',
    cancelled: 'Cancelled',
  },
  connectionLabels: {
    idle: 'Idle',
    connecting: 'Connecting',
    open: 'Live',
    reconnecting: 'Reconnecting',
    error: 'Stream Error',
  },
  daemonStates: {
    online: 'online',
    offline: 'offline',
    reconnecting: 'reconnecting',
  },
  eventEyebrows: {
    streaming: 'Streaming',
    tool: 'Tool',
    approval: 'Approval',
    artifact: 'Artifact',
    runtime: 'Runtime',
    daemon: 'Daemon',
    session: 'Session',
    runner: 'Runner',
    subtask: 'Subtask',
    batch: 'Batch',
    context: 'Context',
  },
  renderResponse: 'Rendering response...',
  responderLabel: 'Responder',
  statusBootstrapClaimSucceeded: 'Bootstrap claim succeeded.',
  statusPairingSucceeded: 'Pairing succeeded.',
  statusSavedAccessToken: 'Saved access token for this browser session.',
  statusClearedAccessToken: 'Cleared the session access token.',
  statusPromptForwarded: 'Prompt forwarded to the local runner.',
  statusInterruptForwarded: 'Interrupt signal forwarded.',
  statusArtifactDownloaded: (fileName) => `Downloading artifact ${fileName}.`,
  statusApprovalDecision: (decision) => `Approval ${decision}.`,
  approvalWaiting: (title) => `${title} is waiting for a decision.`,
  approvalResolved: (approvalId, state) => `Approval ${approvalId} is now ${state}.`,
  artifactCreated: (name, fileName, size) => `${name} (${fileName}) published as ${size}.`,
  artifactManifest: (count) => `${count} artifact reference(s) published to the session.`,
  sessionCreated: (workspaceId) => `Session created for workspace ${workspaceId}.`,
  sessionMoved: (previous, next) => `Session moved from ${previous} to ${next}.`,
  runnerRegistered: (workspaceCount, leaseTtlSeconds) =>
    `Runner registered ${workspaceCount} workspace(s) with a ${leaseTtlSeconds}s lease.`,
  runnerHeartbeat: (activeSessions, queuedSessions) =>
    `Runner heartbeat: ${activeSessions} active, ${queuedSessions} queued.`,
  toolStarted: (toolCallId) => `Started tool call ${toolCallId}.`,
  toolElapsed: (seconds) => `Elapsed ${seconds}s.`,
  toolRunning: 'Tool is still running.',
  toolFailedWithoutSummary: 'Tool failed without a summary.',
  toolCompleted: 'Tool completed.',
  daemonNow: (state) => `Daemon is now ${state}.`,
  runnerUnassigned: 'unassigned runner',
  runnerOfflineLabel: 'offline',
  yes: 'yes',
  no: 'no',
  justNow: 'just now',
  loading: 'Working...',
  errorBoundaryTitle: 'The page hit a runtime error',
  errorBoundaryDescription:
    'Reload the page first. If the browser still shows a blank screen, clear the offline cache and retry.',
  errorBoundaryReload: 'Reload Page',
  errorBoundaryClearCache: 'Clear Cache And Reload',
  errorBoundaryClearingCache: 'Clearing cache...',
  errorBoundaryDetails: 'Error details',
  openSessionDrawer: 'Open session drawer',
  multiUserTitle: 'Sign in with credentials',
  multiUserDescription:
    'Enter the same username and password you provisioned on the desktop and control plane. The server accepts only configured user-key hashes.',
  usernameLabel: 'Username',
  usernamePlaceholder: 'your-name',
  passwordLabel: 'Password',
  passwordPlaceholder: 'your-password',
  signInAction: 'Sign In',
  statusSignInSucceeded: 'Signed in successfully.',
  statusSignInFailed: 'Sign in failed. Please check your credentials.',
  signOutAction: 'Sign Out',
  passwordChangeWarning:
    'Changing your username or password will create a new identity. All previous sessions and data will become inaccessible.',
  strategyDirect: 'Direct',
  strategyRelay: 'Relay',
  strategyPolling: 'Polling',
  strategyHybrid: 'Hybrid',
  strategyQuic: 'QUIC',
  latencyLabel: 'Latency',
  pushNotificationApprovalTitle: 'Approval Required',
  pushNotificationApprovalBody: (title) => `${title} needs your decision.`,
  pushNotificationSessionTitle: 'Session Update',
  pushNotificationSessionBody: (sessionId) => `Session ${sessionId} was updated.`,
  shareArtifact: 'Share',
  shareArtifactTitle: 'Share Artifact',
  deepLinkPairingReceived: 'Pairing details received from link.',
  mobileNotificationsEnabled: 'Push notifications enabled.',
  mobileNotificationsDenied: 'Push notification permission denied.',
  mobileNotificationsUnavailable: 'Push notifications are unavailable; foreground refresh remains active.',
  mobileTabSessions: 'Sessions',
  mobileTabTimeline: 'Timeline',
  mobileTabApprovals: 'Approvals',
  mobileAuthSubtitle: 'Connect to your desktop runner',
  mobileExpandOptions: 'Other sign-in methods',
  mobileCollapseOptions: 'Hide options',
  mobileTabConnect: 'Connect',
  mobileTabConnected: 'Server',
  connectAction: 'Connect',
  disconnectAction: 'Disconnect',
  connectedTitle: 'Connected',
  notConnectedTitle: 'Not Connected',
  notConnectedDescription: 'Enter your server URL or scan a QR code to get started.',
  enterServerUrlTitle: 'Server URL',
  enterServerUrlDescription: 'Enter the address of your Remote Code control plane server.',
  scanQrAction: 'Scan QR Code',
};

const CHINESE_COPY: RemoteCopy = {
  remoteModeNotConfiguredTitle: '未配置远程模式',
  remoteModeNotConfiguredDescription:
    '请从控制面域名打开此界面，或传入 `?mode=remote&control_plane_url=https://你的域名`。',
  contactingControlPlane: '正在连接控制面...',
  remoteShellEyebrow: '远程控制',
  remoteShellDescription:
    '时间线、审批、产物下载和后续控制都通过你自托管的控制面转发。',
  refreshSessions: '刷新会话',
  loadingRemoteSessions: '正在加载远程会话...',
  noSessionsTitle: '还没有会话',
  noSessionsDescription: '先在本地 runner 上启动会话，再回到这里刷新并接入。',
  selectRemoteSession: '请选择远程会话',
  pickSessionTitle: '先选择一个会话',
  pickSessionDescription: '在左侧接入某个会话之前，浏览器端会保持只读。',
  loadingSessionTimeline: '正在加载会话时间线...',
  timelineEmptyTitle: '时间线为空',
  timelineEmptyDescription:
    '本地 runner 开始推送事件后，消息流、审批、工具和产物都会显示在这里。',
  followUpControl: '当前会话的后续控制',
  followUpPlaceholder: '向本地 runner 发送后续提示词。Shift+Enter 可换行。',
  controlUnavailableUnassigned: '当前会话还没有分配本地 runner，等 runner 接管后才能发送后续控制。',
  controlUnavailableRunnerOffline: (runnerId, lastSeenLabel) =>
    lastSeenLabel
      ? `本地 runner ${runnerId} 当前离线，后续控制已暂停，等待其恢复连接。最近一次心跳：${lastSeenLabel}。`
      : `本地 runner ${runnerId} 当前离线，后续控制已暂停，等待其恢复连接。`,
  send: '发送',
  interrupt: '中断',
  interrupting: '正在中断...',
  pendingApprovals: '待处理审批',
  noPendingApprovals: '当前会话没有待处理审批。',
  artifacts: '产物',
  noArtifacts: '当前还没有发布任何产物。',
  authGateEyebrow: '远程访问',
  authGateTitle: '验证当前设备',
  authGateDescription:
    '控制面已经在线，但当前浏览器还不在受信列表中。请先认领 owner 设备，或者接受一台已受信设备发出的短时配对邀请。',
  deviceNameLabel: '设备名称',
  deviceNamePlaceholder: '我的手机',
  bootstrapTitle: '初始化 owner 认领',
  bootstrapDescription: '使用服务器上的 bootstrap secret 生成第一枚受信设备令牌。',
  bootstrapSecretLabel: 'Bootstrap Secret',
  claimOwnerDevice: '认领 Owner 设备',
  acceptPairingTitle: '接受配对邀请',
  acceptPairingDescription:
    '粘贴受信设备生成的 offer id 和 pairing secret，或直接在手机上打开配对链接。',
  offerIdPlaceholder: 'Offer ID',
  pairingSecretPlaceholder: '配对密钥',
  acceptPairingAction: '接受配对邀请',
  existingTokenTitle: '使用已有令牌',
  existingTokenDescription: '如果你已经从 CLI 生成了设备令牌，可以粘贴给当前浏览器会话使用。',
  saveToken: '保存令牌',
  clearSavedToken: '清除已保存令牌',
  controlPlaneEyebrow: '控制面',
  ownerClaimedLabel: 'Owner 已认领',
  trustedDevicesLabel: '受信设备数',
  availableRunnersLabel: '可用 Runner 数',
  activeSessionsLabel: '活跃会话数',
  bootstrapConfiguredLabel: 'Bootstrap 已配置',
  browserTokenNotice:
    '浏览器只在当前会话中保存设备令牌。会话内容仍留在你的本地机器上，控制面只负责把访问路由到 runner。',
  messageHeaders: {
    assistant: '助手',
    user: '用户',
    system: '系统',
  },
  sessionStateLabels: {
    pending: '等待中',
    assigned: '已分配',
    running: '运行中',
    waiting_approval: '等待审批',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  },
  approvalDecisionLabels: {
    approved: '批准',
    denied: '拒绝',
    cancelled: '取消',
  },
  approvalStateLabels: {
    pending: '待处理',
    approved: '已批准',
    denied: '已拒绝',
    cancelled: '已取消',
  },
  connectionLabels: {
    idle: '空闲',
    connecting: '连接中',
    open: '实时连接',
    reconnecting: '重连中',
    error: '流连接异常',
  },
  daemonStates: {
    online: '在线',
    offline: '离线',
    reconnecting: '重连中',
  },
  eventEyebrows: {
    streaming: '流式消息',
    tool: '工具',
    approval: '审批',
    artifact: '产物',
    runtime: '运行时',
    daemon: '守护进程',
    session: '会话',
    runner: 'Runner',
    subtask: '子任务',
    batch: '批量',
    context: '上下文',
  },
  renderResponse: '正在渲染回复...',
  responderLabel: '处理人',
  statusBootstrapClaimSucceeded: 'Bootstrap 认领成功。',
  statusPairingSucceeded: '配对成功。',
  statusSavedAccessToken: '已为当前浏览器会话保存访问令牌。',
  statusClearedAccessToken: '已清除会话访问令牌。',
  statusPromptForwarded: '提示词已转发给本地 runner。',
  statusInterruptForwarded: '中断信号已转发。',
  statusArtifactDownloaded: (fileName) => `正在下载产物 ${fileName}。`,
  statusApprovalDecision: (decision) => `审批已${decision}。`,
  approvalWaiting: (title) => `${title} 正在等待处理。`,
  approvalResolved: (approvalId, state) => `审批 ${approvalId} 当前状态为 ${state}。`,
  artifactCreated: (name, fileName, size) => `${name}（${fileName}）已发布，大小 ${size}。`,
  artifactManifest: (count) => `当前会话已发布 ${count} 个产物引用。`,
  sessionCreated: (workspaceId) => `已为工作区 ${workspaceId} 创建会话。`,
  sessionMoved: (previous, next) => `会话状态已从 ${previous} 变为 ${next}。`,
  runnerRegistered: (workspaceCount, leaseTtlSeconds) =>
    `Runner 已注册 ${workspaceCount} 个工作区，租约时长 ${leaseTtlSeconds} 秒。`,
  runnerHeartbeat: (activeSessions, queuedSessions) =>
    `Runner 心跳：活跃 ${activeSessions} 个，排队 ${queuedSessions} 个。`,
  toolStarted: (toolCallId) => `工具调用 ${toolCallId} 已启动。`,
  toolElapsed: (seconds) => `已运行 ${seconds} 秒。`,
  toolRunning: '工具仍在运行中。',
  toolFailedWithoutSummary: '工具失败，且没有返回摘要。',
  toolCompleted: '工具已完成。',
  daemonNow: (state) => `守护进程当前状态：${state}。`,
  runnerUnassigned: '未分配 Runner',
  runnerOfflineLabel: '离线',
  yes: '是',
  no: '否',
  justNow: '刚刚',
  loading: '处理中...',
  errorBoundaryTitle: '页面运行时发生错误',
  errorBoundaryDescription: '请先刷新页面；如果浏览器仍然白屏，再清理离线缓存后重试。',
  errorBoundaryReload: '刷新页面',
  errorBoundaryClearCache: '清理缓存并重载',
  errorBoundaryClearingCache: '正在清理缓存...',
  errorBoundaryDetails: '错误详情',
  openSessionDrawer: '打开会话抽屉',
  multiUserTitle: '使用账户登录',
  multiUserDescription:
    '输入已在桌面端和控制面配置过的用户名和密码。服务器只接受显式配置的 user-key 哈希。',
  usernameLabel: '用户名',
  usernamePlaceholder: '你的用户名',
  passwordLabel: '密码',
  passwordPlaceholder: '你的密码',
  signInAction: '登录',
  statusSignInSucceeded: '登录成功。',
  statusSignInFailed: '登录失败，请检查用户名和密码。',
  signOutAction: '退出登录',
  passwordChangeWarning:
    '更改用户名或密码会创建新的身份，之前的所有会话和数据将无法访问。',
  strategyDirect: '直连',
  strategyRelay: '中继',
  strategyPolling: '轮询',
  strategyHybrid: '混合',
  strategyQuic: 'QUIC',
  latencyLabel: '延迟',
  pushNotificationApprovalTitle: '需要审批',
  pushNotificationApprovalBody: (title) => `${title} 需要你的决定。`,
  pushNotificationSessionTitle: '会话更新',
  pushNotificationSessionBody: (sessionId) => `会话 ${sessionId} 已更新。`,
  shareArtifact: '分享',
  shareArtifactTitle: '分享产物',
  deepLinkPairingReceived: '已从链接获取配对信息。',
  mobileNotificationsEnabled: '推送通知已启用。',
  mobileNotificationsDenied: '推送通知权限被拒绝。',
  mobileNotificationsUnavailable: '推送通知当前不可用，前台刷新仍会保持运行。',
  mobileTabSessions: '会话',
  mobileTabTimeline: '时间线',
  mobileTabApprovals: '审批',
  mobileAuthSubtitle: '连接到你的桌面 Runner',
  mobileExpandOptions: '其他登录方式',
  mobileCollapseOptions: '收起',
  mobileTabConnect: '连接',
  mobileTabConnected: '服务器',
  connectAction: '连接',
  disconnectAction: '断开连接',
  connectedTitle: '已连接',
  notConnectedTitle: '未连接',
  notConnectedDescription: '输入服务器地址或扫描二维码开始使用。',
  enterServerUrlTitle: '服务器地址',
  enterServerUrlDescription: '输入你的 Remote Code 控制平面服务器地址。',
  scanQrAction: '扫描二维码',
};

export function resolveRemoteLocale(): RemoteLocale {
  if (typeof window === 'undefined') {
    return 'en';
  }

  const params = new URLSearchParams(window.location.search);
  const explicitLang = params.get('lang')?.trim().toLowerCase();
  if (explicitLang?.startsWith('zh')) {
    return 'zh-CN';
  }
  if (explicitLang?.startsWith('en')) {
    return 'en';
  }

  const candidates = [
    ...(window.navigator.languages ?? []),
    window.navigator.language,
    document.documentElement.lang,
  ].filter(Boolean);

  return candidates.some((candidate) => candidate.toLowerCase().startsWith('zh')) ? 'zh-CN' : 'en';
}

export function getRemoteCopy(locale: RemoteLocale): RemoteCopy {
  return locale === 'zh-CN' ? CHINESE_COPY : ENGLISH_COPY;
}

export function formatRemoteRelativeTime(iso: string, locale: RemoteLocale, copy: RemoteCopy): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  if (!Number.isFinite(diffMs)) {
    return new Intl.DateTimeFormat(locale).format(new Date(iso));
  }

  const diffMinutes = Math.floor(diffMs / 60_000);
  if (diffMinutes < 1) {
    return copy.justNow;
  }

  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  if (diffMinutes < 60) {
    return rtf.format(-diffMinutes, 'minute');
  }

  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) {
    return rtf.format(-diffHours, 'hour');
  }

  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) {
    return rtf.format(-diffDays, 'day');
  }

  return new Intl.DateTimeFormat(locale, {
    month: 'numeric',
    day: 'numeric',
  }).format(new Date(iso));
}
