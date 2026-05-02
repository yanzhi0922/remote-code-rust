import { create } from 'zustand';
import type { AgentTypeInfo, AgentType, SessionSubtask } from '../lib/types';
import * as tauri from '../lib/tauri';

function sortTasks(tasks: SessionSubtask[]): SessionSubtask[] {
  return [...tasks].sort((left, right) => {
    const leftUpdated = left.updated_at ?? '';
    const rightUpdated = right.updated_at ?? '';
    return rightUpdated.localeCompare(leftUpdated);
  });
}

interface AgentState {
  availableAgents: AgentTypeInfo[];
  activeAgentType: AgentType | null;
  agentStatuses: Record<string, string>;
  sessionTasks: Record<string, SessionSubtask[]>;

  loadAgents: () => Promise<void>;
  selectAgent: (agentType: AgentType | null) => void;
  refreshSessionTasks: (sessionId: string) => Promise<void>;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  availableAgents: [],
  activeAgentType: null,
  agentStatuses: {},
  sessionTasks: {},

  loadAgents: async () => {
    try {
      const availableAgents = await tauri.listAvailableAgents();
      set({ availableAgents });
    } catch {
      // Ignore non-fatal agent list refresh failures.
    }
  },

  selectAgent: (agentType: AgentType | null) => {
    set({ activeAgentType: agentType });
  },

  refreshSessionTasks: async (sessionId: string) => {
    try {
      const tasks = await tauri.getSessionTasks(sessionId);
      set((state) => ({
        sessionTasks: {
          ...state.sessionTasks,
          [sessionId]: sortTasks(tasks),
        },
      }));
    } catch {
      // Ignore task refresh failures.
    }
  },
}));
