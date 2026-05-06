import { useState, useMemo } from 'react';
import { Search, X, Keyboard } from 'lucide-react';

export interface HelpCommand {
  name: string;
  description: string;
  shortcut?: string;
  category: string;
}

export interface HelpV2Props {
  commands?: HelpCommand[];
  open: boolean;
  onClose: () => void;
}

const DEFAULT_COMMANDS: HelpCommand[] = [
  { name: '/help', description: '显示帮助信息', shortcut: 'F1', category: '通用' },
  { name: '/clear', description: '清空对话', category: '通用' },
  { name: '/compact', description: '压缩上下文', category: '通用' },
  { name: '/goal', description: '管理线程目标 (set/clear/pause/resume)', category: '通用' },
  { name: '/model', description: '切换模型', category: '设置' },
  { name: '/config', description: '打开配置', category: '设置' },
  { name: '/status', description: '查看状态', category: '通用' },
  { name: '/cost', description: '查看费用统计', category: '通用' },
  { name: '/doctor', description: '运行诊断', category: '调试' },
  { name: '/init', description: '初始化项目', category: '设置' },
  { name: '/mcp', description: '管理MCP服务器', category: '设置' },
  { name: '/memory', description: '管理记忆文件', category: '通用' },
  { name: '/permissions', description: '管理权限', category: '设置' },
  { name: '/review', description: '代码审查', category: '工具' },
  { name: '/skills', description: '管理技能', category: '工具' },
  { name: '/tasks', description: '管理任务', category: '工具' },
  { name: '/vim', description: '切换Vim模式', category: '设置' },
  { name: '/undo', description: '撤销上一次操作', category: '通用' },
];

export function HelpV2({ commands = DEFAULT_COMMANDS, open, onClose }: HelpV2Props) {
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter(
      (cmd) =>
        cmd.name.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q) ||
        cmd.category.toLowerCase().includes(q),
    );
  }, [commands, query]);

  const categories = useMemo(() => {
    const map = new Map<string, HelpCommand[]>();
    for (const cmd of filtered) {
      const list = map.get(cmd.category) ?? [];
      list.push(cmd);
      map.set(cmd.category, list);
    }
    return Array.from(map.entries());
  }, [filtered]);

  if (!open) return null;

  return (
    <div
      data-testid="help-v2-dialog"
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]"
    >
      <div
        className="fixed inset-0 bg-black/40"
        data-testid="help-v2-backdrop"
        onClick={onClose}
      />
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-slate-200 bg-white shadow-xl">
        <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-3">
          <Search className="h-4 w-4 text-slate-400" />
          <input
            data-testid="help-v2-search"
            type="text"
            className="flex-1 bg-transparent text-sm text-slate-800 outline-none placeholder:text-slate-400"
            placeholder="搜索命令..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
          <button
            type="button"
            data-testid="help-v2-close"
            className="rounded p-1 hover:bg-slate-100"
            onClick={onClose}
            title="关闭帮助"
          >
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>
        <div className="max-h-[60vh] overflow-y-auto p-4">
          {categories.length === 0 && (
            <div data-testid="help-v2-empty" className="py-8 text-center text-sm text-slate-400">
              没有匹配的命令
            </div>
          )}
          {categories.map(([category, cmds]) => (
            <div key={category} className="mb-4 last:mb-0">
              <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-400">
                {category}
              </h3>
              <div className="space-y-1">
                {cmds.map((cmd) => (
                  <div
                    key={cmd.name}
                    data-testid={`help-v2-command-${cmd.name.slice(1)}`}
                    className="flex items-center justify-between rounded px-2 py-1.5 hover:bg-slate-50"
                  >
                    <div className="flex items-center gap-2">
                      <code className="text-sm font-medium text-slate-700">{cmd.name}</code>
                      <span className="text-xs text-slate-500">{cmd.description}</span>
                    </div>
                    {cmd.shortcut && (
                      <kbd className="inline-flex items-center gap-1 rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-xs text-slate-500">
                        <Keyboard className="h-3 w-3" />
                        {cmd.shortcut}
                      </kbd>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div className="border-t border-slate-100 px-4 py-2">
          <p className="text-xs text-slate-400">
            共 {commands.length} 个命令，显示 {filtered.length} 个
          </p>
        </div>
      </div>
    </div>
  );
}