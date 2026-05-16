import { create } from 'zustand';
import type {
  BatchProgressInfo,
  CodexGoalState,
  CodexThreadGoalInfo,
  ConversationEntry,
  ContextCompactedInfo,
  ContextOverflowInfo,
  ContextUsageInfo,
  FullSettings,
  PermissionRequestInfo,
  ProjectInfo,
  PromptResult,
  ProviderConfig,
  ProviderConfigList,
  ProviderInfo,
  RuntimeStatusInfo,
  SessionSubtask,
  SessionSummary,
  SubtaskCompletedInfo,
  SubtaskProgressInfo,
  SubtaskStartedInfo,
  ToolProgressInfo,
  ToolResultInfo,
} from '../lib/types';
import * as tauri from '../lib/tauri';
import { normalizePathKey } from '../lib/utils';
import { useCodexStore } from './useCodexStore';
import { useAgentStore } from './useAgentStore';

function getProjectPathForSession(
  sessionId: string | null,
  sessions: SessionSummary[],
  projects: ProjectInfo[],
): string | null {
  if (!sessionId) return null;
  const session = sessions.find((item) => item.id === sessionId);
  if (!session) return null;
  const sessionKey = normalizePathKey(session.cwd);
  return projects.find((project) => normalizePathKey(project.path) === sessionKey)?.path ?? null;
}

function defaultProjectPath(projects: ProjectInfo[], activeProjectPath: string | null): string | null {
  if (
    activeProjectPath &&
    projects.some((project) => normalizePathKey(project.path) === normalizePathKey(activeProjectPath))
  ) {
    return activeProjectPath;
  }
  return projects[0]?.path ?? null;
}

function upsertSubtask(
  sessionTasks: Record<string, SessionSubtask[]>,
  sessionId: string,
  updater: (current: SessionSubtask[]) => SessionSubtask[],
): Record<string, SessionSubtask[]> {
  return {
    ...sessionTasks,
    [sessionId]: updater(sessionTasks[sessionId] ?? []),
  };
}

function applySubtaskStarted(tasks: SessionSubtask[], payload: SubtaskStartedInfo): SessionSubtask[] {
  const existing = tasks.find((task) => task.task_id === payload.task_id);
  const nextTask: SessionSubtask = {
    session_id: payload.session_id,
    task_id: payload.task_id,
    parent_task_id: payload.parent_task_id,
    description: payload.description,
    depth: payload.depth,
    status: 'running',
    summary: '等待子代理结果',
    output_preview: null,
    turns_used: null,
  };
  if (!existing) {
    return [...tasks, nextTask];
  }
  return tasks.map((task) => (task.task_id === payload.task_id ? { ...task, ...nextTask } : task));
}

function applySubtaskProgress(tasks: SessionSubtask[], payload: SubtaskProgressInfo): SessionSubtask[] {
  return tasks.map((task) =>
    task.task_id === payload.task_id
      ? {
          ...task,
          status: 'running',
          summary: payload.summary,
        }
      : task,
  );
}

function applySubtaskCompleted(tasks: SessionSubtask[], payload: SubtaskCompletedInfo): SessionSubtask[] {
  return tasks.map((task) =>
    task.task_id === payload.task_id
      ? {
          ...task,
          status: payload.success ? 'completed' : 'failed',
          summary: payload.output_preview || task.summary,
          output_preview: payload.output_preview,
          turns_used: payload.turns_used,
        }
      : task,
  );
}

interface AppState {
  initialised: boolean;
  initError: string | null;
  listenersRegistered: boolean;

  provider: ProviderInfo | null;
  runtimeStatus: RuntimeStatusInfo | null;

  projects: ProjectInfo[];
  activeProjectPath: string | null;

  sessions: SessionSummary[];
  archivedSessions: SessionSummary[];
  sessionsLoading: boolean;
  activeSessionId: string | null;

  conversation: ConversationEntry[];
  conversationLoading: boolean;

  sending: boolean;
  sendError: string | null;
  lastPromptResult: PromptResult | null;
  liveToolProgress: ToolProgressInfo[];
  liveToolResults: ToolResultInfo[];
  batchProgressBySession: Record<string, BatchProgressInfo>;
  contextUsageBySession: Record<string, ContextUsageInfo>;
  contextOverflowBySession: Record<string, ContextOverflowInfo>;
  contextCompactionBySession: Record<string, ContextCompactedInfo>;
  streamingText: string;
  runningSessionIds: Set<string>;

