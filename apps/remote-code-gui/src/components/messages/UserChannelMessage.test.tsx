import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserChannelMessage } from './UserChannelMessage';

describe('UserChannelMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserChannelMessage channel="general" content="Hello" />);
    expect(screen.getByTestId('user-channel-message')).toBeInTheDocument();
  });

  it('displays channel name', () => {
    render(<UserChannelMessage channel="dev" content="msg" />);
    expect(screen.getByText('dev')).toBeInTheDocument();
  });

  it('displays content', () => {
    render(<UserChannelMessage channel="general" content="Hello world" />);
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('shows sender when provided', () => {
    render(<UserChannelMessage channel="general" content="msg" sender="Alice" />);
    expect(screen.getByText(/Alice/)).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <UserChannelMessage channel="c" content="m" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
