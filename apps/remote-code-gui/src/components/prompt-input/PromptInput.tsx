import { useCallback, useRef, useEffect, useState } from 'react';
import { cn } from '../../lib/utils';
import { getModeFromInput } from './inputModes';
import { hasImageInClipboard, extractImageFiles } from './inputPaste';

/** PromptInput 组件属性 */
export interface PromptInputProps {
  /** 当前输入值 */
  value: string;
  /** 输入变化回调 */
  onChange: (value: string) => void;
  /** 提交回调 */
  onSubmit: (value: string) => void;
  /** 是否禁用 */
  disabled?: boolean;
  /** 占位文本 */
  placeholder?: string;
  /** 额外 CSS 类名 */
  className?: string;
}

/** 最大自动扩展行数 */
const MAX_ROWS = 10;

/** 单行高度（像素） */
const LINE_HEIGHT = 24;

/**
 * 高级输入框组件。
 * 支持多行输入、Shift+Enter 换行、Enter 提交、bash 模式、粘贴图片预览。
 */
export function PromptInput({
  value,
  onChange,
  onSubmit,
  disabled = false,
  placeholder = '输入需求，Shift+Enter 换行...',
  className,
}: PromptInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [pastedImages, setPastedImages] = useState<string[]>([]);

  const mode = getModeFromInput(value);

  /** 自动调整 textarea 高度 */
  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = 'auto';
    const maxHeight = LINE_HEIGHT * MAX_ROWS;
    textarea.style.height = `${Math.min(textarea.scrollHeight, maxHeight)}px`;
  }, []);

  useEffect(() => {
    adjustHeight();
  }, [value, adjustHeight]);

  /** 键盘事件处理 */
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (value.trim() && !disabled) {
          onSubmit(value);
        }
      }
    },
    [value, disabled, onSubmit],
  );

  /** 粘贴事件处理 */
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (hasImageInClipboard(e.nativeEvent)) {
        e.preventDefault();
        const files = extractImageFiles(e.clipboardData.items);
        const names = files.map((f) => f.name || 'image');
        setPastedImages((prev) => [...prev, ...names]);
      }
    },
    [],
  );

  /** 移除粘贴图片标签 */
  const removePastedImage = useCallback((index: number) => {
    setPastedImages((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const isBashMode = mode === 'bash';

  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border border-slate-200 bg-white dark:border-slate-700 dark:bg-slate-900',
        isBashMode && 'border-red-300 dark:border-red-700',
        disabled && 'opacity-50 cursor-not-allowed',
        className,
      )}
      data-testid="prompt-input"
    >
      {/* 粘贴图片预览标签 */}
      {pastedImages.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-3 pt-2">
          {pastedImages.map((name, idx) => (
            <span
              key={idx}
              className="inline-flex items-center gap-1 rounded-md bg-blue-50 px-2 py-0.5 text-xs text-blue-700 dark:bg-blue-900/30 dark:text-blue-300"
            >
              📷 {name}
              <button
                type="button"
                onClick={() => removePastedImage(idx)}
                className="ml-0.5 text-blue-400 hover:text-blue-600"
              >
                ×
              </button>
            </span>
          ))}
        </div>
      )}

      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
        disabled={disabled}
        placeholder={placeholder}
        rows={1}
        className={cn(
          'w-full resize-none border-0 bg-transparent px-3 py-2.5 text-sm text-slate-900 placeholder:text-slate-400 focus:outline-none dark:text-slate-100 dark:placeholder:text-slate-500',
          isBashMode && 'font-mono text-red-600 dark:text-red-400',
        )}
        style={{ maxHeight: `${LINE_HEIGHT * MAX_ROWS}px` }}
      />
    </div>
  );
}
