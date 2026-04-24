/**
 * Tooltip — 工具提示组件。
 *
 * 鼠标悬停时在触发元素周围显示深色背景提示文字。
 */

import {
  type ReactElement,
  cloneElement,
  useCallback,
  useRef,
  useState,
} from 'react';
import { cn } from '@/lib/utils';

export interface TooltipProps {
  content: string;
  children: ReactElement;
  position?: 'top' | 'bottom' | 'left' | 'right';
  className?: string;
}

const positionStyles: Record<NonNullable<TooltipProps['position']>, string> = {
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
  left: 'right-full top-1/2 -translate-y-1/2 mr-2',
  right: 'left-full top-1/2 -translate-y-1/2 ml-2',
};

const arrowStyles: Record<NonNullable<TooltipProps['position']>, string> = {
  top: 'top-full left-1/2 -translate-x-1/2 border-t-slate-800 border-x-transparent border-b-transparent border-4',
  bottom:
    'bottom-full left-1/2 -translate-x-1/2 border-b-slate-800 border-x-transparent border-t-transparent border-4',
  left: 'left-full top-1/2 -translate-y-1/2 border-l-slate-800 border-y-transparent border-r-transparent border-4',
  right:
    'right-full top-1/2 -translate-y-1/2 border-r-slate-800 border-y-transparent border-l-transparent border-4',
};

export function Tooltip({
  content,
  children,
  position = 'top',
  className,
}: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const show = useCallback(() => {
    clearTimeout(timeoutRef.current);
    setVisible(true);
  }, []);

  const hide = useCallback(() => {
    timeoutRef.current = setTimeout(() => setVisible(false), 100);
  }, []);

  return (
    <div
      className="relative inline-flex"
      onMouseEnter={show}
      onMouseLeave={hide}
      data-testid="tooltip-wrapper"
    >
      {cloneElement(children, {
        'aria-describedby': visible ? 'tooltip-content' : undefined,
      } as Record<string, unknown>)}
      {visible && (
        <div
          id="tooltip-content"
          role="tooltip"
          className={cn(
            'absolute z-50 whitespace-nowrap rounded-lg bg-slate-800 px-3 py-1.5 text-xs text-white shadow-lg',
            positionStyles[position],
            className,
          )}
          data-testid="tooltip-content"
        >
          {content}
          <span
            className={cn('absolute h-0 w-0', arrowStyles[position])}
            data-testid="tooltip-arrow"
          />
        </div>
      )}
    </div>
  );
}
