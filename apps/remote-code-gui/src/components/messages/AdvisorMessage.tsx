import { useState, useMemo } from 'react';
import {
  Lightbulb,
  ChevronDown,
  ChevronUp,
  AlertCircle,
  CheckCircle2,
  Cpu,
  Loader2,
  Wrench,
} from 'lucide-react';
import { cn } from '../../lib/utils';

export type AdvisorBlockType =
  | 'text'
  | 'tool_result_error'
  | 'advisor_result'
  | 'advisor_redacted_result'
  | 'server_tool_use';

export interface AdvisorBlock {
  type: AdvisorBlockType;
  id?: string;
  content?: string;
  error_code?: string;
  text?: string;
  input?: Record<string, unknown>;
}

export interface AdvisorMessageProps {
  content: string;
  sender?: string;
  timestamp?: string;
  className?: string;
  /** Structured advisor block data */
  block?: AdvisorBlock;
  /** Whether the tool use is still in progress */
  isLoading?: boolean;
  /** Whether the tool use resulted in an error */
  hasError?: boolean;
  /** Model name used by the advisor */
  modelName?: string;
  /** Whether to show verbose output */
  verbose?: boolean;
  /** Set of resolved tool use IDs */
  resolvedToolUseIDs?: Set<string>;
  /** Set of errored tool use IDs */
  erroredToolUseIDs?: Set<string>;
}

function formatJsonInput(input: Record<string, unknown>): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input);
  }
}

export function AdvisorMessage({
  content,
  sender = 'Advisor',
  timestamp,
  className,
  block,
  isLoading = false,
  hasError: _hasError = false,
  modelName,
  verbose = false,
  resolvedToolUseIDs,
  erroredToolUseIDs,
}: AdvisorMessageProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  // Determine if tool use is resolved or errored
  const isToolResolved = block?.id ? resolvedToolUseIDs?.has(block.id) ?? false : false;
  const isToolErrored = block?.id ? erroredToolUseIDs?.has(block.id) ?? false : false;

  // Determine block rendering
  const blockBody = useMemo(() => {
    if (!block) {
      return (
        <p className="text-sm whitespace-pre-wrap text-purple-900 dark:text-purple-200">
          {content}
        </p>
      );
    }

    switch (block.type) {
      case 'server_tool_use': {
        const inputStr =
          block.input && Object.keys(block.input).length > 0
            ? formatJsonInput(block.input)
            : null;
        return (
          <div data-testid="advisor-tool-use">
            <div className="flex items-center gap-2">
              {isLoading && !isToolResolved ? (
                <Loader2 className="h-4 w-4 animate-spin text-purple-400" data-testid="advisor-tool-loading" />
              ) : isToolErrored ? (
                <AlertCircle className="h-4 w-4 text-red-400" data-testid="advisor-tool-error" />
              ) : (
                <CheckCircle2 className="h-4 w-4 text-emerald-400" />
              )}
              <Wrench className="h-4 w-4 text-purple-400" />
              <span className="text-xs font-semibold text-purple-700">Advising</span>
              {modelName && (
                <span className="flex items-center gap-0.5 text-[10px] text-slate-400">
                  <Cpu className="h-2.5 w-2.5" />
                  {modelName}
                </span>
              )}
            </div>
            {inputStr && (
              <div className="mt-2">
                <button
                  type="button"
                  className="flex items-center gap-1 text-[10px] text-slate-400 hover:text-slate-600"
                  onClick={() => setIsExpanded((prev) => !prev)}
                  data-testid="advisor-tool-input-toggle"
                >
                  {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                  工具输入
                </button>
                {isExpanded && (
                  <pre className="mt-1 max-h-40 overflow-auto rounded-md bg-slate-50 p-2 text-[11px] font-mono text-slate-600">
                    {inputStr}
                  </pre>
                )}
              </div>
            )}
          </div>
        );
      }

      case 'tool_result_error': {
        return (
          <div className="flex items-start gap-2" data-testid="advisor-error">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-red-400" />
            <div>
              <p className="text-xs font-medium text-red-600">
                Advisor 不可用
              </p>
              {block.error_code && (
                <p className="mt-0.5 text-[11px] text-red-400">
                  错误代码: {block.error_code}
                </p>
              )}
            </div>
          </div>
        );
      }

      case 'advisor_result': {
        const resultText = block.text || block.content || '';
        if (verbose) {
          return (
            <div data-testid="advisor-result-verbose">
              <div className="flex items-start gap-2">
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-400" />
                <p className="text-xs text-slate-500 whitespace-pre-wrap">{resultText}</p>
              </div>
            </div>
          );
        }
        return (
          <div className="flex items-center gap-2" data-testid="advisor-result">
            <CheckCircle2 className="h-4 w-4 text-emerald-400" />
            <span className="text-xs text-slate-500">
              Advisor 已审查对话并将应用反馈
            </span>
            <button
              type="button"
              className="flex items-center gap-0.5 text-[10px] text-purple-400 hover:text-purple-600"
              onClick={() => setIsExpanded((prev) => !prev)}
            >
              {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
              {isExpanded ? '收起' : '展开'}
            </button>
          </div>
        );
      }

      case 'advisor_redacted_result': {
        return (
          <div className="flex items-center gap-2" data-testid="advisor-redacted">
            <CheckCircle2 className="h-4 w-4 text-emerald-400" />
            <span className="text-xs text-slate-500">
              Advisor 已审查对话并将应用反馈
            </span>
          </div>
        );
      }

      case 'text':
      default: {
        const textContent = block.text || block.content || content;
        return (
          <p className="text-sm whitespace-pre-wrap text-purple-900 dark:text-purple-200">
            {textContent}
          </p>
        );
      }
    }
  }, [block, content, isLoading, isToolResolved, isToolErrored, modelName, verbose, isExpanded]);

  // For server_tool_use blocks, render with tool styling
  if (block?.type === 'server_tool_use') {
    return (
      <div
        className={cn(
          'rounded-lg border border-purple-200 bg-purple-50 px-4 py-3 dark:border-purple-800 dark:bg-purple-950/30',
          className,
        )}
        data-testid="advisor-message"
      >
        {blockBody}
        {/* Expanded advisor result text */}
        {isExpanded && block.text && (
          <div className="mt-2 rounded-md bg-white/50 p-2">
            <p className="text-xs text-slate-500 whitespace-pre-wrap">{block.text}</p>
          </div>
        )}
      </div>
    );
  }

  // For error blocks
  if (block?.type === 'tool_result_error') {
    return (
      <div
        className={cn(
          'rounded-lg border border-red-200 bg-red-50 px-4 py-3',
          className,
        )}
        data-testid="advisor-message"
      >
        {blockBody}
      </div>
    );
  }

  // Default text rendering
  return (
    <div
      className={cn(
        'rounded-lg border border-purple-200 bg-purple-50 px-4 py-3 dark:border-purple-800 dark:bg-purple-950/30',
        className,
      )}
      data-testid="advisor-message"
    >
      <div className="flex items-start gap-2">
        <Lightbulb className="mt-0.5 h-4 w-4 shrink-0 text-purple-500" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-purple-700 dark:text-purple-400">
              {sender}
            </span>
            {modelName && (
              <span className="flex items-center gap-0.5 text-[10px] text-slate-400">
                <Cpu className="h-2.5 w-2.5" />
                {modelName}
              </span>
            )}
            {timestamp && (
              <span className="text-xs text-purple-400">{timestamp}</span>
            )}
          </div>
          {blockBody}
        </div>
      </div>
    </div>
  );
}
