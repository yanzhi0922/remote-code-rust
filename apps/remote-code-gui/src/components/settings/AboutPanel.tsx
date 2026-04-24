import { ExternalLink, Info } from 'lucide-react';

declare const __APP_BUILD_ID__: string;

export function AboutPanel() {
  return (
    <div className="space-y-6" data-testid="about-panel">
      <h3 className="text-lg font-semibold text-slate-800">关于</h3>

      <div className="rounded-xl border border-slate-200 bg-white p-4">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-blue-100">
            <Info size={20} className="text-blue-600" />
          </div>
          <div>
            <div className="text-base font-semibold text-slate-800">Remote Code</div>
            <div className="text-xs text-slate-500">AI 驱动的远程编码助手</div>
          </div>
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between rounded-xl border border-slate-200 bg-white px-4 py-3">
          <span className="text-sm text-slate-600">版本</span>
          <span className="text-sm font-medium text-slate-800">0.1.0</span>
        </div>

        <div className="flex items-center justify-between rounded-xl border border-slate-200 bg-white px-4 py-3">
          <span className="text-sm text-slate-600">构建 ID</span>
          <span className="text-sm font-mono text-slate-800">{__APP_BUILD_ID__}</span>
        </div>

        <div className="flex items-center justify-between rounded-xl border border-slate-200 bg-white px-4 py-3">
          <span className="text-sm text-slate-600">许可证</span>
          <span className="text-sm font-medium text-slate-800">MIT</span>
        </div>
      </div>

      <div className="space-y-2">
        <h4 className="text-sm font-medium text-slate-700">项目链接</h4>
        <a
          href="https://github.com"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-blue-600 hover:bg-slate-50"
          data-testid="link-github"
        >
          <ExternalLink size={14} />
          GitHub 仓库
        </a>
        <a
          href="https://github.com"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-blue-600 hover:bg-slate-50"
          data-testid="link-docs"
        >
          <ExternalLink size={14} />
          文档
        </a>
      </div>

      <div className="space-y-2">
        <h4 className="text-sm font-medium text-slate-700">系统信息</h4>
        <div className="rounded-xl border border-slate-200 bg-white p-4">
          <div className="grid grid-cols-2 gap-2 text-sm">
            <span className="text-slate-500">运行时</span>
            <span className="text-slate-800">Tauri (Rust)</span>
            <span className="text-slate-500">前端</span>
            <span className="text-slate-800">React + TypeScript</span>
            <span className="text-slate-500">UI 框架</span>
            <span className="text-slate-800">Tailwind CSS</span>
          </div>
        </div>
      </div>
    </div>
  );
}
