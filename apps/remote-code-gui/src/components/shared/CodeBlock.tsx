/**
 * CodeBlock — 代码块组件。
 *
 * 用于消息中的代码渲染，支持语言标签、行号、复制按钮和最大高度滚动。
 */

import { useState } from 'react';
import { Check, Copy } from 'lucide-react';

export interface CodeBlockProps {
  code: string;
  language?: string;
  showLineNumbers?: boolean;
  maxHeight?: number;
}

export function CodeBlock({
  code,
  language,
  showLineNumbers = false,
  maxHeight,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const lines = code.split('\n');

  return (
    <div
      className="relative rounded-xl bg-slate-900 text-sm text-slate-100"
      data-testid="code-block"
    >
      {/* Language label */}
      {language && (
        <span
          className="absolute right-12 top-2 rounded-md bg-slate-700 px-2 py-0.5 text-xs text-slate-300"
          data-testid="code-language"
        >
          {language}
        </span>
      )}

      {/* Copy button */}
      <button
        onClick={handleCopy}
        className="absolute right-2 top-2 rounded-md p-1 text-slate-400 hover:bg-slate-700 hover:text-white"
        data-testid="copy-button"
        aria-label="Copy code"
      >
        {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
      </button>

      {/* Code content */}
      <div
        className="overflow-auto p-4"
        style={maxHeight ? { maxHeight } : undefined}
      >
        <pre className="font-mono">
          <code>
            {lines.map((line, i) => (
              <div key={i} className="flex">
                {showLineNumbers && (
                  <span className="mr-4 inline-block w-8 select-none text-right text-slate-500">
                    {i + 1}
                  </span>
                )}
                <span>{line}</span>
              </div>
            ))}
          </code>
        </pre>
      </div>
    </div>
  );
}
