import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Button } from './Button';

afterEach(() => {
  cleanup();
});

describe('Button', () => {
  it('renders children text', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByTestId('button')).toHaveTextContent('Click me');
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    fireEvent.click(screen.getByTestId('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('applies primary variant styles by default', () => {
    render(<Button>Primary</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('bg-slate-800');
    expect(btn.className).toContain('text-white');
  });

  it('applies secondary variant styles', () => {
    render(<Button variant="secondary">Secondary</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('border-slate-300');
    expect(btn.className).toContain('text-slate-700');
  });

  it('applies ghost variant styles', () => {
    render(<Button variant="ghost">Ghost</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('text-slate-600');
  });

  it('applies danger variant styles', () => {
    render(<Button variant="danger">Danger</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('bg-red-600');
    expect(btn.className).toContain('text-white');
  });

  it('applies size sm styles', () => {
    render(<Button size="sm">Small</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('h-8');
  });

  it('applies size lg styles', () => {
    render(<Button size="lg">Large</Button>);
    const btn = screen.getByTestId('button');
    expect(btn.className).toContain('h-12');
  });

  it('is disabled when disabled prop is true', () => {
    const onClick = vi.fn();
    render(<Button disabled onClick={onClick}>Disabled</Button>);
    const btn = screen.getByTestId('button');
    expect(btn).toBeDisabled();
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it('shows spinner and disables when loading', () => {
    const onClick = vi.fn();
    render(<Button loading onClick={onClick}>Loading</Button>);
    const btn = screen.getByTestId('button');
    expect(btn).toBeDisabled();
    expect(screen.getByTestId('button-spinner')).toBeInTheDocument();
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it('renders icon on the left', () => {
    render(<Button icon={<span data-testid="custom-icon">🔥</span>}>With Icon</Button>);
    expect(screen.getByTestId('button-icon')).toBeInTheDocument();
    expect(screen.getByTestId('custom-icon')).toBeInTheDocument();
  });

  it('defaults to type="button"', () => {
    render(<Button>Default</Button>);
    expect(screen.getByTestId('button')).toHaveAttribute('type', 'button');
  });

  it('supports type="submit"', () => {
    render(<Button type="submit">Submit</Button>);
    expect(screen.getByTestId('button')).toHaveAttribute('type', 'submit');
  });

  it('merges custom className', () => {
    render(<Button className="my-custom">Custom</Button>);
    expect(screen.getByTestId('button').className).toContain('my-custom');
  });
});
