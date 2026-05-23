import { lazy, Suspense } from 'react';
import {
  AlertTriangle,
  Database,
  FileOutput,
  Layers,
  GitBranch,
  LoaderCircle,
  MessageSquareText,
  Shield,
  Wifi,
  WifiOff,
} from 'lucide-react';
import { TimelineEventCard } from '../components/shared/TimelineEventCard';
import { TimelineMessageCard } from '../components/shared/TimelineMessageCard';
import { formatBytes } from '../components/shared/formatBytes';
import { formatRemoteRelativeTime, getRemoteCopy, resolveRemoteLocale } from './i18n';
import type { RemoteTimelineEvent, RemoteTimelineEventDetail } from './types';

const LazyMarkdownRenderer = lazy(() => import('../components/chat/MarkdownRenderer'));

// ─── Helpers used by TimelineCard ──────────────────────────────────────────

function toolLabel(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>,
): string {
  if ('tool_name' in detail && detail.tool_name) {
    return detail.tool_name;
  }
  return detail.tool_call_id ?? 'tool';
}

function toolSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_started' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_progress' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'tool_finished' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'tool_started') {
    return copy.toolStarted(detail.tool_call_id);
  }
  if (detail.kind === 'tool_progress') {
    if (detail.delta) {
      return detail.delta;
    }
    if (detail.elapsed_time_seconds != null) {
      return copy.toolElapsed(detail.elapsed_time_seconds);
    }
    return copy.toolRunning;
  }
  if (detail.summary) {
    return detail.summary;
  }
  return detail.is_error ? copy.toolFailedWithoutSummary : copy.toolCompleted;
}

function approvalSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_requested' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'approval_resolved' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'approval_requested') {
    return copy.approvalWaiting(detail.title);
  }
  return copy.approvalResolved(detail.approval_id, copy.approvalStateLabels[detail.state]);
}

function artifactSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'artifact_manifest' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'artifact_created') {
    return copy.artifactCreated(detail.name, detail.file_name, formatBytes(detail.size_bytes));
  }
  return copy.artifactManifest(detail.artifact_ids.length);
}

function sessionEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'session_created' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'session_state_changed' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'session_created') {
    return copy.sessionCreated(detail.workspace_id);
  }
  return copy.sessionMoved(
    copy.sessionStateLabels[detail.previous_state],
    copy.sessionStateLabels[detail.state],
  );
}

function runnerEventSummary(
  detail:
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_registered' }>
    | Extract<RemoteTimelineEventDetail, { kind: 'runner_heartbeat' }>,
  copy: ReturnType<typeof getRemoteCopy>,
): string {
  if (detail.kind === 'runner_registered') {
    return copy.runnerRegistered(detail.workspace_ids.length, detail.lease_ttl_secs);
  }
  return copy.runnerHeartbeat(detail.active_sessions, detail.queued_sessions);
}

// ─── TimelineCard component ────────────────────────────────────────────────

