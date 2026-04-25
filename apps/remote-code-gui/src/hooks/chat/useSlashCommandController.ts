/**
 * 斜杠命令控制器 Hook — 管理斜杠命令的匹配、过滤、键盘导航和执行
 * Slash command controller — manages matching, filtering, keyboard nav and execution
 *
 * Adapted from AionUi useSlashCommandController pattern.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';

/** 匹配 /command 形式的输入 */
const SLASH_QUERY_RE = /^\/([a-zA-Z0-9_-]*)$/;

export function matchSlashQuery(input: string): string | null {
  const match = input.match(SLASH_QUERY_RE);
  return match ? match[1] : null;
}

export interface SlashCommandItem {
  name: string;
  description?: string;
  kind: 'builtin' | 'template';
  selectionBehavior?: 'execute' | 'insert';
}

export interface ActiveItemScrollInput {
  containerScrollTop: number;
  containerHeight: number;
  itemOffsetTop: number;
  itemOffsetHeight: number;
}

/** 计算活动项所需的滚动位置，确保可见 */
export function getScrollTopForActiveItem(input: ActiveItemScrollInput): number {
  const { containerScrollTop, containerHeight, itemOffsetTop, itemOffsetHeight } = input;
  if (containerHeight <= 0) {
    return containerScrollTop;
  }

  const viewportTop = containerScrollTop;
  const viewportBottom = containerScrollTop + containerHeight;
  const itemTop = itemOffsetTop;
  const itemBottom = itemOffsetTop + itemOffsetHeight;

  if (itemTop < viewportTop) {
    return itemTop;
  }
  if (itemBottom > viewportBottom) {
    return itemBottom - containerHeight;
  }
  return containerScrollTop;
}

function getSelectionBehavior(command: SlashCommandItem): 'execute' | 'insert' {
  if (command.selectionBehavior) {
    return command.selectionBehavior;
  }
  return command.kind === 'builtin' ? 'execute' : 'insert';
}

interface UseSlashCommandControllerOptions {
  input: string;
  commands: SlashCommandItem[];
  onExecuteBuiltin?: (name: string) => void;
  onSelectTemplate?: (name: string) => void;
}

export function useSlashCommandController(options: UseSlashCommandControllerOptions) {
  const { input, commands, onExecuteBuiltin, onSelectTemplate } = options;
  const query = useMemo(() => matchSlashQuery(input), [input]);
  const [activeIndex, setActiveIndex] = useState(0);
  const [dismissed, setDismissed] = useState(false);

  // 当 query 变化时重置状态
  useEffect(() => {
    setActiveIndex(0);
    setDismissed(false);
  }, [query]);

  const filteredCommands = useMemo(() => {
    if (query === null) {
      return [];
    }
    const keyword = query.trim().toLowerCase();
    if (!keyword) {
      return commands;
    }
    return commands.filter((command) => command.name.toLowerCase().startsWith(keyword));
  }, [commands, query]);

  const isOpen = query !== null && !dismissed && filteredCommands.length > 0;

  const executeCommand = useCallback(
    (index: number) => {
      const command = filteredCommands[index];
      if (!command) {
        return false;
      }
      if (getSelectionBehavior(command) === 'insert') {
        onSelectTemplate?.(command.name);
      } else if (command.kind === 'builtin') {
        onExecuteBuiltin?.(command.name);
      } else {
        onSelectTemplate?.(command.name);
      }
      setDismissed(true);
      return true;
    },
    [filteredCommands, onExecuteBuiltin, onSelectTemplate],
  );

  const onKeyDown = useCallback(
    (event: ReactKeyboardEvent) => {
      if (!isOpen) {
        return false;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        setDismissed(true);
        return true;
      }

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setActiveIndex((prev) => (prev + 1) % filteredCommands.length);
        return true;
      }

      if (event.key === 'ArrowUp') {
        event.preventDefault();
        setActiveIndex((prev) => (prev - 1 + filteredCommands.length) % filteredCommands.length);
        return true;
      }

      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        return executeCommand(activeIndex);
      }

      return false;
    },
    [activeIndex, executeCommand, filteredCommands.length, isOpen],
  );

  return {
    isOpen,
    activeIndex,
    filteredCommands,
    onKeyDown,
    onSelectByIndex: executeCommand,
    setDismissed,
    setActiveIndex,
  };
}
