import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Card } from './Card';

afterEach(() => {
  cleanup();
});

describe('Card', () => {
  it('renders children', () => {
    render(<Card>Card content</Card>);
    expect(screen.getByTestId('card')).toHaveTextContent('Card content');
  });

  it('applies default md padding', () => {
    render(<Card>Content</Card>);
    expect(screen.getByTestId('card').className).toContain('p-4');
  });

  it('applies sm padding', () => {
    render(<Card padding="sm">Content</Card>);
    expect(screen.getByTestId('card').className).toContain('p-3');
  });

  it('applies lg padding', () => {
    render(<Card padding="lg">Content</Card>);
    expect(screen.getByTestId('card').className).toContain('p-6');
  });

  it('applies no padding when padding is none', () => {
    render(<Card padding="none">Content</Card>);
    const card = screen.getByTestId('card');
    expect(card.className).not.toContain('p-3');
    expect(card.className).not.toContain('p-4');
    expect(card.className).not.toContain('p-6');
  });

  it('applies hover styles when hover is true', () => {
    render(<Card hover>Content</Card>);
    expect(screen.getByTestId('card').className).toContain('hover:shadow-md');
  });

  it('does not apply hover styles by default', () => {
    render(<Card>Content</Card>);
    expect(screen.getByTestId('card').className).not.toContain('hover:shadow-md');
  });

  it('applies selected ring when selected is true', () => {
    render(<Card selected>Content</Card>);
    expect(screen.getByTestId('card').className).toContain('ring-2');
    expect(screen.getByTestId('card').className).toContain('ring-slate-800');
  });

  it('calls onClick when clicked and onClick is provided', () => {
    const onClick = vi.fn();
    render(<Card onClick={onClick}>Clickable</Card>);
    fireEvent.click(screen.getByTestId('card'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('renders as button when onClick is provided', () => {
    render(<Card onClick={vi.fn()}>Clickable</Card>);
    expect(screen.getByTestId('card').tagName.toLowerCase()).toBe('button');
  });

  it('renders as div when onClick is not provided', () => {
    render(<Card>Static</Card>);
    expect(screen.getByTestId('card').tagName.toLowerCase()).toBe('div');
  });

  it('merges custom className', () => {
    render(<Card className="my-card">Custom</Card>);
    expect(screen.getByTestId('card').className).toContain('my-card');
  });
});
