import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../stores/useAppStore', () => ({
  useAppStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({
      provider: { name: 'test-provider', model: 'test-model' },
      runtimeStatus: {
        provider: { name: 'test-provider', model: 'test-model' },
        allowed_tools: ['read_file'],
        disallowed_tools: ['rm'],
        mcp: { status_counts: { connected: 2, failed: 0, needs_auth: 0 }, enabled_servers: 2, warning_count: 0 },
        permission_mode: 'default',
      },
      activeSessionId: 'session-123',
      activeProjectPath: '/home/user/project',
      sessions: [{ id: 'session-123', title: 'Test Session', model: 'test-model', agent_type: 'remote_claude' }],
      contextUsageBySession: {
        'session-123': { estimated_tokens: 1000, max_input_tokens: 128000, threshold_tokens: 100000, ratio: 0.05 },
      },
      workspacePrivacyMode: false,
      lastPromptResult: { usage: { input_tokens: 500, output_tokens: 200, total_tokens: 700 } },
      settings: { permission_mode: 'default' },
      conversation: [{ role: 'user', content: 'hi' }, { role: 'assistant', content: 'hello' }],
    }),
}));

vi.mock('../../stores/useAgentStore', () => ({
  useAgentStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({ activeAgentType: 'remote_claude' }),
}));

import { StatusBar } from './StatusBar';

afterEach(() => { cleanup(); });

describe('StatusBar', () => {
  it('renders status segments but no workbench chips without onOpenBottomTab', () => {
    render(<StatusBar />);
    // Without onOpenBottomTab, the 5 workbench chips (terminal / diff / approvals / artifacts / browser) are hidden.
    expect(screen.queryByRole('button', { name: '终端' })).not.toBeInTheDocument();
    // The 6 status segments (project / session / permission / mcp / context + Wifi) are still present.
    expect(screen.getByRole('button', { name: '项目' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '会话' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '权限' })).toBeInTheDocument();
  });

  it('toggles a status segment popover', () => {
    render(<StatusBar />);
    const projectChip = screen.getByRole('button', { name: '项目' });
    fireEvent.click(projectChip);
    // Popover renders as a dialog with the same aria-label.
    expect(screen.getByRole('dialog', { name: '项目' })).toBeInTheDocument();
    // Click the chip again to close.
    fireEvent.click(projectChip);
    expect(screen.queryByRole('dialog', { name: '项目' })).not.toBeInTheDocument();
  });

  it('does not render workbench chips anymore (deferred to Layout floating buttons)', () => {
    render(<StatusBar />);
    expect(screen.queryByRole('button', { name: '终端' })).not.toBeInTheDocument();
  });

  it('shows approval permission segment with no warning when no pending permission', () => {
    render(<StatusBar />);
    const approvals = screen.getByRole('button', { name: '权限' });
    expect(approvals).toBeInTheDocument();
    expect(approvals).toHaveAttribute('aria-expanded', 'false');
  });
});
