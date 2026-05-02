import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FullSettings } from '../../lib/types';
import { PermissionSettings } from './PermissionSettings';

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

describe('PermissionSettings', () => {
  afterEach(cleanup);

  it('renders the section title', () => {
    render(<PermissionSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('权限设置')).toBeInTheDocument();
  });

  it('renders all permission modes', () => {
    render(<PermissionSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('默认')).toBeInTheDocument();
    expect(screen.getByText('自动编辑')).toBeInTheDocument();
    expect(screen.getByText('不询问')).toBeInTheDocument();
    expect(screen.getByText('全自动')).toBeInTheDocument();
    expect(screen.getByText('规划')).toBeInTheDocument();
  });

  it('highlights the current mode', () => {
    render(<PermissionSettings settings={mockSettings} onUpdate={vi.fn()} />);
    const defaultBtn = screen.getByTestId('permission-mode-default');
    expect(defaultBtn.className).toContain('border-blue-500');
  });

  it('calls onUpdate when a different mode is selected', () => {
    const onUpdate = vi.fn();
    render(<PermissionSettings settings={mockSettings} onUpdate={onUpdate} />);
    fireEvent.click(screen.getByTestId('permission-mode-plan'));
    expect(onUpdate).toHaveBeenCalledWith({ permission_mode: 'plan' });
  });

  it('renders BypassPermissions component', () => {
    render(<PermissionSettings settings={mockSettings} onUpdate={vi.fn()} />);
    expect(screen.getByTestId('bypass-permissions')).toBeInTheDocument();
  });

  it('shows bypass as enabled when mode is bypassPermissions', () => {
    const bypassSettings = { ...mockSettings, permission_mode: 'bypassPermissions' as const };
    render(<PermissionSettings settings={bypassSettings} onUpdate={vi.fn()} />);
    expect(screen.getByText('权限已绕过')).toBeInTheDocument();
  });

  it('switches to bypassPermissions when bypass is toggled on', () => {
    const onUpdate = vi.fn();
    render(<PermissionSettings settings={mockSettings} onUpdate={onUpdate} />);
    // The BypassPermissions switch requires confirmation to enable
    fireEvent.click(screen.getByRole('switch', { name: '切换权限绕过' }));
    fireEvent.click(screen.getByText('确认绕过'));
    expect(onUpdate).toHaveBeenCalledWith({ permission_mode: 'bypassPermissions' });
  });
});
