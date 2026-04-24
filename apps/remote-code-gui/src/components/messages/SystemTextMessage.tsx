import { memo, useState, useCallback } from 'react';
import {
  AlertTriangle,
  AlertCircle,
  Info,
  Minimize2,
  Clock,
  Brain,
  HardDrive,
  Wifi,
  WifiOff,
  ChevronDown,
  ChevronRight,
  FileText,
  Zap,
  Shield,
  RotateCcw,
  StopCircle,
  CheckCircle2,
  XCircle,
  Activity,
} from 'lucide-react';
import type { ConversationEntry } from '../../lib/types';
import { cn } from '../../lib/utils';

/** 系统文本消息组件属性 */
export interface SystemTextMessageProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 是否显示详细信息 */
  verbose?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 系统消息子类型 */
type SystemKind =
  | 'compacted'
  | 'error'
  | 'warning'
  | 'turn_duration'
  | 'memory_saved'
  | 'thinking'
  | 'bridge_connected'
  | 'bridge_disconnected'
  | 'stop_hook_summary'
  | 'compact_boundary'
  | 'permission_retry'
  | 'scheduled_task'
  | 'default';

/** 检测到的子类型信息 */
interface DetectedKind {
  kind: SystemKind;
  /** 提取的元数据 */
  meta?: Record<string, string>;
}

/**
 * 精确检测系统消息子类型。
 * 使用多级模式匹配，优先匹配特定模式。
 */
function detectSystemKind(text: string): DetectedKind {
  const lower = text.toLowerCase();

  // Turn duration — 匹配 "turn completed in X.Xs" 或 "本轮耗时"
  const turnDurationMatch = text.match(
    /(?:turn|本轮).*(?:completed in|耗时)\s*([\d.]+)\s*(?:s|秒)/i,
  );
  if (turnDurationMatch) {
    return {
      kind: 'turn_duration',
      meta: { duration: turnDurationMatch[1] },
    };
  }
  if (lower.includes('duration:') || lower.includes('elapsed:')) {
    return { kind: 'turn_duration' };
  }

  // Memory saved — 匹配 "memory saved" 或 "记忆已保存"
  if (
    lower.includes('memory saved') ||
    lower.includes('记忆已保存') ||
    lower.includes('saved to memory')
  ) {
    return { kind: 'memory_saved' };
  }

  // Thinking
  if (lower.includes('thinking') && (lower.includes('...') || lower.includes('中'))) {
    return { kind: 'thinking' };
  }

  // Bridge disconnected (check BEFORE connected since "disconnected" contains "connected")
  if (
    (lower.includes('bridge') && lower.includes('disconnected')) ||
    lower.includes('ide disconnected') ||
    lower.includes('bridge 已断开')
  ) {
    return { kind: 'bridge_disconnected' };
  }

  // Bridge connected
  if (
    (lower.includes('bridge') && lower.includes('connected')) ||
    lower.includes('ide connected') ||
    lower.includes('bridge 已连接')
  ) {
    return { kind: 'bridge_connected' };
  }

  // Stop hook summary
  if (
    lower.includes('stop hook') ||
    lower.includes('hooks executed') ||
    lower.includes('停止 hook')
  ) {
    return { kind: 'stop_hook_summary' };
  }

  // Compact boundary
  if (
    lower.includes('compact boundary') ||
    lower.includes('compaction boundary') ||
    lower.includes('压缩边界')
  ) {
    return { kind: 'compact_boundary' };
  }

  // Permission retry
  if (
    lower.includes('permission retry') ||
    lower.includes('retrying with') ||
    lower.includes('权限重试')
  ) {
    return { kind: 'permission_retry' };
  }

  // Scheduled task
  if (
    lower.includes('scheduled task') ||
    lower.includes('定时任务') ||
    lower.includes('cron')
  ) {
    return { kind: 'scheduled_task' };
  }

  // Compacted
  if (
    lower.includes('compacted') ||
    lower.includes('compaction') ||
    lower.includes('压缩')
  ) {
    return { kind: 'compacted' };
  }

  // Error
  if (lower.includes('error') || lower.includes('错误') || lower.includes('failed')) {
    return { kind: 'error' };
  }

  // Warning
  if (lower.includes('warning') || lower.includes('警告') || lower.includes('warn:')) {
    return { kind: 'warning' };
  }

  return { kind: 'default' };
}

/** 获取子类型对应的图标 */
function getKindIcon(kind: SystemKind) {
  switch (kind) {
    case 'compacted':
    case 'compact_boundary':
      return <Minimize2 className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'error':
      return <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'warning':
      return <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'turn_duration':
      return <Clock className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'thinking':
      return <Brain className="mt-0.5 h-3.5 w-3.5 shrink-0 animate-pulse" />;
    case 'memory_saved':
      return <HardDrive className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'bridge_connected':
      return <Wifi className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'bridge_disconnected':
      return <WifiOff className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'stop_hook_summary':
      return <StopCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'permission_retry':
      return <RotateCcw className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    case 'scheduled_task':
      return <Zap className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
    default:
      return <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />;
  }
}

