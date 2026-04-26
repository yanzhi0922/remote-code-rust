import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ProgressBar } from './ProgressBar';

afterEach(() => { cleanup(); });

describe('ProgressBar', () => {
  it('renders with value', () => {
    const { container } = render(<ProgressBar value={50} />);
    const bar = container.querySelector('[style*="width"]');
    expect(bar).toBeInTheDocument();
  });

  it('renders with zero value', () => {
    const { container } = render(<ProgressBar value={0} />);
    expect(container.firstChild).toBeInTheDocument();
  });

  it('renders with full value', () => {
    const { container } = render(<ProgressBar value={100} />);
    expect(container.firstChild).toBeInTheDocument();
  });
});
