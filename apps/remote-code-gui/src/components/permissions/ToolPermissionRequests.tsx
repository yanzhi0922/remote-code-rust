import { useState } from 'react';
import type { PermissionRequestInfo } from '../../lib/types';
import { PermissionRequestTitle } from './PermissionRequestTitle';
import { PermissionExplanation } from './PermissionExplanation';

/* ------------------------------------------------------------------ */
/* Shared helpers                                                      */
/* ------------------------------------------------------------------ */

function formatInput(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input);
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

const DANGEROUS_PATTERNS = [
  /\brm\s+-rf\b/,
  /\brm\s+--recursive\b/,
  /\bdel\s+\/[sS]\b/,
  /\bformat\s+[A-Z]:/i,
  /\bdd\s+if=/,
  /\bmkfs\b/,
  /\b:\(\)\{.*;\}\s*;/, // fork bomb
];

function highlightDangerousCommand(command: string): boolean {
  return DANGEROUS_PATTERNS.some((p) => p.test(command));
}

/* ------------------------------------------------------------------ */
/* Shared Props                                                        */
/* ------------------------------------------------------------------ */

interface ToolPermissionProps {
  request: PermissionRequestInfo;
  onAllow: (updates?: unknown[]) => void;
  onReject: (feedback?: string) => void;
}

/* ------------------------------------------------------------------ */
/* Action buttons                                                      */
/* ------------------------------------------------------------------ */

function ActionButtons({ onAllow, onReject }: { onAllow: () => void; onReject: () => void }) {
  return (
    <div className="flex gap-2">
      <button
        type="button"
        onClick={onAllow}
        className="rounded-2xl bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
      >
        允许执行
      </button>
      <button
        type="button"
        onClick={onReject}
        className="rounded-2xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
      >
        拒绝
      </button>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* BashPermissionRequest                                               */
/* ------------------------------------------------------------------ */

export function BashPermissionRequest({ request, onAllow, onReject }: ToolPermissionProps) {
  const record = asRecord(request.input);
  const command = typeof record?.command === 'string' ? record.command : formatInput(request.input);
  const isDangerous = highlightDangerousCommand(command);
  const [explanationVisible, setExplanationVisible] = useState(false);

  return (
    <div className="rounded-2xl border border-orange-200 bg-white p-4" data-testid="bash-permission">
      <PermissionRequestTitle title={request.title || 'Bash Command'} subtitle={request.description} />
      <div
        className={`my-3 rounded-lg p-3 font-mono text-sm ${
          isDangerous ? 'bg-red-50 text-red-800' : 'bg-slate-50 text-slate-800'
        }`}
      >
        {isDangerous && (
          <div className="mb-1 text-xs font-semibold text-red-600">⚠ 危险命令</div>
        )}
        <pre className="whitespace-pre-wrap break-all">{command}</pre>
      </div>
      <PermissionExplanation
        toolName={request.tool_name}
        toolInput={request.input}
        visible={explanationVisible}
        onToggle={() => setExplanationVisible((v) => !v)}
      />
      <div className="mt-3">
        <ActionButtons onAllow={() => onAllow()} onReject={() => onReject()} />
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* FileEditPermissionRequest                                           */
/* ------------------------------------------------------------------ */

export function FileEditPermissionRequest({ request, onAllow, onReject }: ToolPermissionProps) {
  const record = asRecord(request.input);
  const filePath = typeof record?.file_path === 'string' ? record.file_path : '';
  const oldText = typeof record?.old_text === 'string' ? record.old_text : '';
  const newText = typeof record?.new_text === 'string' ? record.new_text : '';

  return (
    <div className="rounded-2xl border border-orange-200 bg-white p-4" data-testid="file-edit-permission">
      <PermissionRequestTitle title={request.title || 'File Edit'} subtitle={request.description} />
      {filePath && (
        <div className="my-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
          {filePath}
        </div>
      )}
      <div className="my-2 space-y-2">
        {oldText && (
          <div className="rounded-lg bg-red-50 p-2 font-mono text-xs text-red-800">
            <div className="mb-1 text-xs font-semibold text-red-600">- 旧内容</div>
            <pre className="whitespace-pre-wrap break-all">{oldText}</pre>
          </div>
        )}
        {newText && (
          <div className="rounded-lg bg-emerald-50 p-2 font-mono text-xs text-emerald-800">
            <div className="mb-1 text-xs font-semibold text-emerald-600">+ 新内容</div>
            <pre className="whitespace-pre-wrap break-all">{newText}</pre>
          </div>
        )}
      </div>
      <div className="mt-3">
        <ActionButtons onAllow={() => onAllow()} onReject={() => onReject()} />
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* FileWritePermissionRequest                                          */
/* ------------------------------------------------------------------ */

export function FileWritePermissionRequest({ request, onAllow, onReject }: ToolPermissionProps) {
  const record = asRecord(request.input);
  const filePath = typeof record?.file_path === 'string' ? record.file_path : '';
  const content = typeof record?.content === 'string' ? record.content : formatInput(request.input);

  return (
    <div className="rounded-2xl border border-orange-200 bg-white p-4" data-testid="file-write-permission">
      <PermissionRequestTitle title={request.title || 'File Write'} subtitle={request.description} />
      {filePath && (
        <div className="my-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
          {filePath}
        </div>
      )}
      <div className="my-2 max-h-40 overflow-y-auto rounded-lg bg-slate-50 p-2 font-mono text-xs text-slate-700">
        <pre className="whitespace-pre-wrap break-all">{content}</pre>
      </div>
      <div className="mt-3">
        <ActionButtons onAllow={() => onAllow()} onReject={() => onReject()} />
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* McpPermissionRequest                                                */
/* ------------------------------------------------------------------ */

export function McpPermissionRequest({ request, onAllow, onReject }: ToolPermissionProps) {
  const record = asRecord(request.input);
  const toolName = typeof record?.tool_name === 'string' ? record.tool_name : request.tool_name;
  const args = record?.arguments ?? record?.args ?? request.input;

  return (
    <div className="rounded-2xl border border-orange-200 bg-white p-4" data-testid="mcp-permission">
      <PermissionRequestTitle title={request.title || 'MCP Tool'} subtitle={request.description} />
      <div className="my-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
        MCP 工具: {toolName}
      </div>
      <div className="my-2 max-h-40 overflow-y-auto rounded-lg bg-slate-50 p-2 font-mono text-xs text-slate-700">
        <pre className="whitespace-pre-wrap break-all">{formatInput(args)}</pre>
      </div>
      <div className="mt-3">
        <ActionButtons onAllow={() => onAllow()} onReject={() => onReject()} />
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* GenericPermissionRequest                                            */
/* ------------------------------------------------------------------ */

export function GenericPermissionRequest({ request, onAllow, onReject }: ToolPermissionProps) {
  return (
    <div className="rounded-2xl border border-orange-200 bg-white p-4" data-testid="generic-permission">
      <PermissionRequestTitle title={request.title || request.tool_name} subtitle={request.description} />
      <div className="my-2 max-h-40 overflow-y-auto rounded-lg bg-slate-50 p-2 font-mono text-xs text-slate-700">
        <pre className="whitespace-pre-wrap break-all">{formatInput(request.input)}</pre>
      </div>
      <div className="mt-3">
        <ActionButtons onAllow={() => onAllow()} onReject={() => onReject()} />
      </div>
    </div>
  );
}
