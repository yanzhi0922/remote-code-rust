/**
 * Textarea — 文本域组件。
 *
 * 支持自动调整高度、最大行数限制和禁用状态。
 */

import { useCallback, useLayoutEffect, useRef } from 'react';
import { cn } from '@/lib/utils';

export interface TextareaProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  rows?: number;
  maxRows?: number;
  autoResize?: boolean;
  disabled?: boolean;
  className?: string;
}

const LINE_HEIGHT = 24; // approximate px per line

export function Textarea({
  value,
  onChange,
  placeholder,
  rows = 3,
  maxRows,
  autoResize = false,
  disabled = false,
  className,
}: TextareaProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el || !autoResize) return;

    // Reset height to allow shrinking
    el.style.height = 'auto';
    const newHeight = el.scrollHeight;

    if (maxRows) {
      const maxHeight = maxRows * LINE_HEIGHT;
      el.style.height = `${Math.min(newHeight, maxHeight)}px`;
    } else {
      el.style.height = `${newHeight}px`;
    }
  }, [autoResize, maxRows]);

  useLayoutEffect(() => {
    adjustHeight();
  }, [value, adjustHeight]);

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
  };

  return (
    <textarea
      ref={textareaRef}
      value={value}
      onChange={handleChange}
      placeholder={placeholder}
      rows={rows}
      disabled={disabled}
      className={cn(
        'w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900',
        'placeholder:text-slate-400',
        'focus:outline-none focus:ring-2 focus:ring-slate-400 focus:ring-offset-1',
        'resize-none transition-colors',
        disabled && 'cursor-not-allowed bg-slate-50 text-slate-400',
        className,
      )}
      data-testid="textarea"
    />
  );
}
