/**
 * 防抖 Hook — 延迟执行函数，在连续调用中只执行最后一次
 * Debounce Hook — delays execution, only fires the last call in a series
 *
 * Adapted from AionUi useDebounce pattern.
 */

import { useCallback, useEffect, useRef } from 'react';

/**
 * @param callback 需要防抖的函数
 * @param delay 防抖延迟时间（毫秒）
 * @param deps 依赖数组，变化时重新创建防抖函数
 * @returns 防抖后的函数
 */
export function useDebounce<T extends (...args: unknown[]) => unknown>(
  callback: T,
  delay: number,
  deps: React.DependencyList,
): T {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimer = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      clearTimer();
    };
  }, [clearTimer]);

  const debouncedFunction = useCallback(
    (...args: Parameters<T>) => {
      clearTimer();
      timeoutRef.current = setTimeout(() => {
        callback(...args);
      }, delay);
    },
    [delay, clearTimer, ...deps], // eslint-disable-line react-hooks/exhaustive-deps — clearTimer is stable via useRef
  );

  return debouncedFunction as T;
}
