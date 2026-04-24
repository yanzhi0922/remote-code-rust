import { useState, useCallback } from 'react';
import {
  Terminal,
  FileEdit,
  FileText,
  Globe,
  Wrench,
  ShieldAlert,
  AlertTriangle,
  MessageSquare,
  Server,
} from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

/** 权限请求组件属性 */
export interface PermissionRequestProps {
  /** 权限请求信息 */
  request: PermissionRequestInfo;
  /** 允许回调 */
  onAllow: () => void;
  /** 拒绝回调 */
  onReject: (feedback?: string) => void;
  /** 额外 CSS 类名 */
  className?: string;
  /** 是否为 worker 发起 */
  workerBadge?: { name: string; color: string } | null;
  /** 是否显示详细模式 */
  verbose?: boolean;
}

/** Bash 权限输入类型 */
interface BashInput {
  command?: string;
  timeout?: number;
  cwd?: string;
}

/** FileEdit 权限输入类型 */
interface FileEditInput {
  file_path?: string;
  old_string?: string;
  new_string?: string;
  replace_all?: boolean;
}

/** FileWrite 权限输入类型 */
interface FileWriteInput {
  file_path?: string;
  content?: string;
}

/** MCP 权限输入类型 */
interface McpToolInput {
  server_name?: string;
  tool_name?: string;
  arguments?: Record<string, unknown>;
}

/** WebFetch 权限输入类型 */
interface WebFetchInput {
  url?: string;
}

/**
 * 检测命令是否包含危险操作。
 */
function isDangerousCommand(command: string): boolean {
  const dangerousPatterns = [
    /\brm\s+-rf\b/,
    /\brm\s+-r\b/,
    /\bdrop\s+table\b/i,
    /\bdelete\s+from\b/i,
    /\bformat\s+[a-z]:/i,
    /\bdd\s+if=/,
    /\bmkfs\b/,
    />\s*\/dev\//,
    /\bshutdown\b/,
    /\breboot\b/,
    /\bchmod\s+777\b/,
    /\bchown\s+/,
  ];
  return dangerousPatterns.some((p) => p.test(command));
}

/**
 * 根据工具名获取图标。
 */
function getToolIcon(toolName: string) {
  const lower = toolName.toLowerCase();
  if (lower.includes('bash') || lower.includes('shell')) return <Terminal className="h-4 w-4" />;
  if (lower.includes('edit') || lower.includes('file_edit')) return <FileEdit className="h-4 w-4" />;
  if (lower.includes('write') || lower.includes('file_write')) return <FileText className="h-4 w-4" />;
  if (lower.includes('web') || lower.includes('fetch')) return <Globe className="h-4 w-4" />;
  if (lower.includes('mcp') || lower.includes('tool')) return <Wrench className="h-4 w-4" />;
  return <ShieldAlert className="h-4 w-4" />;
}

/**
 * 渲染 Bash 权限请求详情。
 */
function BashPermissionDetail({ input, verbose }: { input: BashInput; verbose?: boolean }) {
  const command = input.command || '';
  const dangerous = isDangerousCommand(command);

  return (
    <div className="space-y-2" data-testid="bash-permission-detail">
      {dangerous && (
        <div className="flex items-center gap-1.5 rounded-md bg-red-50 px-2 py-1 text-xs text-red-600 dark:bg-red-950/30 dark:text-red-400">
          <AlertTriangle className="h-3 w-3" />
          <span>检测到潜在危险命令</span>
        </div>
      )}
      <div className="rounded-md bg-slate-50 p-2 dark:bg-slate-800/50">
        <div className="mb-1 text-xs text-slate-400">命令</div>
        <code
          className={cn(
            'block whitespace-pre-wrap break-all font-mono text-sm',
            dangerous ? 'text-red-600 dark:text-red-400' : 'text-slate-700 dark:text-slate-300',
          )}
        >
          {command}
        </code>
      </div>
      {input.cwd && (
        <div className="text-xs text-slate-500">
          <span className="text-slate-400">工作目录:</span> {input.cwd}
        </div>
      )}
      {input.timeout != null && (
        <div className="text-xs text-slate-500">
          <span className="text-slate-400">超时:</span> {input.timeout}s
        </div>
      )}
      {!verbose && command.length > 200 && (
        <div className="text-xs text-slate-400">命令过长，使用 verbose 模式查看完整内容</div>
      )}
    </div>
  );
}

/**
 * 渲染 FileEdit 权限请求详情。
 */
