import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatInput } from './ChatInput';
import { resetAppStore } from '../../test/appStoreTestUtils';

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

    const textarea = screen.getByPlaceholderText('向 agent 发送指令或代码片段');

    fireEvent.change(textarea, { target: { value: '请检查当前会话状态' } });
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' });

    await waitFor(() => {
      expect(sendMessage).toHaveBeenCalledWith('请检查当前会话状态');
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

    fireEvent.click(screen.getByText('glm-coding'));
    fireEvent.click(screen.getByText('minimax'));

    await waitFor(() => {
      expect(setActiveProvider).toHaveBeenCalledWith('minimax');
    });

    fireEvent.click(screen.getByRole('button', { name: '默认' }));
    fireEvent.click(screen.getByText('全自动'));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ permission_mode: 'bypassPermissions' });
    });

    const modelInput = screen.getByPlaceholderText('设置模型');
    fireEvent.change(modelInput, { target: { value: 'glm-5v-turbo' } });
    fireEvent.blur(modelInput);

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ provider_model: 'glm-5v-turbo' });
    });
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
