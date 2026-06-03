import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FullSettings, ProviderConfig, ProviderConfigList } from '../../lib/types';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { SettingsPanel } from './SettingsPanel';

const mockSettings: FullSettings = {
  provider_name: 'openai',
  provider_model: 'gpt-4',
  provider_base_url: 'https://api.openai.com',
  provider_protocol: 'openai',
  provider_api_key_set: true,
  max_output_tokens: 4096,
  thinking_budget: null,
  max_retries: 3,
  timeout_ms: 30000,
  retry_initial_backoff_ms: 1000,
  retry_max_backoff_ms: 30000,
  respect_retry_after: true,
  permission_mode: 'default',
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

describe('layout SettingsPanel', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders Codex settings in the layout settings Codex tab', () => {
    resetAppStore({ settings: mockSettings });

    render(<SettingsPanel open onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('tablist', { name: 'Settings sections' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));

    expect(screen.getByRole('tab', { name: 'Codex' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('codex-settings')).toBeInTheDocument();
    expect(screen.getByText('Codex 原生设置')).toBeInTheDocument();
  });

  it('saves Codex settings through the layout draft pipeline', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    resetAppStore({ settings: mockSettings, updateSettings });

    render(<SettingsPanel open onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));
    fireEvent.change(screen.getByTestId('codex-approval-policy'), {
      target: { value: 'on-request' },
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ codex_approval_policy: 'on-request' });
    });
  });

  it('shows canonical runtime paths in the runtime tab', () => {
    resetAppStore({ settings: mockSettings });

    render(<SettingsPanel open onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: '运行参数' }));

    expect(screen.getByText('安装和数据目录')).toBeInTheDocument();
    expect(screen.getByText('Remote Code Home')).toBeInTheDocument();
    expect(screen.getByText('/test/profile')).toBeInTheDocument();
    expect(screen.getByText('/test/logs')).toBeInTheDocument();
    expect(screen.getByText('/test/remote_control.json')).toBeInTheDocument();
  });
});

function makeProviderConfig(overrides: Partial<ProviderConfig>): ProviderConfig {
  return {
    name: 'bigmodel',
    protocol: 'anthropic',
    anthropic_base_url: 'https://open.bigmodel.cn/api/anthropic',
    openai_base_url: 'https://open.bigmodel.cn/api/coding/paas/v4',
    api_key: '',
    api_key_stored: true,
    model: 'glm-5.1',
    models: [
      { id: 'glm-5.1' },
      { id: 'glm-5-turbo' },
    ],
    claude_model_mapping: { opus: 'glm-5.1', sonnet: 'glm-5.1', haiku: 'glm-5.1' },
    group: 'builtin',
    enabled: true,
    profiles: [],
    active_profile: undefined,
    ...overrides,
  };
}

const builtinBigmodel = makeProviderConfig({ name: 'bigmodel', group: 'builtin' });
const builtinZai = makeProviderConfig({ name: 'z.ai', group: 'builtin' });
const customProvider = makeProviderConfig({
  name: 'my-provider',
  group: 'custom',
  anthropic_base_url: 'https://example.com/anthropic',
  openai_base_url: 'https://example.com/v1',
  model: 'foo',
  models: [{ id: 'foo' }, { id: 'bar' }],
  claude_model_mapping: { opus: 'foo', sonnet: 'foo', haiku: 'foo' },
});

