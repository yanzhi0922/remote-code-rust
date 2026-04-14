/**
 * TimelineEventCard — 共享的时间线事件卡片组件。
 *
 * 用于渲染时间线中的各类事件（工具调用、审批、产物、运行器状态等）。
 * 提供统一的 eyebrow + icon + timestamp 布局，内容由调用方通过 children 传入。
 *
 * Props:
 * - eyebrow: 事件类型标签（如 "Tool" / "Approval"）
 * - accent: eyebrow 文字颜色类名（如 "text-emerald-700"）
 * - icon: 左侧图标元素
 * - timestampLabel: 已格式化的时间戳文本
 * - children: 事件内容
 */

import type { ReactNode } from 'react';

export interface TimelineEventCardProps {
  /** 事件类型标签 */
  eyebrow: string;
  /** eyebrow 文字颜色 Tailwind 类名 */
  accent: string;
  /** 左侧图标 */
  icon: ReactNode;
  /** 已格式化的时间戳文本 */
  timestampLabel: string;
  /** 事件内容 */
  children: ReactNode;
}

export function TimelineEventCard({
  eyebrow,
  accent,
  icon,
  timestampLabel,
  children,
}: TimelineEventCardProps) {
  return (
    <div className="rounded-[24px] border border-[#e5ddcf] bg-white px-5 py-4 shadow-[0_14px_32px_rgba(34,32,28,0.06)]">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className={`inline-flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.22em] ${accent}`}>
          {icon}
          {eyebrow}
        </div>
        <div className="text-xs text-slate-400">{timestampLabel}</div>
      </div>
      {children}
    </div>
  );
}
