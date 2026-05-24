import {
  Blocks,
  MessageSquare,
  Settings2,
  Moon,
  Search,
  Sun,
} from 'lucide-react';
import { useTheme } from '../design/ThemeProvider';

export type ActivityTab = 'chat' | 'mcp' | 'settings';

interface ActivityBarProps {
  activeTab: ActivityTab;
  onTabChange: (tab: ActivityTab) => void;
}

export function ActivityBar({ activeTab, onTabChange }: ActivityBarProps) {
  const { isDark, toggle } = useTheme();

  const btnClass = (active: boolean) =>
    `relative flex h-9 w-9 items-center justify-center rounded-md transition-colors focus-visible:outline-none ${
      active
        ? 'bg-rc-bg-active text-rc-text-primary before:absolute before:left-0 before:top-1.5 before:h-6 before:w-0.5 before:rounded-full before:bg-rc-accent-primary'
        : 'text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary'
    }`;

  return (
    <nav
      aria-label="Workbench activity bar"
      className="flex w-activity-bar shrink-0 flex-col items-center gap-1 border-r border-rc-border-secondary glass py-2 select-none"
    >
      <div className="flex h-8 w-8 items-center justify-center" title="Remote Code">
        <svg viewBox="0 0 24 24" className="h-5 w-5 text-rc-accent-info" fill="none" aria-hidden="true">
          <path d="M12 3 4 7.5 12 12l8-4.5L12 3Z" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          <path d="M4 12.5 12 17l8-4.5M4 17.5 12 22l8-4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </div>

      <div className="mt-4 flex flex-col items-center gap-1">
        <button
          aria-label="Explorer"
          title="Explorer"
          onClick={() => onTabChange('chat')}
          className={btnClass(activeTab === 'chat')}
        >
          <MessageSquare size={18} />
        </button>
        <button
          aria-label="Search"
          title="Search"
          onClick={() => onTabChange('chat')}
          className={btnClass(false)}
        >
          <Search size={17} />
        </button>
        <button
          aria-label="MCP"
          title="MCP"
          onClick={() => onTabChange('mcp')}
          className={btnClass(activeTab === 'mcp')}
        >
          <Blocks size={17} />
        </button>
      </div>

      <div className="flex-1" />

      <div className="flex flex-col items-center gap-1">
        <button
          aria-label={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
          title={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
          onClick={toggle}
          className="flex h-9 w-9 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          {isDark ? <Sun size={16} /> : <Moon size={16} />}
        </button>
        <button
          aria-label="Settings"
          title="Settings"
          onClick={() => onTabChange('settings')}
          className={btnClass(activeTab === 'settings')}
        >
          <Settings2 size={16} />
        </button>
      </div>
    </nav>
  );
}
