/**
 * Spinner — 加载动画组件。
 *
 * 使用 CSS 动画的旋转圆圈，支持三种尺寸和自定义颜色。
 */

import { Loader2 } from 'lucide-react';

export interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  color?: string;
}

const sizeMap: Record<NonNullable<SpinnerProps['size']>, string> = {
  sm: 'h-4 w-4',
  md: 'h-6 w-6',
  lg: 'h-8 w-8',
};

export function Spinner({ size = 'md', color }: SpinnerProps) {
  return (
    <Loader2
      className={`animate-spin ${sizeMap[size]}`}
      style={color ? { color } : undefined}
      data-testid="spinner"
    />
  );
}