/** 获取子类型对应的样式类 */
function getKindClasses(kind: SystemKind): string {
  switch (kind) {
    case 'compacted':
    case 'compact_boundary':
      return 'border-slate-300 bg-slate-100 text-slate-600 dark:border-slate-600 dark:bg-slate-800/50 dark:text-slate-400';
    case 'error':
      return 'border-red-200 bg-red-50 text-red-700 dark:border-red-800 dark:bg-red-950/30 dark:text-red-400';
    case 'warning':
      return 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-400';
    case 'turn_duration':
      return 'border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-800 dark:bg-blue-950/30 dark:text-blue-400';
    case 'thinking':
      return 'border-purple-200 bg-purple-50 text-purple-700 dark:border-purple-800 dark:bg-purple-950/30 dark:text-purple-400';
    case 'memory_saved':
      return 'border-green-200 bg-green-50 text-green-700 dark:border-green-800 dark:bg-green-950/30 dark:text-green-400';
    case 'bridge_connected':
      return 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-800 dark:bg-emerald-950/30 dark:text-emerald-400';
    case 'bridge_disconnected':
      return 'border-orange-200 bg-orange-50 text-orange-700 dark:border-orange-800 dark:bg-orange-950/30 dark:text-orange-400';
    case 'stop_hook_summary':
      return 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-800 dark:bg-rose-950/30 dark:text-rose-400';
    case 'permission_retry':
      return 'border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-800 dark:bg-indigo-950/30 dark:text-indigo-400';
    case 'scheduled_task':
      return 'border-yellow-200 bg-yellow-50 text-yellow-700 dark:border-yellow-800 dark:bg-yellow-950/30 dark:text-yellow-400';
    default:
      return 'border-slate-200 bg-slate-50 text-slate-500 dark:border-slate-700 dark:bg-slate-800/30 dark:text-slate-400';
  }
}

/** 可折叠内容组件 */
function CollapsibleContent({
  text,
  maxLength = 200,
  verbose,
}: {
  text: string;
  maxLength?: number;
  verbose: boolean;
}) {
  const [expanded, setExpanded] = useState(verbose);
  const needsTruncation = text.length > maxLength;

  const toggleExpand = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  if (verbose || !needsTruncation) {
    return <span className="whitespace-pre-wrap break-words leading-5">{text}</span>;
  }

  return (
    <div>
      <span className="whitespace-pre-wrap break-words leading-5">
        {expanded ? text : text.slice(0, maxLength)}
      </span>
      {needsTruncation && (
        <button
          type="button"
          className="ml-1 inline-flex items-center gap-0.5 text-xs text-blue-500 hover:text-blue-600 dark:text-blue-400"
          onClick={toggleExpand}
          data-testid="system-message-toggle"
        >
          {expanded ? (
            <>
              <ChevronDown className="h-3 w-3" />
              收起
            </>
          ) : (
            <>
              <ChevronRight className="h-3 w-3" />
              展开全文
            </>
          )}
        </button>
      )}
    </div>
  );
}

/** 文件路径链接组件 */
function FilePathLink({ path }: { path: string }) {
  return (
    <span
      className="inline-flex items-center gap-0.5 font-mono text-blue-600 hover:underline dark:text-blue-400"
      title={path}
      data-testid="file-path-link"
    >
      <FileText className="h-3 w-3" />
      {path}
    </span>
  );
}

/**
 * 从文本中提取文件路径。
 */
