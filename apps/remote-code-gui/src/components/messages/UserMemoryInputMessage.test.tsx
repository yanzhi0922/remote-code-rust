import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserMemoryInputMessage } from './UserMemoryInputMessage';

describe('UserMemoryInputMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<UserMemoryInputMessage content="remember this" />);
    expect(screen.getByTestId('user-memory-input-message')).toBeInTheDocument();
  });

  it('displays content', () => {
    render(<UserMemoryInputMessage content="important fact" />);
    expect(screen.getByText('important fact')).toBeInTheDocument();
  });

  it('shows memory key', () => {
    render(<UserMemoryInputMessage content="c" memoryKey="user-pref" />);
    expect(screen.getByText('user-pref')).toBeInTheDocument();
  });

  it('shows operation label', () => {
    render(<UserMemoryInputMessage content="c" operation="save" />);
    expect(screen.getByText(/保存/)).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <UserMemoryInputMessage content="c" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