describe('layout SettingsPanel — model providers redesign', () => {
  afterEach(() => {
    cleanup();
  });

  function seedProviders(
    overrides: Partial<StoreState> = {},
  ): { providerConfigs: ProviderConfigList } {
    const providerConfigs: ProviderConfigList = {
      providers: [builtinBigmodel, builtinZai, customProvider],
      active_provider: 'bigmodel',
    };
    resetAppStore({
      settings: mockSettings,
      providerConfigs,
      ...overrides,
    });
    return { providerConfigs };
  }

  it('renders a flat list of all providers with no brand-specific grouping', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByText('模型供应商')).toBeInTheDocument();
    });
    // All providers render as peers; no "智谱" / "自定义供应商" headers.
    expect(screen.queryByText('智谱')).not.toBeInTheDocument();
    expect(screen.queryByText('自定义供应商')).not.toBeInTheDocument();
    expect(screen.getByTestId('provider-row-bigmodel')).toBeInTheDocument();
    expect(screen.getByTestId('provider-row-z.ai')).toBeInTheDocument();
    expect(screen.getByTestId('provider-row-my-provider')).toBeInTheDocument();
  });

  it('renders a probe button on every model row', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    // bigmodel has two seeded models — both should have probe buttons.
    expect(screen.getByTestId('probe-model-glm-5.1')).toBeInTheDocument();
    expect(screen.getByTestId('probe-model-glm-5-turbo')).toBeInTheDocument();
    // No agent chips yet — the user hasn't clicked any plug.
    expect(screen.queryByTestId('probe-agent-remote_claude-ok')).not.toBeInTheDocument();
  });

  it('probes a model and shows per-agent availability chips', async () => {
    const probeProviderModel = vi.fn().mockResolvedValue({
      model_id: 'glm-5.1',
      url: 'https://open.bigmodel.cn/api/anthropic',
      outcome: 'reachable',
      detail: 'HTTP 200',
      status_code: 200,
      latency_ms: 312,
      agents: [
        { agent_type: 'remote_claude', agent_name: 'Remote Claude', available: true, detail: 'HTTP 200', status_code: 200, latency_ms: 312 },
      ],
    });
    seedProviders({ probeProviderModel });
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('probe-model-glm-5.1')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('probe-model-glm-5.1'));

    await waitFor(() => {
      expect(probeProviderModel).toHaveBeenCalledWith('bigmodel', 'glm-5.1');
    });
    await waitFor(() => {
      expect(screen.getByTestId('probe-agent-remote_claude-ok')).toBeInTheDocument();
    });
  });

  it('opens the detail panel for the active provider by default', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
  });

  it('switches detail panel when a different provider row is clicked', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    fireEvent.click(screen.getByTestId('provider-row-my-provider'));
    expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('my-provider');
  });

  it('hides the delete button for built-in providers', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    expect(screen.queryByTestId('provider-delete-btn')).not.toBeInTheDocument();
  });

  it('shows the delete button for custom providers', async () => {
    seedProviders();
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    fireEvent.click(screen.getByTestId('provider-row-my-provider'));
    expect(screen.getByTestId('provider-delete-btn')).toBeInTheDocument();
  });

  it('calls setProviderEnabled when the disable button is clicked', async () => {
    const setProviderEnabled = vi.fn().mockResolvedValue(undefined);
    seedProviders({ setProviderEnabled });
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    fireEvent.click(screen.getByTestId('provider-enable-btn'));
    await waitFor(() => {
      expect(setProviderEnabled).toHaveBeenCalledWith('bigmodel', false);
    });
  });

  it('calls addProviderModel when a new model is added', async () => {
    const addProviderModel = vi.fn().mockResolvedValue(undefined);
    seedProviders({ addProviderModel });
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    fireEvent.change(screen.getByTestId('add-model-input'), {
      target: { value: 'glm-5-new' },
    });
    fireEvent.click(screen.getByTestId('add-model-btn'));
    await waitFor(() => {
      expect(addProviderModel).toHaveBeenCalledWith('bigmodel', { id: 'glm-5-new' });
    });
  });

  it('calls setClaudeModelMapping when the Opus tier dropdown changes', async () => {
    const setClaudeModelMapping = vi.fn().mockResolvedValue(undefined);
    seedProviders({ setClaudeModelMapping });
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByTestId('provider-detail-name')).toHaveTextContent('bigmodel');
    });
    fireEvent.change(screen.getByTestId('tier-opus'), {
      target: { value: 'glm-5-turbo' },
    });
    await waitFor(() => {
      expect(setClaudeModelMapping).toHaveBeenCalledWith('bigmodel', {
        opus: 'glm-5-turbo',
        sonnet: 'glm-5.1',
        haiku: 'glm-5.1',
      });
    });
  });

  it('calls refreshProviders when the refresh button is clicked', async () => {
    const refreshProviders = vi.fn().mockResolvedValue(undefined);
    seedProviders({ refreshProviders });
    render(<SettingsPanel open onClose={vi.fn()} initialTab="provider" />);

    await waitFor(() => {
      expect(screen.getByText('模型供应商')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => {
      expect(refreshProviders).toHaveBeenCalledTimes(1);
    });
  });
});

type StoreState = ReturnType<typeof import('../../stores/useAppStore').useAppStore.getState>;
