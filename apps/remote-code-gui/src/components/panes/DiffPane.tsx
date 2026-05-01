import { FileText } from 'lucide-react';

interface DiffPaneProps {
  oldContent?: string;
  newContent?: string;
  fileName?: string;
  className?: string;
}

export function DiffPane({ oldContent, newContent, fileName, className = '' }: DiffPaneProps) {
  return (
    <div className={`flex h-full flex-col bg-rc-bg-primary ${className}`}>
      <div className="flex items-center gap-2 border-b border-rc-border-primary px-3 py-1.5">
        <FileText size={14} className="text-rc-text-secondary" />
        <span className="text-xs font-medium text-rc-text-primary">Diff</span>
        {fileName && <span className="text-2xs text-rc-text-tertiary">{fileName}</span>}
      </div>
      <div className="flex-1 overflow-y-auto p-3 font-mono text-xs">
        {oldContent === undefined && newContent === undefined ? (
          <div className="text-rc-text-tertiary">
            <p>Diff 面板已就绪</p>
            <p className="mt-1">当 Agent 执行文件修改时，将自动显示变更内容</p>
          </div>
        ) : (
          <div className="space-y-0.5">
            {oldContent && (
              <div className="rounded bg-rc-accent-error-bg/50 px-2 py-0.5 text-rc-accent-error">
                - {oldContent}
              </div>
            )}
            {newContent && (
              <div className="rounded bg-rc-accent-success-bg/50 px-2 py-0.5 text-rc-accent-success">
                + {newContent}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
