import {
  Bot,
  CheckCircle2,
  CircleEllipsis,
  FileText,
  GitCompare,
  Globe2,
  Image,
  Layers3,
  ListChecks,
  Network,
  Sparkles,
  Terminal,
  TriangleAlert,
  Wrench,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { CodexTimelineDescriptor, CodexTimelineKind, CodexTimelineStatus } from '../../lib/codexTimeline';
import { cn } from '../../lib/utils';
import { InlineDiffView } from './InlineDiffView';
import CollapsibleBlock from './CollapsibleBlock';

function KindIcon({ kind, status }: { kind: CodexTimelineKind; status: CodexTimelineStatus }) {
  const className =
    status === 'error'
      ? 'text-rc-accent-error'
      : status === 'running'
        ? 'text-rc-accent-warning'
        : status === 'success'
          ? 'text-rc-accent-success'
          : 'text-rc-text-tertiary';

  switch (kind) {
    case 'command': return <Terminal size={13} className={className} />;
    case 'file': return <FileText size={13} className={className} />;
    case 'mcp': return <Network size={13} className={className} />;
    case 'dynamic': return <Sparkles size={13} className={className} />;
    case 'collab': return <Bot size={13} className={className} />;
    case 'web': return <Globe2 size={13} className={className} />;
    case 'image': return <Image size={13} className={className} />;
    case 'plan': return <ListChecks size={13} className={className} />;
    case 'reasoning': return <CircleEllipsis size={13} className={className} />;
    case 'context': return <Layers3 size={13} className={className} />;
    default: return <Wrench size={13} className={className} />;
  }
}

function StatusIcon({ status }: { status: CodexTimelineStatus }) {
  if (status === 'error') return <TriangleAlert size={13} className="text-rc-accent-error" />;
  if (status === 'success') return <CheckCircle2 size={13} className="text-rc-accent-success" />;
  if (status === 'running') {
    return <span className="h-2 w-2 rounded-full bg-rc-accent-warning animate-pulse" />;
  }
  return <span className="h-2 w-2 rounded-full bg-rc-text-tertiary" />;
}

function formatDuration(ms: number | null | undefined): string | null {
  if (ms == null || !Number.isFinite(ms)) return null;
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function MetaPill({ children, tone = 'default' }: { children: React.ReactNode; tone?: 'default' | 'success' | 'error' }) {
  return (
    <span
      className={cn(
        'inline-flex min-w-0 items-center gap-1 rounded border px-1.5 py-0.5 text-[10px]',
        tone === 'success'
          ? 'border-rc-accent-success-border bg-rc-accent-success-bg text-rc-accent-success'
          : tone === 'error'
            ? 'border-rc-accent-error-border bg-rc-accent-error-bg text-rc-accent-error'
            : 'border-rc-border-secondary bg-rc-bg-tertiary text-rc-text-tertiary',
      )}
    >
      {children}
    </span>
  );
}

export function CodexTimelineCard({
  item,
  defaultOpen,
  className,
}: {
  item: CodexTimelineDescriptor;
  defaultOpen?: boolean;
  className?: string;
}) {
  const { t } = useTranslation();
  const duration = formatDuration(item.durationMs);
  const hasDiff = !!item.diff;

  return (
    <CollapsibleBlock
      defaultOpen={defaultOpen ?? item.status === 'error'}
      buttonLabel={t('chatArea.toggleTimelineItem')}
      iconColor={
        item.status === 'error'
          ? 'text-rc-accent-error'
          : item.status === 'running'
            ? 'text-rc-accent-warning'
            : 'text-rc-text-tertiary'
      }
      className={cn(
        item.status === 'running' && 'border-rc-accent-warning-border',
        item.status === 'error' && 'border-rc-accent-error-border',
        className,
      )}
      summary={
        <div className="flex min-w-0 items-center gap-2">
          <StatusIcon status={item.status} />
          <KindIcon kind={item.kind} status={item.status} />
          <span className="min-w-0 truncate font-mono text-xs font-medium text-rc-text-primary">
            {item.title}
          </span>
          <span className="min-w-0 flex-1 truncate text-xs text-rc-text-tertiary">
            {item.subtitle}
          </span>
          {item.exitCode !== null && item.exitCode !== undefined && (
            <MetaPill tone={item.exitCode === 0 ? 'success' : 'error'}>
              exit {item.exitCode}
            </MetaPill>
          )}
          {duration && <MetaPill>{duration}</MetaPill>}
          {hasDiff && (
            <MetaPill tone="success">
              <GitCompare size={10} />
              +{item.diff!.added} -{item.diff!.removed}
            </MetaPill>
          )}
        </div>
      }
    >
      <div className="space-y-2">
        {(item.command || item.cwd || item.path || item.server) && (
          <div className="flex flex-wrap gap-1.5">
            {item.command && <MetaPill>{item.command}</MetaPill>}
            {item.cwd && <MetaPill>cwd {item.cwd}</MetaPill>}
            {item.path && <MetaPill>{item.path}</MetaPill>}
            {item.server && <MetaPill>MCP {item.server}</MetaPill>}
          </div>
        )}

        {hasDiff ? (
          <InlineDiffView content={item.detail} />
        ) : (
          <pre
            className={cn(
              'max-h-[420px] overflow-auto whitespace-pre-wrap rounded bg-rc-bg-code p-3 text-xs font-mono leading-relaxed text-rc-text-primary',
              item.status === 'error' && 'bg-rc-accent-error-bg text-rc-accent-error',
            )}
          >
            {item.detail}
          </pre>
        )}
      </div>
    </CollapsibleBlock>
  );
}
