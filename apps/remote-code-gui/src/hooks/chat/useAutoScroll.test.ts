import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, cleanup } from '@testing-library/react';
import { useAutoScroll } from './useAutoScroll';

describe('useAutoScroll', () => {
  afterEach(() => {
    cleanup();
  });

  it('does not scroll when disabled', () => {
    const scrollTo = vi.fn();
    const container = document.createElement('div');
    container.scrollTo = scrollTo;
    const ref = { current: container };

    renderHook(() =>
      useAutoScroll({ containerRef: ref, content: 'hello', enabled: false }),
    );

    expect(scrollTo).not.toHaveBeenCalled();
  });

  it('does not crash when containerRef is null', () => {
    const ref = { current: null };

    expect(() => {
      renderHook(() =>
        useAutoScroll({ containerRef: ref, content: 'hello' }),
      );
    }).not.toThrow();
  });

  it('scrolls to bottom when near bottom', () => {
    const scrollTo = vi.fn();
    const container = document.createElement('div');
    container.scrollTo = scrollTo;

    // Simulate near-bottom: scrollHeight - scrollTop - clientHeight < threshold
    Object.defineProperty(container, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(container, 'scrollTop', { value: 790, configurable: true });
    Object.defineProperty(container, 'clientHeight', { value: 200, configurable: true });

    const ref = { current: container };

    renderHook(() =>
      useAutoScroll({ containerRef: ref, content: 'hello', threshold: 200 }),
    );

    expect(scrollTo).toHaveBeenCalledWith({ top: 1000, behavior: 'smooth' });
  });

  it('does not scroll when far from bottom', () => {
    const scrollTo = vi.fn();
    const container = document.createElement('div');
    container.scrollTo = scrollTo;

    // Far from bottom: scrollHeight - scrollTop - clientHeight >= threshold
    Object.defineProperty(container, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(container, 'scrollTop', { value: 100, configurable: true });
    Object.defineProperty(container, 'clientHeight', { value: 200, configurable: true });

    const ref = { current: container };

    renderHook(() =>
      useAutoScroll({ containerRef: ref, content: 'hello', threshold: 200 }),
    );

    expect(scrollTo).not.toHaveBeenCalled();
  });

  it('uses custom behavior', () => {
    const scrollTo = vi.fn();
    const container = document.createElement('div');
    container.scrollTo = scrollTo;

    Object.defineProperty(container, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(container, 'scrollTop', { value: 790, configurable: true });
    Object.defineProperty(container, 'clientHeight', { value: 200, configurable: true });

    const ref = { current: container };

    renderHook(() =>
      useAutoScroll({ containerRef: ref, content: 'hello', behavior: 'auto' }),
    );

    expect(scrollTo).toHaveBeenCalledWith({ top: 1000, behavior: 'auto' });
  });

  it('re-triggers on content change', () => {
    const scrollTo = vi.fn();
    const container = document.createElement('div');
    container.scrollTo = scrollTo;

    Object.defineProperty(container, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(container, 'scrollTop', { value: 790, configurable: true });
    Object.defineProperty(container, 'clientHeight', { value: 200, configurable: true });

    const ref = { current: container };

    const { rerender } = renderHook(
      ({ content }) => useAutoScroll({ containerRef: ref, content }),
      { initialProps: { content: 'first' } },
    );

    expect(scrollTo).toHaveBeenCalledTimes(1);

    rerender({ content: 'second' });

    expect(scrollTo).toHaveBeenCalledTimes(2);
  });
});