function FileEditPermissionDetail({ input }: { input: FileEditInput }) {
  return (
    <div className="space-y-2" data-testid="file-edit-permission-detail">
      {input.file_path && (
        <div className="text-xs text-slate-500">
          <span className="text-slate-400">文件:</span>{' '}
          <code className="font-mono text-blue-600 dark:text-blue-400">{input.file_path}</code>
        </div>
      )}
      {input.old_string && (
        <div className="rounded-md bg-red-50 p-2 dark:bg-red-950/20">
          <div className="mb-1 text-xs text-red-400">删除内容</div>
          <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-red-700 dark:text-red-300">
            {input.old_string.length > 500
              ? input.old_string.slice(0, 500) + '…'
              : input.old_string}
          </pre>
        </div>
      )}
      {input.new_string && (
        <div className="rounded-md bg-green-50 p-2 dark:bg-green-950/20">
          <div className="mb-1 text-xs text-green-400">替换为</div>
          <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-green-700 dark:text-green-300">
            {input.new_string.length > 500
              ? input.new_string.slice(0, 500) + '…'
              : input.new_string}
          </pre>
        </div>
      )}
      {input.replace_all && (
        <div className="text-xs text-amber-600 dark:text-amber-400">
          ⚠ 将替换所有匹配项
        </div>
      )}
    </div>
  );
}

/**
 * 渲染 FileWrite 权限请求详情。
 */
function FileWritePermissionDetail({ input }: { input: FileWriteInput }) {
  return (
    <div className="space-y-2" data-testid="file-write-permission-detail">
      {input.file_path && (
        <div className="text-xs text-slate-500">
          <span className="text-slate-400">文件:</span>{' '}
          <code className="font-mono text-blue-600 dark:text-blue-400">{input.file_path}</code>
        </div>
      )}
      {input.content && (
        <div className="rounded-md bg-slate-50 p-2 dark:bg-slate-800/50">
          <div className="mb-1 text-xs text-slate-400">写入内容</div>
          <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-slate-700 dark:text-slate-300">
            {input.content.length > 500
              ? input.content.slice(0, 500) + '…'
              : input.content}
          </pre>
        </div>
      )}
    </div>
  );
}

/**
 * 渲染 MCP 权限请求详情。
 */
