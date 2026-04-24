import { Monitor, Wifi, WifiOff } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface IdeStatusIndicatorProps {
  status: 'connected' | 'disconnected' | null;
  filePath?: string;
  selectedLineCount?: number;
}

export function IdeStatusIndicator({ status, filePath, selectedLineCount }: IdeStatusIndicatorProps) {
  if (status === null) return null;

  const isConnected = status === 'connected';
  const showSelection = isConnected && selectedLineCount && selectedLineCount > 0;
  const showFile = isConnected && filePath && !showSelection;

  return (
    <div data-testid="ide-status-indicator" className="inline-flex items-center gap-1.5 text-xs">
      {isConnected ? (
        <Wifi className="h-3.5 w-3.5 text-green-500" />
      ) : (
        <WifiOff className="h-3.5 w-3.5 text-slate-400" />
      )}
      <span className={cn('font-medium', isConnected ? 'text-green-600' : 'text-slate-400')}>
        {isConnected ? 'IDE已连接' : 'IDE未连接'}
      </span>
      {showSelection && (
        <span className="text-slate-500">
          ⧉ {selectedLineCount} {selectedLineCount === 1 ? '行' : '行'} 已选中
        </span>
      )}
      {showFile && (
        <span className="flex items-center gap-1 text-slate-500">
          <Monitor className="h-3 w-3" />
          {filePath.split(/[/\\]/).pop()}
        </span>
      )}
    </div>
  );
}
