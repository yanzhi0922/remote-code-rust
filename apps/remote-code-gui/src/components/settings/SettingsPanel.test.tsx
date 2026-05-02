import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { SettingsPanel } from './SettingsPanel';
import type { FullSettings } from '../../lib/types';

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

describe('SettingsPanel', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders the settings panel', () => {
    resetAppStore({ settings: mockSettings });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByTestId('settings-panel')).toBeInTheDocument();
  });

  it('renders header with title and close button', () => {
    resetAppStore({ settings: mockSettings });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByText('设置')).toBeInTheDocument();
    expect(screen.getByTestId('close-settings-btn')).toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    resetAppStore({ settings: mockSettings });
    const onClose = vi.fn();
    render(<SettingsPanel onClose={onClose} />);
    fireEvent.click(screen.getByTestId('close-settings-btn'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when backdrop is clicked', () => {
    resetAppStore({ settings: mockSettings });
    const onClose = vi.fn();
    render(<SettingsPanel onClose={onClose} />);
    fireEvent.click(screen.getByTestId('settings-backdrop'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('renders all tab navigation items', () => {
    resetAppStore({ settings: mockSettings });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByTestId('tab-general')).toBeInTheDocument();
    expect(screen.getByTestId('tab-provider')).toBeInTheDocument();
    expect(screen.getByTestId('tab-permissions')).toBeInTheDocument();
    expect(screen.getByTestId('tab-appearance')).toBeInTheDocument();
    expect(screen.getByTestId('tab-hooks')).toBeInTheDocument();
    expect(screen.getByTestId('tab-about')).toBeInTheDocument();
  });

  it('switches tab content when tab is clicked', () => {
    resetAppStore({ settings: mockSettings });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByTestId('general-settings')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('tab-about'));
    expect(screen.getByTestId('about-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('general-settings')).toBeNull();
  });

  it('shows save button when there are draft changes', () => {
    resetAppStore({ settings: mockSettings });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.queryByTestId('save-settings-btn')).toBeNull();
    // Click verbose toggle to create a draft change
    fireEvent.click(screen.getByRole('switch', { name: 'Verbose 模式' }));
    expect(screen.getByTestId('save-settings-btn')).toBeInTheDocument();
  });

  it('shows loading state when settings is null', () => {
    resetAppStore({ settings: null });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByText('加载设置中...')).toBeInTheDocument();
  });
});
