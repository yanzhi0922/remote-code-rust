import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { StatusNotices, type StatusNotice } from './StatusNotices';

describe('StatusNotices', () => {
  afterEach(() => { cleanup(); });

  it('returns null when no notices', () => {
    const { container } = render(<StatusNotices notices={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders notices with messages', () => {
    const notices: StatusNotice[] = [
      { id: '1', type: 'info', message: 'Info message' },
      { id: '2', type: 'warning', message: 'Warning message' },
      { id: '3', type: 'error', message: 'Error message' },
    ];
    const { getByTestId, getByText } = render(
      <StatusNotices notices={notices} />,
    );
    expect(getByTestId('status-notices')).toBeInTheDocument();
    expect(getByText('Info message')).toBeInTheDocument();
    expect(getByText('Warning message')).toBeInTheDocument();
    expect(getByText('Error message')).toBeInTheDocument();
  });

  it('shows dismiss button for dismissible notices', () => {
    const notices: StatusNotice[] = [
      { id: 'a', type: 'info', message: 'Dismissible', dismissible: true },
    ];
    const { getByTestId } = render(
      <StatusNotices notices={notices} onDismiss={() => {}} />,
    );
    expect(getByTestId('dismiss-notice-a')).toBeInTheDocument();
  });

  it('calls onDismiss when dismiss clicked', () => {
    const onDismiss = vi.fn();
    const notices: StatusNotice[] = [
      { id: 'x', type: 'warning', message: 'test', dismissible: true },
    ];
    const { getByTestId } = render(
      <StatusNotices notices={notices} onDismiss={onDismiss} />,
    );
    fireEvent.click(getByTestId('dismiss-notice-x'));
    expect(onDismiss).toHaveBeenCalledWith('x');
  });
});
