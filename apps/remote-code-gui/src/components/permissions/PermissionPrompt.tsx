import { useState, useCallback, useEffect } from 'react';
import { Shield, MessageSquare, ChevronDown, ChevronUp } from 'lucide-react';
import { cn } from '../../lib/utils';

export type FeedbackType = 'accept' | 'reject';

export interface PermissionPromptOption {
  value: string;
  label: string;
  description?: string;
  shortcutKey?: string;
  feedbackConfig?: {
    type: FeedbackType;
    placeholder?: string;
  };
  variant?: 'default' | 'success' | 'danger' | 'warning';
}

export interface ToolAnalyticsContext {
  toolName: string;
  isMcp: boolean;
}

export interface PermissionPromptProps {
  title?: string;
  description?: string;
  options: PermissionPromptOption[];
  onSelect: (value: string, feedback?: string) => void;
  onCancel?: () => void;
  className?: string;
  toolAnalyticsContext?: ToolAnalyticsContext;
  onAnalytics?: (event: string, data: Record<string, unknown>) => void;
}

const DEFAULT_PLACEHOLDERS: Record<FeedbackType, string> = {
  accept: '告诉 AI 接下来该做什么...',
  reject: '告诉 AI 该做什么不同...',
};

const VARIANT_STYLES: Record<string, { border: string; bg: string; text: string }> = {
  default: { border: 'border-slate-200 hover:border-blue-400', bg: 'hover:bg-blue-50', text: 'text-slate-700' },
  success: { border: 'border-emerald-200 hover:border-emerald-400', bg: 'hover:bg-emerald-50', text: 'text-emerald-700' },
  danger: { border: 'border-red-200 hover:border-red-400', bg: 'hover:bg-red-50', text: 'text-red-700' },
  warning: { border: 'border-amber-200 hover:border-amber-400', bg: 'hover:bg-amber-50', text: 'text-amber-700' },
};

