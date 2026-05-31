import { useEffect, useRef, useState, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

export interface ContextMenuItem {
  key: string;
  label: string;
  icon?: React.ReactNode;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  action: () => void;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  x: number;
  y: number;
  onClose: () => void;
}

export function ContextMenu({ items, x, y, onClose }: ContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });

  useEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;

    const rect = menu.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    const adjustedX = x + rect.width > viewportWidth ? Math.max(0, x - rect.width - 4) : x;
    const adjustedY = y + rect.height > viewportHeight ? Math.max(0, y - rect.height - 4) : y;

    if (adjustedX !== x || adjustedY !== y) {
      setPosition({ x: adjustedX, y: adjustedY });
    }
  }, [x, y]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousedown', handleClick);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('mousedown', handleClick);
    };
  }, [onClose]);

  const handleItemClick = useCallback(
    (item: ContextMenuItem) => {
      if (item.disabled) return;
      item.action();
      onClose();
    },
    [onClose],
  );

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-[9999] min-w-[180px] rounded-lg border border-rc-border-primary bg-rc-bg-surface py-1 shadow-lg"
      style={{ left: position.x, top: position.y }}
      role="menu"
      aria-label="Context menu"
    >
      {items.map((item) =>
        item.separator ? (
          <div key={item.key} className="my-1 border-t border-rc-border-secondary" role="separator" />
        ) : (
          <button
            key={item.key}
            type="button"
            role="menuitem"
            disabled={item.disabled}
            onClick={() => handleItemClick(item)}
            className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors ${
              item.disabled
                ? 'cursor-not-allowed text-rc-text-tertiary opacity-50'
                : item.danger
                  ? 'text-rc-accent-error hover:bg-rc-bg-hover'
                  : 'text-rc-text-primary hover:bg-rc-bg-hover'
            }`}
          >
            {item.icon && <span className="shrink-0">{item.icon}</span>}
            <span className="flex-1">{item.label}</span>
          </button>
        ),
      )}
    </div>,
    document.body,
  );
}

/** Hook to manage context menu state for a target element. */
export function useContextMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

  const show = useCallback((e: React.MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, items });
  }, []);

  const hide = useCallback(() => setMenu(null), []);

  const MenuComponent = menu ? (
    <ContextMenu {...menu} onClose={hide} />
  ) : null;

  return { show, hide, MenuComponent };
}
