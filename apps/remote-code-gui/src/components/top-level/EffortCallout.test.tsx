import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { EffortCallout } from './EffortCallout';

afterEach(() => {
  cleanup();
});

describe('EffortCallout', () => {
  it('renders effort callout container', () => {
    render(<EffortCallout level="medium" />);
    expect(screen.getByTestId('effort-callout')).toBeInTheDocument();
  });

  it('shows all three effort options', () => {
    render(<EffortCallout level="medium" />);
    expect(screen.getByTestId('effort-low')).toBeInTheDocument();
    expect(screen.getByTestId('effort-medium')).toBeInTheDocument();
    expect(screen.getByTestId('effort-high')).toBeInTheDocument();
  });

  it('highlights selected level', () => {
    render(<EffortCallout level="high" />);
    expect(screen.getByTestId('effort-high').classList.contains('bg-blue-100')).toBe(true);
  });

  it('calls onDone when option is clicked', () => {
    const onDone = vi.fn();
    render(<EffortCallout level="medium" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('effort-low'));
    expect(onDone).toHaveBeenCalledWith('low');
  });

  it('shows effort symbols in legend', () => {
    render(<EffortCallout level="medium" />);
    // The symbols ○, ◐, ● should be visible
    expect(screen.getByTestId('effort-callout').textContent).toContain('○');
    expect(screen.getByTestId('effort-callout').textContent).toContain('◐');
    expect(screen.getByTestId('effort-callout').textContent).toContain('●');
  });

  it('renders descriptions for each level', () => {
    render(<EffortCallout level="medium" />);
    expect(screen.getByText(/Quick responses/)).toBeInTheDocument();
    expect(screen.getByText(/Balanced speed/)).toBeInTheDocument();
    expect(screen.getByText(/Thorough analysis/)).toBeInTheDocument();
  });
});
