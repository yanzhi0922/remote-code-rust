/**
 * Modal — 通用模态对话框。
 *
 * 遮罩层 + 居中内容，支持 ESC 关闭、点击遮罩关闭和尺寸控制。
 */

import { type ReactNode, useCallback, useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ModalProps {
  visible: boolean;
  onClose: () => void;
  title?: string;
  children: ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  className?: string;
}

const sizeStyles: Record<NonNullable<ModalProps['size']>, string> = {
  sm: 'max-w-sm',
  md: 'max-w-md',
  lg: 'max-w-lg',
  xl: 'max-w-xl',
};

export function Modal({
  visible,
  onClose,
  title,
  children,
  size = 'md',
  className,
}: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    },
    [onClose],
  );

  useEffect(() => {
    if (!visible) return;
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [visible, handleKeyDown]);

  // Prevent body scroll when modal is open
  useEffect(() => {
    if (visible) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [visible]);

  if (!visible) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      data-testid="modal-overlay"
    >
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/50"
        onClick={onClose}
        data-testid="modal-backdrop"
      />

      {/* Dialog */}
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={cn(
          'relative z-10 w-full rounded-2xl bg-white shadow-xl',
          sizeStyles[size],
          className,
        )}
        data-testid="modal-dialog"
      >
        {/* Header */}
        {title && (
          <div
            className="flex items-center justify-between border-b border-slate-200 px-6 py-4"
            data-testid="modal-header"
          >
            <h2 className="text-lg font-semibold text-slate-900">{title}</h2>
            <button
              onClick={onClose}
              className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              aria-label="Close"
              data-testid="modal-close"
            >
              <X className="h-5 w-5" />
            </button>
          </div>
        )}

        {/* Close button when no title */}
        {!title && (
          <button
            onClick={onClose}
            className="absolute right-4 top-4 rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            aria-label="Close"
            data-testid="modal-close"
          >
            <X className="h-5 w-5" />
          </button>
        )}

        {/* Body */}
        <div className="px-6 py-4" data-testid="modal-body">
          {children}
        </div>
      </div>
    </div>
  );
}
