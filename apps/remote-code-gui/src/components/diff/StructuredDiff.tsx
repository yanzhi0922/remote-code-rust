import { useState } from 'react';
import { ChevronDown, ChevronRight, File } from 'lucide-react';
import { DiffLine } from './DiffLine';
import { DiffStats } from './DiffStats';

export interface StructuredDiffChange {
  type: 'add' | 'delete' | 'context';
  content: string;
  old_line?: number;
  new_line?: number;
}

export interface StructuredDiffHunk {
  header: string;
  changes: StructuredDiffChange[];
}

export interface StructuredDiffFile {
  file_path: string;
  hunks: StructuredDiffHunk[];
}

export interface StructuredDiffProps {
  diffs: StructuredDiffFile[];
}

function countChanges(hunks: StructuredDiffHunk[]) {
  let additions = 0;
  let deletions = 0;
  for (const hunk of hunks) {
    for (const change of hunk.changes) {
      if (change.type === 'add') additions++;
      if (change.type === 'delete') deletions++;
    }
  }
  return { additions, deletions };
}

function FileSection({ file }: { file: StructuredDiffFile }) {
  const [expanded, setExpanded] = useState(true);
  const { additions, deletions } = countChanges(file.hunks);

  return (
    <div data-testid={`structured-diff-file`} className="border-b border-slate-100 last:border-b-0">
      <button
        type="button"
        data-testid={`structured-diff-file-header`}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-slate-50"
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 shrink-0 text-slate-400" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0 text-slate-400" />
        )}
        <File className="h-4 w-4 shrink-0 text-slate-500" />
        <span className="min-w-0 flex-1 truncate font-medium text-slate-700">
          {file.file_path}
        </span>
        <span className="shrink-0 text-xs">
          <span className="text-green-600">+{additions}</span>
          {' '}
          <span className="text-red-600">-{deletions}</span>
        </span>
      </button>

      {expanded && (
        <div className="border-t border-slate-50">
          {file.hunks.map((hunk, hi) => (
            <div key={hi}>
              <DiffLine type="hunk_header" content={hunk.header} />
              {hunk.changes.map((change, ci) => (
                <DiffLine
                  key={ci}
                  type={change.type}
                  content={change.content}
                  oldLine={change.old_line}
                  newLine={change.new_line}
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function StructuredDiff({ diffs }: StructuredDiffProps) {
  if (diffs.length === 0) {
    return (
      <div data-testid="structured-diff-empty" className="px-4 py-8 text-center text-sm text-slate-400">
        无变更
      </div>
    );
  }

  const totalAdditions = diffs.reduce((sum, f) => sum + countChanges(f.hunks).additions, 0);
  const totalDeletions = diffs.reduce((sum, f) => sum + countChanges(f.hunks).deletions, 0);

  return (
    <div data-testid="structured-diff" className="rounded-2xl border border-slate-200 bg-white">
      <div className="flex items-center justify-between border-b border-slate-100 px-4 py-2">
        <span className="text-sm font-medium text-slate-700">变更文件</span>
        <DiffStats
          additions={totalAdditions}
          deletions={totalDeletions}
          filesChanged={diffs.length}
        />
      </div>
      {diffs.map((file, i) => (
        <FileSection key={i} file={file} />
      ))}
    </div>
  );
}
