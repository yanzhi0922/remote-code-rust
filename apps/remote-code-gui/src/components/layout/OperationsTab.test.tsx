import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';

vi.mock('../../lib/tauri', () => ({
  listSessions: vi.fn<() => Promise<import('../../lib/types').SessionSummary[]>>(() => Promise.resolve([])),
  exportSessionBundle: vi.fn(() => Promise.resolve('/tmp/export.json')),
  exportDiagnosticBundle: vi.fn(() => Promise.resolve({ path: '/tmp/diagnostics', files: 2, bytes: 120 })),
  runDoctorReport: vi.fn(() => Promise.resolve({ status: 'healthy', checks: [] } as unknown as import('../../lib/types').DoctorReportInfo)),
}));

afterEach(() => { cleanup(); });

describe('OperationsTab', () => {
  it('renders Doctor section heading', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    expect(screen.getByText('Doctor')).toBeInTheDocument();
  });

  it('renders session export section', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    expect(screen.getByText('会话导出')).toBeInTheDocument();
  });

  it('renders diagnostics export section', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    expect(screen.getByText('诊断')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /导出诊断包/ })).toBeInTheDocument();
  });

  it('renders doctor run button', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    expect(screen.getByRole('button', { name: /重新诊断|诊断中/ })).toBeInTheDocument();
  });
});
