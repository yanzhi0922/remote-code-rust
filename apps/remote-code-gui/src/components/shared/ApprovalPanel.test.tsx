import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ApprovalPanel, type ApprovalPanelProps } from './ApprovalPanel';

afterEach(() => { cleanup(); });

const baseProps: ApprovalPanelProps = {
  title: 'Approvals',
  icon: <span>🔔</span>,
  emptyText: 'No pending approvals',
  items: [],
  actions: [],
  approvingId: null,
  loadingText: 'Processing...',
  onDecision: () => {},
};

describe('ApprovalPanel', () => {
  it('renders title', () => {
    render(<ApprovalPanel {...baseProps} />);
    expect(screen.getByText('Approvals')).toBeInTheDocument();
  });

  it('shows empty text when no items', () => {
    render(<ApprovalPanel {...baseProps} />);
    expect(screen.getByText('No pending approvals')).toBeInTheDocument();
  });

  it('renders approval items', () => {
    render(
      <ApprovalPanel
        {...baseProps}
        items={[{ approval_id: '1', title: 'Allow bash', description: 'Run command', metadata: {} }]}
        actions={[{ decision: 'allow', label: 'Allow', className: 'bg-green-500' }]}
      />,
    );
    expect(screen.getByText('Allow bash')).toBeInTheDocument();
  });
});
