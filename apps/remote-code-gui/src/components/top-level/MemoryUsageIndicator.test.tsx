import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryUsageIndicator } from './MemoryUsageIndicator';

afterEach(() => {
  cleanup();
});

describe('MemoryUsageIndicator', () => {
  it('returns null for normal status', () => {
    const { container } = render(
      <MemoryUsageIndicator heapUsed={100} status="normal" />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders for high status', () => {
    render(<MemoryUsageIndicator heapUsed={500000000} status="high" />);
    expect(screen.getByTestId('memory-usage-indicator')).toBeInTheDocument();
  });

  it('renders for critical status', () => {
    render(<MemoryUsageIndicator heapUsed={1000000000} status="critical" />);
    expect(screen.getByTestId('memory-usage-indicator')).toBeInTheDocument();
  });

  it('shows yellow color for high status', () => {
    render(<MemoryUsageIndicator heapUsed={500000000} status="high" />);
    const el = screen.getByTestId('memory-usage-indicator');
    expect(el.classList.contains('text-yellow-500')).toBe(true);
  });

  it('shows red color for critical status', () => {
    render(<MemoryUsageIndicator heapUsed={1000000000} status="critical" />);
    const el = screen.getByTestId('memory-usage-indicator');
    expect(el.classList.contains('text-red-500')).toBe(true);
  });

  it('formats heap size correctly', () => {
    render(<MemoryUsageIndicator heapUsed={536870912} status="high" />);
    expect(screen.getByText(/512\.0 MB/)).toBeInTheDocument();
  });
});
