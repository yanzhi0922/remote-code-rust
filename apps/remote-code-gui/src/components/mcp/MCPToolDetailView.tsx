import { ArrowLeft, Copy, Wrench } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';
import type { McpToolInfo } from '../../lib/types';

export interface MCPToolDetailViewProps {
  tool: McpToolInfo;
  serverName: string;
  onBack: () => void;
  className?: string;
}

export function MCPToolDetailView({ tool, serverName, onBack, className }: MCPToolDetailViewProps) {
  const [copied, setCopied] = useState(false);

  const schemaText =
    tool.inputSchema != null
      ? typeof tool.inputSchema === 'string'
        ? tool.inputSchema
        : JSON.stringify(tool.inputSchema, null, 2)
      : '';

  function handleCopy() {
    navigator.clipboard.writeText(schemaText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  return (
    <div className={cn('flex flex-col gap-4', className)} data-testid="mcp-tool-detail-view">
      {/* Header */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1 rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          data-testid="mcp-tool-detail-back"
        >
          <ArrowLeft size={14} />
          返回
        </button>
      </div>

      {/* Tool card */}
      <div className="rounded-2xl border border-slate-200 bg-white p-4">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-50">
            <Wrench size={18} className="text-blue-600" />
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="text-base font-semibold text-slate-800">{tool.name}</h3>
            <span className="text-xs text-slate-500">来自 {serverName}</span>
          </div>
        </div>

        {tool.description && (
          <div className="mt-4">
            <div className="text-sm font-medium text-slate-600">描述</div>
            <p className="mt-1 text-sm leading-relaxed text-slate-700">{tool.description}</p>
          </div>
        )}
      </div>

      {/* Schema section */}
      {schemaText && (
        <div className="rounded-2xl border border-slate-200 bg-white p-4">
          <div className="flex items-center justify-between">
            <div className="text-sm font-medium text-slate-600">输入 Schema</div>
            <button
              type="button"
              onClick={handleCopy}
              className="flex items-center gap-1 rounded-lg border border-slate-200 px-2 py-1 text-xs text-slate-500 hover:bg-slate-50"
              data-testid="mcp-tool-detail-copy"
              title="复制 Schema"
            >
              <Copy size={12} />
              {copied ? '已复制' : '复制'}
            </button>
          </div>
          <pre className="mt-2 max-h-64 overflow-auto rounded-xl bg-slate-50 p-3 text-xs text-slate-700">
            {schemaText}
          </pre>
        </div>
      )}
    </div>
  );
}
