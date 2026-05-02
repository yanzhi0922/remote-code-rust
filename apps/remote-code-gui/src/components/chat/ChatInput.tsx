import { ChevronDown, Cpu, MessageSquareText, Mic, Send, Shield, Sparkles } from 'lucide-react';
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
          <div className="absolute bottom-full left-0 z-20 mb-2 min-w-[220px] overflow-hidden rounded-2xl border border-rc-border-primary bg-rc-bg-primary shadow-xl">
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
      className={`flex w-full items-start gap-2 rounded-xl px-3 py-2 text-left transition-colors ${
        active ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-primary hover:bg-rc-bg-hover'
      }`}
      onClick={onClick}
    >
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">{title}</div>
        {subtitle && <div className="mt-0.5 text-xs text-rc-text-secondary">{subtitle}</div>}
      </div>
    </button>
  );
}

export function ChatInput() {
  const [input, setInput] = useState('');
  const [modelDraft, setModelDraft] = useState('');
  const [openMenu, setOpenMenu] = useState<'provider' | 'permission' | null>(null);
  const [voiceToast, setVoiceToast] = useState(false);
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
    <div className="border-t border-rc-border-primary bg-rc-bg-chat px-4 pb-4 pt-3 sm:px-6">
      <div className="mx-auto w-full max-w-5xl">
        <div className="rounded-[28px] border border-rc-border-primary bg-rc-bg-primary shadow-lg">
          <div className="border-b border-rc-border-secondary px-4 py-3">
            {activeSession ? (
              <div className="inline-flex max-w-full items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm font-medium text-rc-text-primary">
                <MessageSquareText size={14} className="text-rc-text-secondary" />
                <span className="truncate">当前会话 · {currentSessionLabel}</span>
              </div>
            ) : (
              <div className="text-sm text-rc-text-secondary">选择 Provider、模型和权限后直接发送即可。</div>
            )}
          </div>

          <div className="flex items-end gap-3 px-4 py-3">
            <textarea
              ref={textAreaRef}
              value={input}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={(event) => {
                void handleKeyDown(event);
              }}
              disabled={sending}
              rows={1}
              placeholder="输入需求，直接在 GUI 中运行、改代码、调用工具。Shift+Enter 换行。"
              className="min-h-[56px] flex-1 resize-none bg-transparent px-1 py-1 text-[15px] leading-6 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed"
            />

            {/* Voice input */}
            <div className="relative">
              <button
                title="语音输入"
                onClick={() => {
                  setVoiceToast(true);
                  setTimeout(() => setVoiceToast(false), 2500);
                }}
                className="flex h-12 w-12 items-center justify-center rounded-2xl border border-rc-border-primary bg-rc-bg-secondary text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
              >
                <Mic size={17} />
              </button>
              {voiceToast && (
                <div className="absolute bottom-full right-0 mb-2 whitespace-nowrap rounded-lg bg-rc-bg-user-bubble px-3 py-1.5 text-xs text-rc-text-inverse shadow-lg">
                  🎤 语音输入功能即将推出
                </div>
              )}
            </div>

            <button
              onClick={() => {
                void handleSend();
              }}
              disabled={sending || !input.trim()}
              className="flex h-12 w-12 items-center justify-center rounded-2xl bg-rc-bg-user-bubble text-rc-text-inverse shadow-lg transition-colors hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {sending ? (
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-white/25 border-t-white" />
              ) : (
                <Send size={17} />
              )}
            </button>
          </div>

          <div className="border-t border-rc-border-secondary px-4 pb-3 pt-3">
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
                  <button
                    title="为下一次发送选择 Provider"
                    className="inline-flex items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-hover"
                  >
                    <Cpu size={14} className="text-rc-text-secondary" />
                    <span className="max-w-[240px] truncate">{activeProviderName}</span>
                    <ChevronDown size={14} className="text-rc-text-tertiary" />
                  </button>
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

              <div className="inline-flex min-w-0 items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm text-rc-text-primary">
                <Sparkles size={14} className="text-rc-text-secondary" />
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
                  className="w-[220px] min-w-0 bg-transparent outline-none placeholder:text-rc-text-tertiary"
                  placeholder="为下一次发送设置模型"
                  title="为下一次发送设置模型"
                />
              </div>

              <Dropdown
                open={openMenu === 'permission'}
                onToggle={() => setOpenMenu((state) => (state === 'permission' ? null : 'permission'))}
                trigger={
                  <button className="inline-flex items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-hover">
                    <Shield size={14} className="text-rc-text-secondary" />
                    <span>{permissionLabel}</span>
                    <ChevronDown size={14} className="text-rc-text-tertiary" />
                  </button>
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

            <div className="mt-3 text-xs leading-5 text-rc-text-tertiary">
              {activeSession
                ? '继续发送时会保留当前会话的工作目录，但 Provider、模型和权限模式以这里当前选择为准。'
                : '当前选择会用于下一次发送和新建会话。'}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
