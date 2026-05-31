import { useEffect, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Clock, Folder } from 'lucide-react';
import type { SessionSummary } from '../../lib/types';
import { truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';

interface SessionSwitcherProps {
  sessions: SessionSummary[];
  activeSessionId: string | null;
}

function formatTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return 'now';
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  return `${Math.floor(hrs / 24)}d`;
}

export function SessionSwitcher({ sessions, activeSessionId }: SessionSwitcherProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const ctrlHeld = useRef(false);
  const selectSession = useAppStore((state) => state.selectSession);

  const sorted = [...sessions].sort(
    (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey) {
        if (ctrlHeld.current) {
          ctrlHeld.current = false;
          if (open) {
            const session = sorted[selectedIndex];
            if (session) void selectSession(session.id);
            setOpen(false);
          }
        }
        return;
      }

      if (e.key === 'Tab') {
        e.preventDefault();
        e.stopPropagation();

        if (!ctrlHeld.current) {
          ctrlHeld.current = true;
          setOpen(true);
          setSelectedIndex(activeSessionId ? sorted.findIndex((s) => s.id === activeSessionId) + 1 : 0);
        } else {
          setSelectedIndex((prev) => (prev + 1) % Math.max(sorted.length, 1));
        }
      }
    },
    [sorted, selectedIndex, activeSessionId, open, selectSession],
  );

  const handleKeyUp = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Control' || e.key === 'Meta') {
        ctrlHeld.current = false;
        if (open) {
          const session = sorted[selectedIndex];
          if (session) void selectSession(session.id);
          setOpen(false);
        }
      }
    },
    [open, sorted, selectedIndex, selectSession],
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);
    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [handleKeyDown, handleKeyUp]);

  if (!open || sorted.length === 0) return null;

  const visibleCount = Math.min(sorted.length, 8);
  const startIdx = Math.max(0, Math.min(selectedIndex - 2, sorted.length - visibleCount));
  const visible = sorted.slice(startIdx, startIdx + visibleCount);

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
      <div className="fixed inset-0 bg-black/40 backdrop-blur-sm" />
      <div className="relative w-[420px] overflow-hidden rounded-xl border border-rc-border-primary bg-rc-bg-surface shadow-2xl animate-fade-in-up">
        <div className="border-b border-rc-border-secondary px-4 py-2.5 text-xs font-medium text-rc-text-tertiary">
          {t('sessionSwitcher.title')}
          <span className="ml-2 rounded bg-rc-bg-code px-1.5 py-0.5 text-[10px]">Ctrl+Tab</span>
        </div>
        <div className="max-h-[360px] overflow-y-auto p-2">
          {visible.map((session, idx) => {
            const globalIdx = startIdx + idx;
            const isActive = session.id === activeSessionId;
            const isSelected = globalIdx === selectedIndex;
            return (
              <button
                key={session.id}
                type="button"
                onClick={() => {
                  void selectSession(session.id);
                  setOpen(false);
                  ctrlHeld.current = false;
                }}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors ${
                  isSelected
                    ? 'bg-rc-bg-selected border border-rc-accent-primary/30'
                    : isActive
                      ? 'bg-rc-bg-hover border border-transparent'
                      : 'border border-transparent hover:bg-rc-bg-hover'
                }`}
              >
                <div className="min-w-0 flex-1">
                  <div className={`truncate text-sm ${isSelected ? 'font-semibold text-rc-text-primary' : 'text-rc-text-primary'}`}>
                    {truncateMiddle(session.title, 48)}
                  </div>
                  <div className="mt-0.5 flex items-center gap-2 text-xs text-rc-text-tertiary">
                    <Folder size={10} />
                    <span className="truncate">{truncateMiddle(session.cwd.split(/[\\/]/).pop() ?? session.cwd, 20)}</span>
                    <span>·</span>
                    <Clock size={10} />
                    <span>{formatTime(session.updated_at)}</span>
                  </div>
                </div>
                {isActive && <span className="h-2 w-2 rounded-full bg-rc-accent-primary" />}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
