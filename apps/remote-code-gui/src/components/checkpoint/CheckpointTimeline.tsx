import React, { useState, useEffect, useCallback } from 'react';
import { checkpointList } from '../../lib/tauri';
import type { CheckpointSummaryInfo } from '../../lib/types';

/**
 * Checkpoint Timeline component inspired by ZCode's conversation-level version control.
 *
 * Shows a timeline of checkpoints in the current session, allowing users to:
 * - Review changes at any checkpoint (multi-file diff)
 * - Undo the last interaction
 * - Restore to any historical checkpoint
 */

interface CheckpointTimelineProps {
  sessionId: string | null;
  onUndo: (checkpointId: string) => void;
  onRestore: (checkpointId: string) => void;
  onReview: (checkpointId: string) => void;
  className?: string;
}

export const CheckpointTimeline: React.FC<CheckpointTimelineProps> = ({
  sessionId,
  onUndo,
  onRestore,
  onReview,
  className = '',
}) => {
  const [checkpoints, setCheckpoints] = useState<CheckpointSummaryInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadCheckpoints = useCallback(async () => {
    if (!sessionId) return;
    setLoading(true);
    setError(null);
    try {
      setCheckpoints(await checkpointList(sessionId));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setCheckpoints([]);
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    loadCheckpoints();
  }, [loadCheckpoints]);

  const formatTime = (isoString: string) => {
    try {
      const date = new Date(isoString);
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } catch {
      return '—';
    }
  };

  const formatStats = (stats: CheckpointSummaryInfo['stats']) => {
    const parts: string[] = [];
    if (stats.filesAdded > 0) parts.push(`+${stats.filesAdded} added`);
    if (stats.filesModified > 0) parts.push(`~${stats.filesModified} modified`);
    if (stats.filesDeleted > 0) parts.push(`-${stats.filesDeleted} deleted`);
    return parts.length > 0 ? parts.join(', ') : 'No changes';
  };

  const totalChanges = (stats: CheckpointSummaryInfo['stats']) =>
    stats.filesAdded + stats.filesModified + stats.filesDeleted;

  if (!sessionId) {
    return (
      <div className={`flex items-center justify-center h-full ${className}`}>
        <p className="text-xs text-[var(--color-text-muted)]">No active session</p>
      </div>
    );
  }

  return (
    <div className={`flex flex-col h-full ${className}`}>
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--color-border)]">
        <h3 className="text-xs font-semibold text-[var(--color-text)]">
          ⏮ Checkpoints
        </h3>
        <div className="flex items-center gap-1">
          {checkpoints.length > 0 && (
            <button
              onClick={() => {
                const latest = checkpoints[checkpoints.length - 1];
                if (latest) onUndo(latest.id);
              }}
              className="px-2 py-1 text-[10px] font-medium rounded
                bg-[var(--color-surface)] hover:bg-[var(--color-surface-hover)]
                border border-[var(--color-border)] text-[var(--color-text)]
                transition-colors"
              title="Undo last interaction"
            >
              ↩ Undo Last
            </button>
          )}
        </div>
      </div>

      {/* Timeline */}
      <div className="flex-1 overflow-y-auto">
        {error ? (
          <div className="text-xs text-red-400 text-center py-4 px-3">
            {error}
          </div>
        ) : loading ? (
          <div className="text-xs text-[var(--color-text-muted)] text-center py-4">
            Loading checkpoints...
          </div>
        ) : checkpoints.length === 0 ? (
          <div className="text-xs text-[var(--color-text-muted)] text-center py-8 px-4">
            <p className="text-lg mb-2">📸</p>
            <p>No checkpoints yet.</p>
            <p className="mt-1 text-[10px]">
              Checkpoints are created automatically when you send messages.
            </p>
          </div>
        ) : (
          <div className="relative p-2">
            {/* Timeline line */}
            <div className="absolute left-5 top-4 bottom-4 w-px bg-[var(--color-border)]" />

            {checkpoints.map((checkpoint, index) => {
              const isExpanded = expandedId === checkpoint.id;
              const isLatest = index === checkpoints.length - 1;
              const changes = totalChanges(checkpoint.stats);

              return (
                <div key={checkpoint.id} className="relative flex gap-3 mb-2">
                  {/* Timeline dot */}
                  <div className={`
                    relative z-10 flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center
                    ${isLatest
                      ? 'bg-[var(--color-accent)] text-white'
                      : changes > 0
                        ? 'bg-[var(--color-surface)] border-2 border-[var(--color-accent)]'
                        : 'bg-[var(--color-surface)] border-2 border-[var(--color-border)]'
                    }
                  `}>
                    <span className="text-[10px] font-bold">
                      {changes > 0 ? changes : '—'}
                    </span>
                  </div>

                  {/* Content */}
                  <div
                    className={`
                      flex-1 p-2 rounded-lg text-xs cursor-pointer
                      border transition-colors
                      ${isExpanded
                        ? 'border-[var(--color-accent)] bg-[var(--color-accent)] bg-opacity-5'
                        : 'border-[var(--color-border)] hover:border-[var(--color-accent)] hover:bg-[var(--color-surface-hover)]'
                      }
                    `}
                    onClick={() => setExpandedId(isExpanded ? null : checkpoint.id)}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-medium text-[var(--color-text)]">
                        {checkpoint.summary || `Message ${checkpoint.messageIndex + 1}`}
                      </span>
                      <span className="text-[10px] text-[var(--color-text-muted)]">
                        {formatTime(checkpoint.createdAt)}
                      </span>
                    </div>

                    <div className="text-[10px] text-[var(--color-text-muted)] mt-0.5">
                      {formatStats(checkpoint.stats)}
                      {checkpoint.stats.linesAdded > 0 && (
                        <span className="text-green-400 ml-1">+{checkpoint.stats.linesAdded}</span>
                      )}
                      {checkpoint.stats.linesRemoved > 0 && (
                        <span className="text-red-400 ml-1">-{checkpoint.stats.linesRemoved}</span>
                      )}
                    </div>

                    {/* Expanded actions */}
                    {isExpanded && (
                      <div className="flex items-center gap-2 mt-2 pt-2 border-t border-[var(--color-border)]">
                        <button
                          onClick={(e) => { e.stopPropagation(); onReview(checkpoint.id); }}
                          className="px-2 py-1 text-[10px] rounded bg-[var(--color-surface)]
                            border border-[var(--color-border)] hover:bg-[var(--color-surface-hover)]
                            transition-colors"
                        >
                          📋 Review Changes
                        </button>
                        {!isLatest && (
                          <button
                            onClick={(e) => { e.stopPropagation(); onRestore(checkpoint.id); }}
                            className="px-2 py-1 text-[10px] rounded bg-yellow-600 bg-opacity-20
                              border border-yellow-600 text-yellow-400
                              hover:bg-opacity-30 transition-colors"
                          >
                            ⏮ Restore Here
                          </button>
                        )}
                        {isLatest && (
                          <button
                            onClick={(e) => { e.stopPropagation(); onUndo(checkpoint.id); }}
                            className="px-2 py-1 text-[10px] rounded bg-red-600 bg-opacity-20
                              border border-red-600 text-red-400
                              hover:bg-opacity-30 transition-colors"
                          >
                            ↩ Undo
                          </button>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default CheckpointTimeline;
