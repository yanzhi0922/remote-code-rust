import {
  ChevronDown,
  Cpu,
  Send,
  Shield,
  Slash,
  Sparkles,
  Square,
} from 'lucide-react';
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';
import { AgentSelector } from '../agent/AgentSelector';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import type { AgentType, FullSettings } from '../../lib/types';

type PermissionOption = {
  key: string;
  label: string;
  desc: string;
  active: boolean;
  updates: Record<string, unknown>;
};

const CLAUDE_PERMISSION_MODES = [
  { value: 'default', label: '默认', desc: '读取自动执行，写入和命令需确认' },
  { value: 'acceptEdits', label: '自动编辑', desc: '文件编辑自动执行，命令仍需确认' },
  { value: 'dontAsk', label: '不询问', desc: '仅自动放行低风险读取工具' },
  { value: 'bypassPermissions', label: '全自动', desc: '跳过全部权限确认' },
  { value: 'plan', label: '规划', desc: '只规划，不执行工具' },
] as const;

const ROO_PERMISSION_MODES = [
  { value: 'default', label: '每次询问', desc: '工具执行前确认，保留 Roo Code 的手动审批节奏' },
  { value: 'dontAsk', label: '自动批准读取', desc: '低风险读取工具自动执行，写入和命令仍确认' },
  { value: 'acceptEdits', label: '自动批准编辑', desc: '文件读写自动执行，命令和外部访问仍确认' },
  { value: 'bypassPermissions', label: '自动批准全部', desc: '自动放行工具调用，仅用于可信工作区' },
  { value: 'plan', label: '仅规划', desc: '只生成计划，不执行工具' },
] as const;

const CODEX_PERMISSION_MODES = [
  {
    key: 'codex-on-request-workspace',
    label: '请求批准',
    desc: 'workspace-write 沙盒，越界写入或高风险命令时请求确认',
    approval: 'on-request',
    sandbox: 'workspace-write',
  },
  {
    key: 'codex-never-workspace',
    label: '沙盒自动',
    desc: 'workspace-write 沙盒内自动执行，不弹权限确认',
    approval: 'never',
    sandbox: 'workspace-write',
  },
  {
    key: 'codex-on-request-readonly',
    label: '只读沙盒',
    desc: 'read-only 沙盒，写入和命令需要显式批准',
    approval: 'on-request',
    sandbox: 'read-only',
  },
  {
    key: 'codex-on-request-full',
    label: '完全访问',
    desc: 'danger-full-access，无沙盒；危险操作仍请求批准',
    approval: 'on-request',
    sandbox: 'danger-full-access',
  },
  {
    key: 'codex-never-full',
    label: '全自动访问',
    desc: 'danger-full-access 且不请求确认，仅用于完全可信环境',
    approval: 'never',
    sandbox: 'danger-full-access',
  },
] as const;

const MODEL_CONTEXT_WINDOWS: Array<[RegExp, number]> = [
  [/gpt-4\.1/i, 1_000_000],
  [/gpt-5/i, 400_000],
  [/gpt-4o/i, 128_000],
  [/\bo[34]\b/i, 200_000],
  [/claude.*(sonnet|opus|haiku|3\.5|3-5|3\.7|3-7|4)/i, 200_000],
  [/gemini/i, 1_000_000],
  [/deepseek/i, 128_000],
  [/qwen/i, 128_000],
  [/glm-(5\.1|4\.5|4)/i, 128_000],
  [/minimax|m2\.7/i, 200_000],
];

function permissionOptionsForAgent(
  agentType: AgentType,
  settings: FullSettings | null,
): PermissionOption[] {
  if (agentType === 'remote_codex') {
    const approval = settings?.codex_approval_policy ?? 'on-request';
    const sandbox = settings?.codex_sandbox_mode ?? 'workspace-write';
    return CODEX_PERMISSION_MODES.map((mode) => ({
      key: mode.key,
      label: mode.label,
      desc: mode.desc,
      active: approval === mode.approval && sandbox === mode.sandbox,
      updates: {
        codex_approval_policy: mode.approval,
        codex_sandbox_mode: mode.sandbox,
      },
    }));
  }

  const modes = agentType === 'remote_roo' ? ROO_PERMISSION_MODES : CLAUDE_PERMISSION_MODES;
  return modes.map((mode) => ({
    key: mode.value,
    label: mode.label,
    desc: mode.desc,
    active: (settings?.permission_mode ?? 'default') === mode.value,
    updates: { permission_mode: mode.value },
  }));
}

