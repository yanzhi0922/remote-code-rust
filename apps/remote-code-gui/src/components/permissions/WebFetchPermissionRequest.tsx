import { Globe } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringField(record: Record<string, unknown> | null, ...keys: string[]): string | null {
  if (!record) return null;
  for (const key of keys) {
    const val = record[key];
    if (typeof val === 'string' && val.trim()) return val;
  }
  return null;
}

function isSecureUrl(url: string): boolean {
  return url.startsWith('https://');
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface WebFetchPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function WebFetchPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: WebFetchPermissionRequestProps) {
  const record = asRecord(request.input);
  const url = stringField(record, 'url', 'uri', 'endpoint') ?? '';
  const secure = url ? isSecureUrl(url) : true;

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="web-fetch-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'Web Fetch'}
        subtitle={request.description}
      />

      {url ? (
        <div className="my-3 flex items-start gap-2 rounded-lg bg-slate-50 p-3">
          <Globe size={16} className="mt-0.5 shrink-0 text-slate-400" />
          <div className="min-w-0 flex-1">
            <a
              href={url}
              target="_blank"
              rel="noopener noreferrer"
              className="break-all font-mono text-sm text-blue-600 underline decoration-blue-300 hover:text-blue-800"
            >
              {url}
            </a>
            {!secure && (
              <div className="mt-1 text-xs text-amber-600">
                ⚠ 非 HTTPS 连接，数据传输可能不安全
              </div>
            )}
          </div>
        </div>
      ) : (
        <div className="my-3 rounded-lg bg-slate-50 p-3 text-sm text-slate-400">
          无 URL 信息
        </div>
      )}

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className="rounded-2xl bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          允许执行
        </button>
        <button
          type="button"
          onClick={() => onReject()}
          className="rounded-2xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
