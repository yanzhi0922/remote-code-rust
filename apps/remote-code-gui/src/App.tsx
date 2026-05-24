import { lazy, Suspense, useState, useEffect, useCallback } from 'react';
import { LockKeyhole, TriangleAlert } from 'lucide-react';
import { Layout } from './components/layout/Layout';
import { PermissionModal } from './components/layout/PermissionModal';
import { ChatArea } from './components/chat/ChatArea';
import { ChatInput } from './components/chat/ChatInput';
import { ThemeProvider, useTheme } from './components/design/ThemeProvider';
import { AppErrorBoundary } from './components/layout/AppErrorBoundary';
import { hasTauriRuntime, shouldUseRemoteMode } from './lib/runtime';
import { isMobileSync, isTouchDevice } from './lib/mobile';
import { MarketingSite } from './marketing/MarketingSite';
import type {
  AgentType,
  ConversationEntry,
  FullSettings,
  PermissionRequestInfo,
  ProjectInfo,
  ProviderConfigList,
  ProviderInfo,
  RuntimeStatusInfo,
  SessionSummary,
} from './lib/types';
import {
  performBiometricCheck,
  initNetworkMonitoring,
  getNetworkStatus,
  onNetworkChange,
  describeConnectionType,
  hapticSuccess,
  hapticError,
  hapticWarning,
} from './lib/mobile';
import { useAppStore } from './stores/useAppStore';
import { useAgentStore } from './stores/useAgentStore';

const RemoteApp = lazy(() => import('./remote/RemoteApp'));
const MobileRemoteApp = lazy(() => import('./remote/MobileRemoteApp'));

type MobileInitPhase = 'loading' | 'biometric' | 'ready' | 'error';
type WorkbenchDemoScene = 'main' | 'empty' | 'running' | 'permission' | 'settings' | 'mcp' | 'light' | 'dark';

function shouldUseWorkbenchDemo(): boolean {
  if (!import.meta.env.DEV || typeof window === 'undefined') return false;
  return new URLSearchParams(window.location.search).has('workbench-demo');
}

function shouldUseLocalWorkbenchPreview(nativeRuntime: boolean): boolean {
  if (nativeRuntime || typeof window === 'undefined') return false;
  const params = new URLSearchParams(window.location.search);
  if (params.get('mode') === 'local' || params.has('workbench')) return true;
  return import.meta.env.DEV;
}

function getWorkbenchDemoScene(): WorkbenchDemoScene {
  if (typeof window === 'undefined') return 'main';
  const value = new URLSearchParams(window.location.search).get('workbench-demo');
  if (
    value === 'empty' ||
    value === 'running' ||
    value === 'permission' ||
    value === 'settings' ||
    value === 'mcp' ||
    value === 'light' ||
    value === 'dark'
  ) {
    return value;
  }
  return 'main';
}

const demoProjectPath = 'D:\\remote-code-rust';
const demoIsoNow = '2026-05-23T09:10:00.000Z';

const demoProvider: ProviderInfo = {
  name: 'glm-coding',
  model: 'glm-5.1-coder',
  protocol: 'openai',
  base_url: 'https://api.example.invalid/v1',
};

const demoSettings: FullSettings = {
  provider_name: demoProvider.name,
  provider_model: demoProvider.model,
  provider_base_url: demoProvider.base_url,
  provider_protocol: demoProvider.protocol,
  provider_api_key_set: true,
  max_output_tokens: 4096,
  thinking_budget: null,
  max_retries: 2,
  timeout_ms: 30000,
  retry_initial_backoff_ms: 800,
  retry_max_backoff_ms: 10000,
  respect_retry_after: true,
  permission_mode: 'acceptEdits',
  max_turns: 128,
  verbose: false,
  codex_model_provider: null,
  codex_approval_policy: null,
  codex_sandbox_mode: null,
  codex_persist_extended_history: true,
  codex_memories_enabled: true,
  codex_thread_store_endpoint: null,
  codex_config_overrides: {},
  codex_permission_profile: null,
  codex_service_tier: null,
  codex_ephemeral: null,
};

