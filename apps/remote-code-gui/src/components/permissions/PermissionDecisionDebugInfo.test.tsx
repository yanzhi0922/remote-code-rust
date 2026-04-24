import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PermissionDecisionDebugInfo } from './PermissionDecisionDebugInfo';

const baseDecision = {
  classifier: 'bash-classifier',
  rule: 'allow-ls',
  autoApproved: false,
  checkInProgress: false,
};

describe('PermissionDecisionDebugInfo', () => {
  afterEach(cleanup);

  it('renders nothing when verbose is false', () => {
    const { container } = render(
      <PermissionDecisionDebugInfo decision={baseDecision} verbose={false} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders debug info when verbose is true', () => {
    render(<PermissionDecisionDebugInfo decision={baseDecision} verbose={true} />);
    expect(screen.getByTestId('debug-info')).toBeInTheDocument();
  });

  it('shows classifier name', () => {
    render(<PermissionDecisionDebugInfo decision={baseDecision} verbose={true} />);
    expect(screen.getByText('bash-classifier')).toBeInTheDocument();
  });

  it('shows rule when provided', () => {
    render(<PermissionDecisionDebugInfo decision={baseDecision} verbose={true} />);
    expect(screen.getByText('allow-ls')).toBeInTheDocument();
  });

  it('hides rule when not provided', () => {
    const decision = { ...baseDecision, rule: undefined };
    render(<PermissionDecisionDebugInfo decision={decision} verbose={true} />);
    expect(screen.queryByText('allow-ls')).toBeNull();
  });

  it('shows auto-approved status', () => {
    render(<PermissionDecisionDebugInfo decision={baseDecision} verbose={true} />);
    const noElements = screen.getAllByText('No');
    expect(noElements.length).toBeGreaterThanOrEqual(1);
  });

  it('shows check in progress status', () => {
    const decision = { ...baseDecision, checkInProgress: true };
    render(<PermissionDecisionDebugInfo decision={decision} verbose={true} />);
    const elements = screen.getAllByText('Yes');
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });
});