function McpPermissionDetail({ input }: { input: McpToolInput }) {
  return (
    <div className="space-y-2" data-testid="mcp-permission-detail">
      {input.server_name && (
        <div className="flex items-center gap-1.5 text-xs text-slate-500">
          <Server className="h-3 w-3" />
          <span className="text-slate-400">服务器:</span> {input.server_name}
        </div>
      )}
      {input.tool_name && (
        <div className="flex items-center gap-1.5 text-xs text-slate-500">
          <Wrench className="h-3 w-3" />
          <span className="text-slate-400">工具:</span>{' '}
          <code className="font-mono">{input.tool_name}</code>
        </div>
      )}
      {input.arguments && Object.keys(input.arguments).length > 0 && (
        <div className="rounded-md bg-slate-50 p-2 dark:bg-slate-800/50">
          <div className="mb-1 text-xs text-slate-400">输入参数</div>
          <pre className="max-h-40 overflow-auto text-xs text-slate-700 dark:text-slate-300">
            {JSON.stringify(input.arguments, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}

/**
 * 渲染 WebFetch 权限请求详情。
 */
function WebFetchPermissionDetail({ input }: { input: WebFetchInput }) {
  return (
    <div className="space-y-2" data-testid="webfetch-permission-detail">
      {input.url && (
        <div className="flex items-center gap-1.5 text-xs text-slate-500">
          <Globe className="h-3 w-3" />
          <span className="text-slate-400">URL:</span>{' '}
          <a
            href={input.url}
            className="font-mono text-blue-600 hover:underline dark:text-blue-400"
            target="_blank"
            rel="noopener noreferrer"
          >
            {input.url}
          </a>
        </div>
      )}
    </div>
  );
}

/**
 * 渲染通用权限请求详情（降级显示）。
 */
function FallbackPermissionDetail({
  toolName,
  input,
}: {
  toolName: string;
  input: unknown;
}) {
  return (
    <div className="space-y-2" data-testid="fallback-permission-detail">
      <div className="text-xs text-slate-500">
        <span className="text-slate-400">工具:</span> {toolName}
      </div>
      {input != null && (
        <div className="rounded-md bg-slate-50 p-2 dark:bg-slate-800/50">
          <div className="mb-1 text-xs text-slate-400">输入数据</div>
          <pre className="max-h-40 overflow-auto text-xs text-slate-700 dark:text-slate-300">
            {typeof input === 'string' ? input : JSON.stringify(input as object, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}

/**
 * 权限请求组件。
 * 根据 tool_name 路由到对应的权限请求详情组件，
 * 支持 Bash、FileEdit、FileWrite、MCP、WebFetch 等多种工具类型。
 */
export function PermissionRequest({
  request,
  onAllow,
  onReject,
  className,
  workerBadge,
  verbose = false,
}: PermissionRequestProps) {
  const [feedbackText, setFeedbackText] = useState('');
  const [showFeedback, setShowFeedback] = useState(false);

  /** 处理拒绝并附带反馈 */
  const handleRejectWithFeedback = useCallback(() => {
    onReject(feedbackText.trim() || undefined);
    setFeedbackText('');
    setShowFeedback(false);
  }, [feedbackText, onReject]);

  /** 获取工具图标 */
  const toolIcon = getToolIcon(request.tool_name);

  /** 根据工具名路由渲染权限详情 */
  const renderToolDetail = () => {
    const lower = request.tool_name.toLowerCase();
    const input = request.input as Record<string, unknown>;

    if (lower.includes('bash') || lower.includes('shell')) {
      return <BashPermissionDetail input={input as BashInput} verbose={verbose} />;
    }
    if (lower.includes('file_edit') || lower.includes('fileedit')) {
      return <FileEditPermissionDetail input={input as FileEditInput} />;
    }
    if (lower.includes('file_write') || lower.includes('filewrite') || lower.includes('write')) {
      return <FileWritePermissionDetail input={input as FileWriteInput} />;
    }
    if (lower.includes('mcp') || lower.includes('tool_use')) {
      return <McpPermissionDetail input={input as McpToolInput} />;
    }
    if (lower.includes('web') || lower.includes('fetch')) {
      return <WebFetchPermissionDetail input={input as WebFetchInput} />;
    }

    return (
      <FallbackPermissionDetail
        toolName={request.tool_name}
        input={request.input}
      />
    );
  };

  /** 渲染权限建议 */
  const renderSuggestions = () => {
    if (
      !request.permission_suggestions ||
      !Array.isArray(request.permission_suggestions) ||
      request.permission_suggestions.length === 0
    ) {
      return null;
    }

    return (
      <div className="mt-2" data-testid="permission-suggestions">
        <div className="mb-1 text-xs text-slate-400">权限建议</div>
        <div className="space-y-1">
          {request.permission_suggestions.map((suggestion, idx) => (
            <div
              key={idx}
              className="rounded-md bg-blue-50 px-2 py-1 text-xs text-blue-600 dark:bg-blue-950/20 dark:text-blue-400"
            >
              {typeof suggestion === 'string'
                ? suggestion
                : JSON.stringify(suggestion)}
            </div>
          ))}
        </div>
      </div>
    );
  };

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4 dark:border-orange-800 dark:bg-slate-900', className)}
      data-testid="permission-request"
    >
      {/* 标题 */}
      <PermissionRequestTitle
        title={request.title || '权限请求'}
        subtitle={request.description}
        workerBadge={workerBadge ?? undefined}
      />

      {/* 工具图标和名称 */}
      <div className="mt-2 flex items-center gap-1.5 text-xs text-slate-400">
        {toolIcon}
        <span>工具: {request.tool_name}</span>
      </div>

      {/* 工具详情 */}
      <div className="mt-3">{renderToolDetail()}</div>

      {/* 权限建议 */}
      {renderSuggestions()}

      {/* 阻塞路径 */}
      {request.blocked_path && (
        <div className="mt-2 text-xs text-slate-400">
          <span>阻塞路径:</span>{' '}
          <code className="font-mono">{request.blocked_path}</code>
        </div>
      )}

      {/* 反馈输入 */}
      {showFeedback && (
        <div className="mt-3" data-testid="permission-feedback">
          <textarea
            value={feedbackText}
            onChange={(e) => setFeedbackText(e.target.value)}
            className="w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-600 dark:bg-slate-800"
            placeholder="输入反馈（可选）..."
            rows={2}
            data-testid="permission-feedback-input"
          />
        </div>
      )}

      {/* 操作按钮 */}
      <div className="mt-3 flex items-center gap-2">
        <button
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700 transition-colors"
          onClick={onAllow}
          data-testid="permission-allow"
        >
          允许执行
        </button>
        <button
          className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50 transition-colors dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-800"
          onClick={handleRejectWithFeedback}
          data-testid="permission-reject"
        >
          拒绝
        </button>
        {!showFeedback && (
          <button
            className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
            onClick={() => setShowFeedback(true)}
            data-testid="permission-show-feedback"
          >
            <MessageSquare className="h-3 w-3" />
            添加反馈
          </button>
        )}
      </div>
    </div>
  );
}
