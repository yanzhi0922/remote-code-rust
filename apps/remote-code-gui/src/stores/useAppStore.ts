import { create } from 'zustand';
import type {
  AgentTypeInfo,
  AgentType,
  BatchProgressInfo,
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

function sortTasks(tasks: SessionSubtask[]): SessionSubtask[] {
  return [...tasks].sort((left, right) => {
    const leftUpdated = left.updated_at ?? '';
    const rightUpdated = right.updated_at ?? '';
    return rightUpdated.localeCompare(leftUpdated);
  });
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
  sessionTasks: Record<string, SessionSubtask[]>;
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

  availableAgents: AgentTypeInfo[];
  activeAgentType: AgentType | null;

  init: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  loadArchivedSessions: () => Promise<void>;
  selectSession: (sessionId: string) => Promise<void>;
  createSession: (title?: string, projectPath?: string) => Promise<string>;
  archiveSession: (sessionId: string) => Promise<void>;
  restoreSession: (sessionId: string) => Promise<void>;
  sendMessage: (text: string) => Promise<void>;
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
  loadAgents: () => Promise<void>;
  selectAgent: (agentType: AgentType | null) => void;
}

async function registerEventListeners() {
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

  await Promise.all([
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
        liveToolProgress: [...state.liveToolProgress, event.payload],
      }));
      refreshActiveConversation();
    }),
    tauri.onToolProgress((event) => {
      useAppStore.setState((state) => ({
        liveToolProgress: [...state.liveToolProgress, event.payload],
      }));
    }),
    tauri.onToolResult((event) => {
      useAppStore.setState((state) => ({
        liveToolResults: [...state.liveToolResults, event.payload],
      }));
      refreshActiveConversation();
    }),
    tauri.onStreamingDelta((event) => {
      const { session_id, delta } = event.payload;
      const activeSessionId = useAppStore.getState().activeSessionId;
      if (session_id !== activeSessionId) return;
      useAppStore.setState((state) => {
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
        });
      }

      void tauri
        .getSessionTasks(session_id)
        .then((tasks) => {
          useAppStore.setState((state) => ({
            sessionTasks: {
              ...state.sessionTasks,
              [session_id]: sortTasks(tasks),
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
      useAppStore.setState((state) => ({
        sessionTasks: upsertSubtask(state.sessionTasks, event.payload.session_id, (tasks) =>
          applySubtaskStarted(tasks, event.payload),
        ),
      }));
    }),
    tauri.onSubtaskProgress((event) => {
      useAppStore.setState((state) => ({
        sessionTasks: upsertSubtask(state.sessionTasks, event.payload.session_id, (tasks) =>
          applySubtaskProgress(tasks, event.payload),
        ),
      }));
    }),
    tauri.onSubtaskCompleted((event) => {
      useAppStore.setState((state) => ({
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
      useAppStore.setState((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [event.payload.session_id]: sortTasks(event.payload.tasks),
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
      const { agentType, newStatus } = event.payload;
      useAppStore.setState((state) => ({
        availableAgents: state.availableAgents.map((agent) =>
          agent.agentType === agentType ? { ...agent, status: newStatus } : agent,
        ),
      }));
    }),
  ]);
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
  sessionTasks: {},
  batchProgressBySession: {},
  contextUsageBySession: {},
  contextOverflowBySession: {},
  contextCompactionBySession: {},
  streamingText: '',
  runningSessionIds: new Set<string>(),
  settings: null,
  settingsLoading: false,
  providerConfigs: null,
  pendingPermission: null,
  availableAgents: [],
  activeAgentType: null,

  init: async () => {
    try {
      if (!get().listenersRegistered) {
        await registerEventListeners();
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
        get().loadAgents(),
      ]);

      if (get().activeSessionId) {
        await get().selectSession(get().activeSessionId as string);
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
    const activeProjectPath = getProjectPathForSession(sessionId, get().sessions, get().projects);
    set({
      activeSessionId: sessionId,
      activeProjectPath,
      conversationLoading: true,
    });
    try {
      const [conversation, tasks] = await Promise.all([
        tauri.getSessionConversation(sessionId),
        tauri.getSessionTasks(sessionId).catch(() => [] as SessionSubtask[]),
      ]);
      set((state) => ({
        conversation,
        conversationLoading: false,
        sessionTasks: {
          ...state.sessionTasks,
          [sessionId]: sortTasks(tasks),
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
    const { activeAgentType } = get();
    const sessionId = await tauri.createSession(title, effectiveProjectPath, activeAgentType ?? undefined);
    set({
      activeSessionId: sessionId,
      activeProjectPath: effectiveProjectPath,
      conversation: [],
    });
    try {
      const tasks = await tauri.getSessionTasks(sessionId);
      set((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [sessionId]: sortTasks(tasks),
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

    let sessionId = get().activeSessionId;
    if (!sessionId) {
      if (!get().activeProjectPath) {
        set({ sendError: '请先选择项目文件夹，再开始会话。' });
        return;
      }
      sessionId = await get().createSession(undefined, undefined);
    }

    const sid = sessionId as string;

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
    if (
      get().activeProjectPath &&
      normalizePathKey(get().activeProjectPath as string) === normalizePathKey(path)
    ) {
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

  loadAgents: async () => {
    try {
      const availableAgents = await tauri.listAvailableAgents();
      set({ availableAgents });
    } catch {
      // Ignore non-fatal agent list refresh failures.
    }
  },

  selectAgent: (agentType: AgentType | null) => {
    set({ activeAgentType: agentType });
  },
}));
