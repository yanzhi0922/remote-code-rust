import { FolderTree, MessageSquareText, ShieldCheck } from 'lucide-react';
import { useMemo } from 'react';
import { normalizePathKey, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';

export function Header() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const settings = useAppStore((state) => state.settings);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const lastPromptResult = useAppStore((state) => state.lastPromptResult);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const contextOverflowBySession = useAppStore((state) => state.contextOverflowBySession);
  const contextCompactionBySession = useAppStore((state) => state.contextCompactionBySession);

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

  const activeContextUsage = activeSessionId ? contextUsageBySession[activeSessionId] ?? null : null;
  const activeContextOverflow = activeSessionId ? contextOverflowBySession[activeSessionId] ?? null : null;
  const activeContextCompaction = activeSessionId
    ? contextCompactionBySession[activeSessionId] ?? null
    : null;

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
              : runtimeStatus
                ? `${runtimeStatus.provider.name}${runtimeStatus.provider.model ? ` / ${runtimeStatus.provider.model}` : ''}`
                : provider
                  ? `${provider.name}${provider.model ? ` / ${provider.model}` : ''}`
                  : '未连接'}
          </div>
          <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-slate-600">
            {runtimeStatus
              ? `${runtimeStatus.provider.protocol}${runtimeStatus.provider.effort ? ` · ${runtimeStatus.provider.effort}` : ''}`
              : provider
                ? provider.protocol
                : '未连接'}
          </div>
          <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-slate-600">
            <span className="inline-flex items-center gap-1.5">
              <ShieldCheck size={14} />
              {runtimeStatus?.permission_mode ?? settings?.permission_mode ?? 'default'}
            </span>
          </div>
          {runtimeStatus?.provider.auth_source && (
            <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-xs text-slate-500">
              auth {runtimeStatus.provider.auth_source.replace(/^env:/, '').replace(/^settings:/, 'settings')}
            </div>
          )}
          {runtimeStatus?.provider.fallback_model && (
            <div
              className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-xs text-slate-500"
              title={`Fallback model: ${runtimeStatus.provider.fallback_model}`}
            >
              fallback {runtimeStatus.provider.fallback_model}
            </div>
          )}
          {runtimeStatus && runtimeStatus.setting_sources.length > 0 && (
            <div
              className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-xs text-slate-500"
              title={runtimeStatus.setting_sources.join('\n')}
            >
              settings {runtimeStatus.setting_sources.length}
            </div>
          )}
          {runtimeStatus && runtimeStatus.allowed_setting_sources.length > 0 && (
            <div
              className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-xs text-slate-500"
              title={runtimeStatus.allowed_setting_sources.join('\n')}
            >
              scope {runtimeStatus.allowed_setting_sources.join('/')}
            </div>
          )}
          {runtimeStatus && (runtimeStatus.allowed_tools.length > 0 || runtimeStatus.disallowed_tools.length > 0) && (
            <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 text-xs text-slate-500">
              tools +{runtimeStatus.allowed_tools.length} / -{runtimeStatus.disallowed_tools.length}
            </div>
          )}
          {lastPromptResult && (
            <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 font-mono text-xs text-slate-600">
              in {lastPromptResult.usage.input_tokens} / out {lastPromptResult.usage.output_tokens}
            </div>
          )}
          {activeContextUsage && (
            <div className="rounded-full border border-[#e2dbcf] bg-[#fbfaf7] px-3 py-1.5 font-mono text-xs text-slate-600">
              ctx {(activeContextUsage.ratio * 100).toFixed(0)}% · {activeContextUsage.estimated_tokens}/
              {activeContextUsage.max_input_tokens}
            </div>
          )}
          {activeContextCompaction && (
            <div className="rounded-full border border-[#eadfcd] bg-[#fff8ea] px-3 py-1.5 text-xs text-amber-700">
              compacted {activeContextCompaction.entries_removed} · {(activeContextCompaction.usage_ratio * 100).toFixed(0)}%
            </div>
          )}
          {!activeContextCompaction && activeContextOverflow && (
            <div className="rounded-full border border-[#f1d7d4] bg-[#fff4f2] px-3 py-1.5 text-xs text-rose-700">
              near limit {(activeContextOverflow.ratio * 100).toFixed(0)}%
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
