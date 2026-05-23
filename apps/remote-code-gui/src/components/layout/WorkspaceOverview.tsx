import { FolderGit2, MessageSquareText, Network, TerminalSquare } from 'lucide-react';
import { useMemo } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { formatSensitivePath } from '../../lib/utils';

function Metric({
  label,
  value,
  detail,
}: {
  label: string;
  value: string | number;
  detail?: string;
}) {
  return (
    <div className="border-r border-rc-border-secondary px-4 py-3 last:border-r-0">
      <div className="text-[10px] font-semibold uppercase text-rc-text-tertiary">
        {label}
      </div>
      <div className="mt-1 text-sm font-semibold text-rc-text-primary">{value}</div>
      {detail && <div className="mt-0.5 truncate text-xs text-rc-text-tertiary">{detail}</div>}
    </div>
  );
}

export function WorkspaceOverview() {
  const projects = useAppStore((state) => state.projects);
  const sessions = useAppStore((state) => state.sessions);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);

  const recentSessions = useMemo(
    () =>
      [...sessions]
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
        .slice(0, 5),
    [sessions],
  );

  const activeProject = useMemo(
    () =>
      projects.find((project) => project.path === activeProjectPath) ??
      projects[0] ??
      null,
    [activeProjectPath, projects],
  );

  const mcpSummary = runtimeStatus?.mcp;

  return (
    <div className="flex h-full min-h-0 flex-col bg-rc-bg-chat">
      <div className="border-b border-rc-border-secondary bg-rc-bg-surface px-4 py-2">
        <div className="flex items-center gap-2 text-xs text-rc-text-secondary">
          <TerminalSquare size={14} className="text-rc-text-tertiary" />
          <span className="font-medium text-rc-text-primary">Workbench</span>
          <span className="text-rc-text-tertiary">/</span>
          <span>{activeProject ? activeProject.name : 'No project'}</span>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-5">
        <div className="mx-auto w-full max-w-5xl">
          <div className="grid overflow-hidden rounded-md border border-rc-border-secondary bg-rc-bg-surface md:grid-cols-4">
            <Metric label="Projects" value={projects.length} detail={activeProject?.name ?? 'none'} />
            <Metric label="Sessions" value={sessions.length} detail={`${recentSessions.length} recent`} />
            <Metric label="Agent" value={activeAgentType ?? 'remote_claude'} detail="selected" />
            <Metric
              label="MCP"
              value={mcpSummary ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}` : '—'}
              detail={runtimeStatus ? 'runtime inventory' : 'offline'}
            />
          </div>

          <div className="mt-5 grid gap-5 lg:grid-cols-[1.2fr_0.8fr]">
            <section className="rounded-md border border-rc-border-secondary bg-rc-bg-surface">
              <div className="flex items-center gap-2 border-b border-rc-border-secondary px-3 py-2 text-xs font-semibold uppercase text-rc-text-tertiary">
                <MessageSquareText size={13} />
                Recent Sessions
              </div>
              <div className="divide-y divide-rc-border-secondary">
                {recentSessions.length > 0 ? (
                  recentSessions.map((session) => (
                    <div key={session.id} className="px-3 py-2.5">
                      <div className="truncate text-sm font-medium text-rc-text-primary">
                        {privacyMode ? 'Hidden session' : session.title}
                      </div>
                      <div className="mt-1 flex min-w-0 items-center gap-2 text-xs text-rc-text-tertiary">
                        <span className="truncate">{session.provider_name}</span>
                        {session.model && <span className="truncate font-mono">{session.model}</span>}
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="px-3 py-8 text-sm text-rc-text-tertiary">No sessions</div>
                )}
              </div>
            </section>

            <section className="rounded-md border border-rc-border-secondary bg-rc-bg-surface">
              <div className="flex items-center gap-2 border-b border-rc-border-secondary px-3 py-2 text-xs font-semibold uppercase text-rc-text-tertiary">
                <FolderGit2 size={13} />
                Active Project
              </div>
              <div className="space-y-3 px-3 py-3 text-sm">
                <div>
                  <div className="text-xs text-rc-text-tertiary">Name</div>
                  <div className="mt-1 truncate font-medium text-rc-text-primary">
                    {activeProject?.name ?? '—'}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-rc-text-tertiary">Path</div>
                  <div className="mt-1 break-all font-mono text-xs text-rc-text-secondary">
                    {activeProject ? formatSensitivePath(activeProject.path, privacyMode) : '—'}
                  </div>
                </div>
                <div className="flex items-center gap-2 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-2.5 py-2 text-xs text-rc-text-secondary">
                  <Network size={13} className={runtimeStatus ? 'text-rc-accent-success' : 'text-rc-text-tertiary'} />
                  <span>{runtimeStatus ? 'Runtime online' : 'Runtime offline'}</span>
                </div>
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
