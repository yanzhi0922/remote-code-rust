import { memo, type ReactNode } from 'react';
import { Bot, User } from 'lucide-react';
import type { ConversationRole } from '../lib/types';
import { cn } from '../lib/utils';
import { MessageTimestamp } from './MessageTimestamp';
import { MessageActions } from './messageActions';

/** 消息行组件属性 */
export interface MessageRowProps {
  /** 消息内容节点 */
  children: ReactNode;
  /** 消息角色 */
  role: ConversationRole;
  /** ISO 8601 时间戳 */
  timestamp?: string | null;
  /** 消息文本（用于操作按钮） */
  messageText?: string;
  /** 消息 ID */
  messageId?: string;
  /** 是否为 transcript 模式 */
  isTranscriptMode?: boolean;
  /** 是否为用户消息的连续（合并显示） */
  isUserContinuation?: boolean;
  /** 是否显示操作按钮 */
  showActions?: boolean;
  /** 重新发送回调 */
  onResend?: (messageId: string) => void;
  /** 编辑回调 */
  onEdit?: (messageId: string) => void;
  /** 额外的 CSS 类名 */
  className?: string;
}

const roleAvatarConfig: Record<ConversationRole, { icon: typeof Bot; bg: string; label: string }> = {
  assistant: {
    icon: Bot,
    bg: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300',
    label: '助手',
  },
  user: {
    icon: User,
    bg: 'bg-slate-200 text-slate-700 dark:bg-slate-600 dark:text-slate-200',
    label: '用户',
  },
  tool: {
    icon: Bot,
    bg: 'bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300',
    label: '工具',
  },
  system: {
    icon: Bot,
    bg: 'bg-slate-100 text-slate-500 dark:bg-slate-700 dark:text-slate-400',
    label: '系统',
  },
};

/**
 * 消息行组件。
 * 包装单条消息的渲染，包含头像、时间戳和操作按钮。
 */
export const MessageRow = memo(function MessageRow({
  children,
  role,
  timestamp,
  messageText,
  messageId,
  isTranscriptMode = false,
  isUserContinuation = false,
  showActions = true,
  onResend,
  onEdit,
  className,
}: MessageRowProps) {
  if (role === 'system') return null;

  const config = roleAvatarConfig[role];
  const AvatarIcon = config.icon;
  const isUser = role === 'user';

  return (
    <div
      className={cn(
        'group relative flex gap-3',
        isUserContinuation && isUser && 'mt-1',
        !isUserContinuation && 'mt-4',
        className,
      )}
    >
      {/* 头像 */}
      {!isUserContinuation && (
        <div className="flex shrink-0 flex-col items-center pt-1">
          <div
            className={cn(
              'flex h-8 w-8 items-center justify-center rounded-full',
              config.bg,
            )}
            aria-label={config.label}
          >
            <AvatarIcon className="h-4 w-4" />
          </div>
        </div>
      )}

      {/* 消息主体 */}
      <div className="min-w-0 flex-1">
        {/* 头部：角色标签 + 时间戳 + 操作 */}
        {!isUserContinuation && (
          <div className="mb-1.5 flex items-center gap-2">
            <span className="text-xs font-semibold text-slate-500 dark:text-slate-400">
              {config.label}
            </span>
            <MessageTimestamp
              timestamp={timestamp ?? null}
              isTranscriptMode={isTranscriptMode}
            />
            {showActions && messageText && (
              <MessageActions
                text={messageText}
                messageId={messageId}
                canResend={isUser}
                canEdit={isUser}
                onResend={onResend}
                onEdit={onEdit}
              />
            )}
          </div>
        )}

        {/* 消息内容 */}
        <div>{children}</div>
      </div>
    </div>
  );
});
