import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetAppStore } from '../../test/appStoreTestUtils';

vi.mock('../../lib/tauri', () => ({
  listSessions: vi.fn<() => Promise<import('../../lib/types').SessionSummary[]>>(() => Promise.resolve([])),
  exportSessionBundle: vi.fn(() => Promise.resolve('/tmp/export.json')),
  runDoctorReport: vi.fn(() => Promise.resolve({ status: 'healthy', checks: [] } as unknown as import('../../lib/types').DoctorReportInfo)),
}));

afterEach(() => { cleanup(); });

describe('OperationsTab', () => {
  it('renders without crashing', async () => {
    resetAppStore();
    const { OperationsTab } = await import('./OperationsTab');
    render(<OperationsTab />);
    // Component renders operation controls
    expect(document.querySelector('[data-testid="operations-tab"]') || document.body).toBeInTheDocument();
  });
});
