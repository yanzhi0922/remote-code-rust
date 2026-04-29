import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodexOperationsPanel } from './CodexOperationsPanel';
import * as tauri from '../../lib/tauri';

vi.mock('../../lib/tauri', () => ({
  codexListThreads: vi.fn(() =>
    Promise.resolve({
      data: [
        {
          id: 'thread-1',
          preview: 'Thread one',
          name: null,
          modelProvider: 'openai',
          createdAt: 1,
          updatedAt: 2,
          status: 'idle',
          cwd: 'C:\\repo',
        },
      ],
      nextCursor: null,
    }),
  ),
  codexReadThread: vi.fn(() => Promise.resolve({ thread: { id: 'thread-1' } })),
  codexResumeThread: vi.fn(() => Promise.resolve({ thread: { id: 'thread-1' } })),
  codexForkThread: vi.fn(() => Promise.resolve({ thread: { id: 'thread-1' } })),
  codexArchiveThread: vi.fn(() => Promise.resolve({})),
  codexUnarchiveThread: vi.fn(() => Promise.resolve({ thread: { id: 'thread-1' } })),
  codexMcpRefresh: vi.fn(() => Promise.resolve({})),
  codexMcpStatus: vi.fn(() => Promise.resolve({ data: [], nextCursor: null })),
  codexMcpReadResource: vi.fn(() => Promise.resolve({ contents: [] })),
  codexMcpCallTool: vi.fn(() => Promise.resolve({ content: [] })),
  codexReadConfig: vi.fn(() => Promise.resolve({ config: {}, origins: {}, layers: [] })),
  codexWriteConfigValue: vi.fn(() =>
    Promise.resolve({ status: 'written', version: '1', filePath: 'config.toml', overriddenMetadata: null }),
  ),
  codexExec: vi.fn(() => Promise.resolve({ exitCode: 0, stdout: '', stderr: '' })),
  codexAppServerRequest: vi.fn(() => Promise.resolve({ ok: true })),
  codexSetThreadMemoryMode: vi.fn(() => Promise.resolve({})),
  codexResetMemories: vi.fn(() => Promise.resolve({})),
  codexUploadFeedback: vi.fn(() => Promise.resolve({ threadId: 'thread-1' })),
}));

describe('CodexOperationsPanel', () => {
  const confirmSpy = vi.spyOn(window, 'confirm');

  beforeEach(() => {
    confirmSpy.mockReturnValue(false);
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it('guards exec behind confirm', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'run' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Run this command'));
    await waitFor(() => {
      expect(tauri.codexExec).not.toHaveBeenCalled();
    });
  });

  it('guards config writes behind confirm', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.change(screen.getByPlaceholderText('key.path'), { target: { value: 'model' } });
    fireEvent.change(screen.getByPlaceholderText('"value", true, 123, or JSON'), {
      target: { value: '"gpt-5"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'write' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Write Codex config'));
    await waitFor(() => {
      expect(tauri.codexWriteConfigValue).not.toHaveBeenCalled();
    });
  });

  it('guards archive, unarchive, and memory reset behind confirm', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await screen.findByText('Thread one');

    fireEvent.click(screen.getByRole('button', { name: 'archive' }));
    fireEvent.click(screen.getByRole('button', { name: 'unarchive' }));
    fireEvent.click(screen.getByRole('button', { name: 'reset memories' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Archive Codex thread'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Unarchive Codex thread'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Reset all Codex memories'));
    expect(tauri.codexArchiveThread).not.toHaveBeenCalled();
    expect(tauri.codexUnarchiveThread).not.toHaveBeenCalled();
    expect(tauri.codexResetMemories).not.toHaveBeenCalled();
  });

  it('routes MCP tool calls to the selected thread without using thread id as adapter session id', async () => {
    confirmSpy.mockReturnValue(true);
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await screen.findByText('Thread one');
    fireEvent.change(screen.getAllByPlaceholderText('server name')[1], {
      target: { value: 'MiniMax' },
    });
    fireEvent.change(screen.getByPlaceholderText('tool name'), {
      target: { value: 'plan' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'call tool' }));

    await waitFor(() => {
      expect(tauri.codexMcpCallTool).toHaveBeenCalledWith(
        expect.objectContaining({
          sessionId: null,
          threadId: 'thread-1',
          server: 'MiniMax',
          tool: 'plan',
        }),
      );
    });
  });

  it('supports the official app-server passthrough request surface', async () => {
    confirmSpy.mockReturnValue(true);
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'request' }));

    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledWith(
        expect.stringContaining('Send Codex app-server request "model/list"'),
      );
      expect(tauri.codexAppServerRequest).toHaveBeenCalledWith({
        method: 'model/list',
        params: {},
      });
    });
  });
});
