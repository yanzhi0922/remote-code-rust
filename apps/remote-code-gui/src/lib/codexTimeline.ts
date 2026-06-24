import type { ConversationEntry, ToolCallInfo, ToolProgressInfo, ToolResultInfo } from './types';
import { truncateMiddle } from './utils';

export type CodexTimelineKind =
  | 'command'
  | 'file'
  | 'mcp'
  | 'dynamic'
  | 'collab'
  | 'web'
  | 'image'
  | 'plan'
  | 'reasoning'
  | 'context'
  | 'generic';

export type CodexTimelineStatus = 'pending' | 'running' | 'success' | 'error' | 'info';

export interface CodexTimelineDescriptor {
  kind: CodexTimelineKind;
  status: CodexTimelineStatus;
  title: string;
  subtitle: string;
  detail: string;
  command?: string | null;
  cwd?: string | null;
  path?: string | null;
  server?: string | null;
  durationMs?: number | null;
  exitCode?: number | null;
  diff?: {
    added: number;
    removed: number;
    files: string[];
  } | null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

export function parseMaybeJson(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  const trimmed = value.trim();
  if (!trimmed) return value;
  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
}

function stringField(record: Record<string, unknown> | null, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = record?.[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
}

function numberField(record: Record<string, unknown> | null, ...keys: string[]): number | null {
  for (const key of keys) {
    const value = record?.[key];
    if (typeof value === 'number' && Number.isFinite(value)) return value;
  }
  return null;
}

function compactJson(value: unknown, max = 120): string {
  if (typeof value === 'string') return truncateMiddle(value.replace(/\s+/g, ' ').trim(), max);
  try {
    return truncateMiddle(JSON.stringify(value), max);
  } catch {
    return truncateMiddle(String(value), max);
  }
}

export function formatTimelineDetail(value: unknown): string {
  if (typeof value === 'string') {
    const parsed = parseMaybeJson(value);
    if (parsed !== value) return JSON.stringify(parsed, null, 2);
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function getDiffStats(text: string): CodexTimelineDescriptor['diff'] {
  if (!/^--- a\/.*\n\+\+\+ b\//m.test(text) && !/^diff --git /m.test(text)) return null;
  const files = Array.from(text.matchAll(/(?:--- a\/|diff --git a\/)([^\s\n]+)/g))
    .map((match) => match[1])
    .filter(Boolean);
  const added = text.split('\n').filter((line) => line.startsWith('+') && !line.startsWith('+++')).length;
  const removed = text.split('\n').filter((line) => line.startsWith('-') && !line.startsWith('---')).length;
  return { added, removed, files: Array.from(new Set(files)).slice(0, 8) };
}

export function inferTimelineKind(name: string, inputOrOutput: unknown): CodexTimelineKind {
  const lower = name.toLowerCase();
  const parsed = parseMaybeJson(inputOrOutput);
  const record = asRecord(parsed);
  const text = typeof inputOrOutput === 'string' ? inputOrOutput : compactJson(inputOrOutput, 500).toLowerCase();

  if (lower.includes('shell') || lower.includes('bash') || lower.includes('exec') || stringField(record, 'command', 'cmd')) return 'command';
  if (lower.includes('mcp') || stringField(record, 'server', 'serverName')) return 'mcp';
  if (lower.includes('agent') || lower.includes('subagent') || lower.includes('delegate')) return 'collab';
  if (lower.includes('web') || lower.includes('search') || stringField(record, 'query')) return 'web';
  if (lower.includes('image') || lower.includes('screenshot') || stringField(record, 'path')?.match(/\.(png|jpe?g|gif|webp)$/i)) return 'image';
  if (lower.includes('plan') || stringField(record, 'plan')) return 'plan';
  if (lower.includes('compact') || lower.includes('context')) return 'context';
  if (
    lower.includes('file') ||
    lower.includes('patch') ||
    lower.includes('edit') ||
    lower.includes('write') ||
    lower.includes('read') ||
    stringField(record, 'path', 'file_path', 'filePath') ||
    /^--- a\/.*\n\+\+\+ b\//m.test(text)
  ) {
    return 'file';
  }
  if (lower.includes('dynamic') || lower.includes('tool_search')) return 'dynamic';
  return 'generic';
}

export function describeToolCall(toolCall: ToolCallInfo): CodexTimelineDescriptor {
  const parsed = parseMaybeJson(toolCall.input);
  const record = asRecord(parsed);
  const kind = inferTimelineKind(toolCall.name, parsed);
  const command = stringField(record, 'command', 'cmd', 'active_form');
  const path = stringField(record, 'path', 'file_path', 'filePath', 'cwd');
  const server = stringField(record, 'server', 'serverName', 'namespace');
  const cwd = stringField(record, 'cwd', 'workingDirectory');
  const detail = formatTimelineDetail(parsed);
  const diff = getDiffStats(detail);
  const primary =
    command ??
    path ??
    stringField(record, 'query', 'prompt', 'tool', 'name') ??
    compactJson(parsed);

  return {
    kind,
    status: 'pending',
    title: toolCall.name,
    subtitle: truncateMiddle(primary, 100),
    detail,
    command,
    cwd,
    path,
    server,
    durationMs: numberField(record, 'durationMs', 'duration_ms'),
    exitCode: numberField(record, 'exitCode', 'exit_code'),
    diff,
  };
}

export function describeToolResult(entry: Pick<ConversationEntry, 'name' | 'text' | 'is_error'>): CodexTimelineDescriptor {
  const name = entry.name ?? 'tool';
  const parsed = parseMaybeJson(entry.text);
  const record = asRecord(parsed);
  const detail = formatTimelineDetail(parsed);
  const diff = getDiffStats(detail);
  const kind = inferTimelineKind(name, parsed);
  const command = stringField(record, 'command', 'cmd');
  const path = stringField(record, 'path', 'file_path', 'filePath');
  const server = stringField(record, 'server', 'serverName', 'namespace');
  const exitCode = numberField(record, 'exitCode', 'exit_code');
  const durationMs = numberField(record, 'durationMs', 'duration_ms');
  const status: CodexTimelineStatus = entry.is_error || exitCode !== null && exitCode !== 0 ? 'error' : 'success';

  return {
    kind,
    status,
    title: name,
    subtitle: truncateMiddle(
      stringField(record, 'summary', 'message', 'output_preview') ?? (detail.replace(/\s+/g, ' ').trim() || 'Completed'),
      120,
    ),
    detail,
    command,
    cwd: stringField(record, 'cwd', 'workingDirectory'),
    path,
    server,
    durationMs,
    exitCode,
    diff,
  };
}

export function describeLiveProgress(progress: ToolProgressInfo): CodexTimelineDescriptor {
  const detail = progress.active_form ?? progress.message;
  return {
    kind: inferTimelineKind(progress.tool_name, detail),
    status: 'running',
    title: progress.tool_name || 'tool',
    subtitle: truncateMiddle(detail, 110),
    detail,
    command: progress.active_form ?? null,
    cwd: null,
    path: null,
    server: null,
    durationMs: null,
    exitCode: null,
    diff: null,
  };
}

export function describeLiveResult(result: ToolResultInfo): CodexTimelineDescriptor {
  return describeToolResult({
    name: result.tool_name,
    text: result.output,
    is_error: result.is_error,
  });
}

export function collectCodexSurfaceStats(conversation: ConversationEntry[]) {
  const stats: Record<CodexTimelineKind, number> = {
    command: 0,
    file: 0,
    mcp: 0,
    dynamic: 0,
    collab: 0,
    web: 0,
    image: 0,
    plan: 0,
    reasoning: 0,
    context: 0,
    generic: 0,
  };

  for (const entry of conversation) {
    if (entry.role === 'assistant') {
      for (const toolCall of entry.tool_calls ?? []) {
        stats[inferTimelineKind(toolCall.name, toolCall.input)] += 1;
      }
      if ((entry.content_blocks ?? []).some((block) => asRecord(block)?.type === 'thinking')) stats.reasoning += 1;
    }
    if (entry.role === 'tool') {
      stats[inferTimelineKind(entry.name ?? 'tool', entry.text)] += 1;
    }
  }

  return stats;
}
