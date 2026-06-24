import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';
import { useAgentStore } from '../../stores/useAgentStore';
import { CommandPalette } from './CommandPalette';

vi.mock('../../lib/tauri', () => ({
  runDoctorReport: vi.fn(),
  exportSessionBundle: vi.fn(),
  exportDiagnosticBundle: vi.fn(),
  codexAdapterStop: vi.fn(),
  codexAdapterRestart: vi.fn(),
}));

function renderPalette() {
  return render(
    <CommandPalette
      open
      onClose={vi.fn()}
      onNewSession={vi.fn()}
      onAddProject={vi.fn()}
      onOpenSettings={vi.fn()}
      onOpenMcp={vi.fn()}
      onToggleTheme={vi.fn()}
    />,
  );
}

describe('CommandPalette agent-specialized commands', () => {
  beforeEach(() => {
    resetAppStore();
    useAgentStore.setState({ activeAgentType: null });
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    useAgentStore.setState({ activeAgentType: null });
    vi.clearAllMocks();
  });

  it('switches Claude into safe edit mode from the palette', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    resetAppStore({ activeSessionId: 'session-1', updateSettings });
    useAgentStore.setState({ activeAgentType: 'remote_claude' });

    renderPalette();

    fireEvent.click(screen.getByRole('button', { name: /Claude 安全编辑模式/ }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ permission_mode: 'acceptEdits' });
    });
  });

  it('switches Roo into orchestrator mode from the palette', async () => {
    const updateSettings = vi.fn().mockResolvedValue(undefined);
    resetAppStore({ activeSessionId: 'session-1', updateSettings });
    useAgentStore.setState({ activeAgentType: 'remote_roo' });

    renderPalette();

    fireEvent.click(screen.getByRole('button', { name: /Roo Orchestrator 模式/ }));

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({ roo_mode: 'orchestrator' });
    });
  });
});
