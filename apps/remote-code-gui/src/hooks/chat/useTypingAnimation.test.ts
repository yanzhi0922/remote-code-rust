import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useTypingAnimation } from './useTypingAnimation';

describe('useTypingAnimation', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns content directly when disabled', () => {
    const { result } = renderHook(() =>
      useTypingAnimation({ content: 'Hello world', enabled: false }),
    );

    expect(result.current.displayedContent).toBe('Hello world');
    expect(result.current.isAnimating).toBe(false);
  });

  it('returns content directly on first render', () => {
    const { result } = renderHook(() =>
      useTypingAnimation({ content: 'Initial', enabled: true }),
    );

    expect(result.current.displayedContent).toBe('Initial');
    expect(result.current.isAnimating).toBe(false);
  });

  it('shows full content immediately when content shrinks', () => {
    const { result, rerender } = renderHook(
      ({ content }) => useTypingAnimation({ content, enabled: true }),
      { initialProps: { content: 'Hello world' } },
    );

    rerender({ content: 'Hi' });

    expect(result.current.displayedContent).toBe('Hi');
    expect(result.current.isAnimating).toBe(false);
  });

  it('shows full content immediately when content grows by more than 1000 chars', () => {
    const { result, rerender } = renderHook(
      ({ content }) => useTypingAnimation({ content, enabled: true }),
      { initialProps: { content: 'Short' } },
    );

    const longContent = 'A'.repeat(1500);
    rerender({ content: longContent });

    expect(result.current.displayedContent).toBe(longContent);
    expect(result.current.isAnimating).toBe(false);
  });

  it('starts animation for small incremental updates', () => {
    vi.useFakeTimers();

    const { result, rerender } = renderHook(
      ({ content }) => useTypingAnimation({ content, enabled: true, speed: 1000 }),
      { initialProps: { content: 'Hello' } },
    );

    rerender({ content: 'Hello world' });

    // Animation should be started
    expect(result.current.isAnimating).toBe(true);

    vi.useRealTimers();
  });

  it('cleans up animation frame on unmount', () => {
    vi.useFakeTimers();

    const { unmount, rerender } = renderHook(
      ({ content }) => useTypingAnimation({ content, enabled: true, speed: 1000 }),
      { initialProps: { content: 'Hello' } },
    );

    rerender({ content: 'Hello world' });
    unmount();

    // Should not throw
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    vi.useRealTimers();
  });
});
