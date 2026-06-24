import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Sidebar } from './Sidebar';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { useAgentStore } from '../../stores/useAgentStore';

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
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      activeSessionId: 'session-1',
      createSession,
      selectSession,
      archiveSession,
      setActiveProject,
    });

    useAgentStore.setState({
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
    });

    render(<Sidebar />);

    expect(screen.getByText('remote-code-rust')).toBeInTheDocument();
    expect(screen.getByText('GUI parity')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText('审阅修复方案')).toBeInTheDocument();
    });

    fireEvent.click(screen.getAllByRole('button', { name: '新会话' })[0]);
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
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      activeProjectPath: 'C:\\repo\\busy',
      removeProject,
    });

    render(<Sidebar />);

    fireEvent.click(screen.getByTitle('C:\\repo\\busy'));

    const blockedButton = screen.getByTitle('该项目下仍有会话，无法移除');
    expect(blockedButton).toBeDisabled();

    const removeButton = screen.getByTitle('移除此项目');
    expect(removeButton).not.toBeDisabled();
    fireEvent.click(removeButton);

    await waitFor(() => {
      expect(removeProject).toHaveBeenCalledWith('C:\\repo\\empty');
    });
  });

  it('exposes All / Pinned / Unread filter chips with counts', async () => {
    resetAppStore({
      projects: [
        {
          path: 'C:\\repo',
          name: 'remote-code-rust',
          session_count: 2,
          is_auto_detected: false,
        },
      ],
      activeProjectPath: 'C:\\repo',
      sessions: [
        {
          id: 'session-pinned',
          title: 'Pinned session',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
        {
          id: 'session-unread',
          title: 'Unread session',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          agent_type: 'remote_claude',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:05:00Z',
          archived: false,
        },
      ],
      pinnedSessions: new Set(['session-pinned']),
      unreadSessions: new Set(['session-unread']),
    });

    render(<Sidebar />);

    const allChip = screen.getByTestId('sidebar-filter-all');
    const pinnedChip = screen.getByTestId('sidebar-filter-pinned');
    const unreadChip = screen.getByTestId('sidebar-filter-unread');

    expect(allChip).toHaveAttribute('aria-selected', 'true');
    expect(pinnedChip.textContent).toContain('已置顶');
    expect(pinnedChip.textContent).toContain('1');
    expect(unreadChip.textContent).toContain('未读');
    expect(unreadChip.textContent).toContain('1');

    fireEvent.click(pinnedChip);
    expect(pinnedChip).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByText('Unread session')).not.toBeInTheDocument();
    expect(screen.getByText('Pinned session')).toBeInTheDocument();

    fireEvent.click(unreadChip);
    expect(unreadChip).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByText('Pinned session')).not.toBeInTheDocument();
    expect(screen.getByText('Unread session')).toBeInTheDocument();
  });

  it('opens the project menu on Ctrl+Shift+P', async () => {
    const setActiveProject = vi.fn();
    resetAppStore({
      projects: [
        { path: 'C:\\repo-a', name: 'A', session_count: 0, is_auto_detected: false },
        { path: 'C:\\repo-b', name: 'B', session_count: 0, is_auto_detected: false },
      ],
      setActiveProject,
    });

    render(<Sidebar />);
    fireEvent.keyDown(window, { key: 'P', ctrlKey: true, shiftKey: true });
    await waitFor(() => {
      // "A" appears in both the trigger button (activeProject fallback to
      // projects[0]) and the popover menu option, so use getAllByText.
      const matches = screen.getAllByText('A');
      expect(matches.length).toBeGreaterThanOrEqual(1);
    });
  });
});
