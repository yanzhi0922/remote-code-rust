import { Children, isValidElement, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import { truncateMiddle } from '../../lib/utils';
import CollapsibleBlock from './CollapsibleBlock';

interface MarkdownRendererProps {
  content: string;
}

function summarizeCodeBlock(children: React.ReactNode): { label: string; preview: string } {
  const elements = Children.toArray(children);
  const codeElement = elements.find((child) => isValidElement(child));

  if (!isValidElement(codeElement)) {
    return { label: '代码', preview: '展开查看代码块' };
  }

  const props = codeElement.props as { className?: string; children?: React.ReactNode };
  const className =
    typeof props.className === 'string' ? props.className : '';
  const language = /language-(\w+)/.exec(className)?.[1]?.toUpperCase() ?? '代码';
  const rawText = extractTextContent(props.children);
  const firstLine = rawText.split('\n').find((line) => line.trim().length > 0)?.trim() ?? '展开查看代码块';

  return {
    label: language,
    preview: truncateMiddle(firstLine, 68),
  };
}

function extractTextContent(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') {
    return String(node);
  }
  if (!node) {
    return '';
  }
  return Children.toArray(node)
    .map((child) => {
      if (typeof child === 'string' || typeof child === 'number') {
        return String(child);
      }
      if (isValidElement(child)) {
        return extractTextContent((child.props as { children?: ReactNode }).children);
      }
      return '';
    })
    .join('');
}

export default function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const components: Components = {
    pre: ({ children }) => {
      const summary = summarizeCodeBlock(children);
      return (
        <CollapsibleBlock
          summary={
            <div className="flex min-w-0 items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-500">
                {summary.label}
              </span>
              <span className="truncate text-sm text-slate-600">{summary.preview}</span>
            </div>
          }
          iconColor="text-slate-500"
          className="my-4"
        >
          <pre className="overflow-x-auto rounded-xl bg-slate-800 p-4 text-xs leading-relaxed text-slate-100">
            {children}
          </pre>
        </CollapsibleBlock>
      );
    },
    code: ({ className, children, ...props }) => {
      // `react-markdown` can surface a `ref` typed from a sibling React install
      // when this renderer is reused across package boundaries. Drop it so the
      // shared remote UI remains buildable in both the GUI and mobile app.
      const { ref: _ref, ...safeProps } = props as typeof props & { ref?: unknown };
      const match = /language-(\w+)/.exec(className || '');
      const isInline = !match && !className;
      if (isInline) {
        return (
          <code
            className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-xs text-slate-800"
            {...safeProps}
          >
            {children}
          </code>
        );
      }
      return (
        <code className={className} {...safeProps}>
          {children}
        </code>
      );
    },
    blockquote: ({ children }) => (
      <blockquote className="my-4 rounded-r-2xl border-l-4 border-[#d8d1c3] bg-[#faf7f1] px-4 py-3 text-slate-700">
        {children}
      </blockquote>
    ),
    table: ({ children }) => (
      <div className="my-4 overflow-x-auto">
        <table className="min-w-full border-collapse overflow-hidden rounded-2xl border border-[#e7e1d6] bg-white">
          {children}
        </table>
      </div>
    ),
    th: ({ children }) => <th className="bg-[#f7f3eb] px-3 py-2 text-left text-xs font-semibold text-slate-600">{children}</th>,
    td: ({ children }) => <td className="border-t border-[#efe8dc] px-3 py-2 text-sm text-slate-700">{children}</td>,
  };

  return (
    <div className="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex, rehypeHighlight]}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
