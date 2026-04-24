import { Eye } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface PreviewBoxProps {
  content: string;
  language?: string;
  className?: string;
}

export function PreviewBox({ content, language, className }: PreviewBoxProps) {
  return (
    <div className={cn('rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-800/50', className)} data-testid="preview-box">
      <div className="flex items-center gap-2 border-b border-slate-200 px-3 py-1.5 dark:border-slate-700">
        <Eye className="h-3.5 w-3.5 text-slate-400" />
        <span className="text-xs font-medium text-slate-500">预览</span>
        {language && <span className="text-xs text-slate-400">{language}</span>}
      </div>
      <pre className="max-h-48 overflow-auto p-3 text-xs text-slate-700 dark:text-slate-300">{content}</pre>
    </div>
  );
}
