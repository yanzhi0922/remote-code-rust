import { useState, useMemo } from 'react';
import { FileText, ChevronDown, ChevronRight, Split, AlignLeft } from 'lucide-react';

interface DiffPaneProps {
  oldContent?: string;
  newContent?: string;
  fileName?: string;
  className?: string;
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
  let oi = 0;
  let ni = 0;

  while (oi < oldLines.length || ni < newLines.length) {
    if (oi < oldLines.length && ni < newLines.length) {
      if (oldLines[oi] === newLines[ni]) {
        rows.push({ type: 'context', content: oldLines[oi], oldLine: oi + 1, newLine: ni + 1 });
        oi++;
        ni++;
      } else {
        const oldInNew = newLines.indexOf(oldLines[oi], ni);
        const newInOld = oldLines.indexOf(newLines[ni], oi);
        if (oldInNew === -1 && newInOld === -1) {
          rows.push({ type: 'delete', content: oldLines[oi], oldLine: oi + 1 });
          deletions++;
          rows.push({ type: 'add', content: newLines[ni], newLine: ni + 1 });
          additions++;
          oi++;
          ni++;
        } else if (newInOld !== -1 && (oldInNew === -1 || newInOld <= oldInNew)) {
          rows.push({ type: 'add', content: newLines[ni], newLine: ni + 1 });
          additions++;
          ni++;
        } else {
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

function DiffLine({ row, collapsed }: { row: DiffRow; collapsed: boolean }) {
  const bg =
    row.type === 'add'
      ? 'bg-rc-accent-success-bg/40'
      : row.type === 'delete'
        ? 'bg-rc-accent-error-bg/40'
        : row.type === 'hunk_header'
          ? 'bg-rc-bg-secondary'
          : '';
  const text =
    row.type === 'add'
      ? 'text-rc-accent-success'
      : row.type === 'delete'
        ? 'text-rc-accent-error'
        : row.type === 'hunk_header'
          ? 'text-rc-text-tertiary'
          : 'text-rc-text-inverse';
  const prefix = row.type === 'add' ? '+' : row.type === 'delete' ? '-' : row.type === 'hunk_header' ? '' : ' ';

  if (collapsed && row.type === 'context') return null;

  return (
    <div className={`flex font-mono text-xs leading-5 ${bg}`}>
      <span className="w-10 shrink-0 select-none text-right text-rc-text-tertiary">{row.oldLine ?? ''}</span>
      <span className="w-10 shrink-0 select-none text-right text-rc-text-tertiary">{row.newLine ?? ''}</span>
      <span className={`w-5 shrink-0 select-none text-center ${text}`}>{prefix}</span>
      <span className={`whitespace-pre ${text}`}>{row.content}</span>
    </div>
  );
}

export function DiffPane({ oldContent, newContent, fileName, className = '' }: DiffPaneProps) {
  const [mode, setMode] = useState<'unified' | 'side-by-side'>('unified');
  const [collapsed, setCollapsed] = useState(false);

  const hasDiff = oldContent !== undefined || newContent !== undefined;
  const { rows, additions, deletions } = useMemo(
    () => (hasDiff ? computeDiff(oldContent ?? '', newContent ?? '') : { rows: [], additions: 0, deletions: 0 }),
    [oldContent, newContent, hasDiff],
  );

  return (
    <div className={`flex h-full flex-col bg-rc-bg-code ${className}`}>
      <div className="flex items-center justify-between border-b border-rc-border-primary px-3 py-1.5">
        <div className="flex items-center gap-2">
          <FileText size={14} className="text-rc-text-secondary" />
          <span className="text-xs font-medium text-rc-text-primary">Diff</span>
          {fileName && <span className="text-2xs text-rc-text-tertiary">{fileName}</span>}
          {hasDiff && (
            <div className="flex items-center gap-2 text-2xs">
              <span className="text-rc-accent-success">+{additions}</span>
              <span className="text-rc-accent-error">-{deletions}</span>
            </div>
          )}
        </div>
        {hasDiff && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => setMode(mode === 'unified' ? 'side-by-side' : 'unified')}
              className="flex items-center gap-1 rounded px-1.5 py-0.5 text-2xs text-rc-text-tertiary hover:bg-rc-bg-hover"
              title={mode === 'unified' ? '并排模式' : '统一模式'}
            >
              {mode === 'unified' ? <Split size={12} /> : <AlignLeft size={12} />}
              {mode === 'unified' ? '并排' : '统一'}
            </button>
            <button
              type="button"
              onClick={() => setCollapsed(!collapsed)}
              className="flex items-center gap-1 rounded px-1.5 py-0.5 text-2xs text-rc-text-tertiary hover:bg-rc-bg-hover"
              title={collapsed ? '展开' : '折叠未更改行'}
            >
              {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 overflow-auto">
        {hasDiff ? (
          mode === 'unified' ? (
            <div className="p-1">
              {rows.map((row, i) => (
                <DiffLine key={i} row={row} collapsed={collapsed} />
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-2 divide-x divide-rc-border-primary">
              <div className="p-1">
                {rows
                  .filter((r) => r.type === 'delete' || r.type === 'context' || r.type === 'hunk_header')
                  .map((row, i) => (
                    <DiffLine key={i} row={row} collapsed={collapsed} />
                  ))}
              </div>
              <div className="p-1">
                {rows
                  .filter((r) => r.type === 'add' || r.type === 'context' || r.type === 'hunk_header')
                  .map((row, i) => (
                    <DiffLine key={i} row={row} collapsed={collapsed} />
                  ))}
              </div>
            </div>
          )
        ) : (
          <div className="flex h-full items-center justify-center p-3 text-rc-text-tertiary text-xs">
            <div className="text-center">
              <FileText size={24} className="mx-auto mb-2 opacity-40" />
              <p>当 Agent 执行文件修改时，将自动显示变更内容</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}