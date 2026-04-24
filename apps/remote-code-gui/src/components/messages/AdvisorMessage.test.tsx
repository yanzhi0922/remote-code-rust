import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AdvisorMessage } from './AdvisorMessage';

describe('AdvisorMessage', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<AdvisorMessage content="Try refactoring" />);
    expect(screen.getByTestId('advisor-message')).toBeInTheDocument();
  });

  it('displays content text', () => {
    render(<AdvisorMessage content="Consider using useMemo here" />);
    expect(screen.getByText('Consider using useMemo here')).toBeInTheDocument();
  });

  it('shows default sender when not provided', () => {
    render(<AdvisorMessage content="Tip" />);
    expect(screen.getByText('Advisor')).toBeInTheDocument();
  });

  it('shows custom sender', () => {
    render(<AdvisorMessage content="Tip" sender="Expert" />);
    expect(screen.getByText('Expert')).toBeInTheDocument();
  });

  it('shows timestamp when provided', () => {
    render(<AdvisorMessage content="Tip" timestamp="10:30" />);
    expect(screen.getByText('10:30')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <AdvisorMessage content="Tip" className="custom-class" />,
    );
    expect(container.firstChild).toHaveClass('custom-class');
  });
});
