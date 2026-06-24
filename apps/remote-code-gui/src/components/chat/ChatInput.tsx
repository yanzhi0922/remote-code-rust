import {
  AtSign,
  ChevronDown,
  Cpu,
  GitBranch,
  Image as ImageIcon,
  Paperclip,
  Pencil,
  Send,
  Shield,
  Sparkles,
  Square,
  Wand2,
  X,
  Zap,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';
import { useTranslation } from 'react-i18next';
import { AgentSelector } from '../agent/AgentSelector';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import * as tauri from '../../lib/tauri';
import type { AgentType, FullSettings } from '../../lib/types';

type PermissionOption = {
  key: string;
  label: string;
  desc: string;
  active: boolean;
  updates: Record<string, unknown>;
};

type TFn = (key: string) => string;

function claudePermissionModes(t: TFn) {
  return [
    { value: 'default', label: t('chatInput.permission.claude.default'), desc: t('chatInput.permission.claude.defaultDesc') },
    { value: 'acceptEdits', label: t('chatInput.permission.claude.acceptEdits'), desc: t('chatInput.permission.claude.acceptEditsDesc') },
    { value: 'dontAsk', label: t('chatInput.permission.claude.dontAsk'), desc: t('chatInput.permission.claude.dontAskDesc') },
    { value: 'bypassPermissions', label: t('chatInput.permission.claude.bypassPermissions'), desc: t('chatInput.permission.claude.bypassPermissionsDesc') },
    { value: 'plan', label: t('chatInput.permission.claude.plan'), desc: t('chatInput.permission.claude.planDesc') },
  ] as const;
}

function rooPermissionModes(t: TFn) {
  return [
    { value: 'code', label: t('chatInput.permission.roo.code'), desc: t('chatInput.permission.roo.codeDesc') },
    { value: 'architect', label: t('chatInput.permission.roo.architect'), desc: t('chatInput.permission.roo.architectDesc') },
    { value: 'ask', label: t('chatInput.permission.roo.ask'), desc: t('chatInput.permission.roo.askDesc') },
    { value: 'debug', label: t('chatInput.permission.roo.debug'), desc: t('chatInput.permission.roo.debugDesc') },
    { value: 'orchestrator', label: t('chatInput.permission.roo.orchestrator'), desc: t('chatInput.permission.roo.orchestratorDesc') },
  ] as const;
}

function codexPermissionModes(t: TFn) {
  return [
    {
      key: 'codex-on-request-workspace',
      label: t('chatInput.permission.codex.requestApproval'),
      desc: t('chatInput.permission.codex.requestApprovalDesc'),
      approval: 'on-request' as const,
      sandbox: 'workspace-write' as const,
    },
    {
      key: 'codex-never-workspace',
      label: t('chatInput.permission.codex.sandboxAuto'),
      desc: t('chatInput.permission.codex.sandboxAutoDesc'),
      approval: 'never' as const,
      sandbox: 'workspace-write' as const,
    },
    {
      key: 'codex-on-request-readonly',
      label: t('chatInput.permission.codex.readonlySandbox'),
      desc: t('chatInput.permission.codex.readonlySandboxDesc'),
      approval: 'on-request' as const,
      sandbox: 'read-only' as const,
    },
    {
      key: 'codex-on-request-full',
      label: t('chatInput.permission.codex.fullAccess'),
      desc: t('chatInput.permission.codex.fullAccessDesc'),
      approval: 'on-request' as const,
      sandbox: 'danger-full-access' as const,
    },
    {
      key: 'codex-never-full',
      label: t('chatInput.permission.codex.fullAuto'),
      desc: t('chatInput.permission.codex.fullAutoDesc'),
      approval: 'never' as const,
      sandbox: 'danger-full-access' as const,
    },
  ] as const;
}

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
  t: TFn,
): PermissionOption[] {
  if (agentType === 'remote_codex') {
    const approval = settings?.codex_approval_policy ?? 'on-request';
    const sandbox = settings?.codex_sandbox_mode ?? 'workspace-write';
    return codexPermissionModes(t).map((mode) => ({
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

  const modes = agentType === 'remote_roo' ? rooPermissionModes(t) : claudePermissionModes(t);
  if (agentType === 'remote_roo') {
    return modes.map((mode) => ({
      key: mode.value,
      label: mode.label,
      desc: mode.desc,
      active: (settings?.roo_mode ?? 'code') === mode.value,
      updates: { roo_mode: mode.value },
    }));
  }
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

function slashCommands(t: TFn): SlashCommand[] {
  return [
    { name: '/goal', description: t('chatInput.slash.goal'), icon: '◎' },
    { name: '/compact', description: t('chatInput.slash.compact'), icon: '⊞' },
    { name: '/clear', description: t('chatInput.slash.clear'), icon: '⊘' },
    { name: '/plan', description: t('chatInput.slash.plan'), icon: '▶' },
    { name: '/review', description: t('chatInput.slash.review'), icon: '⊡' },
    { name: '/doctor', description: t('chatInput.slash.doctor'), icon: '⊕' },
  ];
}

/** File/folder mention candidates sourced from the active project's cwd.
 *  We read the directory once on demand; in the future this can move to
 *  a tauri fs plugin call. */
function mentionCandidates(): SlashCommand[] {
  // For the MVP, ship a small canonical list. A real impl would `fs.readdir`
  // the active project's cwd on demand and surface real paths.
  return [
    { name: '@README.md', description: 'Project readme', icon: '📄' },
    { name: '@package.json', description: 'Package manifest', icon: '📦' },
    { name: '@Cargo.toml', description: 'Rust manifest', icon: '🦀' },
    { name: '@src/main.rs', description: 'Entry point', icon: '🚪' },
    { name: '@src/lib', description: 'Library module', icon: '📁' },
    { name: '@tests', description: 'Test directory', icon: '🧪' },
  ];
}

/** Skill candidates surfaced by the $ trigger. */
function skillCandidates(t: TFn): SlashCommand[] {
  return [
    { name: '$review', description: t('chatInput.slash.review'), icon: '🔍' },
    { name: '$plan', description: t('chatInput.slash.plan'), icon: '📋' },
    { name: '$doctor', description: t('chatInput.slash.doctor'), icon: '🩺' },
    { name: '$compact', description: t('chatInput.slash.compact'), icon: '📦' },
    { name: '$clear', description: t('chatInput.slash.clear'), icon: '🧹' },
  ];
}

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
  const { t } = useTranslation();
  return (
    <div className="relative">
      <div onClick={onToggle}>{trigger}</div>
      {open && (
        <>
          <button
            aria-label={t('chatInput.closeDropdown')}
            className="fixed inset-0 z-10 cursor-default"
            onClick={onToggle}
          />
          <div className="codex-popover absolute bottom-full left-0 z-20 mb-2 min-w-[260px] animate-fade-in-up">
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
  label: React.ReactNode;
  onClick?: () => void;
  active?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`codex-chip ${
        active
          ? 'border-rc-accent-primary/25 bg-rc-bg-surface text-rc-text-primary shadow-xs'
          : ''
      } focus-visible:outline-none`}
    >
      <Icon size={14} className={active ? 'text-rc-accent-primary' : 'text-rc-text-tertiary'} />
      <span className="max-w-[180px] truncate font-medium">{label}</span>
      <ChevronDown size={12} className="text-rc-text-tertiary" />
    </button>
  );
}

function ComposerPalette({
  kind,
  commands,
  filter,
  onSelect,
  highlightedIndex,
}: {
  kind: 'slash' | 'mention' | 'skill';
  commands: SlashCommand[];
  filter: string;
  onSelect: (cmd: SlashCommand) => void;
  highlightedIndex: number;
}) {
  const { t } = useTranslation();
  const filtered = commands.filter(
    (cmd) =>
      cmd.name.toLowerCase().includes(filter.toLowerCase()) ||
      cmd.description.toLowerCase().includes(filter.toLowerCase()),
  );

  if (filtered.length === 0) return null;

  const labels: Record<typeof kind, string> = {
    slash: t('chatInput.slashCommands'),
    mention: t('chatInput.mentionsTitle'),
    skill: t('chatInput.skillsTitle'),
  };

  return (
    <div
      role="listbox"
      aria-label={labels[kind]}
      data-testid={`composer-palette-${kind}`}
      className="codex-popover absolute bottom-full left-0 right-0 z-20 mb-2 animate-fade-in-up"
    >
      <div className="flex items-center gap-2 px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
        <span>{labels[kind]}</span>
        {filter && (
          <span className="ml-auto rounded bg-rc-bg-tertiary px-1.5 py-0.5 font-mono text-[10px] normal-case text-rc-text-secondary">
            {kind === 'slash' ? '/' : kind === 'mention' ? '@' : '$'}{filter}
          </span>
        )}
      </div>
      <div className="max-h-60 overflow-y-auto pb-1">
        {filtered.map((cmd, index) => (
          <button
            key={cmd.name}
            role="option"
            aria-selected={index === highlightedIndex}
            id={`${kind}-option-${index}`}
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
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [modelDraft, setModelDraft] = useState('');
  const modelDraftRef = useRef(modelDraft);
  const [openMenu, setOpenMenu] = useState<'agent' | 'provider' | 'effort' | 'permission' | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  // The composer supports three palette triggers — / for commands, @ for file
  // mentions, $ for skills. They share the same overlay machinery but each
  // has its own filter and source list.
  const [paletteKind, setPaletteKind] = useState<'slash' | 'mention' | 'skill' | null>(null);
  const [paletteFilter, setPaletteFilter] = useState('');
  const [highlightedPaletteIndex, setHighlightedPaletteIndex] = useState(0);
  // Legacy alias kept for readability of existing code paths.
  const showSlashPalette = paletteKind === 'slash';
  const slashFilter = paletteFilter;
  const [modelError, setModelError] = useState<string | null>(null);
  const [pendingAttachments, setPendingAttachments] = useState<
    Array<{ mediaType: string; data: string; preview: string }>
  >([]);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const lastCommittedModel = useRef('');

  const sending = useAppStore((state) => state.sending);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessions = useAppStore((state) => state.sessions);
  const conversation = useAppStore((state) => state.conversation);
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
  const pendingChatAttachment = useAppStore((state) => state.pendingChatAttachment);
  const consumeChatAttachment = useAppStore((state) => state.consumeChatAttachment);

  // Consume file path injected from FileExplorer "add to chat"
  useEffect(() => {
    if (!pendingChatAttachment) return;
    const attachment = consumeChatAttachment();
    if (attachment) {
      setInput((prev) => prev ? `${prev}\n${attachment}` : attachment);
      textAreaRef.current?.focus();
    }
  }, [pendingChatAttachment, consumeChatAttachment]);

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
  const lockedAgentType = conversation.length > 0 ? (activeSession?.agent_type ?? null) : null;
  const effectiveAgentType = lockedAgentType ?? activeAgentType ?? 'remote_claude';
  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;

  const permissionOptions = useMemo(
    () => permissionOptionsForAgent(effectiveAgentType, settings ?? null, t),
    [effectiveAgentType, settings, t],
  );
  const permissionLabel = permissionOptions.find((mode) => mode.active)?.label ?? permissionOptions[0]?.label ?? t('chatInput.permissionLabel');

  const activeProviderName = providerConfigs?.active_provider ?? provider?.name ?? t('chatInput.unconfiguredProvider');
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
        t('chatInput.contextError', { used: formatTokenCount(usedTokens), model, next: formatTokenCount(nextWindow) }),
      );
      return false;
    }
    setModelError(null);
    return true;
  };

  const handlePaste = useCallback(async (e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;

    for (const item of items) {
      if (item.type.startsWith('image/')) {
        e.preventDefault();
        const blob = item.getAsFile();
        if (!blob) continue;

        const mediaType = item.type;
        const reader = new FileReader();
        reader.onload = () => {
          const dataUrl = reader.result as string;
          // dataUrl is "data:image/png;base64,XXXXX" — extract the base64 part.
          const base64 = dataUrl.split(',')[1];
          if (!base64) return;
          setPendingAttachments((prev) => [
            ...prev,
            { mediaType, data: base64, preview: dataUrl },
          ]);
        };
        reader.readAsDataURL(blob);
        return; // Only process the first image.
      }
    }
  }, []);

  const handleSend = async () => {
    if ((!input.trim() && pendingAttachments.length === 0) || sending) return;
    // Flush any pending model change before sending to avoid race condition.
    await commitModelDraft();
    const current = input;
    const attachments: tauri.AttachmentInput[] | undefined =
      pendingAttachments.length > 0
        ? pendingAttachments.map((a) => ({
            media_type: a.mediaType,
            data: a.data,
          }))
        : undefined;
    setInput('');
    setPendingAttachments([]);
    setPaletteKind(null);
    await sendMessage(current, attachments);
  };

  const handleCancel = async () => {
    if (!activeSessionId) return;
    await cancelPrompt(activeSessionId);
  };

  const handleKeyDown = async (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (paletteKind) {
      const sourceCommands =
        paletteKind === 'slash'
          ? slashCommands(t)
          : paletteKind === 'mention'
            ? mentionCandidates()
            : skillCandidates(t);
      const filteredCommands = sourceCommands.filter(
        (cmd) =>
          cmd.name.toLowerCase().includes(paletteFilter.toLowerCase()) ||
          cmd.description.toLowerCase().includes(paletteFilter.toLowerCase()),
      );

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setHighlightedPaletteIndex((prev) =>
          prev < filteredCommands.length - 1 ? prev + 1 : 0,
        );
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        setHighlightedPaletteIndex((prev) =>
          prev > 0 ? prev - 1 : filteredCommands.length - 1,
        );
        return;
      }
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        if (filteredCommands[highlightedPaletteIndex]) {
          handlePaletteSelect(paletteKind, filteredCommands[highlightedPaletteIndex].name);
        }
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        setPaletteKind(null);
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

    // Detect any of the three palette triggers ( / / @ / $ ) at the start of
    // the buffer or directly after a space, before any other text.
    // The trigger must occupy the end of the buffer to count as "active".
    const match = /(?:^|\s)([\/@\$])([\w-]*)$/.exec(value);
    if (match) {
      const trigger = match[1] as '/' | '@' | '$';
      const filter = match[2];
      const kind = trigger === '/' ? 'slash' : trigger === '@' ? 'mention' : 'skill';
      setPaletteKind(kind);
      setPaletteFilter(filter);
      setHighlightedPaletteIndex(0);
    } else {
      setPaletteKind(null);
    }
  };

  const handlePaletteSelect = (kind: 'slash' | 'mention' | 'skill', token: string) => {
    // Insert the chosen token at the current caret position, replacing the
    // trigger expression. We append a trailing space so the user can keep
    // typing without the palette re-opening on every keystroke.
    setInput((prev) => {
      const triggerIndex = prev.search(/[\/@\$][\w-]*$/);
      if (triggerIndex === -1) return `${prev}${token} `;
      return `${prev.slice(0, triggerIndex)}${token} `;
    });
    setPaletteKind(null);
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
    <div role="form" aria-label="Prompt composer" className="bg-transparent px-6 pb-7 pt-3">
      <div className="mx-auto w-full max-w-input">
        <div
          className="codex-composer"
        >
          {paletteKind && (
            <ComposerPalette
              kind={paletteKind}
              commands={
                paletteKind === 'slash'
                  ? slashCommands(t)
                  : paletteKind === 'mention'
                    ? mentionCandidates()
                    : skillCandidates(t)
              }
              filter={paletteFilter}
              onSelect={(cmd) => handlePaletteSelect(paletteKind, cmd.name)}
              highlightedIndex={highlightedPaletteIndex}
            />
          )}

          <div className="px-6 pb-2 pt-5">
            <textarea
              ref={textAreaRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={(event) => {
                void handleKeyDown(event);
              }}
              disabled={sending}
              onPaste={handlePaste}
              rows={1}
              aria-label="Prompt input"
              aria-activedescendant={paletteKind ? `${paletteKind}-option-${highlightedPaletteIndex}` : undefined}
              placeholder={t('chatInput.placeholder')}
              className="min-h-[82px] max-h-[200px] w-full resize-none border-0 bg-transparent px-1 py-1 text-[15px] leading-7 text-rc-text-primary outline-none placeholder:text-rc-text-tertiary disabled:cursor-not-allowed focus-visible:outline-none"
            />

            {pendingAttachments.length > 0 && (
              <div className="flex flex-wrap gap-2 px-1 pt-1">
                {pendingAttachments.map((att, idx) => (
                  <div
                    key={idx}
                    className="group relative h-16 w-16 overflow-hidden rounded-2xl border border-rc-border-secondary bg-rc-bg-elevated shadow-xs"
                  >
                    <img
                      src={att.preview}
                      alt="Pasted"
                      className="h-full w-full object-cover"
                    />
                    <button
                      type="button"
                      onClick={() =>
                        setPendingAttachments((prev) => prev.filter((_, i) => i !== idx))
                      }
                      className="absolute right-0.5 top-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-rc-bg-elevated/80 text-rc-text-tertiary opacity-0 transition-opacity group-hover:opacity-100"
                    >
                      <X size={10} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div role="group" aria-label="Composer controls" className="border-t border-rc-border-secondary/45 bg-transparent px-4 py-3">
            <div className="flex min-h-10 items-center gap-2">
              {/* ── Left: 4 quick-action icon buttons (Codex-style round icons) ── */}
              <button
                type="button"
                data-testid="composer-attach"
                className="flex h-8 w-8 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline-none"
                title={t('chatInput.appshotDesc')}
                aria-label={t('chatInput.appshot')}
                onClick={() => textAreaRef.current?.focus()}
              >
                <Paperclip size={15} />
              </button>

              <button
                type="button"
                data-testid="composer-mention"
                className="flex h-8 w-8 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline-none"
                title={t('chatInput.mentionsTitle')}
                aria-label="Mention file (@)"
                onClick={() => {
                  setInput((prev) => (prev.trim() ? `${prev} @` : '@'));
                  setPaletteKind('mention');
                  setPaletteFilter('');
                  setHighlightedPaletteIndex(0);
                  textAreaRef.current?.focus();
                }}
              >
                <AtSign size={15} />
              </button>

              <button
                type="button"
                data-testid="composer-edit"
                className="flex h-8 w-8 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline-none"
                title={t('chatInput.composerSettings')}
                aria-label="Composer settings"
                onClick={() => {
                  setAdvancedOpen((value) => !value);
                  setOpenMenu(null);
                }}
              >
                <Pencil size={15} />
              </button>

              <button
                type="button"
                data-testid="composer-slash"
                className="flex h-8 w-8 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline-none"
                title={t('chatInput.slashCommands')}
                aria-label={t('chatInput.slashCommands')}
                onClick={() => {
                  setInput((prev) => (prev.trim() ? `${prev} /` : '/'));
                  setPaletteKind('slash');
                  setPaletteFilter('');
                  setHighlightedPaletteIndex(0);
                  textAreaRef.current?.focus();
                }}
              >
                <Wand2 size={15} />
              </button>

              <button
                type="button"
                data-testid="composer-skill"
                className="flex h-8 w-8 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline-none"
                title={t('chatInput.skillsTitle')}
                aria-label={t('chatInput.skillsTitle')}
                onClick={() => {
                  setInput((prev) => (prev.trim() ? `${prev} $` : '$'));
                  setPaletteKind('skill');
                  setPaletteFilter('');
                  setHighlightedPaletteIndex(0);
                  textAreaRef.current?.focus();
                }}
              >
                <Sparkles size={15} />
              </button>

              {/* Advanced (legacy) — kept for tests/transition; opens the
                  legacy advanced pane with Codex approval & sandbox modes. */}
              <button
                type="button"
                aria-expanded={advancedOpen}
                aria-label={t('chatInput.composerSettings')}
                data-testid="composer-advanced-toggle"
                onClick={() => {
                  setAdvancedOpen((value) => !value);
                  setOpenMenu(null);
                }}
                className="hidden h-7 items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 text-[12px] text-rc-text-secondary transition-colors hover:bg-rc-bg-hover sm:inline-flex"
              >
                <span className="font-medium">…</span>
              </button>

              <div className="flex-1" />

              {sending && activeSessionId ? (
                <button
                  type="button"
                  aria-label={t('chatInput.interrupt')}
                  title={t('chatInput.interrupt')}
                  onClick={() => {
                    void handleCancel();
                  }}
                  className="flex h-8 w-8 items-center justify-center rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg text-rc-accent-warning transition-colors hover:bg-rc-bg-hover focus-visible:outline-none"
                >
                  <Square size={15} />
                </button>
              ) : (
                <button
                  type="button"
                  aria-label={t('chatInput.send')}
                  onClick={() => {
                    void handleSend();
                  }}
                  disabled={sending || (!input.trim() && pendingAttachments.length === 0)}
                  className="flex h-8 w-8 items-center justify-center rounded-md bg-rc-text-primary text-rc-text-inverse transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:opacity-45 focus-visible:outline-none"
                >
                  <Send size={16} />
                </button>
              )}
            </div>

            {/* ── Bottom chip strip: agent / model / effort / permission / branch ── */}
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              {/* Agent chip */}
              <Dropdown
                open={openMenu === 'agent'}
                onToggle={() => setOpenMenu((state) => (state === 'agent' ? null : 'agent'))}
                trigger={
                  <Chip
                    icon={Sparkles}
                    label={
                      <span className="flex items-center gap-1.5">
                        <span className="font-medium">
                          {availableAgents.find((a) => a.agentType === effectiveAgentType)?.displayName ?? 'Claude CLI'}
                        </span>
                        {activeSession?.agent_type && activeSession.agent_type !== effectiveAgentType && (
                          <span className="rounded bg-rc-accent-warning-bg px-1 text-[10px] text-rc-accent-warning">
                            {t('common.locked')}
                          </span>
                        )}
                      </span>
                    }
                    active={openMenu === 'agent'}
                  />
                }
              >
                <AgentSelector
                  availableAgents={availableAgents}
                  activeAgentType={effectiveAgentType}
                  lockedAgentType={lockedAgentType}
                  lockedReason={t('chatInput.lockReason')}
                  defaultOpen
                  onSelect={(agentType) => {
                    if (lockedAgentType && agentType !== lockedAgentType) return;
                    selectAgent(agentType);
                    if (agentType && activeSessionId && conversation.length === 0) {
                      void tauri.updateSessionAgent(activeSessionId, agentType);
                    }
                    setOpenMenu(null);
                  }}
                />
              </Dropdown>

              {/* Provider chip */}
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
                        providerOption.model ?? t('chatInput.defaultModel'),
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
                    title={provider?.name ?? t('chatInput.noProvider')}
                    subtitle={t('chatInput.addProvider')}
                    onClick={() => setOpenMenu(null)}
                  />
                )}
              </Dropdown>

              {/* Model chip — always visible so the model input is accessible */}
              <div className="codex-chip min-w-0" data-testid="composer-model-chip">
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
                  data-testid="composer-model-input"
                  className="w-32 min-w-0 border-0 bg-transparent text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-0 focus:outline-none focus:ring-0 focus-visible:outline-none"
                  style={{ outline: 'none' }}
                  placeholder={t('chatInput.selectModel')}
                  title={t('chatInput.modelTitle')}
                />
              </div>

              {/* Effort chip (Codex-style Max / L / M) */}
              <Dropdown
                open={openMenu === 'effort'}
                onToggle={() => setOpenMenu((state) => (state === 'effort' ? null : 'effort'))}
                trigger={
                  <Chip
                    icon={Zap}
                    label={t('chatInput.effort')}
                    active={openMenu === 'effort'}
                  />
                }
              >
                <DropdownItem
                  title="Max"
                  subtitle={t('chatInput.effortMax')}
                  onClick={() => setOpenMenu(null)}
                />
                <DropdownItem
                  title="L"
                  subtitle={t('chatInput.effortL')}
                  onClick={() => setOpenMenu(null)}
                />
                <DropdownItem
                  title="M"
                  subtitle={t('chatInput.effortM')}
                  onClick={() => setOpenMenu(null)}
                />
                <DropdownItem
                  title="S"
                  subtitle={t('chatInput.effortS')}
                  onClick={() => setOpenMenu(null)}
                />
              </Dropdown>

              {/* Permission chip */}
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

              {/* Branch chip — placeholder (read-only decoration in MVP) */}
              <span
                aria-label="Git branch"
                title="Git branch"
                data-testid="composer-branch"
                className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-1 text-[12px] text-rc-text-secondary"
              >
                <GitBranch size={12} className="text-rc-text-tertiary" />
                <span className="font-medium">main</span>
              </span>
            </div>

            {advancedOpen && (
              <div className="mt-3 grid gap-2 rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-2.5 animate-fade-in-up">
                <div
                  aria-label="Codex execution modes"
                  className="inline-flex items-center rounded-md border border-rc-border-primary bg-rc-bg-surface p-0.5 text-[12px] text-rc-text-tertiary"
                >
                  <span className="rounded-sm bg-rc-bg-active px-2.5 py-1 font-medium text-rc-text-primary">
                    {t('chatInput.localMode')}
                  </span>
                  <span className="px-2.5 py-1">{t('chatInput.worktreeMode')}</span>
                  <span className="px-2.5 py-1">{t('chatInput.cloudMode')}</span>
                </div>
              </div>
            )}
          </div>

          {modelError && (
            <div className="border-t border-rc-border-secondary bg-rc-accent-warning-bg px-4 py-2 text-xs text-rc-accent-warning">
              {modelError}
            </div>
          )}
        </div>

        {showSlashPalette && input.startsWith('/') && (
          <div className="mt-1 text-[10px] text-rc-text-tertiary">
            {t('chatInput.slashHint')}
          </div>
        )}
      </div>
    </div>
  );
}
