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

const tabs: { id: ActivityTab; icon: typeof MessageSquare; label: string }[] = [
  { id: 'chat', icon: MessageSquare, label: '会话' },
];

export function ActivityBar({ activeTab, onTabChange }: ActivityBarProps) {
  const { isDark, toggle } = useTheme();

  return (
    <div className="flex h-full w-[88px] shrink-0 flex-col items-center justify-center px-4 py-5">
      <div className="flex min-h-[520px] w-[64px] flex-col items-center rounded-[32px] border border-white/80 bg-white/90 py-4 shadow-[0_24px_70px_rgba(15,23,42,0.16)] backdrop-blur-xl dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
        <div className="mb-5 flex h-11 w-11 items-center justify-center rounded-[22px] bg-[#111827] shadow-[0_14px_28px_rgba(37,99,235,0.28)]">
          <img src="/brand-mark.svg" alt="" className="h-8 w-8" draggable={false} />
        </div>

        <div className="flex flex-col items-center gap-2">
          {tabs.map(({ id, icon: Icon, label }) => {
            const isActive = activeTab === id;
            return (
              <button
                key={id}
                title={label}
                onClick={() => onTabChange(id)}
                className={`relative flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 active:scale-[0.98] ${
                  isActive
                    ? 'bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] text-white shadow-[0_12px_24px_rgba(37,99,235,0.28)]'
                    : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
                }`}
              >
                <Icon size={20} strokeWidth={isActive ? 2.5 : 2} />
              </button>
            );
          })}
        </div>

        <div className="flex-1" />

        <div className="flex flex-col items-center gap-2">
          <button
            title={isDark ? '切换到亮色模式' : '切换到暗色模式'}
            onClick={toggle}
            className="flex h-11 w-11 items-center justify-center rounded-2xl text-rc-text-secondary transition-all duration-200 hover:bg-rc-bg-hover hover:text-rc-text-primary active:scale-[0.98]"
          >
            {isDark ? <Sun size={18} /> : <Moon size={18} />}
          </button>
          <button
            title="设置"
            onClick={() => onTabChange('settings')}
            className={`relative flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 active:scale-[0.98] ${
              activeTab === 'settings'
                ? 'bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] text-white shadow-[0_12px_24px_rgba(37,99,235,0.28)]'
                : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
            }`}
          >
            <Settings2 size={18} strokeWidth={activeTab === 'settings' ? 2.5 : 2} />
          </button>
        </div>
      </div>
    </div>
  );
}
