import { useCallback, useRef, useEffect, useState } from 'react';
import {
  ChevronDown,
  ChevronUp,
  Command,
  Image,
  Loader2,
  Sparkles,
  X,
  Zap,
  Brain,
  Shield,
  Hash,
} from 'lucide-react';
import { cn } from '../../lib/utils';
import { getModeFromInput } from './inputModes';
import { hasImageInClipboard, extractImageFiles } from './inputPaste';

/** PromptInput 组件属性 */
export interface PromptInputProps {
  /** 当前输入值 */
  value: string;
  /** 输入变化回调 */
  onChange: (value: string) => void;
  /** 提交回调 */
  onSubmit: (value: string) => void;
  /** 是否禁用 */
  disabled?: boolean;
  /** 占位文本 */
  placeholder?: string;
  /** 额外 CSS 类名 */
  className?: string;
  /** 当前模型名称 */
  modelName?: string;
  /** 权限模式 */
  permissionMode?: 'auto-allow' | 'ask' | 'deny';
  /** Token 用量 */
  tokenUsage?: { input: number; output: number } | null;
  /** 是否启用 thinking 模式 */
  thinkingEnabled?: boolean;
  /** thinking 模式切换回调 */
  onThinkingToggle?: () => void;
  /** 是否为 fast 模式 */
  fastMode?: boolean;
  /** 是否正在加载 */
  isLoading?: boolean;
  /** 模型选择回调 */
  onModelSelect?: () => void;
}

/** 最大自动扩展行数 */
const MAX_ROWS = 10;

/** 单行高度（像素） */
const LINE_HEIGHT = 24;

/** 斜杠命令定义 */
export interface SlashCommand {
  name: string;
  description: string;
  usage?: string;
}

/** 内置斜杠命令列表 */
const SLASH_COMMANDS: SlashCommand[] = [
  { name: '/help', description: '显示帮助信息' },
  { name: '/model', description: '切换模型' },
  { name: '/clear', description: '清空对话' },
  { name: '/compact', description: '压缩上下文' },
  { name: '/cost', description: '显示费用统计' },
  { name: '/doctor', description: '运行诊断检查' },
  { name: '/init', description: '初始化项目' },
  { name: '/login', description: '登录账户' },
  { name: '/logout', description: '登出账户' },
  { name: '/permissions', description: '管理权限' },
  { name: '/review', description: '代码审查' },
  { name: '/status', description: '显示运行状态' },
  { name: '/tasks', description: '管理任务' },
  { name: '/vim', description: '切换 vim 模式' },
];

/** @ 提及建议项 */
export interface MentionSuggestion {
  id: string;
  label: string;
  type: 'file' | 'agent' | 'tool';
}

/** 内置 @ 提及建议 */
const MENTION_SUGGESTIONS: MentionSuggestion[] = [
  { id: 'file', label: '文件', type: 'file' },
  { id: 'agent', label: '代理', type: 'agent' },
  { id: 'tool', label: '工具', type: 'tool' },
];

/**
 * 过滤匹配的斜杠命令。
 */
function filterSlashCommands(input: string): SlashCommand[] {
  if (!input.startsWith('/')) return [];
  const query = input.toLowerCase();
  return SLASH_COMMANDS.filter(
    (cmd) =>
      cmd.name.toLowerCase().startsWith(query) ||
      cmd.description.toLowerCase().includes(query.slice(1)),
  );
}

/**
 * 过滤匹配的 @ 提及建议。
 */
function filterMentions(input: string): MentionSuggestion[] {
  const atIndex = input.lastIndexOf('@');
  if (atIndex === -1) return [];
  const query = input.slice(atIndex + 1).toLowerCase();
  return MENTION_SUGGESTIONS.filter(
    (s) =>
      s.label.toLowerCase().includes(query) ||
      s.type.toLowerCase().includes(query),
  );
}

/**
 * 格式化 token 数量为可读字符串。
 */
function formatTokenCount(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return String(count);
}

/**
 * 高级输入框组件。
 * 支持多行输入、Shift+Enter 换行、Enter 提交、bash 模式、粘贴图片预览。
 * 增强功能：斜杠命令 typeahead、历史搜索、模型选择器、权限模式指示器、
 * thinking toggle、fast mode、token 用量、@ 提及补全、字数统计。
 */
