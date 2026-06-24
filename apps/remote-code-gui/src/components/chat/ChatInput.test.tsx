import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatInput } from './ChatInput';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { useAgentStore } from '../../stores/useAgentStore';

const DEFAULT_SETTINGS = {
  provider_name: 'glm-coding',
  provider_model: 'glm-5.1',
  provider_base_url: 'https://open.bigmodel.cn/api/anthropic',
  provider_protocol: 'anthropic',
  provider_api_key_set: true,
  max_output_tokens: 4096,
  thinking_budget: null,
  max_retries: 3,
  timeout_ms: 60_000,
  retry_initial_backoff_ms: 500,
  retry_max_backoff_ms: 5_000,
  respect_retry_after: true,
  permission_mode: 'default',
  verbose: false,
  max_turns: 128,
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
  roo_mode: null,
  runtime_paths: {
    profile_dir: '/test/profile',
    sessions_dir: '/test/sessions',
    artifacts_dir: '/test/artifacts',
    logs_dir: '/test/logs',
    cache_dir: '/test/cache',
    agents_dir: '/test/agents',
    remote_control_file: '/test/remote_control.json',
    gui_projects_file: '/test/gui-projects.json',
    gui_providers_file: '/test/gui-providers.json',
    gui_settings_file: '/test/gui-settings.json',
  },
};

