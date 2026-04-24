import { useState, useCallback } from 'react';
import {
  Settings,
  Server,
  ToggleLeft,
  ToggleRight,
  Trash2,
  Plus,
  ChevronRight,
  ChevronLeft,
  Globe,
  Terminal,
  Wrench,
  Shield,
  AlertTriangle,
  CheckCircle2,
  XCircle,
} from 'lucide-react';
import type { McpServerInfo, McpToolInfo, ConfigScope } from '../../lib/types';
import { cn } from '../../lib/utils';

/** MCP 设置组件属性 */
export interface MCPSettingsProps {
  /** 默认超时秒数 */
  defaultTimeout?: number;
  /** 超时变更回调 */
  onTimeoutChange?: (timeout: number) => void;
  /** MCP 服务器列表 */
  servers?: McpServerInfo[];
  /** 添加服务器回调 */
  onAddServer?: (draft: McpServerDraftInternal) => void;
  /** 删除服务器回调 */
  onRemoveServer?: (name: string) => void;
  /** 切换服务器启用状态回调 */
  onToggleServer?: (name: string, enabled: boolean) => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/** 内部使用的服务器草稿类型 */
export interface McpServerDraftInternal {
  name: string;
  transport: 'stdio' | 'http' | 'websocket';
  command?: string;
  url?: string;
  scope: ConfigScope;
}

/** 视图状态类型 */
type ViewState =
  | { type: 'list' }
  | { type: 'server-detail'; server: McpServerInfo }
  | { type: 'tool-detail'; server: McpServerInfo; tool: McpToolInfo }
  | { type: 'add-server' };

/** 获取服务器传输类型图标 */
function getTransportIcon(transport: string) {
  switch (transport) {
    case 'stdio':
      return <Terminal className="h-4 w-4" />;
    case 'http':
    case 'sse':
    case 'websocket':
      return <Globe className="h-4 w-4" />;
    default:
      return <Server className="h-4 w-4" />;
  }
}

/** 获取服务器状态徽章 */
function getStatusBadge(status: string) {
  switch (status) {
    case 'connected':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-green-600 dark:text-green-400">
          <CheckCircle2 className="h-3 w-3" />
          已连接
        </span>
      );
    case 'disconnected':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-slate-400">
          <XCircle className="h-3 w-3" />
          未连接
        </span>
      );
    case 'error':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-red-500">
          <AlertTriangle className="h-3 w-3" />
          错误
        </span>
      );
    default:
      return (
        <span className="inline-flex items-center gap-1 text-xs text-slate-400">
          <XCircle className="h-3 w-3" />
          未知
        </span>
      );
  }
}

/** 获取传输类型标签 */
function getTransportLabel(transport: string): string {
  switch (transport) {
    case 'stdio':
      return 'STDIO';
    case 'http':
      return 'HTTP';
    case 'sse':
      return 'SSE';
    case 'websocket':
      return 'WebSocket';
    default:
      return transport.toUpperCase();
  }
}

/** 服务器分类类型 */
type ServerCategory = 'stdio' | 'remote' | 'agent';

/** 获取服务器分类 */
function getServerCategory(server: McpServerInfo): ServerCategory {
  if (server.transport === 'stdio') return 'stdio';
  if (server.url) return 'remote';
  return 'agent';
}

/**
 * MCP 设置组件。
 * 支持服务器列表管理、添加/删除服务器、启用/禁用、工具浏览、视图导航。
 */
