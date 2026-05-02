import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FullSettings } from '../../lib/types';
import { ProviderSettings } from './ProviderSettings';

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

describe('ProviderSettings', () => {
  afterEach(cleanup);

  it('renders the section title', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('提供商设置')).toBeInTheDocument();
  });

  it('renders provider name input with current value', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByTestId('provider-name-input')).toHaveValue('openai');
  });

  it('calls onUpdate when provider name changes', () => {
    const onUpdate = vi.fn();
    render(<ProviderSettings settings={mockSettings} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByTestId('provider-name-input'), { target: { value: 'anthropic' } });
    expect(onUpdate).toHaveBeenCalledWith({ provider_name: 'anthropic' });
  });

  it('renders protocol select with current value', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByTestId('protocol-select')).toHaveValue('openai');
  });

  it('calls onUpdate when protocol changes', () => {
    const onUpdate = vi.fn();
    render(<ProviderSettings settings={mockSettings} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByTestId('protocol-select'), { target: { value: 'anthropic' } });
    expect(onUpdate).toHaveBeenCalledWith({ provider_protocol: 'anthropic' });
  });

  it('shows model select with predefined models', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByTestId('model-select')).toBeInTheDocument();
    expect(screen.getByText('gpt-4o')).toBeInTheDocument();
  });

  it('toggles to custom model input', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    fireEvent.click(screen.getByTestId('toggle-custom-model'));
    expect(screen.getByTestId('custom-model-input')).toBeInTheDocument();
  });

  it('shows URL error for invalid base URL', () => {
    const onUpdate = vi.fn();
    render(<ProviderSettings settings={mockSettings} onUpdate={onUpdate} />);
    const urlInputs = screen.getAllByTestId('setting-input');
    const urlInput = urlInputs.find((el) => el.getAttribute('placeholder') === 'https://api.openai.com');
    fireEvent.change(urlInput!, { target: { value: 'not-a-url' } });
    expect(screen.getByTestId('url-error')).toBeInTheDocument();
  });

  it('renders API Key input with correct description', () => {
    render(<ProviderSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('API Key 已设置')).toBeInTheDocument();
  });
});
