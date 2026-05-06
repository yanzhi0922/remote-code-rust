import { useState, useEffect, useCallback } from 'react';
import { Brain, FileText, Plus, RefreshCw, Trash2 } from 'lucide-react';
import type { MemoryFileInfo } from './MemoryFileSelector';

export interface MemoryEntry {
  name: string;
  description: string;
  type: 'user' | 'project';
  path: string;
  lastModified?: string;
}

interface MemoryPanelProps {
  memories?: MemoryEntry[];
  onReadMemory?: (path: string) => void;
  onWriteMemory?: (path: string, content: string) => void;
  onDeleteMemory?: (path: string) => void;
  onRefresh?: () => void;
  className?: string;
}

export function MemoryPanel({
  memories = [],
  onReadMemory,
  onWriteMemory,
  onDeleteMemory,
  onRefresh,
  className = '',
}: MemoryPanelProps) {
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [content, setContent] = useState('');
  const [editing, setEditing] = useState(false);

  const handleSelect = useCallback(
    (path: string) => {
      setSelectedPath(path);
      onReadMemory?.(path);
    },
    [onReadMemory],
  );

  const handleSave = useCallback(() => {
    if (selectedPath) {
      onWriteMemory?.(selectedPath, content);
      setEditing(false);
    }
  }, [selectedPath, content, onWriteMemory]);

  const handleNew = useCallback(() => {
    const path = prompt('输入记忆文件路径（相对于项目根目录）:');
    if (path) {
      setSelectedPath(path);
      setContent('');
      setEditing(true);
    }
  }, []);

  const selectedMemory = memories.find((m) => m.path === selectedPath);

  return (
    <div className={`flex h-full flex-col bg-rc-bg-primary ${className}`}>
      <div className="flex items-center justify-between border-b border-rc-border-primary px-3 py-1.5">
        <div className="flex items-center gap-2">
          <Brain size={14} className="text-rc-accent-primary" />
          <span className="text-xs font-medium text-rc-text-primary">Memory</span>
          <span className="text-2xs text-rc-text-tertiary">{memories.length} 条</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handleNew}
            className="rounded p-1 text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary transition-colors"
            title="新建记忆"
          >
            <Plus size={14} />
          </button>
          <button
            type="button"
            onClick={onRefresh}
            className="rounded p-1 text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary transition-colors"
            title="刷新"
          >
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      <div className="flex flex-1 min-h-0">
        {/* File list sidebar */}
        <div className="w-48 shrink-0 border-r border-rc-border-primary overflow-y-auto">
          {memories.length === 0 ? (
            <div className="p-3 text-center text-2xs text-rc-text-tertiary">
              <Brain size={20} className="mx-auto mb-1 opacity-40" />
              <p>暂无记忆条目</p>
              <p className="mt-0.5">使用 /memory 命令管理</p>
            </div>
          ) : (
            <div className="py-1">
              {memories.map((mem) => (
                <button
                  key={mem.path}
                  type="button"
                  onClick={() => handleSelect(mem.path)}
                  className={`flex w-full items-start gap-2 px-3 py-1.5 text-left transition-colors ${
                    selectedPath === mem.path
                      ? 'bg-rc-bg-active text-rc-text-primary'
                      : 'text-rc-text-secondary hover:bg-rc-bg-hover'
                  }`}
                >
                  <FileText
                    size={12}
                    className={`mt-0.5 shrink-0 ${mem.type === 'user' ? 'text-rc-accent-primary' : 'text-rc-accent-success'}`}
                  />
                  <div className="min-w-0">
                    <div className="truncate text-xs">{mem.name}</div>
                    <div className="truncate text-2xs text-rc-text-tertiary">{mem.description}</div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Content area */}
        <div className="flex-1 flex flex-col min-w-0">
          {selectedPath ? (
            <>
              <div className="flex items-center justify-between border-b border-rc-border-primary px-3 py-1">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="truncate text-xs text-rc-text-primary">{selectedPath.split('/').pop()}</span>
                  {selectedMemory && (
                    <span className={`text-2xs px-1 rounded ${
                      selectedMemory.type === 'user'
                        ? 'bg-rc-accent-primary/10 text-rc-accent-primary'
                        : 'bg-rc-accent-success/10 text-rc-accent-success'
                    }`}>
                      {selectedMemory.type === 'user' ? '全局' : '项目'}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1">
                  {editing ? (
                    <button
                      type="button"
                      onClick={handleSave}
                      className="rounded px-2 py-0.5 text-2xs bg-rc-accent-primary text-white hover:opacity-90"
                    >
                      保存
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={() => setEditing(true)}
                      className="rounded px-2 py-0.5 text-2xs text-rc-text-tertiary hover:bg-rc-bg-hover"
                    >
                      编辑
                    </button>
                  )}
                  {onDeleteMemory && selectedPath && (
                    <button
                      type="button"
                      onClick={() => {
                        if (confirm('确认删除此记忆?')) {
                          onDeleteMemory(selectedPath);
                          setSelectedPath(null);
                        }
                      }}
                      className="rounded p-1 text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-accent-error"
                      title="删除"
                    >
                      <Trash2 size={12} />
                    </button>
                  )}
                </div>
              </div>
              <div className="flex-1 overflow-auto">
                {editing ? (
                  <textarea
                    value={content}
                    onChange={(e) => setContent(e.target.value)}
                    className="h-full w-full resize-none bg-transparent p-3 font-mono text-xs text-rc-text-inverse outline-none"
                    placeholder="输入记忆内容（Markdown 格式）..."
                  />
                ) : (
                  <div className="p-3 text-xs text-rc-text-inverse whitespace-pre-wrap">{content || '(空)'}</div>
                )}
              </div>
            </>
          ) : (
            <div className="flex h-full items-center justify-center text-rc-text-tertiary text-xs">
              <div className="text-center">
                <Brain size={28} className="mx-auto mb-2 opacity-40" />
                <p>选择一个记忆条目查看</p>
                <p className="mt-1 text-2xs">记忆分为全局和项目两种作用域</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}