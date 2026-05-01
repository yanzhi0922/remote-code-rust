import { Terminal as TerminalIcon } from 'lucide-react';

interface TerminalPaneProps {
  className?: string;
}

export function TerminalPane({ className = '' }: TerminalPaneProps) {
  return (
    <div className={`flex h-full flex-col bg-rc-bg-code ${className}`}>
      <div className="flex items-center gap-2 border-b border-rc-border-primary px-3 py-1.5">
        <TerminalIcon size={14} className="text-rc-text-inverse" />
        <span className="text-xs font-medium text-rc-text-inverse">Terminal</span>
        <span className="text-2xs text-rc-text-tertiary">集成终端（即将支持 xterm.js）</span>
      </div>
      <div className="flex-1 overflow-y-auto p-3 font-mono text-xs text-rc-text-inverse">
        <div className="text-rc-text-tertiary">
          <p>$ 终端面板已就绪</p>
          <p>将集成 xterm.js 实现完整终端功能</p>
          <p className="mt-2">快捷键: Ctrl+` 切换终端面板</p>
        </div>
      </div>
    </div>
  );
}
