import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
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

  it('calls onDecision when action button is clicked', async () => {
    const onDecision = vi.fn();
    render(
      <ApprovalPanel
        {...baseProps}
        items={[{ approval_id: '1', title: 'Allow bash', description: 'Run command', metadata: {} }]}
        actions={[{ decision: 'allow', label: 'Allow', className: 'bg-green-500' }]}
        onDecision={onDecision}
      />,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Allow' }));
    expect(onDecision).toHaveBeenCalledWith('1', 'allow');
  });

  it('shows loading text when approvingId matches an item', () => {
    render(
      <ApprovalPanel
        {...baseProps}
        items={[{ approval_id: '1', title: 'Allow bash', description: 'Run command', metadata: {} }]}
        actions={[{ decision: 'allow', label: 'Allow', className: 'bg-green-500' }]}
        approvingId="1"
      />,
    );
    expect(screen.getByText('Processing...')).toBeInTheDocument();
  });
});
