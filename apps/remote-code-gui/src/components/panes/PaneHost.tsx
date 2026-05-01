import { X } from 'lucide-react';
import type { ReactNode } from 'react';

export interface PaneConfig {
  id: string;
  title: string;
  icon: ReactNode;
  content: ReactNode;
  closable?: boolean;
}

interface PaneHostProps {
  panes: PaneConfig[];
  onClose?: (paneId: string) => void;
}

export function PaneHost({ panes, onClose }: PaneHostProps) {
  if (panes.length === 0) return null;

  return (
    <div className="flex h-full flex-col border-t border-rc-border-primary bg-rc-bg-primary">
      {/* Tab bar */}
      <div className="flex h-9 shrink-0 items-center border-b border-rc-border-primary bg-rc-bg-secondary px-2">
        {panes.map((pane) => (
          <div
            key={pane.id}
            className="group flex items-center gap-1.5 rounded-md px-3 py-1 text-xs font-medium text-rc-text-primary hover:bg-rc-bg-hover"
          >
            <span className="text-rc-text-secondary">{pane.icon}</span>
            <span>{pane.title}</span>
            {pane.closable !== false && onClose && (
              <button
                title={`关闭 ${pane.title}`}
                onClick={() => onClose(pane.id)}
                className="ml-1 flex h-4 w-4 items-center justify-center rounded text-rc-text-tertiary opacity-0 transition-opacity group-hover:opacity-100 hover:bg-rc-bg-active hover:text-rc-text-primary"
              >
                <X size={10} />
              </button>
            )}
          </div>
        ))}
      </div>

      {/* Pane content */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {panes.length > 0 && panes[panes.length - 1].content}
      </div>
    </div>
  );
}
