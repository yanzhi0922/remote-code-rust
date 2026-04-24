import React from 'react';
import { cn } from '../../lib/utils';

type Props = {
  children: string;
  dimColor?: boolean;
};

export function Markdown({ children, dimColor = false }: Props): React.ReactElement {
  // Simple markdown rendering - handles code blocks, bold, italic, links
  const lines = children.split('\n');
  const elements: React.ReactNode[] = [];
  let inCodeBlock = false;
  let codeBlockContent: string[] = [];
  let codeBlockLang = '';

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.startsWith('```')) {
      if (inCodeBlock) {
        elements.push(
          <pre
            key={`code-${i}`}
            data-testid="markdown-code-block"
            className="my-2 overflow-x-auto rounded-md bg-gray-100 p-3 dark:bg-gray-800"
          >
            <code className="text-sm text-gray-800 dark:text-gray-200">
              {codeBlockContent.join('\n')}
            </code>
          </pre>,
        );
        codeBlockContent = [];
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
        codeBlockLang = line.slice(3).trim();
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockContent.push(line);
      continue;
    }

    // Headers
    if (line.startsWith('### ')) {
      elements.push(
        <h4 key={`h3-${i}`} className="mt-3 mb-1 text-sm font-bold text-gray-900 dark:text-gray-100">
          {renderInline(line.slice(4))}
        </h4>,
      );
    } else if (line.startsWith('## ')) {
      elements.push(
        <h3 key={`h2-${i}`} className="mt-3 mb-1 text-base font-bold text-gray-900 dark:text-gray-100">
          {renderInline(line.slice(3))}
        </h3>,
      );
    } else if (line.startsWith('# ')) {
      elements.push(
        <h2 key={`h1-${i}`} className="mt-3 mb-1 text-lg font-bold text-gray-900 dark:text-gray-100">
          {renderInline(line.slice(2))}
        </h2>,
      );
    } else if (line.startsWith('- ') || line.startsWith('* ')) {
      elements.push(
        <li key={`li-${i}`} className="ml-4 list-disc text-sm text-gray-700 dark:text-gray-300">
          {renderInline(line.slice(2))}
        </li>,
      );
    } else if (line.trim() === '') {
      elements.push(<div key={`br-${i}`} className="h-2" />);
    } else {
      elements.push(
        <p key={`p-${i}`} className="text-sm text-gray-700 dark:text-gray-300">
          {renderInline(line)}
        </p>,
      );
    }
  }

  return (
    <div
      data-testid="markdown"
      className={cn(dimColor && 'opacity-60')}
    >
      {elements}
    </div>
  );
}

function renderInline(text: string): React.ReactNode {
  // Handle inline code, bold, italic
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let keyIdx = 0;

  while (remaining.length > 0) {
    // Inline code
    const codeMatch = remaining.match(/^(.*?)`([^`]+)`(.*)$/s);
    if (codeMatch) {
      if (codeMatch[1]) parts.push(<span key={keyIdx++}>{codeMatch[1]}</span>);
      parts.push(
        <code
          key={keyIdx++}
          className="rounded bg-gray-100 px-1 py-0.5 font-mono text-sm text-pink-600 dark:bg-gray-800 dark:text-pink-400"
        >
          {codeMatch[2]}
        </code>,
      );
      remaining = codeMatch[3];
      continue;
    }

    // Bold
    const boldMatch = remaining.match(/^(.*?)\*\*([^*]+)\*\*(.*)$/s);
    if (boldMatch) {
      if (boldMatch[1]) parts.push(<span key={keyIdx++}>{boldMatch[1]}</span>);
      parts.push(
        <strong key={keyIdx++} className="font-semibold text-gray-900 dark:text-gray-100">
          {boldMatch[2]}
        </strong>,
      );
      remaining = boldMatch[3];
      continue;
    }

    parts.push(<span key={keyIdx++}>{remaining}</span>);
    break;
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>;
}
