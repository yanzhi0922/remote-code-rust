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
    <div className="rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-3 shadow-sm">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className={`inline-flex items-center gap-2 text-xs font-semibold uppercase ${accent}`}>
          {icon}
          {eyebrow}
        </div>
        <div className="text-xs text-rc-text-tertiary">{timestampLabel}</div>
      </div>
      {children}
    </div>
  );
}
