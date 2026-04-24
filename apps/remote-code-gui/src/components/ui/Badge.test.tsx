import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Badge } from './Badge';

afterEach(() => {
  cleanup();
});

describe('Badge', () => {
  it('renders children text', () => {
    render(<Badge>Active</Badge>);
    expect(screen.getByTestId('badge')).toHaveTextContent('Active');
  });

  it('applies default variant styles', () => {
    render(<Badge>Default</Badge>);
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-slate-100');
    expect(badge.className).toContain('text-slate-700');
  });

  it('applies success variant styles', () => {
    render(<Badge variant="success">Success</Badge>);
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-emerald-50');
    expect(badge.className).toContain('text-emerald-700');
  });

  it('applies warning variant styles', () => {
    render(<Badge variant="warning">Warning</Badge>);
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-amber-50');
    expect(badge.className).toContain('text-amber-700');
  });

  it('applies error variant styles', () => {
    render(<Badge variant="error">Error</Badge>);
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-red-50');
    expect(badge.className).toContain('text-red-700');
  });

  it('applies info variant styles', () => {
    render(<Badge variant="info">Info</Badge>);
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-blue-50');
    expect(badge.className).toContain('text-blue-700');
  });

  it('renders dot indicator when dot prop is true', () => {
    render(<Badge dot>With Dot</Badge>);
    expect(screen.getByTestId('badge-dot')).toBeInTheDocument();
  });

  it('does not render dot by default', () => {
    render(<Badge>No Dot</Badge>);
    expect(screen.queryByTestId('badge-dot')).not.toBeInTheDocument();
  });

  it('applies sm size styles', () => {
    render(<Badge size="sm">Small</Badge>);
    expect(screen.getByTestId('badge').className).toContain('text-xs');
  });

  it('applies md size styles by default', () => {
    render(<Badge>Medium</Badge>);
    expect(screen.getByTestId('badge').className).toContain('text-sm');
  });

  it('has rounded-full class for pill shape', () => {
    render(<Badge>Pill</Badge>);
    expect(screen.getByTestId('badge').className).toContain('rounded-full');
  });

  it('merges custom className', () => {
    render(<Badge className="my-badge">Custom</Badge>);
    expect(screen.getByTestId('badge').className).toContain('my-badge');
  });
});
