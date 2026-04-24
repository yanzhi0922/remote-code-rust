import { useMemo } from 'react';

/**
 * 输入截断 Hook。
 * 提供截断超长输入的工具方法。
 */
export function useMaybeTruncateInput(maxLength: number): {
  truncate: (input: string) => string;
  isTruncated: (input: string) => boolean;
  truncatedLength: number;
} {
  return useMemo(
    () => ({
      truncate: (input: string): string => {
        if (input.length <= maxLength) return input;
        return input.slice(0, maxLength);
      },
      isTruncated: (input: string): boolean => {
        return input.length > maxLength;
      },
      truncatedLength: maxLength,
    }),
    [maxLength],
  );
}