export function TimelineCard({
  copy,
  event,
  locale,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  event: RemoteTimelineEvent;
  locale: ReturnType<typeof resolveRemoteLocale>;
}) {
  const { detail } = event;
  const ts = formatRemoteRelativeTime(event.recorded_at, locale, copy);

  if (detail.kind === 'message_committed') {
    return (
      <TimelineMessageCard role={detail.role} header={copy.messageHeaders[detail.role]}>
        {detail.role === 'assistant' ? (
          <Suspense
            fallback={
              <div className="space-y-2">
                <div className="h-4 w-3/4 animate-pulse rounded bg-rc-bg-tertiary" />
                <div className="h-4 w-1/2 animate-pulse rounded bg-rc-bg-tertiary" />
              </div>
            }
          >
            <LazyMarkdownRenderer content={detail.text} />
          </Suspense>
        ) : (
          <div className="whitespace-pre-wrap break-words text-[15px] leading-7 text-rc-text-primary">
            {detail.text}
          </div>
        )}
      </TimelineMessageCard>
    );
  }

  if (detail.kind === 'message_delta') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.streaming}
        accent="text-rc-accent-warning"
        icon={<LoaderCircle size={16} className="animate-spin" />}
        timestampLabel={ts}
      >
        <div className="whitespace-pre-wrap break-words text-sm leading-6 text-rc-text-secondary">
          {detail.delta}
        </div>
      </TimelineEventCard>
    );
  }

  if (
    detail.kind === 'tool_started' ||
    detail.kind === 'tool_finished' ||
    detail.kind === 'tool_progress'
  ) {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.tool}
        accent={
          detail.kind === 'tool_finished' && detail.is_error
            ? 'text-rc-accent-error'
            : 'text-rc-accent-success'
        }
        icon={<Layers size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-2 text-sm text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">{toolLabel(detail)}</div>
          <div className="rounded-2xl bg-rc-bg-secondary px-3 py-2 text-sm leading-6 text-rc-text-secondary">
            {toolSummary(detail, copy)}
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'approval_requested' || detail.kind === 'approval_resolved') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.approval}
        accent="text-rc-accent-warning"
        icon={<Shield size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-2 text-sm leading-6 text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">{approvalSummary(detail, copy)}</div>
          {'responder' in detail && detail.responder && (
            <div className="text-xs uppercase tracking-[0.18em] text-rc-text-tertiary">
              {copy.responderLabel}: {detail.responder}
            </div>
          )}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'artifact_created' || detail.kind === 'artifact_manifest') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.artifact}
        accent="text-rc-accent-info"
        icon={<FileOutput size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm leading-6 text-rc-text-secondary">
          {artifactSummary(detail, copy)}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'runtime_error') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.runtime}
        accent="text-rc-accent-error"
        icon={<AlertTriangle size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm leading-6 text-rc-accent-error">{detail.message}</div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'daemon_presence_changed') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.daemon}
        accent="text-rc-text-secondary"
        icon={detail.state === 'online' ? <Wifi size={16} /> : <WifiOff size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm text-rc-text-secondary">
          {copy.daemonNow(copy.daemonStates[detail.state])}
        </div>
      </TimelineEventCard>
    );
  }

  if (
    detail.kind === 'subtask_started' ||
    detail.kind === 'subtask_progress' ||
    detail.kind === 'subtask_completed'
  ) {
    const stageLabel =
      detail.kind === 'subtask_started'
        ? 'started'
        : detail.kind === 'subtask_completed'
          ? 'completed'
          : 'progress';
    const desc = detail.kind === 'subtask_started' ? detail.description : detail.summary;
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.subtask}
        accent={stageLabel === 'completed' ? 'text-rc-accent-success' : 'text-rc-accent-info'}
        icon={<GitBranch size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">{desc}</div>
          <div className="text-xs text-rc-text-tertiary">
            {detail.task_id} · {stageLabel}
            {'turns_used' in detail && detail.turns_used != null
              ? ` · ${detail.turns_used} turns`
              : ''}
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'batch_progress') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.batch}
        accent="text-rc-accent-info"
        icon={<Layers size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">
            {detail.completed}/{detail.total} completed
          </div>
          {detail.running > 0 && (
            <div className="text-xs text-rc-text-tertiary">{detail.running} running</div>
          )}
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'context_usage' || detail.kind === 'context_overflow') {
    const pct = Math.round(detail.ratio * 100);
    const isOverflow = detail.kind === 'context_overflow';
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.context}
        accent={isOverflow ? 'text-rc-accent-warning' : 'text-rc-text-secondary'}
        icon={<Database size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">
            {isOverflow ? 'Context overflow' : 'Context usage'}: {pct}%
          </div>
          <div className="text-xs text-rc-text-tertiary">
            {detail.estimated_tokens.toLocaleString()} /{' '}
            {detail.max_input_tokens.toLocaleString()} tokens
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'context_compacted') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.context}
        accent="text-rc-text-secondary"
        icon={<Database size={16} />}
        timestampLabel={ts}
      >
        <div className="space-y-1 text-sm text-rc-text-secondary">
          <div className="font-medium text-rc-text-primary">Context compacted</div>
          <div className="text-xs text-rc-text-tertiary">
            {detail.entries_removed} entries removed · ratio{' '}
            {detail.usage_ratio.toFixed(2)}
          </div>
        </div>
      </TimelineEventCard>
    );
  }

  if (detail.kind === 'session_created' || detail.kind === 'session_state_changed') {
    return (
      <TimelineEventCard
        eyebrow={copy.eventEyebrows.session}
        accent="text-rc-text-secondary"
        icon={<MessageSquareText size={16} />}
        timestampLabel={ts}
      >
        <div className="text-sm text-rc-text-secondary">
          {sessionEventSummary(detail, copy)}
        </div>
      </TimelineEventCard>
    );
  }

  return (
    <TimelineEventCard
      eyebrow={copy.eventEyebrows.runner}
      accent="text-rc-text-secondary"
      icon={<GitBranch size={16} />}
      timestampLabel={ts}
    >
      <div className="text-sm text-rc-text-secondary">
        {runnerEventSummary(detail, copy)}
      </div>
    </TimelineEventCard>
  );
}
