import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { UndercoverAutoCallout } from './UndercoverAutoCallout';

afterEach(() => {
  cleanup();
});

describe('UndercoverAutoCallout', () => {
  it('renders nothing when inactive', () => {
    const { container } = render(<UndercoverAutoCallout active={false} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders callout when active', () => {
    render(<UndercoverAutoCallout active={true} />);
    expect(screen.getByTestId('undercover-auto-callout')).toBeInTheDocument();
  });

  it('calls onDismiss', () => {
    const onDismiss = vi.fn();
    render(<UndercoverAutoCallout active={true} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByText('关闭'));
    expect(onDismiss).toHaveBeenCalled();
  });
});
