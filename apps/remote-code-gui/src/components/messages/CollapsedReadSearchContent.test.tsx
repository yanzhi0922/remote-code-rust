import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CollapsedReadSearchContent } from './CollapsedReadSearchContent';

describe('CollapsedReadSearchContent', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<CollapsedReadSearchContent query="test query" />);
    expect(screen.getByTestId('collapsed-search-content')).toBeInTheDocument();
  });

  it('displays query text', () => {
    render(<CollapsedReadSearchContent query="find me" />);
    expect(screen.getByText('find me')).toBeInTheDocument();
  });

  it('displays result count when provided', () => {
    render(<CollapsedReadSearchContent query="q" resultCount={5} />);
    expect(screen.getByText('5 条结果')).toBeInTheDocument();
  });

  it('does not show results when collapsed', () => {
    render(
      <CollapsedReadSearchContent query="q" results={['r1', 'r2']} />,
    );
    expect(screen.queryByText('r1')).not.toBeInTheDocument();
  });

  it('expands to show results on click', () => {
    render(
      <CollapsedReadSearchContent query="q" results={['result-a', 'result-b']} />,
    );
    fireEvent.click(screen.getByTestId('collapsed-search-toggle'));
    expect(screen.getByText('result-a')).toBeInTheDocument();
    expect(screen.getByText('result-b')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <CollapsedReadSearchContent query="q" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
