import { useState } from 'react';
import { Terminal, AlertTriangle } from 'lucide-react';
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

const DANGEROUS_PATTERNS = [
  /\brm\s+-rf\b/,
  /\brm\s+--recursive\b/,
  /\bdel\s+\/[sS]\b/,
  /\bformat\s+[A-Z]:/i,
  /\bdd\s+if=/,
  /\bmkfs\b/,
  /\b:\(\)\{.*;\}\s*;/,
  /\bchmod\s+-R\s+777\b/,
  /\bchown\s+-R\b/,
  /\b>\s*\/dev\/sd/,
];

function isDangerousCommand(command: string): boolean {
  return DANGEROUS_PATTERNS.some((p) => p.test(command));
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface BashPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function BashPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: BashPermissionRequestProps) {
  const record = asRecord(request.input);
  const command = stringField(record, 'command') ?? '';
  const dangerous = command ? isDangerousCommand(command) : false;
  const [mode, setMode] = useState<'once' | 'session'>('once');

  return (
    <div
      className={cn(
        'rounded-2xl border border-orange-200 bg-white p-4',
        dangerous && 'border-red-300',
        className,
      )}
      data-testid="bash-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'Bash Command'}
        subtitle={request.description}
      />

      {command ? (
        <div
          className={cn(
            'my-3 flex items-start gap-2 rounded-lg p-3 font-mono text-sm',
            dangerous ? 'bg-red-50 text-red-800' : 'bg-slate-50 text-slate-800',
          )}
        >
          <Terminal size={16} className="mt-0.5 shrink-0" />
          <div className="min-w-0 flex-1">
            {dangerous && (
              <div className="mb-1 flex items-center gap-1 text-xs font-semibold text-red-600">
                <AlertTriangle size={12} />
                危险命令
              </div>
            )}
            <pre className="whitespace-pre-wrap break-all">{command}</pre>
          </div>
        </div>
      ) : (
        <div className="my-3 rounded-lg bg-slate-50 p-3 text-sm text-slate-400">
          无命令内容
        </div>
      )}

      <div className="mt-3 flex items-center gap-2">
        <label className="flex items-center gap-1 text-xs text-slate-500">
          <input
            type="radio"
            name={`bash-mode-${request.request_id}`}
            checked={mode === 'once'}
            onChange={() => setMode('once')}
            className="accent-blue-600"
          />
          仅本次
        </label>
        <label className="flex items-center gap-1 text-xs text-slate-500">
          <input
            type="radio"
            name={`bash-mode-${request.request_id}`}
            checked={mode === 'session'}
            onChange={() => setMode('session')}
            className="accent-blue-600"
          />
          本次会话
        </label>
      </div>

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className={cn(
            'rounded-2xl px-4 py-2 text-sm font-medium text-white transition-colors',
            dangerous
              ? 'bg-red-600 hover:bg-red-700'
              : 'bg-blue-600 hover:bg-blue-700',
          )}
        >
          {mode === 'session' ? '会话允许' : '允许执行'}
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
