import { memo, useMemo } from 'react';
import { cn } from '../../lib/utils';

/** Markdown 渲染组件属性 */
export interface MarkdownProps {
  /** Markdown 文本内容 */
  children: string;
  /** 是否使用暗淡颜色 */
  dimColor?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

interface MarkdownSegment {
  type: 'text' | 'bold' | 'italic' | 'code' | 'code-block' | 'link';
  content: string;
  href?: string;
}

/**
 * 简化 Markdown 渲染组件。
 * 使用正则处理 bold、italic、code、code blocks、links。
 */
export const Markdown = memo(function Markdown({
  children,
  dimColor = false,
  className,
}: MarkdownProps) {
  const segments = useMemo(() => parseMarkdown(children), [children]);

  return (
    <div
      data-testid="markdown-content"
      className={cn(
        'whitespace-pre-wrap break-words text-sm leading-6',
        dimColor ? 'text-slate-500 dark:text-slate-400' : 'text-slate-800 dark:text-slate-200',
        className,
      )}
    >
      {segments.map((seg, i) => {
        switch (seg.type) {
          case 'bold':
            return (
              <strong key={i} className="font-semibold">
                {seg.content}
              </strong>
            );
          case 'italic':
            return <em key={i}>{seg.content}</em>;
          case 'code':
            return (
              <code
                key={i}
                className="rounded bg-slate-100 px-1.5 py-0.5 text-xs font-mono text-rose-600 dark:bg-slate-800 dark:text-rose-400"
              >
                {seg.content}
              </code>
            );
          case 'code-block':
            return (
              <pre
                key={i}
                className="my-2 overflow-x-auto rounded-lg bg-slate-100 p-3 text-xs font-mono dark:bg-slate-800"
              >
                <code>{seg.content}</code>
              </pre>
            );
          case 'link':
            return (
              <a
                key={i}
                href={seg.href}
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-600 underline hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
              >
                {seg.content}
              </a>
            );
          default:
            return <span key={i}>{seg.content}</span>;
        }
      })}
    </div>
  );
});

/**
 * 将 Markdown 文本解析为片段数组。
 */
function parseMarkdown(text: string): MarkdownSegment[] {
  const segments: MarkdownSegment[] = [];
  const regex =
    /(```[\s\S]*?```|`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: 'text', content: text.slice(lastIndex, match.index) });
    }
    const token = match[0];
    if (token.startsWith('```') && token.endsWith('```')) {
      const content = token.slice(3, -3);
      const firstNewline = content.indexOf('\n');
      const codeContent = firstNewline >= 0 ? content.slice(firstNewline + 1) : content;
      segments.push({ type: 'code-block', content: codeContent });
    } else if (token.startsWith('`') && token.endsWith('`')) {
      segments.push({ type: 'code', content: token.slice(1, -1) });
    } else if (token.startsWith('**') && token.endsWith('**')) {
      segments.push({ type: 'bold', content: token.slice(2, -2) });
    } else if (token.startsWith('*') && token.endsWith('*')) {
      segments.push({ type: 'italic', content: token.slice(1, -1) });
    } else if (token.startsWith('[')) {
      const linkMatch = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(token);
      if (linkMatch) {
        segments.push({ type: 'link', content: linkMatch[1], href: linkMatch[2] });
      } else {
        segments.push({ type: 'text', content: token });
      }
    }
    lastIndex = regex.lastIndex;
  }

  if (lastIndex < text.length) {
    segments.push({ type: 'text', content: text.slice(lastIndex) });
  }

  return segments;
}
