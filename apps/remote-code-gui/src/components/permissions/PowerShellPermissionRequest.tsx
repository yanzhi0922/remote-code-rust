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

const POWERSHELL_DANGEROUS_PATTERNS = [
  /-EncodedCommand/i,
  /Invoke-Expression/i,
  /\bIEX\b/i,
  /Invoke-WebRequest.*\|\s*Invoke-Expression/i,
  /Set-ExecutionPolicy/i,
  /\bNew-Object\s+System\.Net\.WebClient/i,
  /\bStart-Process.*-Verb\s+RunAs/i,
  /\[System\.Reflection\.Assembly\]/i,
  /Remove-Item\s+-Recurse\s+-Force/i,
  /\bOut-File.*-Encoding\b/i,
  /\bInvoke-Command\s+-ScriptBlock/i,
];

function isDangerousPowerShell(command: string): boolean {
  return POWERSHELL_DANGEROUS_PATTERNS.some((p) => p.test(command));
}

function getDangerousWarnings(command: string): string[] {
  const warnings: string[] = [];
  if (/-EncodedCommand/i.test(command)) {
    warnings.push('使用编码命令，可能隐藏恶意代码');
  }
  if (/Invoke-Expression/i.test(command) || /\bIEX\b/i.test(command)) {
    warnings.push('使用 Invoke-Expression，可能执行任意代码');
  }
  if (/Set-ExecutionPolicy/i.test(command)) {
    warnings.push('修改执行策略，可能降低安全性');
  }
  if (/Remove-Item\s+-Recurse\s+-Force/i.test(command)) {
    warnings.push('强制递归删除文件');
  }
  return warnings;
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface PowerShellPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function PowerShellPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: PowerShellPermissionRequestProps) {
  const record = asRecord(request.input);
  const command = stringField(record, 'command') ?? '';
  const dangerous = command ? isDangerousPowerShell(command) : false;
  const warnings = command ? getDangerousWarnings(command) : [];

  return (
    <div
      className={cn(
        'rounded-2xl border border-orange-200 bg-white p-4',
        dangerous && 'border-red-300',
        className,
      )}
      data-testid="powershell-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'PowerShell Command'}
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
                危险 PowerShell 命令
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

      {warnings.length > 0 && (
        <div className="my-2 space-y-1">
          {warnings.map((warning, idx) => (
            <div
              key={idx}
              className="flex items-center gap-1 rounded bg-amber-50 px-2 py-1 text-xs text-amber-700"
            >
              <AlertTriangle size={12} className="shrink-0" />
              {warning}
            </div>
          ))}
        </div>
      )}

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
