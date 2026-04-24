import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TeamMemCollapsed } from './teamMemCollapsed';

describe('TeamMemCollapsed', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<TeamMemCollapsed members={['Alice', 'Bob']} />);
    expect(screen.getByTestId('team-mem-collapsed')).toBeInTheDocument();
  });

  it('shows member count', () => {
    render(<TeamMemCollapsed members={['Alice', 'Bob', 'Charlie']} />);
    expect(screen.getByText('3 成员')).toBeInTheDocument();
  });

  it('shows all members when within limit', () => {
    render(<TeamMemCollapsed members={['Alice', 'Bob']} maxVisible={3} />);
    expect(screen.getByText('Alice')).toBeInTheDocument();
    expect(screen.getByText('Bob')).toBeInTheDocument();
  });

  it('shows expand button when members exceed limit', () => {
    render(
      <TeamMemCollapsed members={['A', 'B', 'C', 'D', 'E']} maxVisible={3} />,
    );
    expect(screen.getByTestId('team-mem-expand')).toBeInTheDocument();
    expect(screen.getByTestId('team-mem-expand').textContent).toBe('+2');
  });

  it('expands to show all members', () => {
    render(
      <TeamMemCollapsed members={['A', 'B', 'C', 'D']} maxVisible={2} />,
    );
    fireEvent.click(screen.getByTestId('team-mem-expand'));
    expect(screen.getByText('D')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <TeamMemCollapsed members={['A']} className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
