import { useCallback, useEffect, useState } from 'react';

export type BottomWorkbenchTab = 'terminal' | 'diff' | 'approvals' | 'logs' | 'artifacts' | 'browser';

export interface WorkbenchLayoutState {
  sidebarCollapsed: boolean;
  inspectorCollapsed: boolean;
  bottomOpen: boolean;
  bottomHeight: number;
  bottomTab: BottomWorkbenchTab;
}

const STORAGE_KEY = 'rc-workbench-layout-v2';
const DEFAULT_STATE: WorkbenchLayoutState = {
  sidebarCollapsed: false,
  inspectorCollapsed: true,
  bottomOpen: false,
  bottomHeight: 320,
  bottomTab: 'terminal',
};

function clampBottomHeight(value: number) {
  return Math.min(520, Math.max(180, value));
}

function readStoredState(): WorkbenchLayoutState {
  if (typeof window === 'undefined') return DEFAULT_STATE;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_STATE;
    const parsed = JSON.parse(raw) as Partial<WorkbenchLayoutState>;
    return {
      ...DEFAULT_STATE,
      ...parsed,
      bottomHeight: clampBottomHeight(Number(parsed.bottomHeight ?? DEFAULT_STATE.bottomHeight)),
      bottomTab: parsed.bottomTab ?? DEFAULT_STATE.bottomTab,
    };
  } catch {
    return DEFAULT_STATE;
  }
}

export function useWorkbenchLayout() {
  const [state, setState] = useState<WorkbenchLayoutState>(() => readStoredState());

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
      // Layout persistence is a convenience only.
    }
  }, [state]);

  const update = useCallback((patch: Partial<WorkbenchLayoutState>) => {
    setState((prev) => ({
      ...prev,
      ...patch,
      bottomHeight: patch.bottomHeight === undefined ? prev.bottomHeight : clampBottomHeight(patch.bottomHeight),
    }));
  }, []);

  const toggleSidebar = useCallback(() => {
    setState((prev) => ({ ...prev, sidebarCollapsed: !prev.sidebarCollapsed }));
  }, []);

  const toggleInspector = useCallback(() => {
    setState((prev) => ({ ...prev, inspectorCollapsed: !prev.inspectorCollapsed }));
  }, []);

  const toggleBottom = useCallback((tab?: BottomWorkbenchTab) => {
    setState((prev) => ({
      ...prev,
      bottomOpen: tab ? true : !prev.bottomOpen,
      bottomTab: tab ?? prev.bottomTab,
    }));
  }, []);

  const openBottomTab = useCallback((tab: BottomWorkbenchTab) => {
    setState((prev) => ({ ...prev, bottomOpen: true, bottomTab: tab }));
  }, []);

  const setBottomHeight = useCallback((height: number) => {
    update({ bottomHeight: height });
  }, [update]);

  return {
    state,
    update,
    toggleSidebar,
    toggleInspector,
    toggleBottom,
    openBottomTab,
    setBottomHeight,
  };
}
