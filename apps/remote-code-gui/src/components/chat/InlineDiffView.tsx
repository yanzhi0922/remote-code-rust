import { useMemo } from 'react';
import { cn } from '../../lib/utils';

interface DiffLine {
  type: 'add' | 'remove' | 'context';
  content: string;
  oldLineNo?: number;
  newLineNo?: number;
}

function parseUnifiedDiff(text: string): DiffLine[] {
  const lines = text.split('\n');
  const result: DiffLine[] = [];
  let oldLine = 0;
  let newLine = 0;

  for (const line of lines) {
    if (line.startsWith('@@')) {
      const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (match) {
        oldLine = parseInt(match[1], 10);
        newLine = parseInt(match[2], 10);
      }
      continue;
    }

    if (line.startsWith('---') || line.startsWith('+++') || line.startsWith('diff ')) {
      continue;
    }

    if (line.startsWith('+')) {
      result.push({ type: 'add', content: line.slice(1), newLineNo: newLine++ });
    } else if (line.startsWith('-')) {
      result.push({ type: 'remove', content: line.slice(1), oldLineNo: oldLine++ });
    } else {
      const content = line.startsWith(' ') ? line.slice(1) : line;
      result.push({ type: 'context', content, oldLineNo: oldLine++, newLineNo: newLine++ });
    }
  }
  return result;
}

function detectDiffContent(text: string): { isDiff: boolean; fileName: string | null } {
  if (text.includes('--- a/') || text.includes('+++ b/') || text.includes('@@ ')) {
    const match = text.match(/--- a\/(.+?)(?:\n|\r\n)\+\+\+ b\//);
    return { isDiff: true, fileName: match?.[1] ?? null };
  }
  return { isDiff: false, fileName: null };
}

function DiffLineRow({ line }: { line: DiffLine }) {
  const bgClass =
    line.type === 'add'
      ? 'bg-rc-accent-success-bg'
      : line.type === 'remove'
        ? 'bg-rc-accent-error-bg'
        : '';

  const textClass =
    line.type === 'add'
      ? 'text-rc-accent-success'
      : line.type === 'remove'
        ? 'text-rc-accent-error'
        : 'text-rc-text-primary';

  const prefix =
    line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' ';

  return (
    <div className={cn('flex font-mono text-xs leading-5', bgClass)}>
      <span className="w-10 shrink-0 select-none text-right pr-2 text-rc-text-tertiary">
        {line.oldLineNo ?? ''}
      </span>
      <span className="w-10 shrink-0 select-none text-right pr-2 text-rc-text-tertiary">
        {line.newLineNo ?? ''}
      </span>
      <span className={cn('shrink-0 w-4 text-center select-none', textClass)}>{prefix}</span>
      <span className={cn('whitespace-pre-wrap break-all', textClass)}>{line.content}</span>
    </div>
  );
}

export function InlineDiffView({ content }: { content: string }) {
  const { lines, fileName } = useMemo(() => {
    const detected = detectDiffContent(content);
    if (detected.isDiff) {
      return { lines: parseUnifiedDiff(content), fileName: detected.fileName };
    }
    return { lines: null, fileName: null };
  }, [content]);

  if (!lines) return null;

  const added = lines.filter((l) => l.type === 'add').length;
  const removed = lines.filter((l) => l.type === 'remove').length;

  return (
    <div className="overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-code">
      <div className="flex items-center gap-2 border-b border-rc-border-secondary bg-rc-bg-code-header px-3 py-1.5">
        <span className="text-xs font-mono text-rc-text-primary">{fileName ?? 'diff'}</span>
        <div className="flex-1" />
        <span className="text-[10px] text-rc-accent-success">+{added}</span>
        <span className="text-[10px] text-rc-accent-error">-{removed}</span>
      </div>
      <div className="overflow-x-auto px-1 py-1">
        {lines.map((line, index) => (
          <DiffLineRow key={index} line={line} />
        ))}
      </div>
    </div>
  );
}

export function detectAndRenderDiff(text: string): { isDiff: boolean; element: React.ReactNode | null } {
  const detected = detectDiffContent(text);
  if (detected.isDiff) {
    return { isDiff: true, element: <InlineDiffView content={text} /> };
  }
  return { isDiff: false, element: null };
}
