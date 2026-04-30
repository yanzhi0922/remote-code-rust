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
  codexThreadSetName: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadGoalSet: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadGoalGet: vi.fn(() => Promise.resolve({ text: 'goal' })),
  codexThreadGoalClear: vi.fn(() => Promise.resolve({})),
  codexThreadCompactStart: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadRollback: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadTurnsList: vi.fn(() => Promise.resolve({ data: [] })),
  codexTurnSteer: vi.fn(() => Promise.resolve({ ok: true })),
  codexTurnInterrupt: vi.fn(() => Promise.resolve({ ok: true })),
  codexModelList: vi.fn(() => Promise.resolve({ data: ['gpt-5'] })),
  codexCollaborationModeList: vi.fn(() => Promise.resolve({ data: [] })),
  codexExperimentalFeatureList: vi.fn(() => Promise.resolve({ data: [] })),
  codexExperimentalFeatureSet: vi.fn(() => Promise.resolve({ ok: true })),
  codexAccountRead: vi.fn(() => Promise.resolve({ id: 'acct' })),
  codexAccountRateLimitsRead: vi.fn(() => Promise.resolve({ data: [] })),
  codexAppsList: vi.fn(() => Promise.resolve({ data: [] })),
  codexSkillsList: vi.fn(() => Promise.resolve({ data: [] })),
  codexSkillsConfigWrite: vi.fn(() => Promise.resolve({ ok: true })),
  codexPluginList: vi.fn(() => Promise.resolve({ data: [] })),
  codexPluginRead: vi.fn(() => Promise.resolve({ id: 'plugin' })),
  codexPluginInstall: vi.fn(() => Promise.resolve({ ok: true })),
  codexPluginUninstall: vi.fn(() => Promise.resolve({ ok: true })),
  codexMarketplaceAdd: vi.fn(() => Promise.resolve({ ok: true })),
  codexMarketplaceRemove: vi.fn(() => Promise.resolve({ ok: true })),
  codexMarketplaceUpgrade: vi.fn(() => Promise.resolve({ ok: true })),
  codexMcpOAuthLogin: vi.fn(() => Promise.resolve({ ok: true })),
  codexMcpRefresh: vi.fn(() => Promise.resolve({})),
  codexMcpStatus: vi.fn(() => Promise.resolve({ data: [], nextCursor: null })),
  codexMcpReadResource: vi.fn(() => Promise.resolve({ contents: [] })),
  codexMcpCallTool: vi.fn(() => Promise.resolve({ content: [] })),
  codexReadConfig: vi.fn(() => Promise.resolve({ config: {}, origins: {}, layers: [] })),
  codexWriteConfigValue: vi.fn(() =>
    Promise.resolve({ status: 'written', version: '1', filePath: 'config.toml', overriddenMetadata: null }),
  ),
  codexWriteConfigBatch: vi.fn(() =>
    Promise.resolve({ status: 'written', version: '2', filePath: 'config.toml', overriddenMetadata: null }),
  ),
  codexUploadFeedback: vi.fn(() => Promise.resolve({ threadId: 'thread-1' })),
  codexSetThreadMemoryMode: vi.fn(() => Promise.resolve({})),
  codexResetMemories: vi.fn(() => Promise.resolve({})),
  codexThreadStart: vi.fn(() => Promise.resolve({ threadId: 'thread-2' })),
  codexThreadUnsubscribe: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadElicitationIncrement: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadElicitationDecrement: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadMetadataUpdate: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadShellCommand: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadBackgroundTerminalsClean: vi.fn(() => Promise.resolve({ ok: true })),
  codexThreadLoadedList: vi.fn(() => Promise.resolve({ data: [] })),
  codexThreadInjectItems: vi.fn(() => Promise.resolve({ ok: true })),
  codexTurnStart: vi.fn(() => Promise.resolve({ ok: true })),
  codexAccountLogin: vi.fn(() => Promise.resolve({ ok: true })),
  codexAccountLoginCancel: vi.fn(() => Promise.resolve({ ok: true })),
  codexAccountLogout: vi.fn(() => Promise.resolve({ ok: true })),
  codexAccountAddCreditsNudge: vi.fn(() => Promise.resolve({ ok: true })),
  codexConfigRequirementsRead: vi.fn(() => Promise.resolve({ requirements: [] })),
  codexExternalAgentConfigDetect: vi.fn(() => Promise.resolve({ data: [] })),
  codexExternalAgentConfigImport: vi.fn(() => Promise.resolve({ ok: true })),
  codexWindowsSandboxSetupStart: vi.fn(() => Promise.resolve({ ok: true })),
  codexRealtimeStart: vi.fn(() => Promise.resolve({ ok: true })),
  codexRealtimeAppendText: vi.fn(() => Promise.resolve({ ok: true })),
  codexRealtimeStop: vi.fn(() => Promise.resolve({ ok: true })),
  codexRealtimeVoicesList: vi.fn(() => Promise.resolve({ data: [] })),
  codexDeviceKeyCreate: vi.fn(() => Promise.resolve({ ok: true })),
  codexDeviceKeyPublic: vi.fn(() => Promise.resolve({ ok: true })),
  codexDeviceKeySign: vi.fn(() => Promise.resolve({ signature: 'sig' })),
  codexFsReadFile: vi.fn(() => Promise.resolve({ contents: 'hello' })),
  codexFsWriteFile: vi.fn(() => Promise.resolve({ ok: true })),
  codexFsCreateDirectory: vi.fn(() => Promise.resolve({ ok: true })),
  codexFsGetMetadata: vi.fn(() => Promise.resolve({ isFile: true })),
  codexFsReadDirectory: vi.fn(() => Promise.resolve({ entries: [] })),
  codexFsRemove: vi.fn(() => Promise.resolve({ ok: true })),
  codexFsCopy: vi.fn(() => Promise.resolve({ ok: true })),
  codexFsWatch: vi.fn(() => Promise.resolve({ watchId: 'watch-1' })),
  codexFsUnwatch: vi.fn(() => Promise.resolve({ ok: true })),
  codexFuzzyFileSearch: vi.fn(() => Promise.resolve({ matches: [] })),
  codexFuzzyFileSearchSessionStart: vi.fn(() => Promise.resolve({ sessionId: 'search-1' })),
  codexFuzzyFileSearchSessionUpdate: vi.fn(() => Promise.resolve({ matches: [] })),
  codexFuzzyFileSearchSessionStop: vi.fn(() => Promise.resolve({ ok: true })),
  codexReviewStart: vi.fn(() => Promise.resolve({ ok: true })),
  codexAppServerRequest: vi.fn(() => Promise.resolve({ ok: true })),
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

  async function selectThread() {
    fireEvent.click(screen.getByRole('button', { name: 'Refresh threads' }));
    await screen.findByText('Thread one');
  }

  it('calls the typed model list wrapper from Discovery', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'List models' }));

    await waitFor(() => {
      expect(tauri.codexModelList).toHaveBeenCalledTimes(1);
      expect(tauri.codexAppServerRequest).not.toHaveBeenCalled();
    });
  });

  it('sets thread name through the typed wrapper instead of raw app-server', async () => {
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.change(screen.getByPlaceholderText('New thread name'), {
      target: { value: 'Renamed thread' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Set name' }));

    await waitFor(() => {
      expect(tauri.codexThreadSetName).toHaveBeenCalledWith({
        sessionId: null,
        threadId: 'thread-1',
        name: 'Renamed thread',
      });
      expect(tauri.codexAppServerRequest).not.toHaveBeenCalled();
    });
  });

  it('calls the typed MCP OAuth wrapper', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.change(screen.getByPlaceholderText('MCP OAuth server'), {
      target: { value: 'github' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'MCP OAuth' }));

    await waitFor(() => {
      expect(tauri.codexMcpOAuthLogin).toHaveBeenCalledWith({ sessionId: null, server: 'github' });
      expect(tauri.codexAppServerRequest).not.toHaveBeenCalled();
    });
  });

  it('lists skills with forceReload instead of the removed includeBundled flag', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.change(screen.getByPlaceholderText('Optional CWDs, one per line'), {
      target: { value: 'C:\\repo\nD:\\other' },
    });
    fireEvent.click(screen.getByLabelText('force reload skills'));
    fireEvent.click(screen.getByRole('button', { name: 'List skills' }));

    await waitFor(() => {
      expect(tauri.codexSkillsList).toHaveBeenCalledWith({
        cwds: ['C:\\repo', 'D:\\other'],
        forceReload: true,
      });
    });
    expect(tauri.codexSkillsList).not.toHaveBeenCalledWith(expect.objectContaining({ includeBundled: expect.anything() }));
  });

  it('calls MCP refresh, status, resource, and tool wrappers with official parameters', async () => {
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.change(screen.getByPlaceholderText('MCP server'), {
      target: { value: 'MiniMax' },
    });
    fireEvent.change(screen.getByPlaceholderText('status limit'), {
      target: { value: '25' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'MCP refresh' }));
    fireEvent.click(screen.getByRole('button', { name: 'MCP status' }));

    fireEvent.change(screen.getByPlaceholderText('MCP resource URI'), {
      target: { value: 'memory://facts' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read resource' }));

    fireEvent.change(screen.getByPlaceholderText('MCP tool'), {
      target: { value: 'plan' },
    });
    fireEvent.change(screen.getByPlaceholderText('{"key":"value"}'), {
      target: { value: '{"task":"audit"}' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Call tool' }));

    await waitFor(() => {
      expect(tauri.codexMcpRefresh).toHaveBeenCalledWith(null);
      expect(tauri.codexMcpStatus).toHaveBeenCalledWith({ sessionId: null, detail: 'full', limit: 50 });
      expect(tauri.codexMcpStatus).toHaveBeenCalledWith({ sessionId: null, detail: 'full', limit: 25 });
      expect(tauri.codexMcpReadResource).toHaveBeenCalledWith({
        sessionId: null,
        server: 'MiniMax',
        uri: 'memory://facts',
      });
      expect(tauri.codexMcpCallTool).toHaveBeenCalledWith({
        sessionId: null,
        threadId: 'thread-1',
        server: 'MiniMax',
        tool: 'plan',
        arguments: { task: 'audit' },
      });
    });
  });

  it('reads config and guards typed config writes with parsed payloads', async () => {
    confirmSpy.mockReturnValue(true);
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Read config' }));
    fireEvent.change(screen.getByPlaceholderText('config key path'), {
      target: { value: 'model' },
    });
    fireEvent.change(screen.getByPlaceholderText('"value", true, 123, or JSON'), {
      target: { value: '"gpt-5"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Write value' }));
    fireEvent.change(screen.getByPlaceholderText('[{"keyPath":"model","value":"gpt-5"}]'), {
      target: { value: '[{"keyPath":"sandbox.mode","value":"workspace-write","mergeStrategy":"replace"}]' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Write batch' }));

    await waitFor(() => {
      expect(tauri.codexReadConfig).toHaveBeenCalledWith(true);
      expect(tauri.codexWriteConfigValue).toHaveBeenCalledWith({
        keyPath: 'model',
        value: 'gpt-5',
        mergeStrategy: 'replace',
      });
      expect(tauri.codexWriteConfigBatch).toHaveBeenCalledWith({
        edits: [{ keyPath: 'sandbox.mode', value: 'workspace-write', mergeStrategy: 'replace' }],
        reloadUserConfig: true,
      });
    });
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Write Codex config value "model"'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Write 1 Codex config edit'));
  });

  it('sets thread memory mode, uploads feedback, and guards memory reset', async () => {
    confirmSpy.mockReturnValue(true);
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.click(screen.getByRole('button', { name: 'Set memory mode' }));
    fireEvent.change(screen.getByPlaceholderText('classification'), {
      target: { value: 'bug' },
    });
    fireEvent.change(screen.getByPlaceholderText('feedback reason'), {
      target: { value: 'native bridge check' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Upload feedback' }));
    fireEvent.click(screen.getByRole('button', { name: 'Reset memories' }));

    await waitFor(() => {
      expect(tauri.codexSetThreadMemoryMode).toHaveBeenCalledWith({
        sessionId: null,
        threadId: 'thread-1',
        enabled: true,
      });
      expect(tauri.codexUploadFeedback).toHaveBeenCalledWith({
        classification: 'bug',
        reason: 'native bridge check',
        threadId: 'thread-1',
        includeLogs: true,
      });
      expect(tauri.codexResetMemories).toHaveBeenCalledTimes(1);
    });
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Reset all Codex memories'));
  });

  it('keeps dangerous operations behind confirm', async () => {
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.click(screen.getByRole('button', { name: 'archive' }));
    fireEvent.click(screen.getByRole('button', { name: 'Clear goal' }));
    fireEvent.change(screen.getByPlaceholderText('rollback turns'), { target: { value: '1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Rollback' }));
    fireEvent.change(screen.getByPlaceholderText('plugin source'), { target: { value: 'https://example.test/plugin' } });
    fireEvent.click(screen.getByRole('button', { name: 'Install plugin' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Archive Codex thread'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Clear goal'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Rollback 1 turn'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Install plugin'));
    expect(tauri.codexArchiveThread).not.toHaveBeenCalled();
    expect(tauri.codexThreadGoalClear).not.toHaveBeenCalled();
    expect(tauri.codexThreadRollback).not.toHaveBeenCalled();
    expect(tauri.codexPluginInstall).not.toHaveBeenCalled();
  });

  it('exposes Advanced Native thread and realtime wrappers', async () => {
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.click(screen.getByRole('button', { name: 'Loaded threads' }));
    fireEvent.change(screen.getByPlaceholderText('Realtime text or turn prompt'), {
      target: { value: 'hello realtime' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Realtime voices' }));
    fireEvent.click(screen.getByRole('button', { name: 'Realtime start' }));
    fireEvent.click(screen.getByRole('button', { name: 'Append text' }));
    fireEvent.click(screen.getByRole('button', { name: 'Realtime stop' }));

    await waitFor(() => {
      expect(tauri.codexThreadLoadedList).toHaveBeenCalledWith({
        sessionId: null,
        threadId: 'thread-1',
        params: {},
      });
      expect(tauri.codexRealtimeVoicesList).toHaveBeenCalledTimes(1);
      expect(tauri.codexRealtimeStart).toHaveBeenCalledWith({ params: {} });
      expect(tauri.codexRealtimeAppendText).toHaveBeenCalledWith({
        text: 'hello realtime',
        params: {},
      });
      expect(tauri.codexRealtimeStop).toHaveBeenCalledTimes(1);
    });
  });

  it('guards Advanced Native shell and filesystem mutations', async () => {
    render(<CodexOperationsPanel />);
    await selectThread();

    fireEvent.change(screen.getByPlaceholderText('Thread shell command'), {
      target: { value: 'whoami' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Thread shell' }));
    fireEvent.change(screen.getByPlaceholderText('FS path or fuzzy cwd'), {
      target: { value: 'C:\\repo\\file.txt' },
    });
    fireEvent.change(screen.getByPlaceholderText('Write file contents'), {
      target: { value: 'changed' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'FS write' }));
    fireEvent.click(screen.getByRole('button', { name: 'FS remove' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Run unsandboxed Codex thread shell command'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Write file'));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Remove'));
    expect(tauri.codexThreadShellCommand).not.toHaveBeenCalled();
    expect(tauri.codexFsWriteFile).not.toHaveBeenCalled();
    expect(tauri.codexFsRemove).not.toHaveBeenCalled();
  });

  it('calls Advanced Native config, account, filesystem read, and fuzzy wrappers', async () => {
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Account login' }));
    fireEvent.click(screen.getByRole('button', { name: 'Config requirements' }));
    fireEvent.click(screen.getByRole('button', { name: 'External detect' }));
    fireEvent.change(screen.getByPlaceholderText('FS path or fuzzy cwd'), {
      target: { value: 'C:\\repo' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'FS dir' }));
    fireEvent.change(screen.getByPlaceholderText('Fuzzy file query'), {
      target: { value: 'main.rs' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Fuzzy search' }));

    await waitFor(() => {
      expect(tauri.codexAccountLogin).toHaveBeenCalledWith({ params: {} });
      expect(tauri.codexConfigRequirementsRead).toHaveBeenCalledTimes(1);
      expect(tauri.codexExternalAgentConfigDetect).toHaveBeenCalledTimes(1);
      expect(tauri.codexFsReadDirectory).toHaveBeenCalledWith({ path: 'C:\\repo', params: {} });
      expect(tauri.codexFuzzyFileSearch).toHaveBeenCalledWith({
        query: 'main.rs',
        cwd: 'C:\\repo',
        params: {},
      });
    });
  });

  it('still exposes the guarded raw app-server escape hatch', async () => {
    confirmSpy.mockReturnValue(true);
    render(<CodexOperationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Raw request' }));

    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledWith(
        expect.stringContaining('Send raw Codex app-server request "model/list"'),
      );
      expect(tauri.codexAppServerRequest).toHaveBeenCalledWith({
        method: 'model/list',
        params: {},
      });
    });
  });
});
