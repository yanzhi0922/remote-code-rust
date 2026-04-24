import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PrBadge } from './PrBadge';

afterEach(() => {
  cleanup();
});

describe('PrBadge', () => {
  it('renders PR number', () => {
    render(<PrBadge prNumber={42} />);
    expect(screen.getByTestId('pr-badge')).toBeInTheDocument();
    expect(screen.getByText('#42')).toBeInTheDocument();
  });

  it('renders PR title', () => {
    render(<PrBadge prNumber={42} prTitle="Fix bug" />);
    expect(screen.getByText('Fix bug')).toBeInTheDocument();
  });

  it('renders as link when url provided', () => {
    render(<PrBadge prNumber={42} url="https://github.com/pr/42" />);
    const link = screen.getByTestId('pr-badge').closest('a');
    expect(link).toHaveAttribute('href', 'https://github.com/pr/42');
  });
});
