import type { ReactNode } from 'react';
import { ShieldAlert } from 'lucide-react';

export interface PermissionRequestTitleProps {
  title: string;
  subtitle?: ReactNode;
  color?: string;
  workerBadge?: { name: string; color: string };
}

export function PermissionRequestTitle({
  title,
  subtitle,
  color = '#b23a2f',
  workerBadge,
}: PermissionRequestTitleProps) {
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-2">
        <ShieldAlert size={18} style={{ color }} />
        <span className="font-semibold text-slate-800">{title}</span>
        {workerBadge && (
          <span
            className="ml-1 inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium text-white"
            style={{ backgroundColor: workerBadge.color }}
          >
            @{workerBadge.name}
          </span>
        )}
      </div>
      {subtitle && (
        <div className="mt-1 text-sm text-slate-500">
          {typeof subtitle === 'string' ? <span>{subtitle}</span> : subtitle}
        </div>
      )}
    </div>
  );
}
