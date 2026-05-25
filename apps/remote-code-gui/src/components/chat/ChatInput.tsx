import {
  ChevronDown,
  Cpu,
  Image as ImageIcon,
  Paperclip,
  Send,
  Shield,
  Slash,
  Sparkles,
  Square,
  X,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
  type KeyboardEvent,
} from 'react';
import { AgentSelector } from '../agent/AgentSelector';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';

const PERMISSION_MODES = [
  { value: 'default', label: '默认', desc: '读取自动执行，写入和命令需确认' },
  { value: 'acceptEdits', label: '自动编辑', desc: '文件编辑自动执行，命令仍需确认' },
  { value: 'dontAsk', label: '不询问', desc: '仅自动放行低风险读取工具' },
  { value: 'bypassPermissions', label: '全自动', desc: '跳过全部权限确认' },
  { value: 'plan', label: '规划', desc: '只规划，不执行工具' },
] as const;

interface AttachedFile {
  id: string;
  name: string;
  type: string;
  size: number;
  preview?: string;
}

interface SlashCommand {
  name: string;
  description: string;
  icon: string;
}

const SLASH_COMMANDS: SlashCommand[] = [
  { name: '/goal', description: 'Set a goal for the current session', icon: '◎' },
  { name: '/compact', description: 'Compact conversation context', icon: '⊞' },
  { name: '/clear', description: 'Clear the current conversation', icon: '⊘' },
  { name: '/plan', description: 'Switch to plan-only mode', icon: '▶' },
  { name: '/review', description: 'Start a code review', icon: '⊡' },
  { name: '/doctor', description: 'Run diagnostics check', icon: '⊕' },
];

function Dropdown({
  open,
  onToggle,
  trigger,
  children,
}: {
  open: boolean;
  onToggle: () => void;
  trigger: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="relative">
      <div onClick={onToggle}>{trigger}</div>
      {open && (
        <>
          <button
            aria-label="关闭下拉菜单"
            className="fixed inset-0 z-10 cursor-default"
            onClick={onToggle}
          />
          <div className="absolute bottom-full left-0 z-20 mb-2 min-w-[240px] overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-lg animate-fade-in-up">
            <div className="max-h-72 overflow-y-auto p-1.5">{children}</div>
          </div>
        </>
      )}
    </div>
  );
}

function DropdownItem({
  title,
  subtitle,
  active,
  onClick,
  icon,
}: {
  title: string;
  subtitle?: string;
  active?: boolean;
  onClick: () => void;
  icon?: string;
}) {
  return (
    <button
      className={`flex w-full items-start gap-2.5 rounded-md px-3 py-2 text-left transition-all duration-150 ${
        active
          ? 'bg-rc-bg-selected text-rc-text-primary'
          : 'text-rc-text-primary hover:bg-rc-bg-hover'
      }`}
      onClick={onClick}
    >
      {icon && <span className="mt-0.5 text-sm text-rc-text-tertiary">{icon}</span>}
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{title}</div>
        {subtitle && <div className="mt-0.5 text-xs text-rc-text-tertiary">{subtitle}</div>}
      </div>
    </button>
  );
}

