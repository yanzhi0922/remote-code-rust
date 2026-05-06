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
    <div className="flex h-full w-activity-bar shrink-0 flex-col items-center border-r border-rc-border-primary bg-rc-bg-activity-bar py-3">
      {/* Logo / Brand mark */}
      <div className="mb-4 flex h-10 w-10 items-center justify-center">
        <div className="h-8 w-8 rounded-xl bg-gradient-to-br from-rc-accent-primary to-purple-500 flex items-center justify-center shadow-lg">
          <span className="text-white text-xs font-bold">RC</span>
        </div>
      </div>

      {/* Navigation icons */}
      <div className="flex flex-col items-center gap-1.5">
        {tabs.map(({ id, icon: Icon, label }) => {
          const isActive = activeTab === id;
          return (
            <button
              key={id}
              title={label}
              onClick={() => onTabChange(id)}
              className={`relative flex h-11 w-11 items-center justify-center rounded-xl transition-all duration-200 ${
                isActive
                  ? 'bg-rc-accent-primary text-white shadow-md'
                  : 'text-rc-text-tertiary hover:text-rc-text-primary hover:bg-rc-bg-hover'
              }`}
            >
              <Icon size={20} strokeWidth={isActive ? 2.5 : 2} />
            </button>
          );
        })}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Bottom actions */}
      <div className="flex flex-col items-center gap-1.5">
        <button
          title={isDark ? '切换到亮色模式' : '切换到暗色模式'}
          onClick={toggle}
          className="flex h-11 w-11 items-center justify-center rounded-xl text-rc-text-tertiary transition-all duration-200 hover:text-rc-text-primary hover:bg-rc-bg-hover"
        >
          {isDark ? <Sun size={18} /> : <Moon size={18} />}
        </button>
        <button
          title="设置"
          onClick={() => onTabChange('settings')}
          className={`relative flex h-11 w-11 items-center justify-center rounded-xl transition-all duration-200 ${
            activeTab === 'settings'
              ? 'bg-rc-accent-primary text-white shadow-md'
              : 'text-rc-text-tertiary hover:text-rc-text-primary hover:bg-rc-bg-hover'
          }`}
        >
          <Settings2 size={18} strokeWidth={activeTab === 'settings' ? 2.5 : 2} />
        </button>
      </div>
    </div>
  );
}