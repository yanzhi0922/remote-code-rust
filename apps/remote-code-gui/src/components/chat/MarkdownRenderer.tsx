import { Children, isValidElement, memo, useState, useCallback, type ReactNode, useDeferredValue, useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import i18n from '../../i18n';
import { truncateMiddle } from '../../lib/utils';
import CollapsibleBlock from './CollapsibleBlock';

interface MarkdownRendererProps {
  content: string;
}

function summarizeCodeBlock(children: React.ReactNode): { label: string; preview: string } {
  const elements = Children.toArray(children);
  const codeElement = elements.find((child) => isValidElement(child));

  if (!isValidElement(codeElement)) {
    return { label: i18n.t('markdownRenderer.codeLabel'), preview: i18n.t('markdownRenderer.codeExpandText') };
  }

  const props = codeElement.props as { className?: string; children?: React.ReactNode };
  const className = typeof props.className === 'string' ? props.className : '';
  const language = /language-(\w+)/.exec(className)?.[1]?.toUpperCase() ?? i18n.t('markdownRenderer.codeLabel');
  const rawText = extractTextContent(props.children);
  const firstLine = rawText.split('\n').find((line) => line.trim().length > 0)?.trim() ?? i18n.t('markdownRenderer.codeExpandText');

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

function CodeBlockCopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => {});
  }, [text]);

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="absolute right-2 top-2 rounded-md px-1.5 py-0.5 text-[10px] font-medium text-rc-text-tertiary opacity-0 transition-opacity hover:bg-rc-bg-hover hover:text-rc-text-primary group-hover/code:opacity-100"
      title={i18n.t('markdownRenderer.copyCode')}
    >
      {copied ? i18n.t('markdownRenderer.copied') : i18n.t('markdownRenderer.copyCode')}
    </button>
  );
}

const MarkdownRenderer = memo(function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const deferredContent = useDeferredValue(content);
  const components = useMemo<Components>(
    () => ({
      pre: ({ children }) => {
        const summary = summarizeCodeBlock(children);
        const codeText = extractTextContent(children);
        return (
          <CollapsibleBlock
            summary={
              <div className="flex min-w-0 items-center gap-2">
                <span className="text-xs font-semibold uppercase text-rc-text-tertiary">
                  {summary.label}
                </span>
                <span className="truncate text-sm text-rc-text-secondary">{summary.preview}</span>
              </div>
            }
            iconColor="text-rc-text-tertiary"
            className="my-4 group/code"
          >
            <div className="relative">
              <CodeBlockCopyButton text={codeText} />
              <pre className="overflow-x-auto rounded-md bg-rc-bg-code p-4 text-xs leading-relaxed text-rc-text-primary">
                {children}
              </pre>
            </div>
          </CollapsibleBlock>
        );
      },
      code: ({ className, children, node, ...props }) => {
        // Explicitly destructure known react-markdown props (`node`, `className`,
        // `children`) and only forward safe HTML attributes via the rest spread.
        // This avoids forwarding unexpected props like `ref` that may appear when
        // the renderer is reused across package boundaries with sibling React installs.
        const { ref: _ref, ...safeProps } = props as typeof props & { ref?: unknown };
        const match = /language-(\w+)/.exec(className || '');
        const isInline = !match && !className;
        if (isInline) {
          return (
            <code
              className="rounded bg-rc-bg-code px-1.5 py-0.5 font-mono text-xs text-rc-text-primary"
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
        <blockquote className="my-4 rounded-r-md border-l-4 border-rc-border-secondary bg-rc-bg-secondary px-4 py-3 text-rc-text-secondary">
          {children}
        </blockquote>
      ),
      table: ({ children }) => (
        <div className="my-4 overflow-x-auto">
          <table className="min-w-full border-collapse overflow-hidden rounded-lg border border-rc-border-secondary bg-rc-bg-surface">
            {children}
          </table>
        </div>
      ),
      th: ({ children }) => (
        <th className="bg-rc-bg-secondary px-3 py-2 text-left text-xs font-semibold text-rc-text-secondary">{children}</th>
      ),
      td: ({ children }) => <td className="border-t border-rc-border-secondary px-3 py-2 text-sm text-rc-text-primary">{children}</td>,
    }),
    [],
  );

  return (
    <div className="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex, rehypeHighlight]}
        components={components}
      >
        {deferredContent}
      </ReactMarkdown>
    </div>
  );
});

export default MarkdownRenderer;
