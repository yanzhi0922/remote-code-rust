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
import { formatSensitivePath } from '../../lib/utils';

interface ApprovalItem {
  approval_id: string;
  title: string;
  description: string;
  metadata: {
    blocked_path?: string;
    [key: string]: unknown;
  };
}

interface ApprovalAction {
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
  /** 为 true 时隐藏审批 metadata 中的本机路径。 */
  privacyMode?: boolean;
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
  privacyMode = false,
}: ApprovalPanelProps) {
  return (
    <section className="rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-4 shadow-sm">
      {!hideTitle && (
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
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
              className="rounded-md border border-rc-border-primary bg-rc-bg-hover px-3 py-3"
            >
              <div className="text-sm font-medium text-rc-text-primary">{approval.title}</div>
              <div className="mt-1 text-sm leading-6 text-rc-text-secondary">
                {approval.description}
              </div>
              {approval.metadata.blocked_path && (
                <div className="mt-2 rounded-md bg-rc-bg-surface px-3 py-2 font-mono text-xs text-rc-text-tertiary">
                  {formatSensitivePath(approval.metadata.blocked_path, privacyMode, 56)}
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
                    className={`rounded-md px-3 py-1.5 text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-60 ${item.className}`}
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
  return <div className="text-sm text-rc-text-tertiary">{children}</div>;
}
