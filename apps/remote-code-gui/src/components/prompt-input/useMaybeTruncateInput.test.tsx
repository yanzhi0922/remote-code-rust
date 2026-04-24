import { describe, expect, it } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useMaybeTruncateInput } from './useMaybeTruncateInput';

describe('useMaybeTruncateInput', () => {
  it('truncate 截断超长输入', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(10));
    expect(result.current.truncate('1234567890abc')).toBe('1234567890');
  });

  it('truncate 不截断短输入', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(10));
    expect(result.current.truncate('hello')).toBe('hello');
  });

  it('truncate 恰好等于最大长度不截断', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(5));
    expect(result.current.truncate('12345')).toBe('12345');
  });

  it('isTruncated 对超长输入返回 true', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(5));
    expect(result.current.isTruncated('123456')).toBe(true);
  });

  it('isTruncated 对短输入返回 false', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(10));
    expect(result.current.isTruncated('hello')).toBe(false);
  });

  it('truncatedLength 返回传入的 maxLength', () => {
    const { result } = renderHook(() => useMaybeTruncateInput(42));
    expect(result.current.truncatedLength).toBe(42);
  });
});
