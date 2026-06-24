import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { BottomWorkbench } from './BottomWorkbench';

afterEach(() => {
  cleanup();
  resetAppStore();
  vi.clearAllMocks();
});

function renderWorkbench(overrides: Partial<React.ComponentProps<typeof BottomWorkbench>> = {}) {
  const props: React.ComponentProps<typeof BottomWorkbench> = {
    open: true,
    activeTab: 'terminal',
    height: 280,
    onTabChange: vi.fn(),
    onClose: vi.fn(),
    onHeightChange: vi.fn(),
    ...overrides,
  };
  render(<BottomWorkbench {...props} />);
  return props;
}

describe('BottomWorkbench', () => {
  it('renders the bottom workbench tabs and switches tabs through callback', () => {
    resetAppStore();
    const onTabChange = vi.fn();
    renderWorkbench({ onTabChange });

    expect(screen.getByRole('tablist', { name: '底部工作台标签' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: '变更' }));

    expect(onTabChange).toHaveBeenCalledWith('diff');
  });

  it('shows live tool progress in the terminal tab', () => {
    resetAppStore({
      liveToolProgress: [
        {
          tool_call_id: 'tool-1',
          tool_name: 'npm_test',
          message: 'Running tests',
          active_form: 'npm test',
        },
      ],
    });

    renderWorkbench({ activeTab: 'terminal' });

    expect(screen.getByText('npm_test')).toBeInTheDocument();
    expect(screen.getByText('Running tests')).toBeInTheDocument();
    expect(screen.getByText('npm test')).toBeInTheDocument();
  });

  it('renders pending approval details in the approvals tab', () => {
    resetAppStore({
      pendingPermission: {
        request_id: 'approval-1',
        tool_name: 'shell_command',
        tool_use_id: 'tool-1',
        title: '需要确认命令执行',
        description: 'npm run build',
        input: { command: 'npm run build' },
        blocked_path: null,
        permission_suggestions: [],
      },
    });

    renderWorkbench({ activeTab: 'approvals' });

    expect(screen.getByText('需要确认命令执行')).toBeInTheDocument();
    expect(screen.getByText('npm run build')).toBeInTheDocument();
  });
});