  settings: FullSettings | null;
  settingsLoading: boolean;

  providerConfigs: ProviderConfigList | null;

  pendingPermission: PermissionRequestInfo | null;

  /** Current goal state for Codex agent (null when no goal is set). */
  goalState: CodexGoalState | null;
  /** Pending objective awaiting /goal confirm after ConfirmIfExists check. */
  pendingGoalObjective: string | null;

  init: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  loadArchivedSessions: () => Promise<void>;
  selectSession: (sessionId: string) => Promise<void>;
  createSession: (title?: string, projectPath?: string) => Promise<string>;
  archiveSession: (sessionId: string) => Promise<void>;
  restoreSession: (sessionId: string) => Promise<void>;
  sendMessage: (text: string) => Promise<void>;
  handleGoalCommand: (sessionId: string, args: string) => Promise<void>;
  addAssistantMessage: (sessionId: string, text: string) => void;
  extractGoalFromResponse: (raw: Record<string, unknown> | undefined) => CodexThreadGoalInfo | null;
  cancelPrompt: (sessionId: string) => Promise<void>;
  refreshProviderInfo: () => Promise<void>;
  refreshRuntimeStatus: () => Promise<void>;
  refreshProjects: () => Promise<void>;
  addProject: (path: string) => Promise<void>;
  removeProject: (path: string) => Promise<void>;
  setActiveProject: (path: string | null) => void;
  loadSettings: () => Promise<void>;
  updateSettings: (updates: Record<string, unknown>) => Promise<void>;
  pickFolderAndAddProject: () => Promise<void>;
  loadProviderConfigs: () => Promise<void>;
  saveProviderConfig: (config: ProviderConfig, setActive: boolean) => Promise<void>;
  deleteProviderConfig: (name: string) => Promise<void>;
  setActiveProvider: (name: string) => Promise<void>;
  switchProfile: (providerName: string, profileName: string | null) => Promise<void>;
  resolvePermission: (resolution: boolean | tauri.PermissionResolutionRequest) => Promise<void>;
}

/**
 * Fix #9: stored cleanup functions for registered event listeners.
 * Written during init so stale listeners can be removed before re-registration.
 */
let listenerCleanupFns: (() => void)[] | null = null;

/** Tear down all registered Tauri event listeners (useful for HMR / tests). */
function cleanupEventListeners(): void {
  if (listenerCleanupFns) {
    listenerCleanupFns.forEach((fn) => fn());
    listenerCleanupFns = null;
  }
}

