import {
  Blocks,
  MessageSquare,
  Settings2,
  Moon,
  Sun,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '../design/ThemeProvider';

export type ActivityTab = 'chat' | 'mcp' | 'settings';

interface ActivityBarProps {
  activeTab: ActivityTab;
  onTabChange: (tab: ActivityTab) => void;
}

export function ActivityBar({ activeTab, onTabChange }: ActivityBarProps) {
  const { t } = useTranslation();
  const { isDark, toggle } = useTheme();
  const [open, setOpen] = useState(false);

  const btnClass = (active: boolean) =>
    `relative flex h-7 w-7 items-center justify-center rounded-md transition-all focus-visible:outline-none ${
      active
        ? 'bg-rc-text-primary text-rc-text-inverse'
        : 'text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary'
    }`;

  return (
    <nav
      aria-label="Workbench activity bar"
      className="pointer-events-auto absolute left-5 top-5 z-40 flex h-10 items-center gap-0.5 rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-1 shadow-xs select-none"
    >
      <button
        type="button"
        aria-expanded={open}
        aria-label="Remote Code"
        onClick={() => setOpen((value) => !value)}
        className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        title="Remote Code"
      >
        <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" aria-hidden="true">
          <path d="M12 3 4 7.5 12 12l8-4.5L12 3Z" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          <path d="M4 12.5 12 17l8-4.5M4 17.5 12 22l8-4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <>
          <div role="tablist" aria-orientation="horizontal" className="flex items-center gap-1 animate-fade-in">
            <button
              role="tab"
              aria-selected={activeTab === 'chat'}
              aria-label={t('activityBar.explorer')}
              title={t('activityBar.explorer')}
              onClick={() => onTabChange('chat')}
              className={btnClass(activeTab === 'chat')}
            >
              <MessageSquare size={18} />
            </button>
            <button
              role="tab"
              aria-selected={activeTab === 'mcp'}
              aria-label={t('activityBar.mcp')}
              title={t('activityBar.mcp')}
              onClick={() => onTabChange('mcp')}
              className={btnClass(activeTab === 'mcp')}
            >
              <Blocks size={17} />
            </button>
          </div>

          <div className="ml-1 h-4 w-px bg-rc-border-secondary/80" />

          <div className="flex items-center gap-1 animate-fade-in">
            <button
              aria-label={isDark ? t('activityBar.switchLight') : t('activityBar.switchDark')}
              title={isDark ? t('activityBar.switchLight') : t('activityBar.switchDark')}
              onClick={toggle}
              className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-tertiary transition-all hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              {isDark ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <button
              role="tab"
              aria-selected={activeTab === 'settings'}
              aria-label={t('activityBar.settings')}
              title={t('activityBar.settings')}
              onClick={() => onTabChange('settings')}
              className={btnClass(activeTab === 'settings')}
            >
              <Settings2 size={16} />
            </button>
          </div>
        </>
      )}
    </nav>
  );
}