function inferModelContextWindow(model: string): number | null {
  const trimmed = model.trim();
  if (!trimmed) return null;
  const explicit = trimmed.match(/(?:^|[^a-z0-9])(\d+(?:\.\d+)?)(k|m)(?:[^a-z0-9]|$)/i);
  if (explicit) {
    const amount = Number(explicit[1]);
    if (Number.isFinite(amount)) {
      return Math.round(amount * (explicit[2].toLowerCase() === 'm' ? 1_000_000 : 1_000));
    }
  }
  return MODEL_CONTEXT_WINDOWS.find(([pattern]) => pattern.test(trimmed))?.[1] ?? null;
}

function formatTokenCount(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(tokens % 1_000_000 === 0 ? 0 : 1)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(tokens % 1_000 === 0 ? 0 : 1)}K`;
  return String(tokens);
}

interface SlashCommand {
  name: string;
  description: string;
  icon: string;
}

const SLASH_COMMANDS: SlashCommand[] = [
  { name: '/goal', description: '设置当前会话目标', icon: '◎' },
  { name: '/compact', description: '压缩当前上下文', icon: '⊞' },
  { name: '/clear', description: '清空当前会话内容', icon: '⊘' },
  { name: '/plan', description: '切换到只规划模式', icon: '▶' },
  { name: '/review', description: '开始代码审查', icon: '⊡' },
  { name: '/doctor', description: '运行诊断检查', icon: '⊕' },
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
    <div role="listbox" aria-label="Slash commands" className="absolute bottom-full left-0 right-0 z-20 mb-2 overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-lg animate-fade-in-up">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
        命令
      </div>
      <div className="max-h-60 overflow-y-auto pb-1">
        {filtered.map((cmd, index) => (
          <button
            key={cmd.name}
            role="option"
            aria-selected={index === highlightedIndex}
            id={`slash-option-${index}`}
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

export function ChatInput() {
  const [input, setInput] = useState('');
  const [modelDraft, setModelDraft] = useState('');
  const modelDraftRef = useRef(modelDraft);
  const [openMenu, setOpenMenu] = useState<'provider' | 'permission' | null>(null);
  const [showSlashPalette, setShowSlashPalette] = useState(false);
  const [slashFilter, setSlashFilter] = useState('');
  const [highlightedSlashIndex, setHighlightedSlashIndex] = useState(0);
  const [modelError, setModelError] = useState<string | null>(null);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const lastCommittedModel = useRef('');

  const sending = useAppStore((state) => state.sending);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessions = useAppStore((state) => state.sessions);
  const settings = useAppStore((state) => state.settings);
  const provider = useAppStore((state) => state.provider);
  const providerConfigs = useAppStore((state) => state.providerConfigs);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
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

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const lockedAgentType = activeSession?.agent_type ?? null;
  const effectiveAgentType = lockedAgentType ?? activeAgentType ?? 'remote_claude';
  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;

  const permissionOptions = useMemo(
    () => permissionOptionsForAgent(effectiveAgentType, settings ?? null),
    [effectiveAgentType, settings],
  );
  const permissionLabel = permissionOptions.find((mode) => mode.active)?.label ?? permissionOptions[0]?.label ?? '权限';

  const activeProviderName = providerConfigs?.active_provider ?? provider?.name ?? '未配置';
  const providerOptions = providerConfigs?.providers ?? [];
  useEffect(() => {
    const nextModel = settings?.provider_model ?? provider?.model ?? '';
    setModelDraft(nextModel);
    lastCommittedModel.current = nextModel.trim();
    setModelError(null);
  }, [provider?.model, settings?.provider_model]);

  const ensureModelFitsContext = (model: string): boolean => {
    const usedTokens = contextUsage?.estimated_tokens ?? 0;
    const nextWindow = inferModelContextWindow(model);
    if (usedTokens > 0 && nextWindow !== null && nextWindow < usedTokens) {
      setModelError(
        `当前会话已使用 ${formatTokenCount(usedTokens)} tokens，${model} 约 ${formatTokenCount(nextWindow)}，不能切换到更小上下文。`,
      );
      return false;
    }
    setModelError(null);
    return true;
  };

  const handleSend = async () => {
    if (!input.trim() || sending) return;
    const current = input;
    setInput('');
    setShowSlashPalette(false);
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

  // Keep ref in sync so commitModelDraft always uses the latest value.
  modelDraftRef.current = modelDraft;
  const commitModelDraft = async () => {
    const trimmed = modelDraftRef.current.trim();
    if (trimmed === lastCommittedModel.current) return;
    if (!ensureModelFitsContext(trimmed)) {
      setModelDraft(lastCommittedModel.current);
      return;
    }
    lastCommittedModel.current = trimmed;
    await updateSettings({ provider_model: trimmed });
  };

  return (
    <div role="form" aria-label="Prompt composer" className="bg-rc-bg-chat px-4 pb-4 pt-3">
      <div className="mx-auto w-full max-w-input">
        <div
          className="relative overflow-visible rounded-lg border border-rc-border-primary bg-rc-bg-surface shadow-sm transition-colors focus-within:border-rc-border-focus"
        >
          {showSlashPalette && (
            <SlashCommandPalette
              commands={SLASH_COMMANDS}
              filter={slashFilter}
              onSelect={handleSlashSelect}
              highlightedIndex={highlightedSlashIndex}
            />
          )}

          <div className="px-4 pb-2 pt-3">
            <textarea
              ref={textAreaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={(event) => {
                void handleKeyDown(event);
              }}
              disabled={sending}
              rows={1}
              aria-label="Prompt input"
              aria-activedescendant={showSlashPalette ? `slash-option-${highlightedSlashIndex}` : undefined}
              placeholder="给 agent 发送任务、补充约束或后续修改"
              className="min-h-[64px] max-h-[180px] w-full resize-none border-0 bg-transparent px-1 py-1 text-sm leading-6 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed focus-visible:outline-none"
            />
          </div>

          <div
            role="group"
            aria-label="Composer controls"
            className="flex min-h-11 flex-wrap items-center gap-2 border-t border-rc-border-secondary bg-rc-bg-elevated px-3 py-2"
          >
            <AgentSelector
              availableAgents={availableAgents}
              activeAgentType={effectiveAgentType}
              lockedAgentType={lockedAgentType}
              lockedReason="该会话已经绑定 Agent，不能切换到其他 Agent"
              onSelect={(agentType) => {
                if (lockedAgentType && agentType !== lockedAgentType) return;
                selectAgent(agentType);
              }}
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
                      if (providerOption.model && !ensureModelFitsContext(providerOption.model)) return;
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
              {permissionOptions.map((mode) => (
                <DropdownItem
                  key={mode.key}
                  title={mode.label}
                  subtitle={mode.desc}
                  active={mode.active}
                  onClick={async () => {
                    setOpenMenu(null);
                    await updateSettings(mode.updates);
                  }}
                />
              ))}
            </Dropdown>

            <div className="flex-1" />

            {sending && activeSessionId && (
              <button
                type="button"
                aria-label="停止当前运行"
                title="停止当前运行"
                onClick={() => {
                  void handleCancel();
                }}
                className="flex h-8 w-8 items-center justify-center rounded-lg border border-rc-accent-warning-border bg-rc-accent-warning-bg text-rc-accent-warning transition-colors hover:bg-rc-bg-hover focus-visible:outline-none"
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
              className="flex h-8 w-8 items-center justify-center rounded-lg bg-rc-accent-primary text-white shadow-sm transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:opacity-45 focus-visible:outline-none"
            >
              {sending ? (
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-white/25 border-t-white" />
              ) : (
                <Send size={16} />
              )}
            </button>
          </div>

          {modelError && (
            <div className="border-t border-rc-border-secondary bg-rc-accent-warning-bg px-4 py-2 text-xs text-rc-accent-warning">
              {modelError}
            </div>
          )}
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
