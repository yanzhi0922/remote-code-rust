import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { FastIcon } from './FastIcon';

describe('FastIcon', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    const { getByTestId } = render(<FastIcon />);
    expect(getByTestId('fast-icon')).toBeInTheDocument();
  });

  it('renders the icon element', () => {
    const { getByTestId } = render(<FastIcon />);
    const icon = getByTestId('fast-icon');
    // SVG element rendered
    expect(icon.tagName.toLowerCase()).toBe('svg');
  });

  it('renders with cooldown prop without error', () => {
    const { getByTestId } = render(<FastIcon cooldown />);
    expect(getByTestId('fast-icon')).toBeInTheDocument();
  });
});
