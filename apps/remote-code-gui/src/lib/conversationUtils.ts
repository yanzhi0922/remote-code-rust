import { truncateMiddle } from './utils';
import type { ConversationEntry, ToolCallInfo } from './types';

export function formatToolInput(input: unknown): string {
  try {
    const normalized = typeof input === 'string' ? JSON.parse(input) : input;
    return JSON.stringify(normalized, null, 2);
  } catch {
    return typeof input === 'string' ? input : JSON.stringify(input, null, 2);
  }
}

export function summarizeToolInput(toolCall: ToolCallInfo): string {
  try {
    const normalized =
      typeof toolCall.input === 'string' ? JSON.parse(toolCall.input) : toolCall.input;
    if (normalized && typeof normalized === 'object') {
      const objectValue = normalized as Record<string, unknown>;
      const preview =
        objectValue.path ??
        objectValue.file_path ??
        objectValue.command ??
        objectValue.query ??
        objectValue.prompt ??
        Object.values(objectValue)[0];
      if (typeof preview === 'string') {
        return truncateMiddle(preview, 84);
      }
    }
  } catch {
    // Ignore summary parsing failures.
  }
  return toolCall.name;
}

export function extractThinkingBlocks(entry: ConversationEntry): string[] {
  return entry.content_blocks
    .filter((block): block is Record<string, unknown> => !!block && typeof block === 'object')
    .filter((block) => block.type === 'thinking' && typeof block.thinking === 'string')
    .map((block) => block.thinking as string);
}

export function estimateEntryHeight(entry: ConversationEntry): number {
  switch (entry.role) {
    case 'assistant':
      return 320;
    case 'tool':
      return 180;
    case 'user':
      return 120;
    default:
      return 64;
  }
}