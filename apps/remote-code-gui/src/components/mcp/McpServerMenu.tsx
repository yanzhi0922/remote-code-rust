import { Cable, Power, PowerOff, Trash2, Wrench } from 'lucide-react';
import { useState } from 'react';
import type { McpServerInfo } from '../../lib/types';

interface McpServerMenuProps {
  server: McpServerInfo;
  onConnect: () => void;
  onDisconnect: () => void;
  onToggle: (enabled: boolean) => void;
  onRemove: () => void;
  onViewTools: () => void;
}

export function McpServerMenu({ server, onConnect, onDisconnect, onToggle, onRemove, onViewTools }: McpServerMenuProps) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const isConnected = server.live?.status === 'connected';

  return (
    <div className="rounded-2xl border border-slate-200 bg-white p-4" data-testid="mcp-server-menu">
      <h3 className="text-lg font-semibold text-slate-800">{server.name}</h3>

      {/* Server details */}
      <div className="mt-3 space-y-1.5 text-sm text-slate-600">
        <div className="flex items-center gap-2">
          <span className="font-medium text-slate-500">传输类型:</span>
          <span className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-xs uppercase">{server.transport}</span>
        </div>
        {server.command && (
          <div>
            <span className="font-medium text-slate-500">命令:</span>{' '}
            <code className="rounded bg-slate-100 px-1 text-xs">{server.command} {server.args.join(' ')}</code>
          </div>
        )}
        {server.url && (
          <div>
            <span className="font-medium text-slate-500">URL:</span>{' '}
            <code className="rounded bg-slate-100 px-1 text-xs">{server.url}</code>
          </div>
        )}
        {server.cwd && (
          <div>
            <span className="font-medium text-slate-500">工作目录:</span>{' '}
            <code className="rounded bg-slate-100 px-1 text-xs">{server.cwd}</code>
          </div>
        )}
        {server.env_keys.length > 0 && (
          <div>
            <span className="font-medium text-slate-500">环境变量:</span>{' '}
            <span className="text-xs">{server.env_keys.join(', ')}</span>
          </div>
        )}
        {server.startup_timeout_secs != null && (
          <div>
            <span className="font-medium text-slate-500">启动超时:</span> {server.startup_timeout_secs}s
          </div>
        )}
        {server.request_timeout_secs != null && (
          <div>
            <span className="font-medium text-slate-500">请求超时:</span> {server.request_timeout_secs}s
          </div>
        )}
      </div>

      {/* Live info */}
      {server.live && isConnected && (
        <div className="mt-3 rounded-xl border border-emerald-200 bg-emerald-50 p-3 text-sm">
          <div className="font-medium text-emerald-700">连接信息</div>
          {server.live.protocol_version && (
            <div className="mt-1 text-emerald-600">协议版本: {server.live.protocol_version}</div>
          )}
          {server.live.peer_name && (
            <div className="text-emerald-600">Peer: {server.live.peer_name}{server.live.peer_version ? ` v${server.live.peer_version}` : ''}</div>
          )}
        </div>
      )}

      {/* Error info */}
      {server.live?.error && (
        <div className="mt-3 rounded-xl border border-red-200 bg-red-50 p-3 text-sm text-red-600">
          错误: {server.live.error}
        </div>
      )}

      {/* Actions */}
      <div className="mt-4 flex flex-wrap gap-2">
        {isConnected ? (
          <button
            type="button"
            onClick={onDisconnect}
            className="flex items-center gap-1 rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
            data-testid="mcp-disconnect-btn"
          >
            <Cable size={14} />
            断开
          </button>
        ) : (
          <button
            type="button"
            onClick={onConnect}
            className="flex items-center gap-1 rounded-xl bg-emerald-600 px-3 py-1.5 text-sm text-white hover:bg-emerald-700"
            data-testid="mcp-connect-btn"
          >
            <Cable size={14} />
            连接
          </button>
        )}

        <button
          type="button"
          onClick={() => onToggle(!server.enabled)}
          className="flex items-center gap-1 rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          data-testid="mcp-toggle-btn"
        >
          {server.enabled ? <PowerOff size={14} /> : <Power size={14} />}
          {server.enabled ? '禁用' : '启用'}
        </button>

        <button
          type="button"
          onClick={onViewTools}
          className="flex items-center gap-1 rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          data-testid="mcp-view-tools-btn"
        >
          <Wrench size={14} />
          工具
        </button>

        {!confirmRemove ? (
          <button
            type="button"
            onClick={() => setConfirmRemove(true)}
            className="flex items-center gap-1 rounded-xl border border-red-200 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50"
            data-testid="mcp-remove-btn"
          >
            <Trash2 size={14} />
            删除
          </button>
        ) : (
          <div className="flex items-center gap-2" data-testid="mcp-confirm-remove">
            <span className="text-sm text-red-600">确认删除?</span>
            <button
              type="button"
              onClick={() => { onRemove(); setConfirmRemove(false); }}
              className="rounded-xl bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700"
              data-testid="mcp-confirm-remove-yes"
            >
              确认
            </button>
            <button
              type="button"
              onClick={() => setConfirmRemove(false)}
              className="rounded-xl border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
              data-testid="mcp-confirm-remove-no"
            >
              取消
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