export function MCPSettings({
  defaultTimeout = 30,
  onTimeoutChange,
  servers = [],
  onAddServer,
  onRemoveServer,
  onToggleServer,
  className,
}: MCPSettingsProps) {
  const [viewState, setViewState] = useState<ViewState>({ type: 'list' });
  const [activeTab, setActiveTab] = useState<ServerCategory | 'all'>('all');
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  // 添加服务器表单状态
  const [newName, setNewName] = useState('');
  const [newTransport, setNewTransport] = useState<'stdio' | 'http' | 'websocket'>('stdio');
  const [newCommand, setNewCommand] = useState('');
  const [newUrl, setNewUrl] = useState('');
  const [newScope, setNewScope] = useState<ConfigScope>('project');

  /** 过滤服务器列表 */
  const filteredServers =
    activeTab === 'all'
      ? servers
      : servers.filter((s) => getServerCategory(s) === activeTab);

  /** 处理添加服务器 */
  const handleAddServer = useCallback(() => {
    if (!newName.trim()) return;
    onAddServer?.({
      name: newName.trim(),
      transport: newTransport,
      command: newTransport === 'stdio' ? newCommand || undefined : undefined,
      url: newTransport !== 'stdio' ? newUrl || undefined : undefined,
      scope: newScope,
    });
    setNewName('');
    setNewCommand('');
    setNewUrl('');
    setViewState({ type: 'list' });
  }, [newName, newTransport, newCommand, newUrl, newScope, onAddServer]);

  /** 处理删除服务器 */
  const handleDelete = useCallback(
    (name: string) => {
      if (confirmDelete === name) {
        onRemoveServer?.(name);
        setConfirmDelete(null);
      } else {
        setConfirmDelete(name);
      }
    },
    [confirmDelete, onRemoveServer],
  );

  // ── 渲染：服务器列表视图 ──
  const renderListView = () => (
    <div data-testid="mcp-server-list">
      {/* 标签页 */}
      <div className="mb-3 flex gap-1 border-b border-slate-200 dark:border-slate-700">
        {(['all', 'stdio', 'remote', 'agent'] as const).map((tab) => (
          <button
            key={tab}
            type="button"
            className={cn(
              'px-3 py-1.5 text-xs font-medium transition-colors',
              activeTab === tab
                ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
                : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300',
            )}
            onClick={() => setActiveTab(tab)}
            data-testid={`mcp-tab-${tab}`}
          >
            {tab === 'all' ? '全部' : tab.toUpperCase()}
          </button>
        ))}
      </div>

      {/* 服务器卡片列表 */}
      {filteredServers.length === 0 ? (
        <div className="py-8 text-center text-sm text-slate-400">
          暂无 MCP 服务器配置
        </div>
      ) : (
        <div className="space-y-2">
          {filteredServers.map((server) => (
            <div
              key={server.name}
              className="group rounded-lg border border-slate-200 bg-white p-3 transition-colors hover:border-slate-300 dark:border-slate-700 dark:bg-slate-800/50 dark:hover:border-slate-600"
              data-testid={`mcp-server-${server.name}`}
            >
              <div className="flex items-start justify-between">
                <button
                  type="button"
                  className="flex items-start gap-2 text-left"
                  onClick={() => setViewState({ type: 'server-detail', server })}
                  data-testid={`mcp-server-detail-${server.name}`}
                >
                  <span className="mt-0.5 text-slate-400">
                    {getTransportIcon(server.transport)}
                  </span>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-slate-800 dark:text-slate-200">
                        {server.name}
                      </span>
                      <span className="rounded-full bg-slate-100 px-1.5 py-0.5 text-[10px] text-slate-500 dark:bg-slate-700">
                        {getTransportLabel(server.transport)}
                      </span>
                    </div>
                    <div className="mt-1 flex items-center gap-3">
                      {server.live ? getStatusBadge(server.live.status) : getStatusBadge('disconnected')}
                      {server.live && server.live.tool_count > 0 && (
                        <span className="text-xs text-slate-400">
                          {server.live.tool_count} 个工具
                        </span>
                      )}
                    </div>
                  </div>
                </button>

                <div className="flex items-center gap-1">
                  {/* 启用/禁用切换 */}
                  <button
                    type="button"
                    className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
                    onClick={() => onToggleServer?.(server.name, !server.enabled)}
                    title={server.enabled ? '禁用' : '启用'}
                    data-testid={`mcp-toggle-${server.name}`}
                  >
                    {server.enabled ? (
                      <ToggleRight className="h-5 w-5 text-green-500" />
                    ) : (
                      <ToggleLeft className="h-5 w-5" />
                    )}
                  </button>

                  {/* 删除按钮 */}
                  <button
                    type="button"
                    className={cn(
                      'text-slate-400 hover:text-red-500 dark:hover:text-red-400',
                      confirmDelete === server.name && 'text-red-500',
                    )}
                    onClick={() => handleDelete(server.name)}
                    title={confirmDelete === server.name ? '确认删除' : '删除'}
                    data-testid={`mcp-delete-${server.name}`}
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>

                  {/* 详情箭头 */}
                  <button
                    type="button"
                    className="text-slate-300 hover:text-slate-500 dark:text-slate-600 dark:hover:text-slate-400"
                    onClick={() => setViewState({ type: 'server-detail', server })}
                    title="查看详情"
                    aria-label={`查看 ${server.name} 详情`}
                  >
                    <ChevronRight className="h-4 w-4" />
                  </button>
                </div>
              </div>

              {/* 删除确认提示 */}
              {confirmDelete === server.name && (
                <div className="mt-2 flex items-center gap-2 rounded-md bg-red-50 px-2 py-1 text-xs text-red-600 dark:bg-red-950/30 dark:text-red-400">
                  <AlertTriangle className="h-3 w-3" />
                  <span>再次点击确认删除</span>
                  <button
                    type="button"
                    className="ml-auto text-red-400 hover:text-red-600"
                    onClick={() => setConfirmDelete(null)}
                  >
                    取消
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* 添加服务器按钮 */}
      {onAddServer && (
        <button
          type="button"
          className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-slate-300 py-2 text-sm text-slate-500 transition-colors hover:border-blue-400 hover:text-blue-500 dark:border-slate-600 dark:hover:border-blue-500"
          onClick={() => setViewState({ type: 'add-server' })}
          data-testid="mcp-add-server"
        >
          <Plus className="h-4 w-4" />
          添加 MCP 服务器
        </button>
      )}
    </div>
  );

  // ── 渲染：服务器详情视图 ──
  const renderServerDetail = (server: McpServerInfo) => (
    <div data-testid="mcp-server-detail-view">
      <button
        type="button"
        className="mb-3 flex items-center gap-1 text-sm text-slate-500 hover:text-slate-700 dark:hover:text-slate-300"
        onClick={() => setViewState({ type: 'list' })}
        data-testid="mcp-back-to-list"
      >
        <ChevronLeft className="h-4 w-4" />
        返回列表
      </button>

      <div className="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700 dark:bg-slate-800/50">
        <div className="flex items-center gap-3">
          <span className="text-slate-400">{getTransportIcon(server.transport)}</span>
          <div>
            <h4 className="text-base font-semibold text-slate-800 dark:text-slate-200">
              {server.name}
            </h4>
            <div className="mt-1 flex items-center gap-3 text-xs text-slate-500">
              <span>{getTransportLabel(server.transport)}</span>
              {server.live ? getStatusBadge(server.live.status) : getStatusBadge('disconnected')}
              {server.command && (
                <span className="font-mono text-slate-400">{server.command}</span>
              )}
              {server.url && (
                <span className="font-mono text-blue-500">{server.url}</span>
              )}
            </div>
          </div>
        </div>

        {/* 认证状态 */}
        {server.url && (
          <div className="mt-3 flex items-center gap-2 text-xs text-slate-500">
            <Shield className="h-3.5 w-3.5" />
            <span>需要认证</span>
          </div>
        )}

        {/* 工具列表 */}
        {server.live && server.live.tools.length > 0 && (
          <div className="mt-4">
            <h5 className="mb-2 flex items-center gap-1.5 text-sm font-medium text-slate-700 dark:text-slate-300">
              <Wrench className="h-3.5 w-3.5" />
              工具列表 ({server.live.tools.length})
            </h5>
            <div className="space-y-1">
              {server.live.tools.map((tool) => (
                <button
                  key={tool.name}
                  type="button"
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-slate-50 dark:hover:bg-slate-700/50"
                  onClick={() =>
                    setViewState({ type: 'tool-detail', server, tool })
                  }
                  data-testid={`mcp-tool-${tool.name}`}
                >
                  <Wrench className="h-3 w-3 text-slate-400" />
                  <span className="font-mono text-xs text-slate-700 dark:text-slate-300">
                    {tool.name}
                  </span>
                  {tool.description && (
                    <span className="ml-2 truncate text-xs text-slate-400">
                      {tool.description}
                    </span>
                  )}
                  <ChevronRight className="ml-auto h-3 w-3 text-slate-300" />
                </button>
              ))}
            </div>
          </div>
        )}

        {/* 服务器配置信息 */}
        <div className="mt-4 space-y-1 text-xs text-slate-500">
          {server.config_path && (
            <div>
              <span className="text-slate-400">配置路径:</span> {server.config_path}
            </div>
          )}
          {server.args.length > 0 && (
            <div>
              <span className="text-slate-400">参数:</span>{' '}
              <code className="font-mono">{server.args.join(' ')}</code>
            </div>
          )}
          {server.cwd && (
            <div>
              <span className="text-slate-400">工作目录:</span> {server.cwd}
            </div>
          )}
          {server.startup_timeout_secs != null && (
            <div>
              <span className="text-slate-400">启动超时:</span> {server.startup_timeout_secs}s
            </div>
          )}
          {server.request_timeout_secs != null && (
            <div>
              <span className="text-slate-400">请求超时:</span> {server.request_timeout_secs}s
            </div>
          )}
        </div>
      </div>
    </div>
  );

  // ── 渲染：工具详情视图 ──
  const renderToolDetail = (server: McpServerInfo, tool: McpToolInfo) => (
    <div data-testid="mcp-tool-detail-view">
      <button
        type="button"
        className="mb-3 flex items-center gap-1 text-sm text-slate-500 hover:text-slate-700 dark:hover:text-slate-300"
        onClick={() => setViewState({ type: 'server-detail', server })}
        data-testid="mcp-back-to-server"
      >
        <ChevronLeft className="h-4 w-4" />
        返回服务器
      </button>

      <div className="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700 dark:bg-slate-800/50">
        <div className="flex items-center gap-2">
          <Wrench className="h-5 w-5 text-blue-500" />
          <h4 className="text-base font-semibold text-slate-800 dark:text-slate-200">
            {tool.name}
          </h4>
        </div>
        {tool.description && (
          <p className="mt-2 text-sm text-slate-600 dark:text-slate-400">
            {tool.description}
          </p>
        )}
        {tool.inputSchema != null && (
          <div className="mt-3">
            <h5 className="mb-1 text-xs font-medium text-slate-500">输入 Schema</h5>
            <pre className="max-h-60 overflow-auto rounded-md bg-slate-50 p-3 text-xs dark:bg-slate-900">
              {JSON.stringify(tool.inputSchema as object, null, 2)}
            </pre>
          </div>
        )}
        <div className="mt-3 text-xs text-slate-400">
          来自服务器: {server.name}
        </div>
      </div>
    </div>
  );

  // ── 渲染：添加服务器视图 ──
  const renderAddServer = () => (
    <div data-testid="mcp-add-server-form">
      <button
        type="button"
        className="mb-3 flex items-center gap-1 text-sm text-slate-500 hover:text-slate-700 dark:hover:text-slate-300"
        onClick={() => setViewState({ type: 'list' })}
        data-testid="mcp-back-to-list"
      >
        <ChevronLeft className="h-4 w-4" />
        返回列表
      </button>

      <div className="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700 dark:bg-slate-800/50">
        <h4 className="mb-3 flex items-center gap-2 text-sm font-semibold text-slate-700 dark:text-slate-300">
          <Plus className="h-4 w-4" />
          添加 MCP 服务器
        </h4>

        <div className="space-y-3">
          {/* 名称 */}
          <div>
            <label className="mb-1 block text-xs text-slate-500">服务器名称</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-600 dark:bg-slate-800"
              placeholder="my-server"
              data-testid="mcp-new-name"
            />
          </div>

          {/* 传输类型 */}
          <div>
            <label className="mb-1 block text-xs text-slate-500">传输类型</label>
            <div className="flex gap-2">
              {(['stdio', 'http', 'websocket'] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  className={cn(
                    'flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors',
                    newTransport === t
                      ? 'border-blue-500 bg-blue-50 text-blue-700 dark:bg-blue-900/20 dark:text-blue-300'
                      : 'border-slate-300 text-slate-600 hover:border-slate-400 dark:border-slate-600 dark:text-slate-400',
                  )}
                  onClick={() => setNewTransport(t)}
                  data-testid={`mcp-new-transport-${t}`}
                >
                  {t === 'stdio' ? <Terminal className="h-3 w-3" /> : <Globe className="h-3 w-3" />}
                  {t.toUpperCase()}
                </button>
              ))}
            </div>
          </div>

          {/* 命令（stdio） */}
          {newTransport === 'stdio' && (
            <div>
              <label className="mb-1 block text-xs text-slate-500">命令</label>
              <input
                type="text"
                value={newCommand}
                onChange={(e) => setNewCommand(e.target.value)}
                className="w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm font-mono dark:border-slate-600 dark:bg-slate-800"
                placeholder="npx -y @example/mcp-server"
                data-testid="mcp-new-command"
              />
            </div>
          )}

          {/* URL（http/websocket） */}
          {newTransport !== 'stdio' && (
            <div>
              <label className="mb-1 block text-xs text-slate-500">URL</label>
              <input
                type="text"
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm font-mono dark:border-slate-600 dark:bg-slate-800"
                placeholder="https://example.com/mcp"
                data-testid="mcp-new-url"
              />
            </div>
          )}

          {/* Scope */}
          <div>
            <label className="mb-1 block text-xs text-slate-500">作用域</label>
            <div className="flex gap-2">
              {(['project', 'profile'] as ConfigScope[]).map((s) => (
                <button
                  key={s}
                  type="button"
                  className={cn(
                    'rounded-md border px-3 py-1.5 text-xs font-medium transition-colors',
                    newScope === s
                      ? 'border-blue-500 bg-blue-50 text-blue-700 dark:bg-blue-900/20 dark:text-blue-300'
                      : 'border-slate-300 text-slate-600 hover:border-slate-400 dark:border-slate-600 dark:text-slate-400',
                  )}
                  onClick={() => setNewScope(s)}
                  data-testid={`mcp-new-scope-${s}`}
                >
                  {s === 'project' ? '项目' : '用户'}
                </button>
              ))}
            </div>
          </div>

          {/* 提交按钮 */}
          <button
            type="button"
            className={cn(
              'w-full rounded-md px-4 py-2 text-sm font-medium text-white transition-colors',
              newName.trim()
                ? 'bg-blue-600 hover:bg-blue-700'
                : 'cursor-not-allowed bg-slate-300 dark:bg-slate-700',
            )}
            onClick={handleAddServer}
            disabled={!newName.trim()}
            data-testid="mcp-new-submit"
          >
            添加服务器
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div
      className={cn('rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700', className)}
      data-testid="mcp-settings"
    >
      {/* 头部 */}
      <div className="flex items-center gap-2">
        <Settings className="h-4 w-4 text-slate-400" />
        <h3 className="text-sm font-semibold text-slate-700 dark:text-slate-300">MCP 设置</h3>
      </div>

      {/* 超时配置 */}
      <div className="mt-3 space-y-3">
        <div>
          <label className="text-xs text-slate-500">默认超时 (秒)</label>
          <input
            type="number"
            value={defaultTimeout}
            onChange={(e) => onTimeoutChange?.(Number(e.target.value))}
            className="mt-1 w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-600"
            data-testid="mcp-timeout-input"
            title="默认超时秒数"
          />
        </div>
      </div>

      {/* 视图路由 */}
      <div className="mt-4">
        {viewState.type === 'list' && renderListView()}
        {viewState.type === 'server-detail' && renderServerDetail(viewState.server)}
        {viewState.type === 'tool-detail' &&
          renderToolDetail(viewState.server, viewState.tool)}
        {viewState.type === 'add-server' && renderAddServer()}
      </div>
    </div>
  );
}
