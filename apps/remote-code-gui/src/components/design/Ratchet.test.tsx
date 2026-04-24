import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Ratchet } from './Ratchet';

describe('Ratchet', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<Ratchet value={0} max={10} />);
    expect(screen.getByTestId('ratchet')).toBeInTheDocument();
  });

  it('renders track and fill elements', () => {
    render(<Ratchet value={5} max={10} />);
    expect(screen.getByTestId('ratchet-track')).toBeInTheDocument();
    expect(screen.getByTestId('ratchet-fill')).toBeInTheDocument();
  });

  it('shows value display', () => {
    render(<Ratchet value={3} max={10} />);
    expect(screen.getByTestId('ratchet-value')).toHaveTextContent('3 / 10');
  });

  it('renders label when provided', () => {
    render(<Ratchet value={5} max={10} label="进度" />);
    expect(screen.getByTestId('ratchet-label')).toHaveTextContent('进度');
  });

  it('does not render label element when not provided', () => {
    render(<Ratchet value={5} max={0} />);
    expect(screen.queryByTestId('ratchet-label')).not.toBeInTheDocument();
  });

  it('applies blue fill for low progress', () => {
    render(<Ratchet value={2} max={10} />);
    expect(screen.getByTestId('ratchet-fill').className).toContain('bg-blue-500');
  });

  it('applies green fill for complete progress', () => {
    render(<Ratchet value={10} max={10} />);
    expect(screen.getByTestId('ratchet-fill').className).toContain('bg-green-500');
  });

  it('applies custom className', () => {
    render(<Ratchet value={5} max={10} className="custom" />);
    expect(screen.getByTestId('ratchet').className).toContain('custom');
  });
});
