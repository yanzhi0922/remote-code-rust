import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentNavigationFooter } from './AgentNavigationFooter';

afterEach(() => {
  cleanup();
});

describe('AgentNavigationFooter', () => {
  it('renders navigation', () => {
    render(<AgentNavigationFooter currentIndex={0} totalCount={5} />);
    expect(screen.getByTestId('agent-nav-footer')).toBeInTheDocument();
    expect(screen.getByTestId('agent-nav-position')).toHaveTextContent('1 / 5');
  });

  it('disables prev at start', () => {
    render(<AgentNavigationFooter currentIndex={0} totalCount={5} />);
    expect(screen.getByTestId('agent-nav-prev')).toBeDisabled();
  });

  it('disables next at end', () => {
    render(<AgentNavigationFooter currentIndex={4} totalCount={5} />);
    expect(screen.getByTestId('agent-nav-next')).toBeDisabled();
  });

  it('calls onPrev', () => {
    const onPrev = vi.fn();
    render(<AgentNavigationFooter currentIndex={2} totalCount={5} onPrev={onPrev} />);
    fireEvent.click(screen.getByTestId('agent-nav-prev'));
    expect(onPrev).toHaveBeenCalled();
  });

  it('calls onNext', () => {
    const onNext = vi.fn();
    render(<AgentNavigationFooter currentIndex={2} totalCount={5} onNext={onNext} />);
    fireEvent.click(screen.getByTestId('agent-nav-next'));
    expect(onNext).toHaveBeenCalled();
  });

  it('calls onAdd', () => {
    const onAdd = vi.fn();
    render(<AgentNavigationFooter currentIndex={0} totalCount={5} onAdd={onAdd} />);
    fireEvent.click(screen.getByTestId('agent-nav-add'));
    expect(onAdd).toHaveBeenCalled();
  });
});
