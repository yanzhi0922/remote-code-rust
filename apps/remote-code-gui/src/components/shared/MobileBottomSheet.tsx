/**
 * MobileBottomSheet — 移动端底部抽屉组件。
 *
 * 基于 @radix-ui/react-dialog 实现，在移动端（< lg）以底部抽屉形式展示内容，
 * 桌面端直接渲染 children 不做包裹。
 *
 * 用法：
 * ```tsx
 * <MobileBottomSheet
 *   trigger={<button>打开审批</button>}
 *   title="待审批"
 *   badge={3}
 * >
 *   {content}
 * </MobileBottomSheet>
 * ```
 */

import * as Dialog from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import type { ReactNode } from 'react';

export interface MobileBottomSheetProps {
  /** 触发按钮（仅移动端可见） */
  trigger: ReactNode;
  /** 抽屉标题 */
  title: string;
  /** 可选的数字徽标 */
  badge?: number;
  /** 抽屉内容 */
  children: ReactNode;
}

export function MobileBottomSheet({
  trigger,
  title,
  badge,
  children,
}: MobileBottomSheetProps) {
  return (
    <>
      {/* Mobile: bottom sheet trigger */}
      <div className="lg:hidden">
        <Dialog.Root>
          <Dialog.Trigger asChild>
            {trigger}
          </Dialog.Trigger>
          <Dialog.Portal>
            <Dialog.Overlay className="fixed inset-0 z-50 bg-slate-950/40 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
            <Dialog.Content className="fixed inset-x-0 bottom-0 z-50 max-h-[80vh] rounded-t-[28px] border-t border-[#e0d6c6] bg-white px-5 py-5 shadow-[0_-12px_40px_rgba(34,32,28,0.12)] focus:outline-none data-[state=open]:animate-in data-[state=open]:slide-in-from-bottom data-[state=closed]:animate-out data-[state=closed]:slide-out-to-bottom">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <Dialog.Title className="text-lg font-semibold text-slate-900">
                    {title}
                  </Dialog.Title>
                  {badge != null && badge > 0 && (
                    <span className="inline-flex h-6 min-w-6 items-center justify-center rounded-full bg-[#fbf3df] px-2 text-xs font-semibold text-[#7c5d12]">
                      {badge}
                    </span>
                  )}
                </div>
                <Dialog.Close asChild>
                  <button
                    type="button"
                    aria-label="Close"
                    className="inline-flex h-9 w-9 items-center justify-center rounded-2xl border border-[#e5ddd4] bg-[#faf6ef] text-slate-700 transition-colors hover:bg-white"
                  >
                    <X size={16} />
                  </button>
                </Dialog.Close>
              </div>
              <div className="mt-4 max-h-[calc(80vh-80px)] overflow-y-auto">
                {children}
              </div>
              {/* Drag indicator */}
              <div className="absolute left-1/2 top-2 h-1 w-10 -translate-x-1/2 rounded-full bg-slate-300" />
            </Dialog.Content>
          </Dialog.Portal>
        </Dialog.Root>
      </div>

      {/* Desktop: render children directly (no trigger shown) */}
      <div className="hidden lg:block">
        {children}
      </div>
    </>
  );
}
