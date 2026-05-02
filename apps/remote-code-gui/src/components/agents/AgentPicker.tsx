import React, { useState, useRef, useEffect, useCallback } from 'react';

/**
 * Agent Picker component inspired by ZCode's @mention system.
 *
 * Allows users to invoke specialized agents by typing @ in the chat input.
 * Shows a dropdown with available agents (built-in + custom).
 */

interface AgentInfo {
  name: string;
  description: string;
  scope: 'built-in' | 'user' | 'project';
  readOnly: boolean;
}

interface AgentPickerProps {
  /** Filter text (what the user typed after @) */
  filter: string;
  /** Position in the input (for dropdown placement) */
  position: { top: number; left: number };
  /** Called when an agent is selected */
  onSelect: (agent: AgentInfo) => void;
  /** Called when the picker is dismissed */
  onDismiss: () => void;
  /** Optional list of agents (will use defaults if not provided) */
  agents?: AgentInfo[];
}

const DEFAULT_AGENTS: AgentInfo[] = [
  {
    name: 'code-reviewer',
    description: '代码审查专家。审查 PR、检测安全漏洞、性能问题。',
    scope: 'built-in',
    readOnly: true,
  },
  {
    name: 'bug-analyzer',
    description: 'Bug 分析专家。深度分析代码执行流、定位根因。',
    scope: 'built-in',
    readOnly: false,
  },
  {
    name: 'dev-planner',
    description: '开发规划专家。将需求拆解为可执行的任务。',
    scope: 'built-in',
    readOnly: true,
  },
  {
    name: 'architect',
    description: '架构设计专家。分析系统架构、设计模块划分。',
    scope: 'built-in',
    readOnly: true,
  },
  {
    name: 'test-writer',
    description: '测试生成专家。为代码生成单元测试和集成测试。',
    scope: 'built-in',
    readOnly: false,
  },
];

const SCOPE_ICONS: Record<string, string> = {
  'built-in': '⚡',
  'user': '👤',
  'project': '📁',
};

export const AgentPicker: React.FC<AgentPickerProps> = ({
  filter,
  position,
  onSelect,
  onDismiss,
  agents = DEFAULT_AGENTS,
}) => {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const filteredAgents = agents.filter(
    (agent) =>
      agent.name.toLowerCase().includes(filter.toLowerCase()) ||
      agent.description.toLowerCase().includes(filter.toLowerCase())
  );

  // Reset selected index when filter changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [filter]);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const selected = list.children[selectedIndex] as HTMLElement;
    if (selected) {
      selected.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex((prev) => Math.min(prev + 1, filteredAgents.length - 1));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex((prev) => Math.max(prev - 1, 0));
          break;
        case 'Enter':
          e.preventDefault();
          if (filteredAgents[selectedIndex]) {
            onSelect(filteredAgents[selectedIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          onDismiss();
          break;
      }
    },
    [filteredAgents, selectedIndex, onSelect, onDismiss]
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  if (filteredAgents.length === 0) {
    return (
      <div
        className="absolute z-50 w-72 rounded-lg border border-[var(--color-border)]
          bg-[var(--color-surface)] shadow-lg p-3"
        style={{ top: position.top, left: position.left }}
      >
        <p className="text-xs text-[var(--color-text-muted)]">
          No agents matching "{filter}"
        </p>
      </div>
    );
  }

  return (
    <div
      className="absolute z-50 w-80 max-h-64 overflow-hidden rounded-lg
        border border-[var(--color-border)] bg-[var(--color-surface)] shadow-xl"
      style={{ top: position.top, left: position.left }}
    >
      {/* Header */}
      <div className="px-3 py-2 border-b border-[var(--color-border)]">
        <p className="text-[10px] text-[var(--color-text-muted)] uppercase tracking-wide">
          Specialized Agents
        </p>
      </div>

      {/* Agent list */}
      <div ref={listRef} className="overflow-y-auto max-h-52">
        {filteredAgents.map((agent, index) => (
          <div
            key={agent.name}
            onClick={() => onSelect(agent)}
            className={`
              flex items-start gap-2 px-3 py-2 cursor-pointer transition-colors
              ${index === selectedIndex
                ? 'bg-[var(--color-accent)] bg-opacity-10'
                : 'hover:bg-[var(--color-surface-hover)]'
              }
            `}
          >
            {/* Icon */}
            <div className={`
              flex-shrink-0 w-8 h-8 rounded-lg flex items-center justify-center text-sm
              ${agent.readOnly
                ? 'bg-blue-500 bg-opacity-20 text-blue-400'
                : 'bg-green-500 bg-opacity-20 text-green-400'
              }
            `}>
              {SCOPE_ICONS[agent.scope] || '🤖'}
            </div>

            {/* Info */}
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5">
                <span className="text-xs font-semibold text-[var(--color-text)]">
                  @{agent.name}
                </span>
                {agent.readOnly && (
                  <span className="text-[9px] px-1 py-0.5 rounded bg-blue-500 bg-opacity-20 text-blue-400">
                    read-only
                  </span>
                )}
              </div>
              <p className="text-[10px] text-[var(--color-text-muted)] mt-0.5 line-clamp-2">
                {agent.description}
              </p>
            </div>
          </div>
        ))}
      </div>

      {/* Footer hint */}
      <div className="px-3 py-1.5 border-t border-[var(--color-border)] text-[10px] text-[var(--color-text-muted)]">
        ↑↓ navigate · Enter select · Esc dismiss
      </div>
    </div>
  );
};

/**
 * Hook to detect @mentions in text and show the agent picker.
 */
export function useAgentMention(
  agents?: AgentInfo[]
): {
  showPicker: boolean;
  pickerFilter: string;
  pickerPosition: { top: number; left: number };
  handleInputChange: (value: string, cursorPosition: number) => void;
  handleAgentSelect: (agent: AgentInfo) => void;
  dismissPicker: () => void;
} {
  const [showPicker, setShowPicker] = useState(false);
  const [pickerFilter, setPickerFilter] = useState('');
  const [pickerPosition, setPickerPosition] = useState({ top: 0, left: 0 });

  const handleInputChange = useCallback((value: string, cursorPosition: number) => {
    // Check if cursor is right after an @mention
    const textBeforeCursor = value.substring(0, cursorPosition);
    const atIndex = textBeforeCursor.lastIndexOf('@');

    if (atIndex >= 0) {
      const afterAt = textBeforeCursor.substring(atIndex + 1);
      // Only show picker if there's no space in the mention (still typing agent name)
      if (!afterAt.includes(' ') && (atIndex === 0 || textBeforeCursor[atIndex - 1] === ' ')) {
        setPickerFilter(afterAt);
        setShowPicker(true);
        // Position would be calculated based on textarea position
        setPickerPosition({ top: -200, left: 0 });
        return;
      }
    }

    setShowPicker(false);
  }, []);

  const handleAgentSelect = useCallback((agent: AgentInfo) => {
    setShowPicker(false);
    setPickerFilter('');
    // The parent component should handle inserting the @mention
  }, []);

  const dismissPicker = useCallback(() => {
    setShowPicker(false);
    setPickerFilter('');
  }, []);

  return {
    showPicker,
    pickerFilter,
    pickerPosition,
    handleInputChange,
    handleAgentSelect,
    dismissPicker,
  };
}

export default AgentPicker;