const demoRuntimeStatus: RuntimeStatusInfo = {
  session_name: 'gui-workbench-redesign',
  provider: {
    name: demoProvider.name,
    model: demoProvider.model,
    protocol: demoProvider.protocol,
    base_url: demoProvider.base_url,
    auth_source: 'keychain',
    effort: 'medium',
    fallback_model: null,
  },
  permission_mode: demoSettings.permission_mode,
  output_style: 'concise',
  language: 'zh-CN',
  brief_enabled: true,
  proactive_active: false,
  setting_sources: ['profile', 'project'],
  allowed_setting_sources: ['profile', 'project'],
  allowed_tools: ['read_file', 'rg', 'apply_patch'],
  disallowed_tools: [],
  mcp: {
    total_servers: 5,
    enabled_servers: 4,
    disabled_servers: 1,
    unique_server_names: 5,
    ambiguous_server_names: 0,
    warning_count: 1,
    origins: { cwd: 2, profile: 2, explicit: 1, plugin: 0 },
    status_counts: { connected: 3, failed: 0, needs_auth: 1, pending: 0, disabled: 1 },
  },
};

const demoProjects: ProjectInfo[] = [
  {
    path: demoProjectPath,
    name: 'remote-code-rust',
    session_count: 3,
    is_auto_detected: false,
  },
  {
    path: 'D:\\remote-code-rust\\apps\\remote-code-gui',
    name: 'remote-code-gui',
    session_count: 2,
    is_auto_detected: true,
  },
];

const demoSessions: SessionSummary[] = [
  {
    id: 'demo-session-1',
    title: 'Remote Code GUI workbench redesign',
    cwd: demoProjectPath,
    provider_name: demoProvider.name,
    model: demoProvider.model,
    created_at: '2026-05-23T08:00:00.000Z',
    updated_at: demoIsoNow,
    archived: false,
  },
  {
    id: 'demo-session-2',
    title: 'MCP inventory audit',
    cwd: demoProjectPath,
    provider_name: 'claude',
    model: 'claude-sonnet-4.5',
    created_at: '2026-05-22T10:00:00.000Z',
    updated_at: '2026-05-23T07:38:00.000Z',
    archived: false,
  },
  {
    id: 'demo-session-3',
    title: 'Provider profile cleanup',
    cwd: 'D:\\remote-code-rust\\apps\\remote-code-gui',
    provider_name: 'codex',
    model: 'gpt-5-codex',
    created_at: '2026-05-21T12:00:00.000Z',
    updated_at: '2026-05-22T16:18:00.000Z',
    archived: false,
  },
];

const demoConversation: ConversationEntry[] = [
  {
    role: 'user',
    text: '把桌面端改成更像长期使用的 coding agent IDE，重点处理聊天、composer、MCP 和设置页。',
    content_blocks: [],
    tool_calls: [],
    tool_call_id: null,
    name: null,
    is_error: false,
  },
  {
    role: 'assistant',
    text: '已把外壳调整为 IDE workbench：左侧 activity bar 和 explorer，中间高密度会话流，右侧 inspector，底部状态栏和固定 composer。',
    content_blocks: [{ type: 'thinking', thinking: '先保留现有 Tauri contract，再只调整前端布局、token 和组件密度。' }],
    tool_calls: [
      {
        id: 'tool-1',
        name: 'shell_command',
        input: { command: 'rg -n "rounded|shadow|gradient" src/components src/styles' },
      },
      {
        id: 'tool-2',
        name: 'apply_patch',
        input: { files: ['Layout.tsx', 'ChatArea.tsx', 'ChatInput.tsx'] },
      },
    ],
    tool_call_id: null,
    name: null,
    is_error: false,
  },
  {
    role: 'tool',
    text: 'src/components/layout/Sidebar.tsx: active row still used light blue dashboard styling\nsrc/components/layout/SettingsPanel.tsx: several rounded-2xl controls remained',
    content_blocks: [],
    tool_calls: [],
    tool_call_id: 'tool-1',
    name: 'shell_command',
    is_error: false,
  },
];

