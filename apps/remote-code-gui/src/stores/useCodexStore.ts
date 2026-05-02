import { create } from 'zustand';

export interface CodexState {
  codexNotifications: Array<{
    session_id: string;
    method: string;
    params?: unknown;
    [key: string]: unknown;
  }>;
  codexGuardianEvents: Array<{
    session_id: string;
    method: string;
    outcome: string;
    risk_level?: string;
  }>;
  codexAccountInfo: Record<string, unknown> | null;
  codexRateLimits: Record<string, unknown> | null;
  codexMcpStatus: Array<Record<string, unknown>>;
  codexRecoverableErrors: Array<{
    session_id: string;
    message: string;
    timestamp: number;
  }>;
}

export const useCodexStore = create<CodexState>(() => ({
  codexNotifications: [],
  codexGuardianEvents: [],
  codexAccountInfo: null,
  codexRateLimits: null,
  codexMcpStatus: [],
  codexRecoverableErrors: [],
}));
