import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { CostThresholdDialog } from './CostThresholdDialog';

afterEach(() => {
  cleanup();
});

describe('CostThresholdDialog', () => {
  it('renders dialog', () => {
    render(<CostThresholdDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('cost-threshold-dialog')).toBeInTheDocument();
  });

  it('shows cost threshold title', () => {
    render(<CostThresholdDialog onDone={vi.fn()} />);
    expect(screen.getByText('Cost Threshold Reached')).toBeInTheDocument();
  });

  it('shows default spend message', () => {
    render(<CostThresholdDialog onDone={vi.fn()} />);
    expect(screen.getByText(/You've spent \$5/)).toBeInTheDocument();
  });

  it('shows custom spend amount', () => {
    render(<CostThresholdDialog onDone={vi.fn()} currentSpend={12.5} />);
    expect(screen.getByText(/\$12\.50/)).toBeInTheDocument();
  });

  it('calls onDone when button is clicked', () => {
    const onDone = vi.fn();
    render(<CostThresholdDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('cost-dialog-done'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('shows documentation link', () => {
    render(<CostThresholdDialog onDone={vi.fn()} />);
    expect(screen.getByText('Cost documentation')).toBeInTheDocument();
  });
});
