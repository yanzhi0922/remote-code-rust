import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PermissionModal } from './PermissionModal';
import { resetAppStore } from '../../test/appStoreTestUtils';

describe('PermissionModal', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('renders the pending permission request and resolves user decisions', async () => {
    const resolvePermission = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      pendingPermission: {
        request_id: 'perm-1',
        tool_name: 'shell_command',
        tool_use_id: 'tool-1',
        title: 'Run shell command',
        description: '需要执行命令来继续修复。',
        input: { command: 'git status --short' },
        blocked_path: 'C:\\repo',
        permission_suggestions: [
          {
            action: 'allow',
            toolPattern: 'shell_command',
            pathPattern: 'C:\\repo',
          },
        ],
      },
      resolvePermission,
    });

    const { rerender } = render(<PermissionModal />);

    expect(screen.getByText('权限确认')).toBeInTheDocument();
    expect(screen.getByText('shell_command')).toBeInTheDocument();
    expect(screen.getByText('需要执行命令来继续修复。')).toBeInTheDocument();
    expect(screen.getByText('C:\\repo')).toBeInTheDocument();
    expect(screen.getByText('权限建议')).toBeInTheDocument();
    expect(screen.getByText(/"action": "allow"/)).toBeInTheDocument();
    expect(screen.getByText(/"toolPattern": "shell_command"/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '允许执行' }));
    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith(true);
    });

    resolvePermission.mockClear();
    fireEvent.click(screen.getByRole('button', { name: '拒绝' }));
    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith(false);
    });

    resetAppStore({ pendingPermission: null });
    rerender(<PermissionModal />);
    expect(screen.queryByText('权限确认')).not.toBeInTheDocument();
  });
});
