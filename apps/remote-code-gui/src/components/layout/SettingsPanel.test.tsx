import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FullSettings } from '../../lib/types';
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
};

describe('layout SettingsPanel', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders Codex settings in the layout settings Codex tab', () => {
    resetAppStore({ settings: mockSettings });

    render(<SettingsPanel open onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Codex' }));

    expect(screen.getByTestId('codex-settings')).toBeInTheDocument();
    expect(screen.getByText('Codex 原生设置')).toBeInTheDocument();
  });

  it('saves Codex settings through the layout draft pipeline', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    resetAppStore({ settings: mockSettings, updateSettings });

    render(<SettingsPanel open onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Codex' }));
    fireEvent.change(screen.getByTestId('codex-approval-policy'), {
      target: { value: 'on-request' },
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ codex_approval_policy: 'on-request' });
    });
  });
});
