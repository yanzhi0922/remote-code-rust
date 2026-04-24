import { Check, RefreshCw } from 'lucide-react';
import { useEffect, useState } from 'react';

interface McpReconnectProps {
  serverName: string;
  reconnecting: boolean;
  onReconnect: () => void;
  error?: string | null;
}

type ReconnectPhase = 'reconnecting' | 'success' | 'error';

export function McpReconnect({ serverName, reconnecting, onReconnect, error }: McpReconnectProps) {
  const [phase, setPhase] = useState<ReconnectPhase>(reconnecting ? 'reconnecting' : 'error');
  const [fadeout, setFadeout] = useState(false);

  useEffect(() => {
    if (reconnecting) {
      setPhase('reconnecting');
      setFadeout(false);
    } else if (error) {
      setPhase('error');
      setFadeout(false);
    } else if (phase === 'reconnecting') {
      setPhase('success');
      const timer = setTimeout(() => setFadeout(true), 1500);
      return () => clearTimeout(timer);
    }
  }, [reconnecting, error, phase]);

  if (phase === 'success' && fadeout) {
    return null;
  }

  return (
    <div className="rounded-2xl border border-slate-200 bg-white p-4" data-testid="mcp-reconnect">
      {phase === 'reconnecting' && (
        <div className="flex items-center gap-3">
          <RefreshCw size={16} className="animate-spin text-emerald-600" />
          <span className="text-sm text-slate-700">
            正在重连 <span className="font-medium">{serverName}</span>...
          </span>
        </div>
      )}

      {phase === 'success' && (
        <div className="flex items-center gap-3">
          <div className="flex h-6 w-6 items-center justify-center rounded-full bg-emerald-100">
            <Check size={14} className="text-emerald-600" />
          </div>
          <span className="text-sm text-emerald-700">
            <span className="font-medium">{serverName}</span> 重连成功
          </span>
        </div>
      )}

      {phase === 'error' && (
        <div className="flex flex-col gap-2">
          {error && (
            <div className="text-sm text-red-600">{error}</div>
          )}
          <button
            type="button"
            onClick={onReconnect}
            className="flex items-center gap-1 rounded-xl bg-emerald-600 px-3 py-1.5 text-sm text-white hover:bg-emerald-700"
            data-testid="mcp-reconnect-retry"
          >
            <RefreshCw size={14} />
            重试
          </button>
        </div>
      )}
    </div>
  );
}