const demoProviderConfigs: ProviderConfigList = {
  active_provider: demoProvider.name,
  providers: [
    {
      name: demoProvider.name,
      protocol: demoProvider.protocol,
      base_url: demoProvider.base_url ?? undefined,
      model: demoProvider.model ?? undefined,
      api_key: '',
      api_key_stored: true,
      profiles: [
        { name: 'fast', model: 'glm-5.1-air' },
        { name: 'reasoning', model: 'glm-5.1-coder' },
      ],
      active_profile: 'reasoning',
    },
    {
      name: 'codex',
      protocol: 'openai',
      base_url: 'https://api.openai.com/v1',
      model: 'gpt-5-codex',
      api_key: '',
      api_key_stored: true,
    },
  ],
};

const demoPermission: PermissionRequestInfo = {
  request_id: 'permission-demo-1',
  tool_name: 'shell_command',
  tool_use_id: 'tool-write-1',
  title: '需要确认命令执行',
  description: 'npm run build',
  input: { command: 'npm run build', cwd: 'D:\\remote-code-rust\\apps\\remote-code-gui' },
  blocked_path: null,
  permission_suggestions: [],
};

function DemoStoreSeeder({ scene }: { scene: WorkbenchDemoScene }) {
  useEffect(() => {
    // Dev-only: guard against double-initialisation in React StrictMode.
    if (useAppStore.getState().initialised) return;

    const empty = scene === 'empty';
    const running = scene === 'running';
    const activeAgentType: AgentType = scene === 'settings' || scene === 'mcp' ? 'remote_codex' : 'remote_claude';

    useAppStore.setState({
      initialised: true,
      initError: null,
      listenersRegistered: true,
      provider: demoProvider,
      runtimeStatus: demoRuntimeStatus,
      projects: empty ? [] : demoProjects,
      activeProjectPath: empty ? null : demoProjectPath,
      workspacePrivacyMode: false,
      sessions: empty ? [] : demoSessions,
      archivedSessions: [],
      sessionsLoading: false,
      activeSessionId: empty ? null : 'demo-session-1',
      conversation: empty ? [] : demoConversation,
      conversationLoading: false,
      sending: running,
      sendError: null,
      lastPromptResult: {
        session_id: 'demo-session-1',
        text: 'done',
        tool_calls: [],
        usage: { input_tokens: 18420, output_tokens: 2384, total_tokens: 20804 },
        num_turns: 7,
        stop_reason: 'end_turn',
      },
      liveToolProgress: running
        ? [
            {
              tool_call_id: 'tool-3',
              tool_name: 'npm_test',
              message: 'Running focused component tests',
              active_form: 'npx vitest run src/components/chat/ChatInput.test.tsx',
            },
          ]
        : [],
      liveToolResults: running
        ? [
            {
              tool_call_id: 'tool-2',
              tool_name: 'apply_patch',
              is_error: false,
              output: 'Updated workbench shell components',
            },
          ]
        : [],
      batchProgressBySession: {},
      contextUsageBySession: {
        'demo-session-1': {
          session_id: 'demo-session-1',
          estimated_tokens: 58200,
          max_input_tokens: 128000,
          threshold_tokens: 102400,
          ratio: 0.45,
        },
      },
      contextOverflowBySession: {},
      contextCompactionBySession: {},
      streamingText: running ? 'Verifying layout density and composer controls...' : '',
      runningSessionIds: running ? new Set(['demo-session-1']) : new Set(),
      settings: demoSettings,
      settingsLoading: false,
      providerConfigs: demoProviderConfigs,
      pendingPermission: scene === 'permission' ? demoPermission : null,
      goalState: null,
      pendingGoalObjective: null,
    });

    useAgentStore.setState({
      activeAgentType,
      availableAgents: [
        { agentType: 'remote_codex', displayName: 'Codex', available: true, installed: true },
        { agentType: 'remote_claude', displayName: 'Claude', available: true, installed: true },
        { agentType: 'remote_roo', displayName: 'Roo', available: true, installed: true },
      ],
      agentStatuses: {
        remote_codex: 'ready',
        remote_claude: running ? 'running' : 'ready',
        remote_roo: 'ready',
      },
      sessionTasks: {
        'demo-session-1': [
          {
            session_id: 'demo-session-1',
            task_id: 'task-layout',
            parent_task_id: null,
            description: 'Refine workbench shell',
            depth: 0,
            status: running ? 'running' : 'completed',
            summary: running ? 'Running visual QA' : 'Layout, inspector, status bar complete',
            output_preview: running ? 'Running visual QA' : 'Layout, inspector, status bar complete',
            turns_used: 3,
            updated_at: demoIsoNow,
          },
        ],
      },
    });
  }, [scene]);

  return null;
}

