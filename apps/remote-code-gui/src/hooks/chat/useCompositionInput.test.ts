import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useCompositionInput } from './useCompositionInput';

describe('useCompositionInput', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns initial non-composing state', () => {
    const { result } = renderHook(() => useCompositionInput());

    expect(result.current.isComposing.current).toBe(false);
    expect(result.current.isComposingState).toBe(false);
  });

  it('provides composition handlers', () => {
    const { result } = renderHook(() => useCompositionInput());

    expect(result.current.compositionHandlers.onCompositionStartCapture).toBeInstanceOf(Function);
    expect(result.current.compositionHandlers.onCompositionEndCapture).toBeInstanceOf(Function);
  });

  it('sets composing state on composition start', () => {
    const { result } = renderHook(() => useCompositionInput());

    act(() => {
      result.current.compositionHandlers.onCompositionStartCapture();
    });

    expect(result.current.isComposing.current).toBe(true);
    expect(result.current.isComposingState).toBe(true);
  });

  it('clears composing state on composition end', () => {
    const { result } = renderHook(() => useCompositionInput());

    result.current.compositionHandlers.onCompositionStartCapture();
    expect(result.current.isComposing.current).toBe(true);

    result.current.compositionHandlers.onCompositionEndCapture();
    expect(result.current.isComposing.current).toBe(false);
    expect(result.current.isComposingState).toBe(false);
  });

  it('createKeyDownHandler triggers onEnterPress for Enter', () => {
    const onEnterPress = vi.fn();
    const { result } = renderHook(() => useCompositionInput());

    const handler = result.current.createKeyDownHandler(onEnterPress);

    const event = { key: 'Enter', shiftKey: false, preventDefault: vi.fn() };
    handler(event as unknown as React.KeyboardEvent);

    expect(onEnterPress).toHaveBeenCalled();
    expect(event.preventDefault).toHaveBeenCalled();
  });

  it('does not trigger onEnterPress for Shift+Enter', () => {
    const onEnterPress = vi.fn();
    const { result } = renderHook(() => useCompositionInput());

    const handler = result.current.createKeyDownHandler(onEnterPress);

    handler({ key: 'Enter', shiftKey: true, preventDefault: vi.fn() } as unknown as React.KeyboardEvent);

    expect(onEnterPress).not.toHaveBeenCalled();
  });

  it('does not trigger onEnterPress during composition', () => {
    const onEnterPress = vi.fn();
    const { result } = renderHook(() => useCompositionInput());

    result.current.compositionHandlers.onCompositionStartCapture();

    const handler = result.current.createKeyDownHandler(onEnterPress);

    handler({ key: 'Enter', shiftKey: false, preventDefault: vi.fn() } as unknown as React.KeyboardEvent);

    expect(onEnterPress).not.toHaveBeenCalled();
  });

  it('calls onKeyDownIntercept when provided', () => {
    const onEnterPress = vi.fn();
    const intercept = vi.fn(() => true);
    const { result } = renderHook(() => useCompositionInput());

    const handler = result.current.createKeyDownHandler(onEnterPress, intercept);

    handler({ key: 'Enter', shiftKey: false, preventDefault: vi.fn() } as unknown as React.KeyboardEvent);

    expect(intercept).toHaveBeenCalled();
    expect(onEnterPress).not.toHaveBeenCalled();
  });

  it('does not call onEnterPress for non-Enter keys', () => {
    const onEnterPress = vi.fn();
    const { result } = renderHook(() => useCompositionInput());

    const handler = result.current.createKeyDownHandler(onEnterPress);

    handler({ key: 'a', shiftKey: false, preventDefault: vi.fn() } as unknown as React.KeyboardEvent);

    expect(onEnterPress).not.toHaveBeenCalled();
  });
});
