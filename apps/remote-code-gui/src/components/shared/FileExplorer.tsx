import { useEffect, useState, useCallback } from 'react';
import {
  ChevronRight,
  ChevronDown,
  File,
  FileCode2,
  FileJson,
  FileText as FileTextIcon,
  Folder,
  FolderOpen,
  ArrowLeft,
  Loader2,
  AlertCircle,
  Image,
  FileType2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import * as tauri from '../../lib/tauri';
import { cn } from '../../lib/utils';

function getFileIcon(name: string) {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (['ts', 'tsx', 'js', 'jsx', 'rs', 'py', 'go', 'java', 'c', 'cpp', 'h', 'rb', 'swift', 'kt'].includes(ext)) return FileCode2;
  if (['json', 'toml', 'yaml', 'yml', 'xml'].includes(ext)) return FileJson;
  if (['md', 'txt', 'log', 'csv'].includes(ext)) return FileTextIcon;
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'].includes(ext)) return Image;
  if (['css', 'scss', 'less', 'sass', 'html', 'htm'].includes(ext)) return FileType2;
  return File;
}

interface FsEntry {
  name: string;
  path: string;
  isDir: boolean;
}

function FileTreeNode({
  entry,
  depth = 0,
  onOpenFile,
}: {
  entry: FsEntry;
  depth?: number;
  onOpenFile: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<FsEntry[] | null>(null);
  const [loading, setLoading] = useState(false);

  const toggle = useCallback(async () => {
    if (!entry.isDir) return;
    if (children !== null) { setExpanded(!expanded); return; }
    setLoading(true);
    try {
      const result = await tauri.codexFsReadDirectory({ path: entry.path });
      if (result && typeof result === 'object' && !Array.isArray(result)) {
        const raw = ((result as Record<string, unknown>).entries ?? []) as Array<{ name: string; path: string; is_dir: boolean }>;
        setChildren(
          raw
            .map((e) => ({ name: e.name, path: e.path, isDir: e.is_dir }))
            .sort((a, b) => {
              if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
              return a.name.localeCompare(b.name);
            }),
        );
      }
      setExpanded(true);
    } catch (_err) {
      setChildren([]);
    } finally {
      setLoading(false);
    }
  }, [entry, children, expanded]);

  if (entry.isDir) {
    return (
      <div>
        <button
          type="button"
          onClick={toggle}
          className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-rc-text-primary hover:bg-rc-bg-hover"
          style={{ paddingLeft: `${depth * 16 + 4}px` }}
        >
          <span className="shrink-0">
            {loading ? (
              <Loader2 size={12} className="animate-spin text-rc-text-tertiary" />
            ) : expanded ? (
              <ChevronDown size={12} className="text-rc-text-tertiary" />
            ) : (
              <ChevronRight size={12} className="text-rc-text-tertiary" />
            )}
          </span>
          {expanded ? (
            <FolderOpen size={13} className="text-rc-accent-primary shrink-0" />
          ) : (
            <Folder size={13} className="text-rc-text-tertiary shrink-0" />
          )}
          <span className="truncate">{entry.name}</span>
        </button>
        {expanded && children !== null && (
          <div>
            {children.length > 0
              ? children.map((child) => (
                  <FileTreeNode key={child.path} entry={child} depth={depth + 1} onOpenFile={onOpenFile} />
                ))
              : null}
          </div>
        )}
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={() => onOpenFile(entry.path)}
      className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
      style={{ paddingLeft: `${depth * 16 + 20}px` }}
      title={entry.path}
    >
      <span className="w-3" />
      {(() => { const Icon = getFileIcon(entry.name); return <Icon size={13} className="shrink-0 text-rc-text-tertiary" />; })()}
      <span className="truncate">{entry.name}</span>
    </button>
  );
}

interface FileExplorerProps {
  rootPath: string;
  projectName: string;
  onBack: () => void;
}

export function FileExplorer({ rootPath, projectName, onBack }: FileExplorerProps) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<FsEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    tauri
      .codexFsReadDirectory({ path: rootPath })
      .then((result) => {
        if (cancelled) return;
        if (result && typeof result === 'object' && !Array.isArray(result)) {
          const raw = ((result as Record<string, unknown>).entries ?? []) as Array<{ name: string; path: string; is_dir: boolean }>;
          setEntries(
            raw
              .map((e) => ({ name: e.name, path: e.path, isDir: e.is_dir }))
              .sort((a, b) => {
                if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
                return a.name.localeCompare(b.name);
              }),
          );
        } else {
          setEntries([]);
        }
      })
      .catch((err) => {
        if (cancelled) return;
        setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [rootPath]);

  const handleOpenFile = useCallback((path: string) => {
    void navigator.clipboard.writeText(path);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-rc-border-secondary bg-rc-bg-surface px-4">
        <button
          type="button"
          onClick={onBack}
          className="flex h-7 w-7 items-center justify-center rounded-md text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          title={t('fileExplorer.backToTasks')}
        >
          <ArrowLeft size={14} />
        </button>
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-semibold text-rc-text-primary">{projectName}</div>
          <div className="truncate text-[11px] text-rc-text-tertiary">{rootPath}</div>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-2">
        {loading ? (
          <div className="flex items-center gap-2 px-2 py-4 text-xs text-rc-text-tertiary">
            <Loader2 size={14} className="animate-spin" />
            {t('fileExplorer.loading')}
          </div>
        ) : error ? (
          <div className="flex items-center gap-2 px-2 py-4 text-xs text-rc-accent-error">
            <AlertCircle size={14} />
            {error}
          </div>
        ) : entries && entries.length > 0 ? (
          <div className="space-y-0.5">
            {entries.map((entry) => (
              <FileTreeNode key={entry.path} entry={entry} depth={0} onOpenFile={handleOpenFile} />
            ))}
          </div>
        ) : (
          <div className="px-2 py-8 text-center text-xs text-rc-text-tertiary">
            {t('fileExplorer.emptyDirectory')}
          </div>
        )}
      </div>
    </div>
  );
}
