import { ArrowLeft, Wrench } from 'lucide-react';
import type { McpToolInfo } from '../../lib/types';

interface McpToolDetailProps {
  tool: McpToolInfo;
  serverName: string;
  onBack: () => void;
}

export function McpToolDetail({ tool, serverName, onBack }: McpToolDetailProps) {
  return (
    <div className="flex flex-col gap-4" data-testid="mcp-tool-detail">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1 rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          data-testid="mcp-tool-back-btn"
        >
          <ArrowLeft size={14} />
          返回工具列表
        </button>
        {tool.inputSchema != null && (
          <div className="mt-4">
            <div className="text-sm font-medium text-slate-600">输入 Schema</div>
            <pre className="mt-1 overflow-x-auto rounded-xl bg-slate-50 p-3 text-xs text-slate-700">
              {typeof tool.inputSchema === 'string'
                ? tool.inputSchema
                : JSON.stringify(tool.inputSchema, null, 2)}
            </pre>
          </div>
        )}
      </div>

      <div className="rounded-2xl border border-slate-200 bg-white p-4">
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-blue-50">
            <Wrench size={16} className="text-blue-600" />
          </div>
          <div>
            <h3 className="font-semibold text-slate-800">{tool.name}</h3>
            <span className="text-xs text-slate-500">来自 {serverName}</span>
          </div>
        </div>

        {tool.description && (
          <div className="mt-3">
            <div className="text-sm font-medium text-slate-600">描述</div>
            <p className="mt-1 text-sm text-slate-700">{tool.description}</p>
          </div>
        )}

      </div>
    </div>
  );
}
