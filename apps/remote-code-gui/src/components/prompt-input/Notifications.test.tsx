import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Notifications } from './Notifications';

afterEach(() => {
  cleanup();
});

describe('Notifications', () => {
  it('renders nothing when empty', () => {
    const { container } = render(<Notifications notifications={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders notifications', () => {
    const notifications = [
      { id: '1', type: 'info' as const, message: '信息通知' },
      { id: '2', type: 'warning' as const, message: '警告通知' },
    ];
    render(<Notifications notifications={notifications} />);
    expect(screen.getByTestId('notifications')).toBeInTheDocument();
    expect(screen.getByText('信息通知')).toBeInTheDocument();
    expect(screen.getByText('警告通知')).toBeInTheDocument();
  });

  it('calls onDismiss', () => {
    const onDismiss = vi.fn();
    const notifications = [{ id: '1', type: 'info' as const, message: 'test' }];
    render(<Notifications notifications={notifications} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByTestId('notification-1').querySelector('button')!);
    expect(onDismiss).toHaveBeenCalledWith('1');
  });
});