async function registerEventListeners(): Promise<(() => void)[]> {
  const refreshActiveConversation = () => {
    const activeSessionId = useAppStore.getState().activeSessionId;
    if (!activeSessionId) return;
    void tauri
      .getSessionConversation(activeSessionId)
      .then((conversation) => {
        if (useAppStore.getState().activeSessionId === activeSessionId) {
          useAppStore.setState({ conversation });
        }
      })
      .catch(() => {
        // Ignore non-fatal conversation refresh failures.
      });
  };

  const unlistenFns = await Promise.all([
    tauri.onPermissionRequest((event) => {
      useAppStore.setState({ pendingPermission: event.payload });
    }),
    tauri.onPermissionResolved((event) => {
      useAppStore.setState((state) => ({
        pendingPermission:
          state.pendingPermission?.request_id === event.payload.request_id
            ? null
            : state.pendingPermission,
      }));
    }),
    tauri.onToolStart((event) => {
      useAppStore.setState((state) => ({
        liveToolProgress: [...state.liveToolProgress.slice(-99), event.payload],
      }));
      refreshActiveConversation();
    }),
    tauri.onToolProgress((event) => {
      useAppStore.setState((state) => ({
        liveToolProgress: [...state.liveToolProgress.slice(-99), event.payload],
      }));
    }),
    tauri.onToolResult((event) => {
      useAppStore.setState((state) => ({
        liveToolResults: [...state.liveToolResults.slice(-49), event.payload],
      }));
      refreshActiveConversation();
    }),
    tauri.onStreamingDelta((event) => {
      const { session_id, delta } = event.payload;
      // Fix #10: verify activeSessionId inside setState callback to close
      // the race window between the guard check and the actual mutation.
      useAppStore.setState((state) => {
        if (state.activeSessionId !== session_id) return state;
        const conversation = [...state.conversation];
        const lastEntry = conversation[conversation.length - 1];
        if (lastEntry && lastEntry.role === 'assistant') {
          conversation[conversation.length - 1] = {
            ...lastEntry,
            text: (lastEntry.text ?? '') + delta,
          };
        } else {
          conversation.push({
            role: 'assistant',
            text: delta,
            content_blocks: [],
            tool_calls: [],
            tool_call_id: null,
            name: null,
            is_error: false,
          });
        }
        return {
          conversation,
          streamingText: (state.streamingText ?? '') + delta,
        };
      });
    }),
    tauri.onPromptDone((event) => {
      const { session_id, is_error, error, result } = event.payload;
      const activeSessionId = useAppStore.getState().activeSessionId;

      // Remove from running set.
      useAppStore.setState((state) => {
        const next = new Set(state.runningSessionIds);
        next.delete(session_id);
        return { runningSessionIds: next };
      });

      // If this is the active session, update UI.
      if (session_id === activeSessionId) {
        useAppStore.setState({
          sending: false,
          sendError: is_error ? (error ?? 'Unknown error') : null,
          lastPromptResult: result,
          streamingText: '',
        });
        // Refresh full conversation for consistency.
        void tauri.getSessionConversation(session_id).then((conversation) => {
          if (useAppStore.getState().activeSessionId === session_id) {
            useAppStore.setState({ conversation });
          }
        }).catch(() => {
          // Non-fatal: conversation refresh after prompt completion.
        });
      }

      void tauri
        .getSessionTasks(session_id)
        .then((tasks) => {
          useAgentStore.setState((state) => ({
            sessionTasks: {
              ...state.sessionTasks,
              [session_id]: [...tasks].sort((a, b) =>
                (b.updated_at ?? '').localeCompare(a.updated_at ?? ''),
              ),
            },
          }));
        })
        .catch(() => {
          // Ignore task refresh failures after prompt completion.
        });

      // Refresh session lists.
      void useAppStore.getState().refreshSessions();
      void useAppStore.getState().refreshProviderInfo();
    }),
    tauri.onSubtaskStarted((event) => {
      useAgentStore.setState((state) => ({
        sessionTasks: upsertSubtask(state.sessionTasks, event.payload.session_id, (tasks) =>
          applySubtaskStarted(tasks, event.payload),
        ),
      }));
    }),
    tauri.onSubtaskProgress((event) => {
      useAgentStore.setState((state) => ({
        sessionTasks: upsertSubtask(state.sessionTasks, event.payload.session_id, (tasks) =>
          applySubtaskProgress(tasks, event.payload),
        ),
      }));
    }),
    tauri.onSubtaskCompleted((event) => {
      useAgentStore.setState((state) => ({
        sessionTasks: upsertSubtask(state.sessionTasks, event.payload.session_id, (tasks) =>
          applySubtaskCompleted(tasks, event.payload),
        ),
      }));
    }),
    tauri.onBatchProgress((event) => {
      useAppStore.setState((state) => ({
        batchProgressBySession: {
          ...state.batchProgressBySession,
          [event.payload.session_id]: event.payload,
        },
      }));
    }),
    tauri.onTaskSnapshot((event) => {
      useAgentStore.setState((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [event.payload.session_id]: [...event.payload.tasks].sort((a, b) =>
            (b.updated_at ?? '').localeCompare(a.updated_at ?? ''),
          ),
        },
      }));
    }),
    tauri.onContextUsage((event) => {
      useAppStore.setState((state) => ({
        contextUsageBySession: {
          ...state.contextUsageBySession,
          [event.payload.session_id]: event.payload,
        },
      }));
    }),
    tauri.onContextOverflow((event) => {
      useAppStore.setState((state) => ({
        contextOverflowBySession: {
          ...state.contextOverflowBySession,
          [event.payload.session_id]: event.payload,
        },
      }));
    }),
    tauri.onContextCompacted((event) => {
      useAppStore.setState((state) => ({
        contextCompactionBySession: {
          ...state.contextCompactionBySession,
          [event.payload.session_id]: event.payload,
        },
      }));
    }),
    tauri.onRuntimeStatus((event) => {
      useAppStore.setState({ runtimeStatus: event.payload });
    }),
    tauri.onAgentStatusChanged((event) => {
      const { agentType, status } = event.payload;
      useAgentStore.setState((state) => ({
        agentStatuses: {
          ...state.agentStatuses,
          [agentType]: status,
        },
      }));
    }),
    tauri.onCodexAppServerNotification((event) => {
      const { method, params } = event.payload;
      const paramsRecord =
        params && typeof params === 'object' && !Array.isArray(params)
          ? (params as Record<string, unknown>)
          : null;

      // Spread to ensure we create a plain object that satisfies CodexState
      useCodexStore.setState((state) => ({
        codexNotifications: [...state.codexNotifications.slice(-199), { ...event.payload }],
      }));

      if (method === 'item/autoApprovalReview/completed' && paramsRecord) {
        useCodexStore.setState((state) => ({
          codexGuardianEvents: [
            ...state.codexGuardianEvents.slice(-99),
            {
              session_id: event.payload.session_id,
              method,
              outcome: String(paramsRecord['outcome'] ?? 'unknown'),
              risk_level: paramsRecord['riskLevel'] != null ? String(paramsRecord['riskLevel']) : undefined,
            },
          ],
        }));
      }

      if (method === 'account/login/completed' && paramsRecord) {
        useCodexStore.setState({ codexAccountInfo: paramsRecord });
      }

      if (method === 'account/rateLimits/updated' && paramsRecord) {
        useCodexStore.setState({ codexRateLimits: paramsRecord });
      }

      if (
        (method === 'mcpServer/statusUpdated' || method === 'mcpServer/oauthLoginCompleted') &&
        paramsRecord
      ) {
        useCodexStore.setState((state) => ({
          codexMcpStatus: [...state.codexMcpStatus.slice(-49), paramsRecord],
        }));
      }

      // ── Thread Goal notifications ────────────────────────────
      if (method === 'thread/goal/updated' && paramsRecord) {
        const goalRaw = paramsRecord['goal'] as Record<string, unknown> | undefined;
        if (goalRaw) {
          const goal: CodexThreadGoalInfo = {
            threadId: String(goalRaw['threadId'] ?? ''),
            objective: String(goalRaw['objective'] ?? ''),
            status: (goalRaw['status'] as CodexThreadGoalInfo['status']) ?? 'Active',
            tokenBudget: typeof goalRaw['tokenBudget'] === 'number' ? goalRaw['tokenBudget'] : null,
            tokensUsed: Number(goalRaw['tokensUsed'] ?? 0),
            timeUsedSeconds: Number(goalRaw['timeUsedSeconds'] ?? 0),
            createdAt: Number(goalRaw['createdAt'] ?? 0),
            updatedAt: Number(goalRaw['updatedAt'] ?? 0),
          };
          useAppStore.setState({ goalState: { goal, lastUpdated: Date.now() } });
        }
      }

      if (method === 'thread/goal/cleared') {
        useAppStore.setState({ goalState: null });
      }
    }),
    tauri.onCodexRecoverableError((event) => {
      const { session_id, message, timestamp } = event.payload;
      useCodexStore.setState((state) => ({
        codexRecoverableErrors: [
          ...state.codexRecoverableErrors.slice(-49),
          { session_id, message, timestamp },
        ],
      }));
    }),
  ]);
  // Return the UnlistenFn array so callers can tear down listeners later.
  return unlistenFns;
}

