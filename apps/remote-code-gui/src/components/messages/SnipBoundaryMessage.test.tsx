import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SnipBoundaryMessage } from './SnipBoundaryMessage';

describe('SnipBoundaryMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<SnipBoundaryMessage />);
    expect(screen.getByTestId('snip-boundary-message')).toBeInTheDocument();
  });

  it('shows default snip text', () => {
    render(<SnipBoundaryMessage />);
    expect(screen.getByText('内容已剪切')).toBeInTheDocument();
  });

  it('shows entries removed count', () => {
    render(<SnipBoundaryMessage entriesRemoved={42} />);
    expect(screen.getByText('内容已剪切 (42 条)')).toBeInTheDocument();
  });

  it('shows summary when provided', () => {
    render(<SnipBoundaryMessage summary="old context removed" />);
    expect(screen.getByText(/old context removed/)).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(<SnipBoundaryMessage className="custom" />);
    expect(container.firstChild).toHaveClass('custom');
  });
});