function Chip({
  icon: Icon,
  label,
  onClick,
  active,
}: {
  icon: React.ElementType;
  label: string;
  onClick?: () => void;
  active?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-xs transition-colors ${
        active
          ? 'border-rc-accent-primary bg-rc-bg-selected text-rc-accent-primary'
          : 'border-rc-border-secondary bg-rc-bg-elevated text-rc-text-secondary hover:border-rc-border-hover hover:bg-rc-bg-hover hover:text-rc-text-primary'
      } focus-visible:outline-none`}
    >
      <Icon size={14} className={active ? 'text-rc-accent-primary' : 'text-rc-text-tertiary'} />
      <span className="max-w-[180px] truncate font-medium">{label}</span>
      <ChevronDown size={12} className="text-rc-text-tertiary" />
    </button>
  );
}

function FileChip({
  file,
  onRemove,
}: {
  file: AttachedFile;
  onRemove: () => void;
}) {
  return (
    <div className="inline-flex h-7 items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-tertiary pl-2 pr-1 text-xs text-rc-text-secondary">
      {file.type.startsWith('image/') ? (
        <ImageIcon size={12} className="text-rc-accent-info" />
      ) : (
        <Paperclip size={12} className="text-rc-text-tertiary" />
      )}
      <span className="max-w-[120px] truncate font-medium">{file.name}</span>
      <button
        type="button"
        onClick={onRemove}
        className="flex h-5 w-5 items-center justify-center rounded hover:bg-rc-bg-hover hover:text-rc-accent-error"
        aria-label={`移除 ${file.name}`}
      >
        <X size={10} />
      </button>
    </div>
  );
}

function SlashCommandPalette({
  commands,
  filter,
  onSelect,
  highlightedIndex,
}: {
  commands: SlashCommand[];
  filter: string;
  onSelect: (cmd: SlashCommand) => void;
  highlightedIndex: number;
}) {
  const filtered = commands.filter(
    (cmd) =>
      cmd.name.toLowerCase().includes(filter.toLowerCase()) ||
      cmd.description.toLowerCase().includes(filter.toLowerCase()),
  );

  if (filtered.length === 0) return null;

  return (
    <div className="absolute bottom-full left-0 right-0 z-20 mb-2 overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-lg animate-fade-in-up">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
        Commands
      </div>
      <div className="max-h-60 overflow-y-auto pb-1">
        {filtered.map((cmd, index) => (
          <button
            key={cmd.name}
            className={`flex w-full items-center gap-3 px-3 py-2 text-left text-sm transition-colors ${
              index === highlightedIndex
                ? 'bg-rc-bg-selected text-rc-text-primary'
                : 'text-rc-text-primary hover:bg-rc-bg-hover'
            }`}
            onClick={() => onSelect(cmd)}
          >
            <span className="text-base">{cmd.icon}</span>
            <div className="min-w-0 flex-1">
              <div className="font-medium">{cmd.name}</div>
              <div className="text-xs text-rc-text-tertiary">{cmd.description}</div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function AttachmentPreview({ file }: { file: AttachedFile }) {
  if (!file.type.startsWith('image/') || !file.preview) return null;
  return (
    <div className="relative inline-block">
      <img
        src={file.preview}
        alt={file.name}
        className="h-20 max-w-[160px] rounded-md border border-rc-border-secondary object-cover"
      />
    </div>
  );
}

export function ChatInput() {
  const [input, setInput] = useState('');
  const [modelDraft, setModelDraft] = useState('');
  const [openMenu, setOpenMenu] = useState<'provider' | 'permission' | null>(null);
  const [attachments, setAttachments] = useState<AttachedFile[]>([]);
  const [showSlashPalette, setShowSlashPalette] = useState(false);
  const [slashFilter, setSlashFilter] = useState('');
  const [highlightedSlashIndex, setHighlightedSlashIndex] = useState(0);
  const [isDragOver, setIsDragOver] = useState(false);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const pendingFileReadersRef = useRef<FileReader[]>([]);

  const sending = useAppStore((state) => state.sending);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const settings = useAppStore((state) => state.settings);
  const provider = useAppStore((state) => state.provider);
  const providerConfigs = useAppStore((state) => state.providerConfigs);
  const availableAgents = useAgentStore((state) => state.availableAgents);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const sendMessage = useAppStore((state) => state.sendMessage);
  const cancelPrompt = useAppStore((state) => state.cancelPrompt);
  const updateSettings = useAppStore((state) => state.updateSettings);
  const setActiveProvider = useAppStore((state) => state.setActiveProvider);
  const selectAgent = useAgentStore((state) => state.selectAgent);

  // Abort any pending FileReader operations on unmount to prevent
  // state updates after the component is gone.
  useEffect(() => {
    return () => {
      for (const reader of pendingFileReadersRef.current) {
        try { reader.abort(); } catch { /* already completed */ }
      }
      pendingFileReadersRef.current = [];
    };
  }, []);

  useEffect(() => {
    const element = textAreaRef.current;
    if (!element) return;
    element.style.height = '0px';
    element.style.height = `${Math.min(element.scrollHeight, 240)}px`;
  }, [input]);

  const permissionLabel = useMemo(
    () =>
      PERMISSION_MODES.find((mode) => mode.value === settings?.permission_mode)?.label ?? '默认',
    [settings?.permission_mode],
  );

  const activeProviderName = providerConfigs?.active_provider ?? provider?.name ?? '未配置';
  const providerOptions = providerConfigs?.providers ?? [];
  useEffect(() => {
    setModelDraft(settings?.provider_model ?? provider?.model ?? '');
  }, [provider?.model, settings?.provider_model]);

  const addFiles = useCallback((files: FileList | File[]) => {
    const newFiles: AttachedFile[] = Array.from(files).map((file) => ({
      id: `${file.name}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name: file.name,
      type: file.type || 'application/octet-stream',
      size: file.size,
    }));
    setAttachments((prev) => [...prev, ...newFiles]);

    Array.from(files).forEach((file, i) => {
      if (file.type.startsWith('image/')) {
        // Skip preview generation for large files to avoid bloating memory
        if (file.size > 500 * 1024) return;
        const targetId = newFiles[i].id;
        const reader = new FileReader();
        pendingFileReadersRef.current.push(reader);
        reader.onload = (e) => {
          const preview = e.target?.result as string;
          setAttachments((prev) =>
            prev.map((a) =>
              a.id === targetId && !a.preview ? { ...a, preview } : a,
            ),
          );
        };
        reader.readAsDataURL(file);
      }
    });
  }, []);

  const removeAttachment = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  }, []);

  const handleSend = async () => {
    if (!input.trim() || sending) return;
    if (attachments.length > 0) {
      console.warn('[ChatInput] file attachments are collected but not yet transmitted. File send is not implemented.');
    }
    const current = input;
    setInput('');
    setShowSlashPalette(false);
    setAttachments([]);
    await sendMessage(current);
  };

  const handleCancel = async () => {
    if (!activeSessionId) return;
    await cancelPrompt(activeSessionId);
  };

  const handleKeyDown = async (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (showSlashPalette) {
      const filteredCommands = SLASH_COMMANDS.filter(
        (cmd) =>
          cmd.name.toLowerCase().includes(slashFilter.toLowerCase()) ||
          cmd.description.toLowerCase().includes(slashFilter.toLowerCase()),
      );

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setHighlightedSlashIndex((prev) =>
          prev < filteredCommands.length - 1 ? prev + 1 : 0,
        );
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        setHighlightedSlashIndex((prev) =>
          prev > 0 ? prev - 1 : filteredCommands.length - 1,
        );
        return;
      }
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        if (filteredCommands[highlightedSlashIndex]) {
          const cmd = filteredCommands[highlightedSlashIndex];
          setInput(cmd.name + ' ');
          setShowSlashPalette(false);
          textAreaRef.current?.focus();
        }
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        setShowSlashPalette(false);
        return;
      }
    }

    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      await handleSend();
    }
  };

  const handleInputChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = event.target.value;
    setInput(value);

    if (value.startsWith('/') && !value.includes(' ')) {
      setShowSlashPalette(true);
      setSlashFilter(value);
      setHighlightedSlashIndex(0);
    } else {
      setShowSlashPalette(false);
    }
  };

  const handleSlashSelect = (cmd: SlashCommand) => {
    setInput(cmd.name + ' ');
    setShowSlashPalette(false);
    textAreaRef.current?.focus();
  };

  const handleDragOver = (event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setIsDragOver(true);
  };

  const handleDragLeave = (event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setIsDragOver(false);
  };

  const handleDrop = (event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setIsDragOver(false);
    if (event.dataTransfer.files.length > 0) {
      addFiles(event.dataTransfer.files);
    }
  };

  const handlePaste = (event: React.ClipboardEvent) => {
    const items = event.clipboardData?.items;
    if (!items) return;

    const imageFiles: File[] = [];
    for (const item of Array.from(items)) {
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile();
        if (file) imageFiles.push(file);
      }
    }
    if (imageFiles.length > 0) {
      event.preventDefault();
      addFiles(imageFiles);
    }
  };

  const lastCommittedModel = useRef(modelDraft.trim());
  const modelDraftRef = useRef(modelDraft);
  modelDraftRef.current = modelDraft;
  const commitModelDraft = async () => {
    const trimmed = modelDraftRef.current.trim();
    if (trimmed === lastCommittedModel.current) return;
    lastCommittedModel.current = trimmed;
    await updateSettings({ provider_model: trimmed });
  };

  const hasAttachments = attachments.length > 0;
  const imageAttachments = attachments.filter((a) => a.type.startsWith('image/'));

  return (
    <div role="form" aria-label="Prompt composer" className="bg-rc-bg-chat px-5 pb-5 pt-3">
      <div className="mx-auto w-full max-w-input">
        <div
          className={`relative overflow-visible rounded-[18px] border shadow-lg transition-colors ${
            isDragOver
              ? 'border-rc-accent-primary bg-rc-bg-selected'
              : 'border-rc-border-primary bg-rc-bg-surface focus-within:border-rc-border-focus'
          }`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {isDragOver && (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-[18px] bg-rc-accent-primary-light">
              <div className="flex items-center gap-2 text-sm font-medium text-rc-accent-primary">
                <Paperclip size={16} />
                拖放文件到此处
              </div>
            </div>
          )}

          {showSlashPalette && (
            <SlashCommandPalette
              commands={SLASH_COMMANDS}
              filter={slashFilter}
              onSelect={handleSlashSelect}
              highlightedIndex={highlightedSlashIndex}
            />
          )}

          {imageAttachments.length > 0 && (
            <div className="flex flex-wrap gap-2 border-b border-rc-border-secondary px-3 py-2">
              {imageAttachments.map((file) => (
                <AttachmentPreview key={file.id} file={file} />
              ))}
            </div>
          )}

          <div className="px-4 pb-2 pt-3">
            <textarea
              ref={textAreaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={(event) => {
                void handleKeyDown(event);
              }}
              onPaste={handlePaste}
              disabled={sending}
              rows={1}
              aria-label="Prompt input"
              placeholder="向 agent 发送指令或代码片段"
              className="min-h-[58px] max-h-[180px] w-full resize-none border-0 bg-transparent px-1 py-1 text-sm leading-6 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed focus-visible:outline-none"
            />
          </div>

          {hasAttachments && (
            <div className="flex flex-wrap gap-1.5 border-t border-rc-border-secondary px-3 py-2">
              {attachments.map((file) => (
                <FileChip
                  key={file.id}
                  file={file}
                  onRemove={() => removeAttachment(file.id)}
                />
              ))}
            </div>
          )}

          <div
            role="group"
            aria-label="Composer controls"
            className="flex min-h-11 flex-wrap items-center gap-2 border-t border-rc-border-secondary bg-rc-bg-elevated px-3 py-2"
          >
            <AgentSelector
              availableAgents={availableAgents}
              activeAgentType={activeAgentType}
              onSelect={selectAgent}
            />

            <Dropdown
              open={openMenu === 'provider'}
              onToggle={() => setOpenMenu((state) => (state === 'provider' ? null : 'provider'))}
              trigger={
                <Chip
                  icon={Cpu}
                  label={activeProviderName}
                  active={openMenu === 'provider'}
                />
              }
            >
              {providerOptions.length > 0 ? (
                providerOptions.map((providerOption) => (
                  <DropdownItem
                    key={providerOption.name}
                    title={providerOption.name}
                    subtitle={[
                      providerOption.model ?? '未设置默认模型',
                      providerOption.protocol,
                    ]
                      .filter(Boolean)
                      .join(' · ')}
                    active={providerConfigs?.active_provider === providerOption.name}
                    onClick={async () => {
                      setOpenMenu(null);
                      await setActiveProvider(providerOption.name);
                    }}
                  />
                ))
              ) : (
                <DropdownItem
                  title={provider?.name ?? '未配置 Provider'}
                  subtitle="去设置面板添加或导入 Provider"
                  onClick={() => setOpenMenu(null)}
                />
              )}
            </Dropdown>

            <div className="inline-flex h-7 min-w-0 items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-2 text-xs text-rc-text-secondary transition-colors hover:border-rc-border-hover">
              <Sparkles size={14} className="shrink-0 text-rc-text-tertiary" />
              <input
                value={modelDraft}
                onChange={(event) => setModelDraft(event.target.value)}
                onBlur={() => {
                  void commitModelDraft();
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    void commitModelDraft();
                  }
                }}
                aria-label="Model for next send"
                className="w-[160px] min-w-0 border-0 bg-transparent text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-0 focus:outline-none focus:ring-0 focus-visible:outline-none"
                style={{ outline: 'none' }}
                placeholder="设置模型"
                title="为下一次发送设置模型"
              />
            </div>

            <Dropdown
              open={openMenu === 'permission'}
              onToggle={() => setOpenMenu((state) => (state === 'permission' ? null : 'permission'))}
              trigger={
                <Chip
                  icon={Shield}
                  label={permissionLabel}
                  active={openMenu === 'permission'}
                />
              }
            >
              {PERMISSION_MODES.map((mode) => (
                <DropdownItem
                  key={mode.value}
                  title={mode.label}
                  subtitle={mode.desc}
                  active={settings?.permission_mode === mode.value}
                  onClick={async () => {
                    setOpenMenu(null);
                    await updateSettings({ permission_mode: mode.value });
                  }}
                />
              ))}
            </Dropdown>

            <div className="flex-1" />

            {/* TODO: Enable file picker once file transmission is implemented */}
            <button
              type="button"
              aria-label="附加文件"
              title="附加文件（暂未实现）"
              disabled
              onClick={() => fileInputRef.current?.click()}
              className="flex h-8 w-8 items-center justify-center rounded-full text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary disabled:cursor-not-allowed disabled:opacity-45 focus-visible:outline-none"
            >
              <Paperclip size={15} />
            </button>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={(event) => {
                if (event.target.files) addFiles(event.target.files);
                event.target.value = '';
              }}
            />

            {sending && activeSessionId && (
              <button
                type="button"
                aria-label="停止当前运行"
                title="停止当前运行"
                onClick={() => {
                  void handleCancel();
                }}
                className="flex h-8 w-8 items-center justify-center rounded-full border border-rc-accent-warning-border bg-rc-accent-warning-bg text-rc-accent-warning transition-colors hover:bg-rc-bg-hover focus-visible:outline-none"
              >
                <Square size={15} />
              </button>
            )}

            <button
              type="button"
              aria-label="发送"
              onClick={() => {
                void handleSend();
              }}
              disabled={sending || !input.trim()}
              className="flex h-8 w-8 items-center justify-center rounded-full bg-rc-text-primary text-rc-bg-base shadow-sm transition-colors hover:bg-rc-accent-primary hover:text-white disabled:cursor-not-allowed disabled:opacity-45 focus-visible:outline-none"
            >
              {sending ? (
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-white/25 border-t-white" />
              ) : (
                <Send size={16} />
              )}
            </button>
          </div>
        </div>

        {showSlashPalette && input.startsWith('/') && (
          <div className="mt-1 text-[10px] text-rc-text-tertiary">
            ↑↓ 导航 · Enter 选择 · Esc 关闭
          </div>
        )}
      </div>
    </div>
  );
}