function extractFilePaths(text: string): string[] {
  const filePattern = /(?:^|[\s(])([\w./\-]+\.(?:ts|tsx|js|jsx|rs|toml|json|yaml|yml|md|py|go|c|cpp|h|java|rb|sh|css|html|sql|proto|graphql|vue|svelte))/g;
  const paths: string[] = [];
  let match: RegExpExecArray | null;
  while ((match = filePattern.exec(text)) !== null) {
    paths.push(match[1]);
  }
  return [...new Set(paths)];
}

/**
 * TurnDuration 子消息渲染。
 */
function TurnDurationMessage({ text, meta }: { text: string; meta?: Record<string, string> }) {
  const duration = meta?.duration;
  return (
    <div className="flex items-center gap-2" data-testid="turn-duration-message">
      <Clock className="h-3.5 w-3.5 text-blue-500" />
      <span className="text-xs">
        {duration ? `本轮对话耗时 ${duration}s` : text}
      </span>
    </div>
  );
}

/**
 * MemorySaved 子消息渲染。
 */
function MemorySavedMessage({ text }: { text: string }) {
  const paths = extractFilePaths(text);
  return (
    <div data-testid="memory-saved-message">
      <div className="flex items-center gap-2">
        <HardDrive className="h-3.5 w-3.5 text-green-500" />
        <span className="text-xs font-medium">记忆已保存</span>
      </div>
      {paths.length > 0 && (
        <div className="ml-5.5 mt-1 space-y-0.5">
          {paths.map((p) => (
            <FilePathLink key={p} path={p} />
          ))}
        </div>
      )}
      {paths.length === 0 && (
        <div className="ml-5.5 mt-1 text-xs text-green-600/70 dark:text-green-400/70">
          {text}
        </div>
      )}
    </div>
  );
}

/**
 * ThinkingMessage 子消息渲染。
 */
function ThinkingMessage() {
  return (
    <div className="flex items-center gap-2" data-testid="thinking-message">
      <Brain className="h-3.5 w-3.5 animate-pulse text-purple-500" />
      <span className="text-xs text-purple-600 dark:text-purple-400">思考中...</span>
    </div>
  );
}

/**
 * BridgeStatusMessage 子消息渲染。
 */
function BridgeStatusMessage({ connected }: { connected: boolean }) {
  return (
    <div
      className={cn(
        'flex items-center gap-2',
        connected ? 'text-emerald-600 dark:text-emerald-400' : 'text-orange-600 dark:text-orange-400',
      )}
      data-testid="bridge-status-message"
    >
      {connected ? (
        <CheckCircle2 className="h-3.5 w-3.5" />
      ) : (
        <XCircle className="h-3.5 w-3.5" />
      )}
      <span className="text-xs">
        {connected ? 'Bridge 已连接' : 'Bridge 已断开'}
      </span>
    </div>
  );
}

/**
 * StopHookSummary 子消息渲染。
 */
function StopHookSummaryMessage({ text }: { text: string }) {
  return (
    <div data-testid="stop-hook-summary-message">
      <div className="flex items-center gap-2">
        <StopCircle className="h-3.5 w-3.5 text-rose-500" />
        <span className="text-xs font-medium">Stop Hook 摘要</span>
      </div>
      <div className="ml-5.5 mt-1 text-xs text-rose-600/70 dark:text-rose-400/70">
        <CollapsibleContent text={text} maxLength={150} verbose={false} />
      </div>
    </div>
  );
}

/**
 * CompactBoundaryMessage 子消息渲染。
 */
function CompactBoundaryMessage() {
  return (
    <div className="flex items-center gap-2" data-testid="compact-boundary-message">
      <Activity className="h-3.5 w-3.5 text-slate-400" />
      <span className="text-xs text-slate-500">— 压缩边界 —</span>
    </div>
  );
}

/**
 * PermissionRetryMessage 子消息渲染。
 */
function PermissionRetryMessage({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-2" data-testid="permission-retry-message">
      <RotateCcw className="h-3.5 w-3.5 text-indigo-500" />
      <span className="text-xs">
        <Shield className="mr-1 inline h-3 w-3" />
        {text}
      </span>
    </div>
  );
}

/**
 * ScheduledTaskMessage 子消息渲染。
 */
function ScheduledTaskMessage({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-2" data-testid="scheduled-task-message">
      <Zap className="h-3.5 w-3.5 text-yellow-500" />
      <span className="text-xs">{text}</span>
    </div>
  );
}

/**
 * 系统文本消息渲染组件。
 * 根据文本内容自动检测子类型并路由到对应渲染组件。
 * 支持多种子类型：turn_duration、memory_saved、thinking、bridge_status、
 * stop_hook_summary、compact_boundary、permission_retry、scheduled_task 等。
 */
export const SystemTextMessage = memo(function SystemTextMessage({
  entry,
  verbose = false,
  className,
}: SystemTextMessageProps) {
  const detected = detectSystemKind(entry.text);
  const kind = detected.kind;

  // 特定子类型路由到专门组件
  if (kind === 'turn_duration') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <TurnDurationMessage text={entry.text} meta={detected.meta} />
      </div>
    );
  }

  if (kind === 'memory_saved') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <MemorySavedMessage text={entry.text} />
      </div>
    );
  }

  if (kind === 'thinking') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <ThinkingMessage />
      </div>
    );
  }

  if (kind === 'bridge_connected') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <BridgeStatusMessage connected={true} />
      </div>
    );
  }

  if (kind === 'bridge_disconnected') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <BridgeStatusMessage connected={false} />
      </div>
    );
  }

  if (kind === 'stop_hook_summary') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <StopHookSummaryMessage text={entry.text} />
      </div>
    );
  }

  if (kind === 'compact_boundary') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <CompactBoundaryMessage />
      </div>
    );
  }

  if (kind === 'permission_retry') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <PermissionRetryMessage text={entry.text} />
      </div>
    );
  }

  if (kind === 'scheduled_task') {
    return (
      <div
        data-testid="system-text-message"
        className={cn('rounded-lg border px-4 py-3 text-xs', getKindClasses(kind), className)}
      >
        <ScheduledTaskMessage text={entry.text} />
      </div>
    );
  }

  // 通用渲染（compacted / error / warning / default）
  return (
    <div
      data-testid="system-text-message"
      className={cn(
        'rounded-lg border px-4 py-3 text-xs',
        getKindClasses(kind),
        className,
      )}
    >
      <div className="flex items-start gap-2">
        {getKindIcon(kind)}
        <CollapsibleContent text={entry.text} verbose={verbose} />
      </div>
    </div>
  );
});
