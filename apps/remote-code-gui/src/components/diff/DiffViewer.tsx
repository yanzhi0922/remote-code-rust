import { useState } from 'react';
import { Split, AlignLeft, ChevronDown, ChevronRight } from 'lucide-react';
import { DiffLine } from './DiffLine';
import { DiffStats } from './DiffStats';

export interface DiffViewerProps {
  oldContent: string;
  newContent: string;
  fileName?: string;
  language?: string;
}

interface DiffRow {
  type: 'add' | 'delete' | 'context' | 'hunk_header';
  content: string;
  oldLine?: number;
  newLine?: number;
}

function computeDiff(oldContent: string, newContent: string): {
  rows: DiffRow[];
  additions: number;
  deletions: number;
} {
  const oldLines = oldContent.split('\n');
  const newLines = newContent.split('\n');
  const rows: DiffRow[] = [];
  let additions = 0;
  let deletions = 0;

  const maxLen = Math.max(oldLines.length, newLines.length);
  let oi = 0;
  let ni = 0;

  while (oi < oldLines.length || ni < newLines.length) {
    if (oi < oldLines.length && ni < newLines.length) {
      if (oldLines[oi] === newLines[ni]) {
        rows.push({ type: 'context', content: oldLines[oi], oldLine: oi + 1, newLine: ni + 1 });
        oi++;
        ni++;
      } else {
        // Check if old line appears later in new
        const oldInNew = newLines.indexOf(oldLines[oi], ni);
        const newInOld = oldLines.indexOf(newLines[ni], oi);

        if (oldInNew === -1 && newInOld === -1) {
          // Lines changed: show as delete + add
          rows.push({ type: 'delete', content: oldLines[oi], oldLine: oi + 1 });
          deletions++;
          rows.push({ type: 'add', content: newLines[ni], newLine: ni + 1 });
          additions++;
          oi++;
          ni++;
        } else if (newInOld !== -1 && (oldInNew === -1 || newInOld <= oldInNew)) {
          // New lines were inserted
          rows.push({ type: 'add', content: newLines[ni], newLine: ni + 1 });
          additions++;
          ni++;
        } else {
          // Old lines were deleted
          rows.push({ type: 'delete', content: oldLines[oi], oldLine: oi + 1 });
          deletions++;
          oi++;
        }
      }
    } else if (oi < oldLines.length) {
      rows.push({ type: 'delete', content: oldLines[oi], oldLine: oi + 1 });
      deletions++;
      oi++;
    } else {
      rows.push({ type: 'add', content: newLines[ni], newLine: ni + 1 });
      additions++;
      ni++;
    }
  }

  // Insert hunk headers at transition points
  const result: DiffRow[] = [];
  let lastType: string | null = null;
  for (const row of rows) {
    if (row.type !== 'context' && lastType === 'context' && result.length > 0) {
      result.push({ type: 'hunk_header', content: '@@ ... @@' });
    }
    result.push(row);
    lastType = row.type === 'hunk_header' ? null : row.type;
  }

  return { rows: result, additions, deletions };
}

export function DiffViewer({ oldContent, newContent, fileName }: DiffViewerProps) {
  const [mode, setMode] = useState<'unified' | 'side-by-side'>('unified');
  const [collapsed, setCollapsed] = useState(false);

  const { rows, additions, deletions } = computeDiff(oldContent, newContent);

  return (
    <div data-testid="diff-viewer" className="rounded-2xl border border-slate-200 bg-white">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-100 px-4 py-2">
        <div className="flex items-center gap-3">
          {fileName && (
            <span className="text-sm font-medium text-slate-700">{fileName}</span>
          )}
          <DiffStats additions={additions} deletions={deletions} filesChanged={1} />
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            data-testid="diff-mode-toggle"
            onClick={() => setMode(mode === 'unified' ? 'side-by-side' : 'unified')}
            className="flex items-center gap-1 rounded-lg px-2 py-1 text-xs text-slate-500 hover:bg-slate-100"
            title={mode === 'unified' ? '切换到并排模式' : '切换到统一模式'}
          >
            {mode === 'unified' ? (
              <>
                <Split className="h-3.5 w-3.5" />
                并排
              </>
            ) : (
              <>
                <AlignLeft className="h-3.5 w-3.5" />
                统一
              </>
            )}
          </button>
          <button
            type="button"
            data-testid="diff-collapse-toggle"
            onClick={() => setCollapsed(!collapsed)}
            className="flex items-center gap-1 rounded-lg px-2 py-1 text-xs text-slate-500 hover:bg-slate-100"
            title={collapsed ? '展开未更改区域' : '折叠未更改区域'}
          >
            {collapsed ? (
              <ChevronRight className="h-3.5 w-3.5" />
            ) : (
              <ChevronDown className="h-3.5 w-3.5" />
            )}
            {collapsed ? '展开' : '折叠'}
          </button>
        </div>
      </div>

      {/* Content */}
      {mode === 'unified' ? (
        <div className="overflow-x-auto p-0 text-xs">
          {rows.map((row, i) => {
            if (collapsed && row.type === 'context') {
              // Show first and last context lines around changes
              const prev = rows[i - 1];
              const next = rows[i + 1];
              if (prev && prev.type !== 'context' && next && next.type === 'context') {
                return null;
              }
              if (next && next.type !== 'context' && prev && prev.type === 'context') {
                return null;
              }
              if (
                prev?.type === 'context' &&
                next?.type === 'context'
              ) {
                return null;
              }
            }
            return (
              <DiffLine
                key={i}
                type={row.type}
                content={row.content}
                oldLine={row.oldLine}
                newLine={row.newLine}
              />
            );
          })}
        </div>
      ) : (
        <div data-testid="diff-side-by-side" className="grid grid-cols-2 divide-x divide-slate-200 overflow-x-auto">
          <div className="overflow-x-auto text-xs">
            {rows
              .filter((r) => r.type === 'delete' || r.type === 'context' || r.type === 'hunk_header')
              .map((row, i) => (
                <DiffLine
                  key={i}
                  type={row.type}
                  content={row.content}
                  oldLine={row.oldLine}
                  newLine={row.type === 'context' ? row.newLine : undefined}
                />
              ))}
          </div>
          <div className="overflow-x-auto text-xs">
            {rows
              .filter((r) => r.type === 'add' || r.type === 'context' || r.type === 'hunk_header')
              .map((row, i) => (
                <DiffLine
                  key={i}
                  type={row.type}
                  content={row.content}
                  oldLine={row.type === 'context' ? row.oldLine : undefined}
                  newLine={row.newLine}
                />
              ))}
          </div>
        </div>
      )}
    </div>
  );
}
