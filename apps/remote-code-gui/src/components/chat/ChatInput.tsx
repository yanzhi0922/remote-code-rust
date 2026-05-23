import { ChevronDown, Cpu, MessageSquareText, Send, Shield, Sparkles, Square } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
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
          <div className="absolute bottom-full left-0 z-20 mb-2 min-w-[240px] overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-lg">
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
}: {
  title: string;
  subtitle?: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`flex w-full items-start gap-2 rounded-md px-3 py-2 text-left transition-all duration-150 ${
        active
          ? 'bg-rc-bg-selected text-rc-text-primary'
          : 'text-rc-text-primary hover:bg-rc-bg-hover'
      }`}
      onClick={onClick}
    >
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
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded border px-2 py-1 text-xs transition-colors ${
        active
          ? 'border-rc-accent-primary bg-rc-bg-selected text-rc-accent-primary'
          : 'border-rc-border-primary bg-rc-bg-surface text-rc-text-secondary hover:border-rc-border-hover hover:bg-rc-bg-hover hover:text-rc-text-primary'
      }`}
    >
      <Icon size={14} className={active ? 'text-rc-accent-primary' : 'text-rc-text-tertiary'} />
      <span className="max-w-[180px] truncate font-medium">{label}</span>
      <ChevronDown size={12} className="text-rc-text-tertiary" />
    </button>
  );
}

export function ChatInput() {
  const [input, setInput] = useState('');
  const [modelDraft, setModelDraft] = useState('');
  const [openMenu, setOpenMenu] = useState<'provider' | 'permission' | null>(null);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);

  const sending = useAppStore((state) => state.sending);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessions = useAppStore((state) => state.sessions);
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
  const activeSession = sessions.find((session) => session.id === activeSessionId) ?? null;
  const currentSessionLabel = activeSession
    ? `${activeSession.provider_name}${activeSession.model ? ` / ${activeSession.model}` : ''}`
    : null;

  useEffect(() => {
    setModelDraft(settings?.provider_model ?? provider?.model ?? '');
  }, [provider?.model, settings?.provider_model]);

  const handleSend = async () => {
    if (!input.trim() || sending) return;
    const current = input;
    setInput('');
    await sendMessage(current);
  };

  const handleCancel = async () => {
    if (!activeSessionId) return;
    await cancelPrompt(activeSessionId);
  };

  const handleKeyDown = async (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      await handleSend();
    }
  };

  const commitModelDraft = async () => {
    await updateSettings({ provider_model: modelDraft.trim() });
  };

  return (
    <div className="border-t border-rc-border-primary bg-rc-bg-surface px-3 pb-3 pt-2">
      <div className="mx-auto w-full max-w-[1100px]">
        <div className="rounded-md border border-rc-border-primary bg-rc-bg-base focus-within:border-rc-border-focus">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-rc-border-secondary px-2.5 py-1.5">
            {activeSession ? (
              <div className="inline-flex max-w-full items-center gap-2 rounded px-2 py-1 text-xs text-rc-text-secondary">
                <MessageSquareText size={14} className="text-rc-text-tertiary" />
                <span className="truncate font-medium">{currentSessionLabel}</span>
              </div>
            ) : (
              <div className="text-xs text-rc-text-tertiary">No active session</div>
            )}

            <div className="flex flex-wrap items-center gap-2">
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
            </div>
          </div>

          <div className="flex items-end gap-2 px-2.5 py-2">
            <textarea
              ref={textAreaRef}
              value={input}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={(event) => {
                void handleKeyDown(event);
              }}
              disabled={sending}
              rows={1}
              placeholder="向 agent 发送指令或代码片段"
              className="min-h-[44px] flex-1 resize-none bg-transparent px-1 py-1 text-sm leading-6 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed"
            />

            {sending && activeSessionId && (
              <button
                type="button"
                aria-label="停止当前运行"
                title="停止当前运行"
                onClick={() => {
                  void handleCancel();
                }}
                className="flex h-9 w-9 items-center justify-center rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg text-rc-accent-warning transition-colors hover:bg-rc-bg-hover"
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
              className="flex h-9 w-9 items-center justify-center rounded-md bg-rc-accent-primary text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:opacity-45"
            >
              {sending ? (
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-white/25 border-t-white" />
              ) : (
                <Send size={17} />
              )}
            </button>
          </div>

          <div className="border-t border-rc-border-secondary px-2.5 py-1.5">
            <div className="flex flex-wrap items-center gap-2">
              <div className="inline-flex min-w-0 items-center gap-2 rounded border border-rc-border-primary bg-rc-bg-surface px-2 py-1 text-xs text-rc-text-secondary transition-colors hover:border-rc-border-hover">
                <Sparkles size={14} className="text-rc-text-tertiary" />
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
                  className="w-[200px] min-w-0 bg-transparent text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary"
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
            </div>

          </div>
        </div>
      </div>
    </div>
  );
}
