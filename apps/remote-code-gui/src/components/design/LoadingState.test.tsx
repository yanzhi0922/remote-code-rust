import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { LoadingState } from './LoadingState';

describe('LoadingState', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<LoadingState />);
    expect(screen.getByTestId('loading-state')).toBeInTheDocument();
  });

  it('renders spinner', () => {
    render(<LoadingState />);
    expect(screen.getByTestId('loading-spinner')).toBeInTheDocument();
  });

  it('renders message when provided', () => {
    render(<LoadingState message="加载中..." />);
    expect(screen.getByTestId('loading-message')).toHaveTextContent('加载中...');
  });

  it('does not render message element when not provided', () => {
    render(<LoadingState />);
    expect(screen.queryByTestId('loading-message')).not.toBeInTheDocument();
  });

  it('applies small size', () => {
    render(<LoadingState size="sm" />);
    const spinner = screen.getByTestId('loading-spinner');
    expect(spinner.getAttribute('class')).toContain('h-4');
  });

  it('applies medium size by default', () => {
    render(<LoadingState />);
    const spinner = screen.getByTestId('loading-spinner');
    expect(spinner.getAttribute('class')).toContain('h-6');
  });

  it('applies large size', () => {
    render(<LoadingState size="lg" />);
    const spinner = screen.getByTestId('loading-spinner');
    expect(spinner.getAttribute('class')).toContain('h-10');
  });

  it('applies custom className', () => {
    render(<LoadingState className="my-class" />);
    expect(screen.getByTestId('loading-state').className).toContain('my-class');
  });
});
