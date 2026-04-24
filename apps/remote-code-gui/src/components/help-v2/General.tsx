import { Info, Keyboard, ExternalLink } from 'lucide-react';

export interface GeneralProps {
  version?: string;
}

export function General({ version = '1.0.0' }: GeneralProps) {
  return (
    <div data-testid="general-help" className="space-y-4 p-4">
      <div className="flex items-center gap-2">
        <Info className="h-5 w-5 text-blue-500" />
        <h2 className="text-lg font-semibold text-slate-800">Remote Code GUI</h2>
      </div>
      <p className="text-sm text-slate-600">
        版本 {version}
      </p>
      <div className="space-y-2">
        <h3 className="flex items-center gap-1.5 text-sm font-medium text-slate-700">
          <Keyboard className="h-4 w-4" />
          常用快捷键
        </h3>
        <div className="space-y-1 text-sm text-slate-600">
          <div className="flex justify-between">
            <span>发送消息</span>
            <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs">Enter</kbd>
          </div>
          <div className="flex justify-between">
            <span>换行</span>
            <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs">Shift+Enter</kbd>
          </div>
          <div className="flex justify-between">
            <span>取消</span>
            <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs">Escape</kbd>
          </div>
          <div className="flex justify-between">
            <span>帮助</span>
            <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs">F1</kbd>
          </div>
        </div>
      </div>
      <a
        href="https://github.com/example/remote-code"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800"
      >
        <ExternalLink className="h-3.5 w-3.5" />
        查看文档
      </a>
    </div>
  );
}
