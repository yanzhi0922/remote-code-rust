export interface DiffLineProps {
  type: 'add' | 'delete' | 'context' | 'hunk_header';
  content: string;
  oldLine?: number;
  newLine?: number;
}

export function DiffLine({ type, content, oldLine, newLine }: DiffLineProps) {
  const bgClass =
    type === 'add'
      ? 'bg-green-50'
      : type === 'delete'
        ? 'bg-red-50'
        : type === 'hunk_header'
          ? 'bg-slate-100'
          : '';

  const textClass =
    type === 'add'
      ? 'text-green-800'
      : type === 'delete'
        ? 'text-red-800'
        : type === 'hunk_header'
          ? 'text-slate-500'
          : 'text-slate-700';

  const prefix =
    type === 'add' ? '+' : type === 'delete' ? '-' : type === 'hunk_header' ? '' : ' ';

  return (
    <div
      data-testid={`diff-line-${type}`}
      className={`flex font-mono text-xs leading-5 ${bgClass}`}
    >
      <span className="w-12 shrink-0 select-none text-right text-slate-400">
        {oldLine ?? ''}
      </span>
      <span className="w-12 shrink-0 select-none text-right text-slate-400">
        {newLine ?? ''}
      </span>
      <span className={`w-5 shrink-0 select-none text-center ${textClass}`}>{prefix}</span>
      <span className={`whitespace-pre ${textClass}`}>{content}</span>
    </div>
  );
}
