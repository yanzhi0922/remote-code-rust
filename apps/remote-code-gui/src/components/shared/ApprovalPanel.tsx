/**
 * ApprovalPanel — 共享的审批队列面板组件。
 *
 * 渲染待审批项列表，每项显示标题、描述、阻塞路径和操作按钮。
 * 本地桌面端和远程 Web/PWA 端共用同一视觉语言。
 *
 * Props:
 * - title: 面板标题
 * - icon: 标题图标
 * - emptyText: 无审批项时的提示文本
 * - items: 审批项列表
 * - actions: 可执行的操作列表
 * - approvingId: 正在处理中的审批 ID
 * - loadingText: 加载中提示文本
 * - onDecision: 审批决策回调
 */

import { LoaderCircle } from 'lucide-react';
import type { ReactNode } from 'react';
import { truncateMiddle } from '../../lib/utils';

export interface ApprovalItem {
  approval_id: string;
  title: string;
  description: string;
  metadata: {
    blocked_path?: string;
    [key: string]: unknown;
  };
}

export interface ApprovalAction {
  decision: string;
  label: string;
  className: string;
}

export interface ApprovalPanelProps {
  title: string;
  icon: ReactNode;
  emptyText: string;
  items: ApprovalItem[];
  actions: ApprovalAction[];
  approvingId: string | null;
  loadingText: string;
  onDecision: (approvalId: string, decision: string) => void;
  /** 为 true 时隐藏面板自带标题（用于外部已有标题的场景，如移动端 bottom sheet） */
  hideTitle?: boolean;
}

export function ApprovalPanel({
  title,
  icon,
  emptyText,
  items,
  actions,
  approvingId,
  loadingText,
  onDecision,
  hideTitle,
}: ApprovalPanelProps) {
  return (
    <section className="rounded-[24px] border border-[#e0d6c6] bg-white px-4 py-4 shadow-[0_12px_30px_rgba(34,32,28,0.06)]">
      {!hideTitle && (
        <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
          {icon}
          {title}
        </div>
      )}
      <div className="mt-4 space-y-3">
        {items.length === 0 ? (
          <PanelHint>{emptyText}</PanelHint>
        ) : (
          items.map((approval) => (
            <div
              key={approval.approval_id}
              className="rounded-2xl border border-[#ebe2d5] bg-[#faf7f1] px-3 py-3"
            >
              <div className="text-sm font-medium text-slate-900">{approval.title}</div>
              <div className="mt-1 text-sm leading-6 text-slate-600">
                {approval.description}
              </div>
              {approval.metadata.blocked_path && (
                <div className="mt-2 rounded-xl bg-white px-3 py-2 font-mono text-xs text-slate-500">
                  {truncateMiddle(approval.metadata.blocked_path, 56)}
                </div>
              )}
              <div className="mt-3 flex flex-wrap gap-2">
                {actions.map((item) => (
                  <button
                    key={item.decision}
                    type="button"
                    onClick={() => {
                      onDecision(approval.approval_id, item.decision);
                    }}
                    disabled={approvingId === approval.approval_id}
                    className={`rounded-full px-3 py-1.5 text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60 ${item.className}`}
                  >
                    {approvingId === approval.approval_id ? (
                      <span className="inline-flex items-center gap-2">
                        <LoaderCircle size={14} className="animate-spin" />
                        {loadingText}
                      </span>
                    ) : (
                      item.label
                    )}
                  </button>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

/** 面板内空状态提示 */
export function PanelHint({ children }: { children: ReactNode }) {
  return <div className="text-sm text-slate-400">{children}</div>;
}
