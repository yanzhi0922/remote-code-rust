import { memo } from 'react';
import { RefreshCw } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 资源更新类型 */
export type ResourceUpdateKind = 'created' | 'updated' | 'deleted' | 'synced';

/** 资源更新消息属性 */
export interface UserResourceUpdateMessageProps {
  /** 资源名称 */
  resourceName: string;
  /** 资源类型（如 file、directory、config） */
  resourceType: string;
  /** 更新类型 */
  kind: ResourceUpdateKind;
  /** 更新描述 */
  description?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

const kindLabels: Record<ResourceUpdateKind, string> = {
  created: '已创建',
  updated: '已更新',
  deleted: '已删除',
  synced: '已同步',
};

const kindColors: Record<ResourceUpdateKind, string> = {
  created: 'text-emerald-700 bg-emerald-50 border-emerald-200 dark:text-emerald-400 dark:bg-emerald-950/30 dark:border-emerald-800',
  updated: 'text-blue-700 bg-blue-50 border-blue-200 dark:text-blue-400 dark:bg-blue-950/30 dark:border-blue-800',
  deleted: 'text-rose-700 bg-rose-50 border-rose-200 dark:text-rose-400 dark:bg-rose-950/30 dark:border-rose-800',
  synced: 'text-slate-700 bg-slate-50 border-slate-200 dark:text-slate-400 dark:bg-slate-800/50 dark:border-slate-700',
};

/**
 * 资源更新消息组件。
 * 显示文件或资源的创建、更新、删除等操作通知。
 */
export const UserResourceUpdateMessage = memo(function UserResourceUpdateMessage({
  resourceName,
  resourceType,
  kind,
  description,
  className,
}: UserResourceUpdateMessageProps) {
  return (
    <div
      className={cn(
        'rounded-xl border px-4 py-3',
        kindColors[kind],
        className,
      )}
    >
      <div className="flex items-center gap-2">
        <RefreshCw className="h-3.5 w-3.5 shrink-0" />
        <span className="text-xs font-medium">
          {resourceType}
          <span className="mx-1 opacity-50">·</span>
          <span className="font-semibold">{resourceName}</span>
          <span className="mx-1 opacity-50">·</span>
          {kindLabels[kind]}
        </span>
      </div>
      {description && (
        <p className="mt-1 pl-5.5 text-xs opacity-80">{description}</p>
      )}
    </div>
  );
});
