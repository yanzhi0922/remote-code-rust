/**
 * 节流 Hook — 限制函数在指定时间间隔内只执行一次
 * Throttle Hook — limits function execution to once per interval
 *
 * Adapted from AionUi useThrottle pattern.
 */

import { useCallback, useEffect, useRef } from 'react';

/**
 * @param callback 需要节流的函数
 * @param delay 节流时间间隔（毫秒）
 * @param deps 依赖数组
 * @returns 节流后的函数
 */
export function useThrottle<T extends (...args: unknown[]) => unknown>(
  callback: T,
  delay: number,
  deps: React.DependencyList,
): T {
  const lastExecTime = useRef<number>(0);
  const timeoutId = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Cleanup pending timer on unmount to prevent stale callbacks.
  useEffect(() => {
    return () => {
      if (timeoutId.current) {
        clearTimeout(timeoutId.current);
        timeoutId.current = null;
      }
    };
  }, []);

  const throttledFunction = useCallback(
    (...args: Parameters<T>) => {
      const now = Date.now();
      const timeSinceLastExec = now - lastExecTime.current;

      if (timeSinceLastExec >= delay) {
        callback(...args);
        lastExecTime.current = now;
      } else {
        if (timeoutId.current) {
          clearTimeout(timeoutId.current);
        }
        timeoutId.current = setTimeout(() => {
          callback(...args);
          lastExecTime.current = Date.now();
          timeoutId.current = null;
        }, delay - timeSinceLastExec);
      }
    },
    [delay, ...deps], // eslint-disable-line react-hooks/exhaustive-deps — deps intentionally spread
  );

  return throttledFunction as T;
}
