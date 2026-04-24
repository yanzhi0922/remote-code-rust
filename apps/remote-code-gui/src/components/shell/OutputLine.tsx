/**
 * OutputLine — 单行输出渲染组件。
 *
 * 根据行类型显示不同颜色：stderr 红色、command 绿色、info 灰色。
 * 支持可选行号显示。
 */

import { cn } from '@/lib/utils';

export interface OutputLineProps {
  content: string;
  lineType: 'stdout' | 'stderr' | 'command' | 'info';
  lineNum?: number;
  className?: string;
}

const lineTypeStyles: Record<OutputLineProps['lineType'], string> = {
  stdout: 'text-slate-200',
  stderr: 'text-red-400',
  command: 'text-green-400',
  info: 'text-slate-500',
};

export function OutputLine({
  content,
  lineType,
  lineNum,
  className,
}: OutputLineProps) {
  return (
    <div
      data-testid="output-line"
      className={cn('flex font-mono text-xs leading-5', className)}
    >
      {lineNum !== undefined && (
        <span className="mr-3 w-8 shrink-0 select-none text-right text-slate-600">
          {lineNum}
        </span>
      )}
      <span className={cn('whitespace-pre-wrap', lineTypeStyles[lineType])}>
        {content}
      </span>
    </div>
  );
}
