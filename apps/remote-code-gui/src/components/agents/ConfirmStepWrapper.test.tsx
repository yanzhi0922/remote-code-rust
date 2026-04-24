import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ConfirmStepWrapper } from './ConfirmStepWrapper';

afterEach(() => {
  cleanup();
});

describe('ConfirmStepWrapper', () => {
  it('renders children when not confirmed', () => {
    render(<ConfirmStepWrapper confirmed={false}>Content</ConfirmStepWrapper>);
    expect(screen.getByText('Content')).toBeInTheDocument();
  });

  it('shows confirmed state', () => {
    render(<ConfirmStepWrapper confirmed={true}>Content</ConfirmStepWrapper>);
    expect(screen.getByTestId('confirm-step-confirmed')).toBeInTheDocument();
  });

  it('calls onConfirm', () => {
    const onConfirm = vi.fn();
    render(<ConfirmStepWrapper confirmed={false} onConfirm={onConfirm}>Content</ConfirmStepWrapper>);
    fireEvent.click(screen.getByTestId('confirm-step-confirm'));
    expect(onConfirm).toHaveBeenCalled();
  });

  it('calls onCancel', () => {
    const onCancel = vi.fn();
    render(<ConfirmStepWrapper confirmed={false} onCancel={onCancel}>Content</ConfirmStepWrapper>);
    fireEvent.click(screen.getByTestId('confirm-step-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });
});