describe('ChatInput', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('sends the current message on Enter and clears the composer', async () => {
    const sendMessage = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      activeSessionId: 'session-1',
      sessions: [
        {
          id: 'session-1',
          title: 'GUI parity',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      settings: DEFAULT_SETTINGS,
      provider: {
        name: 'glm-coding',
        model: 'glm-5.1',
        protocol: 'anthropic',
        base_url: 'https://open.bigmodel.cn/api/anthropic',
      },
      providerConfigs: {
        active_provider: 'glm-coding',
        providers: [
          {
            name: 'glm-coding',
            protocol: 'anthropic',
            base_url: 'https://open.bigmodel.cn/api/anthropic',
            model: 'glm-5.1',
          },
        ],
      },
      sendMessage,
    });

    render(<ChatInput />);

    expect(screen.getByRole('form', { name: 'Prompt composer' })).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'Composer controls' })).toBeInTheDocument();

    const textarea = screen.getByRole('textbox', { name: 'Prompt input' });

    fireEvent.change(textarea, { target: { value: '请检查当前会话状态' } });
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' });

    await waitFor(() => {
      expect(sendMessage).toHaveBeenCalledWith('请检查当前会话状态', undefined);
    });
    expect(textarea).toHaveValue('');
  });

  it('updates provider, model, and permission controls for the next send', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    const setActiveProvider = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      settings: DEFAULT_SETTINGS,
      provider: {
        name: 'glm-coding',
        model: 'glm-5.1',
        protocol: 'anthropic',
        base_url: 'https://open.bigmodel.cn/api/anthropic',
      },
      providerConfigs: {
        active_provider: 'glm-coding',
        providers: [
          {
            name: 'glm-coding',
            protocol: 'anthropic',
            base_url: 'https://open.bigmodel.cn/api/anthropic',
            model: 'glm-5.1',
          },
          {
            name: 'minimax',
            protocol: 'anthropic',
            base_url: 'https://api.minimaxi.com/anthropic',
            model: 'minimax-m2.7',
          },
        ],
      },
      updateSettings,
      setActiveProvider,
    });

    render(<ChatInput />);

    // The provider chip lives in the bottom chip strip (Codex-style).
    const providerLabels = screen.getAllByText('glm-coding');
    const providerButton = providerLabels[providerLabels.length - 1]?.closest('button');
    expect(providerButton).toBeTruthy();
    fireEvent.click(providerButton!);
    fireEvent.click(screen.getByText('minimax'));

    await waitFor(() => {
      expect(setActiveProvider).toHaveBeenCalledWith('minimax');
    });

    fireEvent.click(screen.getByRole('button', { name: '默认模式' }));
    fireEvent.click(screen.getByText('跳过权限检查'));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ permission_mode: 'bypassPermissions' });
    });

    const modelInput = screen.getByLabelText('Model for next send');
    fireEvent.change(modelInput, { target: { value: 'glm-5v-turbo' } });
    fireEvent.blur(modelInput);

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ provider_model: 'glm-5v-turbo' });
    });
  });

  it('uses native Codex approval and sandbox options and locks agent for an existing session', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      activeSessionId: 'session-codex',
      sessions: [
        {
          id: 'session-codex',
          title: 'Codex work',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_codex',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      conversation: [
        {
          role: 'user',
          text: 'hello',
          content: 'hello',
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
        {
          role: 'assistant',
          text: 'hi',
          content: 'hi',
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
      ],
      settings: {
        ...DEFAULT_SETTINGS,
        codex_approval_policy: 'on-request',
        codex_sandbox_mode: 'workspace-write',
      },
      updateSettings,
    });
    useAgentStore.setState({
      activeAgentType: 'remote_codex',
      availableAgents: [
        { agentType: 'remote_codex', displayName: 'Codex', available: true, installed: true },
        { agentType: 'remote_claude', displayName: 'Claude', available: true, installed: true },
        { agentType: 'remote_roo', displayName: 'Roo', available: true, installed: true },
      ],
    });

    render(<ChatInput />);

    // The permission chip in the Codex-style bottom strip hosts the approval
    // policy options directly (no separate "展开 Composer 配置" gate).
    fireEvent.click(screen.getByRole('button', { name: /请求批准|默认模式/ }));
    fireEvent.click(screen.getByText('沙盒自动'));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        codex_approval_policy: 'never',
        codex_sandbox_mode: 'workspace-write',
      });
    });

    // The active agent for the Codex session is "Codex", so the agent chip
    // displays the Codex displayName. Click it to open the agent menu, then
    // pick "Roo" via the menuitemradio (AgentSelector uses menuitemradio
    // semantics). The session is locked to Codex (existing conversation)
    // so the store should keep Codex active.
    fireEvent.click(screen.getByRole('button', { name: /Codex/ }));
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Roo/ }));

    expect(useAgentStore.getState().activeAgentType).toBe('remote_codex');
  });

  it('blocks switching to a model with a smaller known context window than the active session uses', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      activeSessionId: 'session-1',
      sessions: [
        {
          id: 'session-1',
          title: 'Large context session',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      contextUsageBySession: {
        'session-1': {
          session_id: 'session-1',
          estimated_tokens: 12_000,
          max_input_tokens: 128_000,
          threshold_tokens: 102_400,
          ratio: 0.09,
        },
      },
      settings: DEFAULT_SETTINGS,
      updateSettings,
    });

    render(<ChatInput />);

    fireEvent.click(screen.getByRole('button', { name: '展开 Composer 配置' }));
    const modelInput = screen.getByLabelText('Model for next send');
    fireEvent.change(modelInput, { target: { value: 'small-8k' } });
    fireEvent.blur(modelInput);

    expect(await screen.findByText(/不能切换到更小上下文/)).toBeInTheDocument();
    expect(modelInput).toHaveValue('glm-5.1');
    expect(updateSettings).not.toHaveBeenCalledWith({ provider_model: 'small-8k' });
  });

  it('cancels the active running prompt from the composer', async () => {
    const cancelPrompt = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      activeSessionId: 'session-1',
      sessions: [
        {
          id: 'session-1',
          title: 'Running refactor',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      sending: true,
      settings: DEFAULT_SETTINGS,
      provider: {
        name: 'glm-coding',
        model: 'glm-5.1',
        protocol: 'anthropic',
        base_url: 'https://open.bigmodel.cn/api/anthropic',
      },
      cancelPrompt,
    });

    render(<ChatInput />);

    fireEvent.click(screen.getByRole('button', { name: '停止当前运行' }));

    await waitFor(() => {
      expect(cancelPrompt).toHaveBeenCalledWith('session-1');
    });
  });
});
