import {
  MessageSquare,
  FolderTree,
  Search,
  Plug,
  Settings2,
  Moon,
  Sun,
} from 'lucide-react';
import { useTheme } from '../design/ThemeProvider';

export type ActivityTab = 'chat' | 'files' | 'search' | 'mcp' | 'settings';

interface ActivityBarProps {
  activeTab: ActivityTab;
  onTabChange: (tab: ActivityTab) => void;
}

const tabs: { id: ActivityTab; icon: typeof MessageSquare; label: string }[] = [
  { id: 'chat', icon: MessageSquare, label: '会话' },
  { id: 'files', icon: FolderTree, label: '文件' },
  { id: 'search', icon: Search, label: '搜索' },
  { id: 'mcp', icon: Plug, label: 'MCP' },
];

export function ActivityBar({ activeTab, onTabChange }: ActivityBarProps) {
  const { isDark, toggle } = useTheme();

  return (
    <div className="flex h-full w-activity-bar shrink-0 flex-col items-center border-r border-rc-border-primary bg-rc-bg-primary py-2">
      {/* Top navigation icons */}
      <div className="flex flex-col items-center gap-1">
        {tabs.map(({ id, icon: Icon, label }) => (
          <button
            key={id}
            title={label}
            onClick={() => onTabChange(id)}
            className={`relative flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${
              activeTab === id
                ? 'text-rc-accent-primary bg-rc-bg-hover'
                : 'text-rc-text-tertiary hover:text-rc-text-primary hover:bg-rc-bg-hover'
            }`}
          >
            {activeTab === id && (
              <div className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r bg-rc-accent-primary" />
            )}
            <Icon size={20} />
          </button>
        ))}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Bottom: theme toggle + settings */}
      <div className="flex flex-col items-center gap-1">
        <button
          title={isDark ? '切换到亮色模式' : '切换到暗色模式'}
          onClick={toggle}
          className="flex h-10 w-10 items-center justify-center rounded-lg text-rc-text-tertiary transition-colors hover:text-rc-text-primary hover:bg-rc-bg-hover"
        >
          {isDark ? <Sun size={18} /> : <Moon size={18} />}
        </button>
        <button
          title="设置"
          onClick={() => onTabChange('settings')}
          className={`relative flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${
            activeTab === 'settings'
              ? 'text-rc-accent-primary bg-rc-bg-hover'
              : 'text-rc-text-tertiary hover:text-rc-text-primary hover:bg-rc-bg-hover'
          }`}
        >
          {activeTab === 'settings' && (
            <div className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r bg-rc-accent-primary" />
          )}
          <Settings2 size={18} />
        </button>
      </div>
    </div>
  );
}
