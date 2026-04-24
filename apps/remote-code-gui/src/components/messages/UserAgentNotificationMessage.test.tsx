import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserAgentNotificationMessage } from './UserAgentNotificationMessage';

describe('UserAgentNotificationMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserAgentNotificationMessage content="Agent updated" />);
    expect(screen.getByTestId('user-agent-notification-message')).toBeInTheDocument();
  });

  it('displays content text', () => {
    render(<UserAgentNotificationMessage content="New version available" />);
    expect(screen.getByText('New version available')).toBeInTheDocument();
  });

  it('shows notification type badge', () => {
    render(
      <UserAgentNotificationMessage content="msg" notificationType="update" />,
    );
    expect(screen.getByText('update')).toBeInTheDocument();
  });

  it('hides badge when no type', () => {
    const { container } = render(
      <UserAgentNotificationMessage content="msg" />,
    );
    expect(container.querySelector('span')).toBeNull();
  });

  it('applies custom className', () => {
    const { container } = render(
      <UserAgentNotificationMessage content="msg" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
