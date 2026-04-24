import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Divider } from './Divider';

afterEach(() => {
  cleanup();
});

describe('Divider', () => {
  it('renders horizontal divider by default', () => {
    render(<Divider />);
    const divider = screen.getByTestId('divider');
    expect(divider).toBeInTheDocument();
    expect(divider.getAttribute('class')).toContain('border-t');
  });

  it('renders vertical divider', () => {
    render(<Divider orientation="vertical" />);
    const divider = screen.getByTestId('divider');
    expect(divider).toBeInTheDocument();
  });

  it('renders label when provided', () => {
    render(<Divider label="OR" />);
    expect(screen.getByTestId('divider-label')).toHaveTextContent('OR');
  });

  it('does not render label element when label is not provided', () => {
    render(<Divider />);
    expect(screen.queryByTestId('divider-label')).not.toBeInTheDocument();
  });

  it('has separator role', () => {
    render(<Divider />);
    expect(screen.getByTestId('divider')).toHaveAttribute('role', 'separator');
  });

  it('has aria-orientation horizontal by default', () => {
    render(<Divider />);
    expect(screen.getByTestId('divider')).toHaveAttribute('aria-orientation', 'horizontal');
  });

  it('has aria-orientation vertical for vertical divider', () => {
    render(<Divider orientation="vertical" />);
    expect(screen.getByTestId('divider')).toHaveAttribute('aria-orientation', 'vertical');
  });

  it('uses border-slate-200 color', () => {
    render(<Divider />);
    expect(screen.getByTestId('divider').getAttribute('class')).toContain('border-slate-200');
  });

  it('merges custom className', () => {
    render(<Divider className="my-divider" />);
    expect(screen.getByTestId('divider').className).toContain('my-divider');
  });
});
