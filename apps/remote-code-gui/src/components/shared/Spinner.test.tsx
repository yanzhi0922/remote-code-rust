import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Spinner } from './Spinner';

afterEach(() => { cleanup(); });

describe('Spinner', () => {
  it('renders with default size', () => {
    render(<Spinner />);
    expect(screen.getByTestId('spinner')).toBeInTheDocument();
  });

  it('applies small size class', () => {
    render(<Spinner size="sm" />);
    const el = screen.getByTestId('spinner');
    expect(el.classList.contains('h-4')).toBe(true);
  });

  it('applies large size class', () => {
    render(<Spinner size="lg" />);
    const el = screen.getByTestId('spinner');
    expect(el.classList.contains('h-8')).toBe(true);
  });

  it('applies animate-spin class', () => {
    render(<Spinner />);
    const el = screen.getByTestId('spinner');
    expect(el.classList.contains('animate-spin')).toBe(true);
  });
});