function DemoThemeMode({ theme }: { theme: 'light' | 'dark' }) {
  const { setMode } = useTheme();

  useEffect(() => {
    setMode(theme);
  }, [setMode, theme]);

  return null;
}

function DemoLocalApp() {
  const scene = getWorkbenchDemoScene();
  const theme = scene === 'dark' ? 'dark' : 'light';
  const initialSettingsOpen = scene === 'settings' || scene === 'mcp';
  const initialSettingsTab = scene === 'mcp' ? 'mcp' : 'provider';

  useEffect(() => {
    try {
      localStorage.setItem('rc-theme-mode', theme);
    } catch {}
  }, [theme]);

  return (
    <ThemeProvider defaultTheme={theme}>
      <DemoThemeMode theme={theme} />
      <DemoStoreSeeder scene={scene} />
      <Layout initialSettingsOpen={initialSettingsOpen} initialSettingsTab={initialSettingsTab}>
        <div className="flex h-full min-h-0 flex-col bg-transparent">
          <ChatArea />
          <ChatInput />
        </div>
      </Layout>
      <PermissionModal />
    </ThemeProvider>
  );
}

function MobileInitScreen() {
  return (
    <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base">
      <div className="flex flex-col items-center gap-4">
        <img src="/pwa-icon-192.png" alt="" className="h-14 w-14 rounded-2xl shadow-lg" draggable={false} />
        <div className="flex items-center gap-3 text-rc-text-secondary">
          <div role="status" className="h-5 w-5 rounded-full border-2 border-rc-border-primary border-t-rc-text-primary animate-spin" />
          <span className="text-sm font-medium">正在初始化...</span>
        </div>
      </div>
    </div>
  );
}

function RemoteLazyFallback() {
  return (
    <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base">
      <div className="flex items-center gap-3 text-rc-text-secondary">
        <div role="status" className="h-5 w-5 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-text-primary" />
        <span className="text-sm font-medium">正在加载 Remote Code...</span>
      </div>
    </div>
  );
}

function MobileBiometricScreen() {
  return (
    <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base">
      <div className="flex flex-col items-center gap-4">
        <div className="h-14 w-14 rounded-2xl bg-rc-bg-user-bubble flex items-center justify-center shadow-lg">
          <LockKeyhole size={24} className="text-rc-text-inverse" />
        </div>
        <p className="text-sm text-rc-text-secondary font-medium">请验证身份</p>
      </div>
    </div>
  );
}

function MobileErrorScreen({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base px-6">
      <div className="max-w-sm text-center space-y-4">
        <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-rc-accent-error-bg text-rc-accent-error">
          <TriangleAlert size={24} />
        </div>
        <h1 className="text-lg font-bold text-rc-text-primary">初始化失败</h1>
        <p role="alert" className="text-sm text-rc-text-secondary break-all">{error}</p>
        <button
          onClick={onRetry}
          className="px-4 py-2 bg-rc-bg-user-bubble text-rc-text-inverse rounded-lg text-sm font-medium hover:opacity-90 transition-colors"
        >
          重试
        </button>
      </div>
    </div>
  );
}

function MobileNetworkBanner({ online, connectionType }: { online: boolean; connectionType: string }) {
  if (online) return null;
  return (
    <div role="alert" className="fixed top-0 left-0 right-0 z-50 bg-rc-accent-warning text-rc-text-inverse text-center py-1.5 text-xs font-medium shadow-md">
      网络已断开 — {describeConnectionType(connectionType)}
    </div>
  );
}

