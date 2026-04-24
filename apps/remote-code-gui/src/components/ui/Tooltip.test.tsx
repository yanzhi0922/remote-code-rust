import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Tooltip } from './Tooltip';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('Tooltip', () => {
  it('renders children element', () => {
    render(
      <Tooltip content="Hello">
        <button>Hover me</button>
      </Tooltip>,
    );
    expect(screen.getByText('Hover me')).toBeInTheDocument();
  });

  it('shows tooltip content on mouse enter', () => {
    render(
      <Tooltip content="Tip text">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    expect(screen.getByTestId('tooltip-content')).toHaveTextContent('Tip text');
  });

  it('hides tooltip on mouse leave', async () => {
    vi.useFakeTimers();
    render(
      <Tooltip content="Tip text">
        <button>Target</button>
      </Tooltip>,
    );
    const wrapper = screen.getByTestId('tooltip-wrapper');
    fireEvent.mouseEnter(wrapper);
    expect(screen.getByTestId('tooltip-content')).toBeInTheDocument();
    fireEvent.mouseLeave(wrapper);
    // The hide is debounced (100ms), advance past the timeout
    await act(async () => {
      vi.advanceTimersByTime(150);
    });
    expect(screen.queryByTestId('tooltip-content')).not.toBeInTheDocument();
  });

  it('applies top position by default', () => {
    render(
      <Tooltip content="Top tip">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    const content = screen.getByTestId('tooltip-content');
    expect(content.className).toContain('bottom-full');
  });

  it('applies bottom position', () => {
    render(
      <Tooltip content="Bottom tip" position="bottom">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    const content = screen.getByTestId('tooltip-content');
    expect(content.className).toContain('top-full');
  });

  it('applies left position', () => {
    render(
      <Tooltip content="Left tip" position="left">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    const content = screen.getByTestId('tooltip-content');
    expect(content.className).toContain('right-full');
  });

  it('applies right position', () => {
    render(
      <Tooltip content="Right tip" position="right">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    const content = screen.getByTestId('tooltip-content');
    expect(content.className).toContain('left-full');
  });

  it('renders arrow element', () => {
    render(
      <Tooltip content="With arrow">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    expect(screen.getByTestId('tooltip-arrow')).toBeInTheDocument();
  });

  it('has dark background and white text', () => {
    render(
      <Tooltip content="Dark tooltip">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    const content = screen.getByTestId('tooltip-content');
    expect(content.className).toContain('bg-slate-800');
    expect(content.className).toContain('text-white');
  });

  it('merges custom className', () => {
    render(
      <Tooltip content="Custom" className="my-tooltip">
        <button>Target</button>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByTestId('tooltip-wrapper'));
    expect(screen.getByTestId('tooltip-content').className).toContain('my-tooltip');
  });
});
