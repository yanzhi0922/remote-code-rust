import {
  MessageSquare,
  Settings2,
  Moon,
  Sun,
} from 'lucide-react';
import { useTheme } from '../design/ThemeProvider';

export type ActivityTab = 'chat' | 'settings';

interface ActivityBarProps {
  activeTab: ActivityTab;
  onTabChange: (tab: ActivityTab) => void;
}

export function ActivityBar({ activeTab, onTabChange }: ActivityBarProps) {
  const { isDark, toggle } = useTheme();

  const btnClass = (active: boolean) =>
    `flex h-9 w-9 items-center justify-center rounded-md transition-colors ${
      active
        ? 'bg-rc-accent-primary text-white'
        : 'text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary'
    }`;

  return (
    <div className="flex w-activity-bar shrink-0 flex-col items-center gap-1 border-r border-rc-border-secondary bg-rc-bg-activity-bar py-2 select-none">
      <div className="flex h-8 w-8 items-center justify-center">
        <svg viewBox="0 0 24 24" className="h-5 w-5 text-rc-accent-primary" fill="currentColor">
          <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
        </svg>
      </div>

      <div className="mt-4 flex flex-col items-center gap-1">
        <button
          title="Chat"
          onClick={() => onTabChange('chat')}
          className={btnClass(activeTab === 'chat')}
        >
          <MessageSquare size={18} />
        </button>
      </div>

      <div className="flex-1" />

      <div className="flex flex-col items-center gap-1">
        <button
          title={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
          onClick={toggle}
          className="flex h-9 w-9 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          {isDark ? <Sun size={16} /> : <Moon size={16} />}
        </button>
        <button
          title="Settings"
          onClick={() => onTabChange('settings')}
          className={btnClass(activeTab === 'settings')}
        >
          <Settings2 size={16} />
        </button>
      </div>
    </div>
  );
}