export function PromptInput({
  value,
  onChange,
  onSubmit,
  disabled = false,
  placeholder = '输入需求，Shift+Enter 换行...',
  className,
  modelName,
  permissionMode,
  tokenUsage,
  thinkingEnabled,
  onThinkingToggle,
  fastMode,
  isLoading = false,
  onModelSelect,
}: PromptInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [pastedImages, setPastedImages] = useState<string[]>([]);
  const [slashCommands, setSlashCommands] = useState<SlashCommand[]>([]);
  const [selectedCommandIndex, setSelectedCommandIndex] = useState(0);
  const [mentionResults, setMentionResults] = useState<MentionSuggestion[]>([]);
  const [selectedMentionIndex, setSelectedMentionIndex] = useState(0);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [showToolbar, setShowToolbar] = useState(true);

  const mode = getModeFromInput(value);
  const isBashMode = mode === 'bash';
  const charCount = value.length;
  const isDisabled = disabled || isLoading;

  /** 自动调整 textarea 高度 */
  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = 'auto';
    const maxHeight = LINE_HEIGHT * MAX_ROWS;
    textarea.style.height = `${Math.min(textarea.scrollHeight, maxHeight)}px`;
  }, []);

  useEffect(() => {
    adjustHeight();
  }, [value, adjustHeight]);

  /** 更新斜杠命令建议 */
  useEffect(() => {
    const commands = filterSlashCommands(value);
    setSlashCommands(commands);
    setSelectedCommandIndex(0);
  }, [value]);

  /** 更新 @ 提及建议 */
  useEffect(() => {
    const mentions = filterMentions(value);
    setMentionResults(mentions);
    setSelectedMentionIndex(0);
  }, [value]);

  /** 提交并保存到历史 */
  const handleSubmit = useCallback(
    (submitValue: string) => {
      if (submitValue.trim()) {
        setHistory((prev) => {
          const next = [submitValue, ...prev.filter((h) => h !== submitValue)];
          return next.slice(0, 100);
        });
        setHistoryIndex(-1);
        onSubmit(submitValue);
      }
    },
    [onSubmit],
  );

  /** 键盘事件处理 */
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // 斜杠命令导航
      if (slashCommands.length > 0) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          setSelectedCommandIndex((prev) =>
            prev < slashCommands.length - 1 ? prev + 1 : 0,
          );
          return;
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          setSelectedCommandIndex((prev) =>
            prev > 0 ? prev - 1 : slashCommands.length - 1,
          );
          return;
        }
        if (e.key === 'Tab' || e.key === 'Enter') {
          e.preventDefault();
          const cmd = slashCommands[selectedCommandIndex];
          if (cmd) {
            onChange(cmd.name + ' ');
            setSlashCommands([]);
          }
          return;
        }
        if (e.key === 'Escape') {
          setSlashCommands([]);
          return;
        }
      }

      // @ 提及导航
      if (mentionResults.length > 0 && slashCommands.length === 0) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          setSelectedMentionIndex((prev) =>
            prev < mentionResults.length - 1 ? prev + 1 : 0,
          );
          return;
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          setSelectedMentionIndex((prev) =>
            prev > 0 ? prev - 1 : mentionResults.length - 1,
          );
          return;
        }
        if (e.key === 'Tab') {
          e.preventDefault();
          const mention = mentionResults[selectedMentionIndex];
          if (mention) {
            const atIndex = value.lastIndexOf('@');
            const newValue = value.slice(0, atIndex) + '@' + mention.label + ' ';
            onChange(newValue);
            setMentionResults([]);
          }
          return;
        }
        if (e.key === 'Escape') {
          setMentionResults([]);
          return;
        }
      }

      // 历史导航（上下箭头键，仅在无建议时）
      if (
        slashCommands.length === 0 &&
        mentionResults.length === 0 &&
        history.length > 0
      ) {
        if (e.key === 'ArrowUp' && !e.shiftKey) {
          e.preventDefault();
          const nextIndex = Math.min(historyIndex + 1, history.length - 1);
          setHistoryIndex(nextIndex);
          onChange(history[nextIndex]);
          return;
        }
        if (e.key === 'ArrowDown' && !e.shiftKey) {
          e.preventDefault();
          if (historyIndex > 0) {
            const nextIndex = historyIndex - 1;
            setHistoryIndex(nextIndex);
            onChange(history[nextIndex]);
          } else if (historyIndex === 0) {
            setHistoryIndex(-1);
            onChange('');
          }
          return;
        }
      }

      // Enter 提交
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (value.trim() && !isDisabled) {
          handleSubmit(value);
        }
      }
    },
    [
      slashCommands,
      selectedCommandIndex,
      mentionResults,
      selectedMentionIndex,
      history,
      historyIndex,
      value,
      isDisabled,
      onChange,
      handleSubmit,
    ],
  );

  /** 粘贴事件处理 */
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (hasImageInClipboard(e.nativeEvent)) {
        e.preventDefault();
        const files = extractImageFiles(e.clipboardData.items);
        const names = files.map((f) => f.name || 'image');
        setPastedImages((prev) => [...prev, ...names]);
      }
    },
    [],
  );

  /** 移除粘贴图片标签 */
  const removePastedImage = useCallback((index: number) => {
    setPastedImages((prev) => prev.filter((_, i) => i !== index));
  }, []);

  /** 选择斜杠命令 */
  const selectCommand = useCallback(
    (cmd: SlashCommand) => {
      onChange(cmd.name + ' ');
      setSlashCommands([]);
      textareaRef.current?.focus();
    },
    [onChange],
  );

  /** 选择 @ 提及 */
  const selectMention = useCallback(
    (mention: MentionSuggestion) => {
      const atIndex = value.lastIndexOf('@');
      const newValue = value.slice(0, atIndex) + '@' + mention.label + ' ';
      onChange(newValue);
      setMentionResults([]);
      textareaRef.current?.focus();
    },
    [value, onChange],
  );

  /** 获取权限模式的显示文本和颜色 */
  const getPermissionModeDisplay = useCallback(() => {
    switch (permissionMode) {
      case 'auto-allow':
        return { text: 'Auto', color: 'text-green-600 dark:text-green-400' };
      case 'ask':
        return { text: 'Ask', color: 'text-amber-600 dark:text-amber-400' };
      case 'deny':
        return { text: 'Deny', color: 'text-red-600 dark:text-red-400' };
      default:
        return null;
    }
  }, [permissionMode]);

  const permDisplay = getPermissionModeDisplay();

  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border border-slate-200 bg-white dark:border-slate-700 dark:bg-slate-900',
        isBashMode && 'border-red-300 dark:border-red-700',
        isDisabled && 'opacity-50 cursor-not-allowed',
        className,
      )}
      data-testid="prompt-input"
    >
      {/* 粘贴图片预览标签 */}
      {pastedImages.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-3 pt-2" data-testid="pasted-images">
          {pastedImages.map((name, idx) => (
            <span
              key={idx}
              className="inline-flex items-center gap-1 rounded-md bg-blue-50 px-2 py-0.5 text-xs text-blue-700 dark:bg-blue-900/30 dark:text-blue-300"
              data-testid={`pasted-image-${idx}`}
            >
              <Image className="h-3 w-3" />
              {name}
              <button
                type="button"
                onClick={() => removePastedImage(idx)}
                className="ml-0.5 text-blue-400 hover:text-blue-600"
                aria-label={`移除图片 ${name}`}
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
      )}

      {/* 斜杠命令 typeahead */}
      {slashCommands.length > 0 && (
        <div
          className="max-h-60 overflow-y-auto border-b border-slate-100 dark:border-slate-800"
          data-testid="slash-commands"
        >
          {slashCommands.map((cmd, idx) => (
            <button
              key={cmd.name}
              type="button"
              className={cn(
                'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors',
                idx === selectedCommandIndex
                  ? 'bg-blue-50 text-blue-700 dark:bg-blue-900/20 dark:text-blue-300'
                  : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800',
              )}
              onClick={() => selectCommand(cmd)}
              data-testid={`slash-command-${cmd.name.slice(1)}`}
            >
              <Command className="h-3.5 w-3.5 shrink-0 text-slate-400" />
              <span className="font-mono font-medium">{cmd.name}</span>
              <span className="ml-2 text-xs text-slate-400">
                {cmd.description}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* @ 提及建议 */}
      {mentionResults.length > 0 && slashCommands.length === 0 && (
        <div
          className="max-h-40 overflow-y-auto border-b border-slate-100 dark:border-slate-800"
          data-testid="mention-suggestions"
        >
          {mentionResults.map((mention, idx) => (
            <button
              key={mention.id}
              type="button"
              className={cn(
                'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors',
                idx === selectedMentionIndex
                  ? 'bg-purple-50 text-purple-700 dark:bg-purple-900/20 dark:text-purple-300'
                  : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800',
              )}
              onClick={() => selectMention(mention)}
              data-testid={`mention-${mention.id}`}
            >
              <Hash className="h-3.5 w-3.5 shrink-0 text-slate-400" />
              <span className="font-medium">@{mention.label}</span>
              <span className="ml-auto rounded-full bg-slate-100 px-1.5 py-0.5 text-[10px] text-slate-500 dark:bg-slate-800">
                {mention.type}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* 输入区域 */}
      <div className="relative">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          disabled={isDisabled}
          placeholder={placeholder}
          rows={1}
          className={cn(
            'w-full resize-none border-0 bg-transparent px-3 py-2.5 text-sm text-slate-900 placeholder:text-slate-400 focus:outline-none dark:text-slate-100 dark:placeholder:text-slate-500',
            isBashMode && 'font-mono text-red-600 dark:text-red-400',
          )}
          style={{ maxHeight: `${LINE_HEIGHT * MAX_ROWS}px` }}
        />

        {/* 加载指示器 */}
        {isLoading && (
          <div className="absolute right-3 top-1/2 -translate-y-1/2">
            <Loader2
              className="h-4 w-4 animate-spin text-blue-500"
              data-testid="loading-indicator"
            />
          </div>
        )}
      </div>

      {/* 底部工具栏 */}
      {showToolbar && (
        <div
          className="flex items-center gap-2 border-t border-slate-100 px-3 py-1.5 dark:border-slate-800"
          data-testid="prompt-toolbar"
        >
          {/* 模型名称 */}
          {modelName && (
            <button
              type="button"
              className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
              onClick={onModelSelect}
              data-testid="toolbar-model"
              title="切换模型"
            >
              <Sparkles className="h-3 w-3" />
              {modelName}
            </button>
          )}

          {/* 权限模式 */}
          {permDisplay && (
            <span
              className={cn(
                'inline-flex items-center gap-1 text-xs',
                permDisplay.color,
              )}
              data-testid="toolbar-permission"
            >
              <Shield className="h-3 w-3" />
              {permDisplay.text}
            </span>
          )}

          {/* Thinking toggle */}
          {onThinkingToggle && (
            <button
              type="button"
              className={cn(
                'inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs transition-colors',
                thinkingEnabled
                  ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300'
                  : 'text-slate-400 hover:text-slate-600 dark:hover:text-slate-300',
              )}
              onClick={onThinkingToggle}
              data-testid="toolbar-thinking"
              title={thinkingEnabled ? '关闭思考模式' : '开启思考模式'}
            >
              <Brain className="h-3 w-3" />
              {thinkingEnabled ? 'Thinking' : 'Think'}
            </button>
          )}

          {/* Fast mode 指示器 */}
          {fastMode && (
            <span
              className="inline-flex items-center gap-1 text-xs text-yellow-600 dark:text-yellow-400"
              data-testid="toolbar-fast-mode"
            >
              <Zap className="h-3 w-3" />
              Fast
            </span>
          )}

          {/* Token 用量 */}
          {tokenUsage && (
            <span
              className="inline-flex items-center gap-1 text-xs text-slate-400"
              data-testid="toolbar-tokens"
            >
              <span title={`输入: ${tokenUsage.input} / 输出: ${tokenUsage.output}`}>
                {formatTokenCount(tokenUsage.input)}↓ {formatTokenCount(tokenUsage.output)}↑
              </span>
            </span>
          )}

          {/* 弹性空间 */}
          <div className="flex-1" />

          {/* 字数统计 */}
          <span
            className="text-[10px] text-slate-300 dark:text-slate-600"
            data-testid="toolbar-char-count"
          >
            {charCount}
          </span>

          {/* 工具栏折叠按钮 */}
          <button
            type="button"
            className="text-slate-300 hover:text-slate-500 dark:text-slate-600 dark:hover:text-slate-400"
            onClick={() => setShowToolbar(false)}
            aria-label="折叠工具栏"
            data-testid="toolbar-collapse"
          >
            <ChevronDown className="h-3 w-3" />
          </button>
        </div>
      )}

      {/* 工具栏折叠时显示展开按钮 */}
      {!showToolbar && (
        <div className="flex justify-center border-t border-slate-100 py-0.5 dark:border-slate-800">
          <button
            type="button"
            className="text-slate-300 hover:text-slate-500 dark:text-slate-600 dark:hover:text-slate-400"
            onClick={() => setShowToolbar(true)}
            aria-label="展开工具栏"
            data-testid="toolbar-expand"
          >
            <ChevronUp className="h-3 w-3" />
          </button>
        </div>
      )}
    </div>
  );
}
