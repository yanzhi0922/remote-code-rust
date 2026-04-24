import { ChevronRight, Server, Wrench } from 'lucide-react';
import type { McpServerInfo } from '../../lib/types';

interface McpServerCardProps {
  server: McpServerInfo;
  onClick: () => void;
  selected?: boolean;
}

function statusColor(status: string | undefined): string {
  switch (status) {
    case 'connected':
      return 'bg-emerald-500';
    case 'error':
      return 'bg-red-500';
    default:
      return 'bg-slate-400';
  }
}

function statusLabel(status: string | undefined): string {
  switch (status) {
    case 'connected':
      return '已连接';
    case 'error':
      return '错误';
    case 'disconnected':
      return '已断开';
    default:
      return '未连接';
  }
}

function transportIcon(transport: string): string {
  switch (transport) {
    case 'http':
      return 'HTTP';
    case 'websocket':
      return 'WS';
    default:
      return 'stdio';
  }
}

export function McpServerCard({ server, onClick, selected }: McpServerCardProps) {
  const liveStatus = server.live?.status;
  const toolCount = server.live?.tool_count ?? 0;
  const isSelected = selected ?? false;

  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full rounded-2xl border p-3 text-left transition-colors ${
        isSelected
          ? 'border-emerald-300 bg-emerald-50'
          : server.enabled
            ? 'border-slate-200 bg-white hover:border-slate-300 hover:bg-slate-50'
            : 'border-slate-200 bg-slate-50 opacity-60'
      }`}
      data-testid={`mcp-server-card-${server.name}`}
    >
      <div className="flex items-center gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-slate-100">
          <Server size={18} className="text-slate-600" />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-medium text-slate-800">{server.name}</span>
            {!server.enabled && (
              <span className="rounded-md bg-slate-200 px-1.5 py-0.5 text-xs text-slate-500">
                已禁用
              </span>
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-xs text-slate-500">
            <span className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[10px] uppercase">
              {transportIcon(server.transport)}
            </span>
            <span className="flex items-center gap-1">
              <span className={`inline-block h-2 w-2 rounded-full ${statusColor(liveStatus)}`} />
              {statusLabel(liveStatus)}
            </span>
          </div>
        </div>

        {toolCount > 0 && (
          <span className="flex items-center gap-1 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700">
            <Wrench size={12} />
            {toolCount}
          </span>
        )}

        <ChevronRight size={16} className="shrink-0 text-slate-400" />
      </div>
    </button>
  );
}