function MobileGate({ children }: { children: React.ReactNode }) {
  const [phase, setPhase] = useState<MobileInitPhase>('loading');
  const [error, setError] = useState<string | null>(null);
  const [networkOnline, setNetworkOnline] = useState(true);
  const [connectionType, setConnectionType] = useState('unknown');

  const initialize = useCallback(async () => {
    try {
      initNetworkMonitoring();
      const netStatus = getNetworkStatus();
      setNetworkOnline(netStatus.connected);
      setConnectionType(netStatus.connectionType);

      setPhase('biometric');
      const bioOk = await performBiometricCheck();
      if (!bioOk) {
        hapticError();
        setError('身份验证失败');
        setPhase('error');
        return;
      }

      hapticSuccess();
      setPhase('ready');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      setPhase('error');
    }
  }, []);

  useEffect(() => {
    const unsubscribe = onNetworkChange((connected, type) => {
      setNetworkOnline(connected);
      setConnectionType(type);
      if (!connected) hapticWarning();
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  if (phase === 'error' && error) {
    return <MobileErrorScreen error={error} onRetry={() => { setError(null); setPhase('loading'); void initialize(); }} />;
  }
  if (phase === 'loading') return <MobileInitScreen />;
  if (phase === 'biometric') return <MobileBiometricScreen />;

  return (
    <>
      <MobileNetworkBanner online={networkOnline} connectionType={connectionType} />
      {children}
    </>
  );
}

function LocalApp() {
  const initialised = useAppStore((s) => s.initialised);
  const initError = useAppStore((s) => s.initError);
  const init = useAppStore((s) => s.init);

  useEffect(() => {
    init();
  }, [init]);

  if (initError) {
    return (
      <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base">
        <div className="max-w-md text-center space-y-4">
          <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-rc-accent-error-bg text-rc-accent-error">
            <TriangleAlert size={24} />
          </div>
          <h1 className="text-lg font-bold text-rc-text-primary">初始化失败</h1>
          <p className="text-sm text-rc-text-secondary break-all">{initError}</p>
          <button
            onClick={() => init()}
            className="px-4 py-2 bg-rc-bg-user-bubble text-rc-text-inverse rounded-lg text-sm font-medium hover:opacity-90 transition-colors"
          >
            重试
          </button>
        </div>
      </div>
    );
  }

  if (!initialised) {
    return (
      <div className="flex min-h-dvh w-screen items-center justify-center bg-rc-bg-base">
        <div className="flex items-center gap-3 text-rc-text-secondary">
          <div className="w-5 h-5 border-2 border-rc-border-primary border-t-rc-text-primary rounded-full animate-spin" />
          <span className="text-sm font-medium">正在初始化...</span>
        </div>
      </div>
    );
  }

  return (
    <ThemeProvider>
      <Layout>
        <div className="flex h-full min-h-0 flex-col bg-transparent">
          <ChatArea />
          <ChatInput />
        </div>
      </Layout>
      <PermissionModal />
    </ThemeProvider>
  );
}

function App() {
  if (shouldUseWorkbenchDemo()) {
    return (
      <AppErrorBoundary>
        <DemoLocalApp />
      </AppErrorBoundary>
    );
  }

  const nativeRuntime = hasTauriRuntime();
  const nativeMobile = nativeRuntime && isMobileSync();
  const mobileExperience = nativeRuntime
    ? nativeMobile || isTouchDevice()
    : isMobileSync() || isTouchDevice();

  if (shouldUseRemoteMode()) {
    if (mobileExperience) {
      return (
        <AppErrorBoundary>
          <MobileGate>
            <Suspense fallback={<MobileInitScreen />}>
              <MobileRemoteApp />
            </Suspense>
          </MobileGate>
        </AppErrorBoundary>
      );
    }
    return (
      <AppErrorBoundary>
        <Suspense fallback={<RemoteLazyFallback />}>
          <RemoteApp />
        </Suspense>
      </AppErrorBoundary>
    );
  }

  if (!nativeRuntime) {
    if (shouldUseLocalWorkbenchPreview(nativeRuntime)) {
      return (
        <AppErrorBoundary>
          <DemoLocalApp />
        </AppErrorBoundary>
      );
    }

    return (
      <AppErrorBoundary>
        <MarketingSite />
      </AppErrorBoundary>
    );
  }

  if (mobileExperience) {
    return (
      <AppErrorBoundary>
        <MobileGate>
          <LocalApp />
        </MobileGate>
      </AppErrorBoundary>
    );
  }

  return (
    <AppErrorBoundary>
      <LocalApp />
    </AppErrorBoundary>
  );
}

export default App;
