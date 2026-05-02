import React, { useCallback } from 'react';

/**
 * Permission modes inspired by ZCode's Shift+Tab cycling.
 *
 * Allows quick switching between permission levels directly in the chat input area,
 * without navigating to settings.
 */

export type PermissionMode = 'always-ask' | 'accept-edits' | 'plan-mode' | 'bypass';

interface PermissionModeInfo {
  id: PermissionMode;
  label: string;
  icon: string;
  description: string;
  color: string;
}

const PERMISSION_MODES: PermissionModeInfo[] = [
  {
    id: 'always-ask',
    label: 'Always Ask',
    icon: '🔒',
    description: '每个操作都需确认',
    color: 'text-red-400',
  },
  {
    id: 'accept-edits',
    label: 'Accept Edits',
    icon: '✏️',
    description: '自动编辑文件，命令需确认',
    color: 'text-yellow-400',
  },
  {
    id: 'plan-mode',
    label: 'Plan Mode',
    icon: '📋',
    description: '先制定计划再执行',
    color: 'text-blue-400',
  },
  {
    id: 'bypass',
    label: 'Bypass',
    icon: '⚡',
    description: '全自动无确认（仅沙箱）',
    color: 'text-green-400',
  },
];

interface PermissionModeSwitchProps {
  currentMode: PermissionMode;
  onModeChange: (mode: PermissionMode) => void;
  className?: string;
}

/**
 * Permission mode switcher component.
 *
 * Usage:
 * ```tsx
 * <PermissionModeSwitch
 *   currentMode={permissionMode}
 *   onModeChange={setPermissionMode}
 * />
 * ```
 *
 * Keyboard: Shift+Tab cycles through modes.
 */
export const PermissionModeSwitch: React.FC<PermissionModeSwitchProps> = ({
  currentMode,
  onModeChange,
  className = '',
}) => {
  const currentIndex = PERMISSION_MODES.findIndex((m) => m.id === currentMode);

  const cycleMode = useCallback(() => {
    const nextIndex = (currentIndex + 1) % PERMISSION_MODES.length;
    onModeChange(PERMISSION_MODES[nextIndex].id);
  }, [currentIndex, onModeChange]);

  const currentInfo = PERMISSION_MODES[currentIndex];

  return (
    <div className={`flex items-center gap-1 ${className}`}>
      <button
        onClick={cycleMode}
        className={`
          flex items-center gap-1.5 px-2.5 py-1 rounded-md
          text-xs font-medium transition-all duration-150
          bg-[var(--color-surface)] hover:bg-[var(--color-surface-hover)]
          border border-[var(--color-border)]
          ${currentInfo.color}
        `}
        title={`${currentInfo.description}\nShift+Tab 切换`}
      >
        <span>{currentInfo.icon}</span>
        <span>{currentInfo.label}</span>
      </button>
    </div>
  );
};

/**
 * Get the next permission mode (for Shift+Tab handler).
 */
export function getNextPermissionMode(current: PermissionMode): PermissionMode {
  const idx = PERMISSION_MODES.findIndex((m) => m.id === current);
  const nextIdx = (idx + 1) % PERMISSION_MODES.length;
  return PERMISSION_MODES[nextIdx].id;
}

/**
 * Get permission mode display info.
 */
export function getPermissionModeInfo(mode: PermissionMode): PermissionModeInfo {
  return PERMISSION_MODES.find((m) => m.id === mode) ?? PERMISSION_MODES[0];
}

export default PermissionModeSwitch;
