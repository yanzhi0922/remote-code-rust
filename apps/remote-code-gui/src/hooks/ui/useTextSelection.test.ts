import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, cleanup, act } from '@testing-library/react';
import { useTextSelection } from './useTextSelection';

describe('useTextSelection', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns initial empty state', () => {
    const { result } = renderHook(() => {
      const ref = { current: document.createElement('div') };
      return useTextSelection(ref);
    });

    expect(result.current.selectedText).toBe('');
    expect(result.current.selectionPosition).toBeNull();
  });

  it('clearSelection resets state', () => {
    const { result } = renderHook(() => {
      const ref = { current: document.createElement('div') };
      return useTextSelection(ref);
    });

    act(() => {
      result.current.clearSelection();
    });

    expect(result.current.selectedText).toBe('');
    expect(result.current.selectionPosition).toBeNull();
  });

  it('does not crash when containerRef is null', () => {
    const { result } = renderHook(() => {
      const ref = { current: null };
      return useTextSelection(ref);
    });

    expect(result.current.selectedText).toBe('');
  });

  it('does not crash when enabled is false', () => {
    const { result } = renderHook(() => {
      const ref = { current: document.createElement('div') };
      return useTextSelection(ref, false);
    });

    expect(result.current.selectedText).toBe('');
  });

  it('clearSelection calls window.getSelection().removeAllRanges()', () => {
    const mockRemoveAllRanges = vi.fn();
    vi.spyOn(window, 'getSelection').mockReturnValue({
      removeAllRanges: mockRemoveAllRanges,
    } as unknown as Selection);

    const { result } = renderHook(() => {
      const ref = { current: document.createElement('div') };
      return useTextSelection(ref);
    });

    act(() => {
      result.current.clearSelection();
    });

    expect(mockRemoveAllRanges).toHaveBeenCalled();

    vi.restoreAllMocks();
  });
});
