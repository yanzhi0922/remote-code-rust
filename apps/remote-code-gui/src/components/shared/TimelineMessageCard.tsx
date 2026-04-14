/**
 * TimelineMessageCard — 共享的消息气泡组件。
 *
 * 用于渲染时间线中的用户消息或助手回复卡片。
 * 本地桌面端和远程 Web/PWA 端共用同一视觉语言。
 *
 * Props:
 * - role: 消息角色，决定气泡方向和配色
 * - header: 卡片顶部标签（如 "You" / "Assistant"）
 * - children: 消息内容（文本、Markdown、工具调用等）
 */

import type { ReactNode } from 'react';

export interface TimelineMessageCardProps {
  /** 消息角色 */
  role: 'user' | 'assistant' | 'system';
  /** 卡片顶部标签 */
  header: string;
  /** 消息内容 */
  children: ReactNode;
}

export function TimelineMessageCard({ role, header, children }: TimelineMessageCardProps) {
  const isUser = role === 'user';
  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div
        className={`max-w-4xl rounded-[28px] px-5 py-4 shadow-[0_16px_34px_rgba(34,32,28,0.07)] ${
          isUser ? 'bg-[#17181a] text-white' : 'border border-[#e5ddcf] bg-white'
        }`}
      >
        <div
          className={`mb-3 text-xs font-semibold uppercase tracking-[0.22em] ${
            isUser ? 'text-white/60' : 'text-slate-400'
          }`}
        >
          {header}
        </div>
        {children}
      </div>
    </div>
  );
}