export const useAppStore = create<AppState>((set, get) => ({
  initialised: false,
  initError: null,
  listenersRegistered: false,
  provider: null,
  runtimeStatus: null,
  projects: [],
  activeProjectPath: null,
  sessions: [],
  archivedSessions: [],
  sessionsLoading: false,
  activeSessionId: null,
  conversation: [],
  conversationLoading: false,
  sending: false,
  sendError: null,
  lastPromptResult: null,
  liveToolProgress: [],
  liveToolResults: [],
  batchProgressBySession: {},
  contextUsageBySession: {},
  contextOverflowBySession: {},
  contextCompactionBySession: {},
  streamingText: '',
  runningSessionIds: new Set<string>(),
  settings: null,
  settingsLoading: false,
  providerConfigs: null,
  goalState: null,
  pendingGoalObjective: null,
  pendingPermission: null,

  init: async () => {
    try {
      if (!get().listenersRegistered) {
        cleanupEventListeners();
        // Fix #9: store cleanup functions so listeners can be torn down later.
        const unlistenFns = await registerEventListeners();
        listenerCleanupFns = unlistenFns;
        set({ listenersRegistered: true });
      }

      const result = await tauri.initApp();
      set({
        initialised: true,
        initError: null,
        provider: result.provider,
      });

      await Promise.all([
        get().refreshProjects(),
        get().refreshSessions(),
        get().loadArchivedSessions(),
        get().loadSettings(),
        get().loadProviderConfigs(),
        get().refreshRuntimeStatus(),
        useAgentStore.getState().loadAgents(),
      ]);

      const sid = get().activeSessionId;
      if (sid) {
        await get().selectSession(sid);
      }
    } catch (error) {
      set({
        initialised: false,
        initError: typeof error === 'string' ? error : String(error),
      });
    }
  },

  refreshSessions: async () => {
    set({ sessionsLoading: true });
    try {
      const sessions = await tauri.listSessions();
      set((state) => {
        const activeSessionId =
          state.activeSessionId && sessions.some((session) => session.id === state.activeSessionId)
            ? state.activeSessionId
            : sessions[0]?.id ?? null;
        const activeProjectPath =
          getProjectPathForSession(activeSessionId, sessions, state.projects) ??
          defaultProjectPath(state.projects, state.activeProjectPath);
        return { sessions, sessionsLoading: false, activeSessionId, activeProjectPath };
      });
    } catch {
      set({ sessionsLoading: false });
    }
  },

  loadArchivedSessions: async () => {
    try {
      const archivedSessions = await tauri.listArchivedSessions();
      set({ archivedSessions });
    } catch {
      // Ignore non-fatal archived session refresh failures.
    }
  },

  selectSession: async (sessionId: string) => {
    const state = get();
    const activeProjectPath = getProjectPathForSession(sessionId, state.sessions, state.projects);
    set({
      activeSessionId: sessionId,
      activeProjectPath,
      sending: state.runningSessionIds.has(sessionId),
      sendError: null,
      conversationLoading: true,
      liveToolProgress: [],
      liveToolResults: [],
      goalState: null,
      pendingGoalObjective: null,
    });
    try {
      const [conversation, tasks] = await Promise.all([
        tauri.getSessionConversation(sessionId),
        tauri.getSessionTasks(sessionId).catch(() => [] as SessionSubtask[]),
      ]);
      set({ conversation, conversationLoading: false });
      useAgentStore.setState((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [sessionId]: [...tasks].sort((a, b) =>
            (b.updated_at ?? '').localeCompare(a.updated_at ?? ''),
          ),
        },
      }));
    } catch {
      set({ conversationLoading: false });
    }
  },

  createSession: async (title?: string, projectPath?: string) => {
    const effectiveProjectPath = projectPath ?? get().activeProjectPath ?? undefined;
    if (!effectiveProjectPath) {
      throw new Error('请先选择项目文件夹，再新建会话。');
    }
    const { activeAgentType } = useAgentStore.getState();
    const sessionId = await tauri.createSession(title, effectiveProjectPath, activeAgentType ?? undefined);
    set({
      activeSessionId: sessionId,
      activeProjectPath: effectiveProjectPath,
      conversation: [],
    });
    try {
      const tasks = await tauri.getSessionTasks(sessionId);
      useAgentStore.setState((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [sessionId]: [...tasks].sort((a, b) =>
            (b.updated_at ?? '').localeCompare(a.updated_at ?? ''),
          ),
        },
      }));
    } catch {
      // Ignore empty task state for a new session.
    }
    await Promise.all([get().refreshSessions(), get().refreshProjects()]);
    return sessionId;
  },

  archiveSession: async (sessionId: string) => {
    await tauri.archiveSession(sessionId);
    const wasActive = get().activeSessionId === sessionId;
    await Promise.all([get().refreshSessions(), get().refreshProjects(), get().loadArchivedSessions()]);
    if (wasActive) {
      const nextActiveSessionId = useAppStore.getState().activeSessionId;
      if (nextActiveSessionId) {
        await get().selectSession(nextActiveSessionId);
      } else {
        set({ activeSessionId: null, conversation: [], conversationLoading: false });
      }
    }
  },

  restoreSession: async (sessionId: string) => {
    await tauri.restoreSession(sessionId);
    await Promise.all([get().refreshProjects(), get().refreshSessions(), get().loadArchivedSessions()]);
  },

  sendMessage: async (text: string) => {
    const prompt = text.trim();
    if (!prompt) return;

    // ── /goal slash command interception ──────────────────────────
    const goalSlashMatch = prompt.match(/^\/goal(?:\s+(.*))?$/is);
    if (goalSlashMatch) {
      const { activeAgentType } = useAgentStore.getState();
      if (activeAgentType !== 'remote_codex') {
        set({ sendError: '/goal 仅在 Codex agent 下可用。' });
        return;
      }
      const sessionId = get().activeSessionId;
      if (!sessionId) {
        set({ sendError: '请先创建会话再使用 /goal。' });
        return;
      }
      await get().handleGoalCommand(sessionId, goalSlashMatch[1] ?? '');
      return;
    }

    let sessionId = get().activeSessionId;
    if (!sessionId) {
      if (!get().activeProjectPath) {
        set({ sendError: '请先选择项目文件夹，再开始会话。' });
        return;
      }
      sessionId = await get().createSession(undefined, undefined);
    }

    if (!sessionId) {
      set({ sendError: 'Failed to create session' });
      return;
    }

    const sid = sessionId;

    // Add user message to conversation immediately for responsive UI.
    set((state) => ({
      sending: true,
      sendError: null,
      liveToolProgress: [],
      liveToolResults: [],
      streamingText: '',
      activeSessionId: sid,
      conversation: [
        ...state.conversation,
        {
          role: 'user' as const,
          text: prompt,
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
      ],
      runningSessionIds: new Set([...state.runningSessionIds, sid]),
    }));

    try {
      // Fire-and-forget: sendPrompt returns immediately with session_id.
      await tauri.sendPrompt(prompt, sid);
      // Actual result arrives via gui://prompt-done event.
    } catch (error) {
      set({
        sending: false,
        sendError: typeof error === 'string' ? error : String(error),
        runningSessionIds: (() => {
          const next = new Set(get().runningSessionIds);
          next.delete(sid);
          return next;
        })(),
      });
    }
  },

  /** Extract a CodexThreadGoalInfo from a raw response object. */
  extractGoalFromResponse: (raw: Record<string, unknown> | undefined): CodexThreadGoalInfo | null => {
    if (!raw || !raw['threadId']) return null;
    return {
      threadId: String(raw['threadId'] ?? ''),
      objective: String(raw['objective'] ?? ''),
      status: (raw['status'] as CodexThreadGoalInfo['status']) ?? 'Active',
      tokenBudget: typeof raw['tokenBudget'] === 'number' ? raw['tokenBudget'] : null,
      tokensUsed: Number(raw['tokensUsed'] ?? 0),
      timeUsedSeconds: Number(raw['timeUsedSeconds'] ?? 0),
      createdAt: Number(raw['createdAt'] ?? 0),
      updatedAt: Number(raw['updatedAt'] ?? 0),
    };
  },

  handleGoalCommand: async (sessionId: string, args: string) => {
    const base = { sessionId, threadId: '' };
    const trimmed = args.trim();

    // /goal (no args) → show current goal
    if (!trimmed) {
      try {
        const result = await tauri.codexThreadGoalGet(base);
        const { goal } = tauri.asGoalResponse(result);
        if (!goal) {
          get().addAssistantMessage(sessionId, 'No goal is currently set.\nUsage: /goal <objective>');
          set({ goalState: null });
        } else {
          const status = goal.status ?? 'unknown';
          const obj = goal.objective ?? '(none)';
          get().addAssistantMessage(sessionId, `**Goal (${status})**\n${obj}`);
          const info = get().extractGoalFromResponse(goal);
          if (info) set({ goalState: { goal: info, lastUpdated: Date.now() } });
        }
      } catch (err) {
        set({ sendError: `Failed to get goal: ${err}` });
      }
      return;
    }

    // /goal clear
    if (/^clear$/i.test(trimmed)) {
      try {
        const result = await tauri.codexThreadGoalClear(base);
        const { cleared } = tauri.asClearResponse(result);
        get().addAssistantMessage(sessionId, cleared ? 'Goal cleared.' : 'No goal to clear.');
        if (cleared) set({ goalState: null });
      } catch (err) {
        set({ sendError: `Failed to clear goal: ${err}` });
      }
      return;
    }

    // /goal pause
    if (/^pause$/i.test(trimmed)) {
      try {
        const result = await tauri.codexThreadGoalSet({ ...base, text: '', status: 'Paused' });
        const { goal } = tauri.asGoalResponse(result);
        if (goal) {
          get().addAssistantMessage(sessionId, `Goal **paused**.\n${goal.objective ?? ''}`);
          const info = get().extractGoalFromResponse(goal);
          if (info) set({ goalState: { goal: info, lastUpdated: Date.now() } });
        }
      } catch (err) {
        set({ sendError: `Failed to pause goal: ${err}` });
      }
      return;
    }

    // /goal resume
    if (/^resume$/i.test(trimmed)) {
      try {
        const result = await tauri.codexThreadGoalSet({ ...base, text: '', status: 'Active' });
        const { goal } = tauri.asGoalResponse(result);
        if (goal) {
          get().addAssistantMessage(sessionId, `Goal **resumed**.\n${goal.objective ?? ''}`);
          const info = get().extractGoalFromResponse(goal);
          if (info) set({ goalState: { goal: info, lastUpdated: Date.now() } });
        }
      } catch (err) {
        set({ sendError: `Failed to resume goal: ${err}` });
      }
      return;
    }

    // /goal confirm → confirm the pending goal replacement
    if (/^confirm$/i.test(trimmed)) {
      const pendingObj = get().pendingGoalObjective;
      if (!pendingObj) {
        set({ sendError: 'No pending goal to confirm.' });
        return;
      }
      set({ pendingGoalObjective: null });
      try {
        const result = await tauri.codexThreadGoalSet({ ...base, text: pendingObj, status: 'Active' });
        const { goal } = tauri.asGoalResponse(result);
        if (goal) {
          get().addAssistantMessage(sessionId, `Goal replaced (**${goal.status}**).\n${pendingObj}`);
          const info = get().extractGoalFromResponse(goal);
          if (info) set({ goalState: { goal: info, lastUpdated: Date.now() } });
        }
      } catch (err) {
        set({ sendError: `Failed to replace goal: ${err}` });
      }
      return;
    }

    // /goal <objective> → ConfirmIfExists, then set goal
    try {
      const existing = await tauri.codexThreadGoalGet(base);
      const { goal: currentGoal } = tauri.asGoalResponse(existing);
      if (currentGoal) {
        const currentObj = String(currentGoal.objective ?? '');
        set({
          pendingGoalObjective: trimmed,
          sendError: null,
        });
        set((state) => ({
          conversation: [
            ...state.conversation,
            {
              role: 'assistant' as const,
              text: `⚠️ **Replace current goal?**\n\nCurrent: ${currentObj}\nNew: ${trimmed}\n\n— Type **/goal confirm** to replace, or ignore to cancel.`,
              content_blocks: [],
              tool_calls: [],
              tool_call_id: null,
              name: null,
              is_error: false,
            },
          ],
        }));
        return;
      }
    } catch {
      // If get fails, proceed with setting anyway.
    }

    // No existing goal → set directly
    try {
      const result = await tauri.codexThreadGoalSet({ ...base, text: trimmed, status: 'Active' });
      const { goal } = tauri.asGoalResponse(result);
      if (goal) {
        get().addAssistantMessage(sessionId, `Goal set (**${goal.status}**).\n${trimmed}`);
        const info = get().extractGoalFromResponse(goal);
        if (info) set({ goalState: { goal: info, lastUpdated: Date.now() } });
      }
    } catch (err) {
      set({ sendError: `Failed to set goal: ${err}` });
    }
  },

  addAssistantMessage: (sessionId: string, text: string) => {
    const activeId = get().activeSessionId;
    if (activeId !== sessionId) return;
    set((state) => ({
      conversation: [
        ...state.conversation,
        {
          role: 'assistant' as const,
          text,
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
      ],
    }));
  },

  cancelPrompt: async (sessionId: string) => {
    try {
      await tauri.cancelPrompt(sessionId);
    } catch {
      // Ignore — the prompt may have already finished.
    }
  },

  refreshProviderInfo: async () => {
    try {
      const provider = await tauri.getProviderInfo();
      set({ provider });
    } catch {
      // Ignore non-fatal provider info refresh failures.
    }
  },

  refreshRuntimeStatus: async () => {
    try {
      const runtimeStatus = await tauri.getRuntimeStatus();
      set({ runtimeStatus });
    } catch {
      // Ignore non-fatal runtime status refresh failures.
    }
  },

  refreshProjects: async () => {
    try {
      const projects = await tauri.listProjects();
      set((state) => ({
        projects,
        activeProjectPath:
          getProjectPathForSession(state.activeSessionId, state.sessions, projects) ??
          defaultProjectPath(projects, state.activeProjectPath),
      }));
    } catch {
      // Ignore non-fatal project refresh failures.
    }
  },

  addProject: async (path: string) => {
    const project = await tauri.addProject(path);
    await get().refreshProjects();
    set({ activeProjectPath: project.path });
  },

  removeProject: async (path: string) => {
    await tauri.removeProject(path);
    await get().refreshProjects();
    const activePath = get().activeProjectPath;
    if (activePath && normalizePathKey(activePath) === normalizePathKey(path)) {
      set({ activeProjectPath: null });
    }
  },

  setActiveProject: (path: string | null) => {
    set({ activeProjectPath: path });
  },

  loadSettings: async () => {
    set({ settingsLoading: true });
    try {
      const settings = await tauri.getSettings();
      set({ settings, settingsLoading: false });
    } catch {
      set({ settingsLoading: false });
    }
  },

  updateSettings: async (updates: Record<string, unknown>) => {
    await tauri.updateProvider(updates);
    await Promise.all([
      get().loadSettings(),
      get().refreshProviderInfo(),
      get().refreshRuntimeStatus(),
      get().loadProviderConfigs(),
    ]);
  },

  pickFolderAndAddProject: async () => {
    const path = await tauri.pickFolder();
    if (path) {
      await get().addProject(path);
    }
  },

  loadProviderConfigs: async () => {
    try {
      const providerConfigs = await tauri.listProviderConfigs();
      set({ providerConfigs });
    } catch {
      // Ignore non-fatal provider config refresh failures.
    }
  },

  saveProviderConfig: async (config: ProviderConfig, setActive: boolean) => {
    await tauri.saveProviderConfig(config, setActive);
    await Promise.all([
      get().loadProviderConfigs(),
      get().refreshProviderInfo(),
      get().refreshRuntimeStatus(),
      get().loadSettings(),
    ]);
  },

  deleteProviderConfig: async (name: string) => {
    await tauri.deleteProviderConfig(name);
    await Promise.all([
      get().loadProviderConfigs(),
      get().refreshProviderInfo(),
      get().refreshRuntimeStatus(),
      get().loadSettings(),
    ]);
  },

  setActiveProvider: async (name: string) => {
    await tauri.setActiveProvider(name);
    await Promise.all([
      get().loadProviderConfigs(),
      get().refreshProviderInfo(),
      get().refreshRuntimeStatus(),
      get().loadSettings(),
    ]);
  },

  switchProfile: async (providerName: string, profileName: string | null) => {
    await tauri.switchProfile(providerName, profileName);
    await Promise.all([
      get().loadProviderConfigs(),
      get().refreshProviderInfo(),
      get().refreshRuntimeStatus(),
      get().loadSettings(),
    ]);
  },

  resolvePermission: async (resolution: boolean | tauri.PermissionResolutionRequest) => {
    const pendingPermission = get().pendingPermission;
    if (!pendingPermission) return;
    await tauri.resolvePermissionRequest(
      pendingPermission.request_id,
      typeof resolution === 'boolean' ? { allowed: resolution } : resolution,
    );
    set({ pendingPermission: null });
  },
}));
