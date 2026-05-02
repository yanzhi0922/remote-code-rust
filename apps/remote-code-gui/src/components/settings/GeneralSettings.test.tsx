import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FullSettings } from '../../lib/types';
import { GeneralSettings } from './GeneralSettings';

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

describe('GeneralSettings', () => {
  afterEach(cleanup);

  it('renders the section title', () => {
    render(<GeneralSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('通用设置')).toBeInTheDocument();
  });

  it('renders verbose toggle', () => {
    render(<GeneralSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('Verbose 模式')).toBeInTheDocument();
  });

  it('renders timeout input with correct value', () => {
    render(<GeneralSettings settings={mockSettings} onUpdate={vi.fn()} />);
    const inputs = screen.getAllByTestId('setting-input');
    const timeoutInput = inputs.find((el) => el.getAttribute('value') === '30000');
    expect(timeoutInput).toBeTruthy();
  });

  it('calls onUpdate when verbose toggle is clicked', () => {
    const onUpdate = vi.fn();
    render(<GeneralSettings settings={mockSettings} onUpdate={onUpdate} />);
    fireEvent.click(screen.getByRole('switch', { name: 'Verbose 模式' }));
    expect(onUpdate).toHaveBeenCalledWith({ verbose: true });
  });

  it('calls onUpdate when timeout value changes', () => {
    const onUpdate = vi.fn();
    render(<GeneralSettings settings={mockSettings} onUpdate={onUpdate} />);
    const inputs = screen.getAllByTestId('setting-input');
    const timeoutInput = inputs.find((el) => el.getAttribute('value') === '30000');
    fireEvent.change(timeoutInput!, { target: { value: '60000' } });
    expect(onUpdate).toHaveBeenCalledWith({ timeout_ms: 60000 });
  });

  it('renders retry-related inputs', () => {
    render(<GeneralSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('最大重试次数')).toBeInTheDocument();
    expect(screen.getByText('重试初始退避 (ms)')).toBeInTheDocument();
    expect(screen.getByText('重试最大退避 (ms)')).toBeInTheDocument();
  });

  it('calls onUpdate when retry count changes', () => {
    const onUpdate = vi.fn();
    render(<GeneralSettings settings={mockSettings} onUpdate={onUpdate} />);
    const inputs = screen.getAllByTestId('setting-input');
    const retryInput = inputs.find((el) => el.getAttribute('value') === '3');
    fireEvent.change(retryInput!, { target: { value: '5' } });
    expect(onUpdate).toHaveBeenCalledWith({ max_retries: 5 });
  });
});
