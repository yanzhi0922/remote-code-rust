import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Sidebar } from './Sidebar';
import { resetAppStore } from '../../test/appStoreTestUtils';

describe('Sidebar', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('renders project/session tree and forwards session actions', async () => {
    const createSession = vi.fn().mockResolvedValue('session-2');
    const selectSession = vi.fn().mockResolvedValue(undefined);
    const archiveSession = vi.fn().mockResolvedValue(undefined);
    const setActiveProject = vi.fn();

    resetAppStore({
      projects: [
        {
          path: 'C:\\repo',
          name: 'remote-code-rust',
          session_count: 1,
          is_auto_detected: false,
        },
      ],
      activeProjectPath: 'C:\\repo',
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
      activeSessionId: 'session-1',
      sessionTasks: {
        'session-1': [
          {
            session_id: 'session-1',
            task_id: 'task-1',
            parent_task_id: null,
            description: '审阅修复方案',
            depth: 0,
            status: 'running',
            summary: '等待子代理结果',
            output_preview: null,
            turns_used: null,
          },
        ],
      },
      createSession,
      selectSession,
      archiveSession,
      setActiveProject,
    });

    render(<Sidebar />);

    expect(screen.getByText('remote-code-rust')).toBeInTheDocument();
    expect(screen.getByText('GUI parity')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText('审阅修复方案')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: '新会话' }));
    await waitFor(() => {
      expect(createSession).toHaveBeenCalledWith(undefined, 'C:\\repo');
    });

    fireEvent.click(screen.getByText('GUI parity'));
    await waitFor(() => {
      expect(selectSession).toHaveBeenCalledWith('session-1');
    });

    fireEvent.click(screen.getByTitle('归档此会话'));
    await waitFor(() => {
      expect(archiveSession).toHaveBeenCalledWith('session-1');
    });
  });

  it('prevents removing projects that still contain sessions and removes empty projects', async () => {
    const removeProject = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      projects: [
        {
          path: 'C:\\repo\\busy',
          name: 'busy',
          session_count: 1,
          is_auto_detected: false,
        },
        {
          path: 'C:\\repo\\empty',
          name: 'empty',
          session_count: 0,
          is_auto_detected: false,
        },
      ],
      sessions: [
        {
          id: 'session-1',
          title: 'Busy Session',
          cwd: 'C:\\repo\\busy',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      activeProjectPath: 'C:\\repo\\busy',
      removeProject,
    });

    render(<Sidebar />);

    const blockedButton = screen.getByTitle('该项目下仍有会话，无法移除');
    expect(blockedButton).toBeDisabled();

    const removeButton = screen.getByTitle('移除此项目');
    expect(removeButton).not.toBeDisabled();
    fireEvent.click(removeButton);

    await waitFor(() => {
      expect(removeProject).toHaveBeenCalledWith('C:\\repo\\empty');
    });
  });
});
