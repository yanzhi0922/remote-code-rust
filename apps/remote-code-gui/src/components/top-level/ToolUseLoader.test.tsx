import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ToolUseLoader } from './ToolUseLoader';

afterEach(() => {
  cleanup();
});

describe('ToolUseLoader', () => {
  it('renders loader element', () => {
    render(<ToolUseLoader isError={false} isUnresolved={true} shouldAnimate={true} />);
    expect(screen.getByTestId('tool-use-loader')).toBeInTheDocument();
  });

  it('shows green color for resolved success', () => {
    render(<ToolUseLoader isError={false} isUnresolved={false} shouldAnimate={false} />);
    const el = screen.getByTestId('tool-use-loader');
    expect(el.classList.contains('text-green-500')).toBe(true);
  });

  it('shows red color for error', () => {
    render(<ToolUseLoader isError={true} isUnresolved={false} shouldAnimate={false} />);
    const el = screen.getByTestId('tool-use-loader');
    expect(el.classList.contains('text-red-500')).toBe(true);
  });

  it('shows gray color for unresolved', () => {
    render(<ToolUseLoader isError={false} isUnresolved={true} shouldAnimate={false} />);
    const el = screen.getByTestId('tool-use-loader');
    expect(el.classList.contains('text-gray-400')).toBe(true);
  });

  it('applies pulse animation when shouldAnimate is true', () => {
    render(<ToolUseLoader isError={false} isUnresolved={true} shouldAnimate={true} />);
    const el = screen.getByTestId('tool-use-loader');
    expect(el.classList.contains('animate-pulse')).toBe(true);
  });

  it('does not animate when shouldAnimate is false', () => {
    render(<ToolUseLoader isError={false} isUnresolved={true} shouldAnimate={false} />);
    const el = screen.getByTestId('tool-use-loader');
    expect(el.classList.contains('animate-pulse')).toBe(false);
  });
});
