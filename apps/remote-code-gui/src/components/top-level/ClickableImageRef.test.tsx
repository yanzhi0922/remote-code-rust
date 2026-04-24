import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ClickableImageRef } from './ClickableImageRef';

afterEach(() => {
  cleanup();
});

describe('ClickableImageRef', () => {
  it('renders image reference button', () => {
    render(<ClickableImageRef imageId={1} />);
    expect(screen.getByTestId('clickable-image-ref-1')).toBeInTheDocument();
  });

  it('shows correct display text', () => {
    render(<ClickableImageRef imageId={3} />);
    expect(screen.getByText('[Image #3]')).toBeInTheDocument();
  });

  it('applies selected styling', () => {
    render(<ClickableImageRef imageId={1} isSelected={true} />);
    const el = screen.getByTestId('clickable-image-ref-1');
    expect(el.classList.contains('bg-blue-500')).toBe(true);
  });

  it('applies default styling when not selected', () => {
    render(<ClickableImageRef imageId={1} isSelected={false} />);
    const el = screen.getByTestId('clickable-image-ref-1');
    expect(el.classList.contains('bg-gray-100')).toBe(true);
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<ClickableImageRef imageId={1} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('clickable-image-ref-1'));
    expect(onClick).toHaveBeenCalled();
  });

  it('renders different image IDs correctly', () => {
    render(
      <>
        <ClickableImageRef imageId={1} />
        <ClickableImageRef imageId={5} />
      </>,
    );
    expect(screen.getByText('[Image #1]')).toBeInTheDocument();
    expect(screen.getByText('[Image #5]')).toBeInTheDocument();
  });
});
