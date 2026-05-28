import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';

vi.mock('../../lib/tauri', () => ({
  listSessions: vi.fn<() => Promise<import('../../lib/types').SessionSummary[]>>(() => Promise.resolve([])),
  exportSessionBundle: vi.fn(() => Promise.resolve('/tmp/export.json')),
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
    expect(screen.getByText('Session Export')).toBeInTheDocument();
  });

  it('renders doctor run button', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    expect(screen.getByRole('button', { name: /重新诊断|诊断中/ })).toBeInTheDocument();
  });
});