export function PermissionPrompt({
  title = '是否继续？',
  description,
  options,
  onSelect,
  onCancel,
  className,
  toolAnalyticsContext,
  onAnalytics,
}: PermissionPromptProps) {
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [acceptFeedback, setAcceptFeedback] = useState('');
  const [rejectFeedback, setRejectFeedback] = useState('');
  const [acceptInputMode, setAcceptInputMode] = useState(false);
  const [rejectInputMode, setRejectInputMode] = useState(false);
  const [acceptFeedbackModeEntered, setAcceptFeedbackModeEntered] = useState(false);
  const [rejectFeedbackModeEntered, setRejectFeedbackModeEntered] = useState(false);

  const focusedOption = options[focusedIndex] ?? null;
  const focusedFeedbackType = focusedOption?.feedbackConfig?.type;
  const showTabHint =
    (focusedFeedbackType === 'accept' && !acceptInputMode) ||
    (focusedFeedbackType === 'reject' && !rejectInputMode);

  const logAnalyticsEvent = useCallback(
    (event: string, extra: Record<string, unknown> = {}) => {
      onAnalytics?.(event, {
        toolName: toolAnalyticsContext?.toolName,
        isMcp: toolAnalyticsContext?.isMcp ?? false,
        ...extra,
      });
    },
    [onAnalytics, toolAnalyticsContext],
  );

  const handleInputModeToggle = useCallback(
    (value: string) => {
      const option = options.find((o) => o.value === value);
      if (!option?.feedbackConfig) return;
      const { type } = option.feedbackConfig;
      const analyticsProps = {
        toolName: toolAnalyticsContext?.toolName,
        isMcp: toolAnalyticsContext?.isMcp ?? false,
      };

      if (type === 'accept') {
        if (acceptInputMode) {
          setAcceptInputMode(false);
          logAnalyticsEvent('accept_feedback_mode_collapsed', analyticsProps);
        } else {
          setAcceptInputMode(true);
          setAcceptFeedbackModeEntered(true);
          logAnalyticsEvent('accept_feedback_mode_entered', analyticsProps);
        }
      } else if (type === 'reject') {
        if (rejectInputMode) {
          setRejectInputMode(false);
          logAnalyticsEvent('reject_feedback_mode_collapsed', analyticsProps);
        } else {
          setRejectInputMode(true);
          setRejectFeedbackModeEntered(true);
          logAnalyticsEvent('reject_feedback_mode_entered', analyticsProps);
        }
      }
    },
    [acceptInputMode, rejectInputMode, options, toolAnalyticsContext, logAnalyticsEvent],
  );

  const handleSelect = useCallback(
    (value: string) => {
      const option = options.find((o) => o.value === value);
      if (!option) return;

      let feedback: string | undefined;
      if (option.feedbackConfig) {
        const rawFeedback =
          option.feedbackConfig.type === 'accept' ? acceptFeedback : rejectFeedback;
        const trimmedFeedback = rawFeedback.trim();
        if (trimmedFeedback) {
          feedback = trimmedFeedback;
        }
        const analyticsProps = {
          toolName: toolAnalyticsContext?.toolName,
          isMcp: toolAnalyticsContext?.isMcp ?? false,
          has_instructions: !!trimmedFeedback,
          instructions_length: trimmedFeedback?.length ?? 0,
          entered_feedback_mode:
            option.feedbackConfig.type === 'accept'
              ? acceptFeedbackModeEntered
              : rejectFeedbackModeEntered,
        };
        if (option.feedbackConfig.type === 'accept') {
          logAnalyticsEvent('accept_submitted', analyticsProps);
        } else {
          logAnalyticsEvent('reject_submitted', analyticsProps);
        }
      }
      onSelect(value, feedback);
    },
    [
      options,
      onSelect,
      acceptFeedback,
      rejectFeedback,
      acceptFeedbackModeEntered,
      rejectFeedbackModeEntered,
      toolAnalyticsContext,
      logAnalyticsEvent,
    ],
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Tab toggles input mode for focused option
      if (e.key === 'Tab' && focusedOption?.feedbackConfig) {
        e.preventDefault();
        handleInputModeToggle(focusedOption.value);
        return;
      }

      // Escape cancels
      if (e.key === 'Escape') {
        e.preventDefault();
        logAnalyticsEvent('permission_request_escape');
        onCancel?.();
        return;
      }

      // Arrow keys navigate
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        const dir = e.key === 'ArrowUp' ? -1 : 1;
        setFocusedIndex((prev) => {
          const next = prev + dir;
          if (next < 0) return options.length - 1;
          if (next >= options.length) return 0;
          return next;
        });
        return;
      }

      // Enter selects focused
      if (e.key === 'Enter') {
        e.preventDefault();
        handleSelect(focusedOption?.value ?? '');
        return;
      }

      // Shortcut keys
      for (const opt of options) {
        if (opt.shortcutKey && e.key.toLowerCase() === opt.shortcutKey.toLowerCase()) {
          e.preventDefault();
          handleSelect(opt.value);
          return;
        }
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [options, focusedOption, handleSelect, handleInputModeToggle, onCancel, logAnalyticsEvent]);

  const getActiveFeedbackValue = (type: FeedbackType): string =>
    type === 'accept' ? acceptFeedback : rejectFeedback;

  const getActiveFeedbackSetter = (type: FeedbackType): ((v: string) => void) =>
    type === 'accept' ? setAcceptFeedback : setRejectFeedback;

  const isInputModeActive = (type: FeedbackType): boolean =>
    type === 'accept' ? acceptInputMode : rejectInputMode;

  const renderFeedbackInput = (option: PermissionPromptOption) => {
    if (!option.feedbackConfig) return null;
    const { type, placeholder } = option.feedbackConfig;
    if (!isInputModeActive(type)) return null;

    const currentValue = getActiveFeedbackValue(type);
    const setter = getActiveFeedbackSetter(type);

    return (
      <div className="mt-2" data-testid={`feedback-input-${type}`}>
        <div className="flex items-center gap-1 text-xs text-slate-400 mb-1">
          <MessageSquare className="h-3 w-3" />
          <span>{type === 'accept' ? '接受反馈' : '拒绝反馈'}</span>
        </div>
        <textarea
          className="w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-400 resize-none"
          rows={2}
          placeholder={placeholder ?? DEFAULT_PLACEHOLDERS[type]}
          value={currentValue}
          onChange={(e) => setter(e.target.value)}
          data-testid={`feedback-textarea-${type}`}
        />
        <button
          type="button"
          className="mt-1 text-xs text-slate-400 hover:text-slate-600 flex items-center gap-1"
          onClick={() => handleInputModeToggle(option.value)}
          data-testid={`feedback-collapse-${type}`}
        >
          <ChevronUp className="h-3 w-3" />
          收起反馈
        </button>
      </div>
    );
  };

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="permission-prompt"
    >
      {/* Header */}
      <div className="flex items-center gap-2">
        <Shield className="h-5 w-5 text-orange-500" />
        <h4 className="font-semibold text-slate-800">{title}</h4>
      </div>
      {description && <p className="mt-1 text-sm text-slate-500">{description}</p>}

      {/* Options list */}
      <ul className="mt-3 space-y-2" role="listbox" data-testid="permission-options-list">
        {options.map((opt, i) => {
          const variant = opt.variant ?? 'default';
          const styles = VARIANT_STYLES[variant] ?? VARIANT_STYLES.default;
          const isFocused = i === focusedIndex;

          return (
            <li key={opt.value} role="option" aria-selected={String(isFocused) as 'true' | 'false'}>
              <button
                className={cn(
                  'w-full rounded-lg border px-3 py-2 text-left text-sm transition-all',
                  styles.border,
                  styles.bg,
                  styles.text,
                  isFocused && 'ring-2 ring-blue-400 ring-offset-1',
                )}
                onClick={() => {
                  setFocusedIndex(i);
                  handleSelect(opt.value);
                }}
                onMouseEnter={() => setFocusedIndex(i)}
                data-testid={`permission-option-${opt.value}`}
                data-focused={isFocused}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{opt.label}</span>
                    {opt.shortcutKey && (
                      <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px] font-mono text-slate-400">
                        {opt.shortcutKey}
                      </kbd>
                    )}
                  </div>
                  {opt.feedbackConfig && !isInputModeActive(opt.feedbackConfig.type) && (
                    <span
                      className="text-xs text-slate-400 flex items-center gap-0.5"
                      data-testid={`tab-hint-${opt.value}`}
                    >
                      <ChevronDown className="h-3 w-3" />
                      Tab 展开
                    </span>
                  )}
                </div>
                {opt.description && (
                  <p className="mt-0.5 text-xs text-slate-400">{opt.description}</p>
                )}
              </button>
              {isFocused && renderFeedbackInput(opt)}
            </li>
          );
        })}
      </ul>

      {/* Footer hints */}
      <div className="mt-3 flex items-center gap-4 text-xs text-slate-400">
        <span>Esc 取消</span>
        {showTabHint && <span>· Tab 添加反馈</span>}
        <span>· ↑↓ 导航</span>
        <span>· Enter 确认</span>
      </div>
    </div>
  );
}
