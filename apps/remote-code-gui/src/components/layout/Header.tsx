import { FolderTree, MessageSquareText, ShieldCheck } from 'lucide-react';
import { useMemo } from 'react';
import { normalizePathKey, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';

export function Header() {
  const provider = useAppStore((state) => state.provider);
  const settings = useAppStore((state) => state.settings);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const lastPromptResult = useAppStore((state) => state.lastPromptResult);

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  const activeProject = useMemo(
    () =>
      (activeSession &&
        projects.find((project) => normalizePathKey(project.path) === normalizePathKey(activeSession.cwd))) ??
      projects.find(
        (project) =>
          activeProjectPath && normalizePathKey(project.path) === normalizePathKey(activeProjectPath),
      ) ??
      null,
    [activeProjectPath, activeSession, projects],
  );

  return (
    <header className="border-b border-[#ebe6dd] bg-white/90 px-4 py-3 backdrop-blur sm:px-6">
      <div className="mx-auto flex w-full max-w-6xl flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate text-lg font-semibold text-slate-800">
            {activeSession?.title || 'Remote Code GUI'}
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-3 text-sm text-slate-500">
            <span className="inline-flex items-center gap-1.5">
              <MessageSquareText size={14} />
              {activeSession ? '当前会话' : '未选择会话'}
            </span>
            {activeProject && (
              <span className="inline-flex items-center gap-1.5">
                <FolderTree size={14} />
                {activeProject.name}
              </span>
            )}
            {activeSession && !activeProject && (
              <span className="inline-flex items-center gap-1.5">
                <FolderTree size={14} />
                {truncateMiddle(activeSession.cwd, 48)}
              </span>
            )}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 text-sm">
          <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-slate-600">
            {activeSession
              ? `${activeSession.provider_name}${activeSession.model ? ` / ${activeSession.model}` : ''}`
              : provider
                ? `${provider.name}${provider.model ? ` / ${provider.model}` : ''}`
                : '未连接'}
          </div>
          <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-slate-600">
            <span className="inline-flex items-center gap-1.5">
              <ShieldCheck size={14} />
              {settings?.permission_mode ?? 'default'}
            </span>
          </div>
          {lastPromptResult && (
            <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 font-mono text-xs text-slate-600">
              in {lastPromptResult.usage.input_tokens} / out {lastPromptResult.usage.output_tokens}
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
