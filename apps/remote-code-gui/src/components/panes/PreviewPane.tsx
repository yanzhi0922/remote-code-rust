import { Eye } from 'lucide-react';

interface PreviewPaneProps {
  content?: string;
  url?: string;
  className?: string;
}

export function PreviewPane({ content, url, className = '' }: PreviewPaneProps) {
  return (
    <div className={`flex h-full flex-col bg-rc-bg-primary ${className}`}>
      <div className="flex items-center gap-2 border-b border-rc-border-primary px-3 py-1.5">
        <Eye size={14} className="text-rc-text-secondary" />
        <span className="text-xs font-medium text-rc-text-primary">Preview</span>
        {url && <span className="text-2xs text-rc-text-tertiary truncate">{url}</span>}
      </div>
      <div className="flex-1 overflow-y-auto p-3">
        {content ? (
          <div
            className="prose prose-sm max-w-none text-rc-text-primary"
            dangerouslySetInnerHTML={{ __html: content }}
          />
        ) : (
          <div className="text-rc-text-tertiary text-sm">
            <p>预览面板已就绪</p>
            <p className="mt-1">支持 HTML 和 Markdown 内容预览</p>
          </div>
        )}
      </div>
    </div>
  );
}
