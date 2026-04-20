import { describe, expect, it } from 'vitest';
import {
  buildLocalSessionBundle,
  normalizeLocalConversationEntry,
  normalizeLocalSessionSummary,
} from './fromLocal';
import type { ConversationEntry, SessionSubtask, SessionSummary } from '../../lib/types';

describe('fromLocal', () => {
  it('normalizes local session summaries', () => {
    const summary: SessionSummary = {
      id: 'session-local',
      title: 'Workspace Chat',
      cwd: 'C:\\repo',
      provider_name: 'minimax',
      model: 'minimax-m2.7',
      created_at: '2026-04-14T09:00:00Z',
      updated_at: '2026-04-14T09:05:00Z',
      archived: false,
    };

    const vm = normalizeLocalSessionSummary(summary);
    expect(vm.title).toBe('Workspace Chat');
    expect(vm.workspaceLabel).toBe('C:\\repo');
    expect(vm.providerName).toBe('minimax');
  });

  it('extracts thinking blocks and tool call summaries from local conversation entries', () => {
    const entry: ConversationEntry = {
      role: 'assistant',
      text: 'Done.',
      content_blocks: [{ type: 'thinking', thinking: 'Need to inspect file first.' }],
      tool_calls: [
        {
          id: 'tool-1',
          name: 'read_file',
          input: { file_path: 'src/main.ts' },
        },
      ],
      tool_call_id: null,
      name: null,
      is_error: false,
    };

    const vm = normalizeLocalConversationEntry(entry, 1, 'session-local');
    expect(vm.thinkingBlocks).toEqual(['Need to inspect file first.']);
    expect(vm.toolCalls[0].summary).toBe('src/main.ts');
  });

  it('builds a local bundle from conversation, live tool state, permission, and tasks', () => {
    const summary: SessionSummary = {
      id: 'session-local',
      title: 'Workspace Chat',
      cwd: 'C:\\repo',
      provider_name: 'glm',
      model: 'glm-5.1',
      created_at: '2026-04-14T09:00:00Z',
      updated_at: '2026-04-14T09:05:00Z',
      archived: false,
    };
    const tasks: SessionSubtask[] = [
      {
        session_id: 'session-local',
        task_id: 'task-1',
        parent_task_id: null,
        description: 'Investigate issue',
        depth: 0,
        status: 'running',
        summary: 'Reading files',
        output_preview: null,
        turns_used: null,
        kind: 'delegation',
        output_path: null,
        created_at: '2026-04-14T09:01:00Z',
        updated_at: '2026-04-14T09:02:00Z',
      },
    ];

    const bundle = buildLocalSessionBundle({
      session: summary,
      conversation: [
        {
          role: 'user',
          text: 'Please inspect the bug.',
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
      ],
      liveToolProgress: [
        {
          tool_call_id: 'tool-1',
          tool_name: 'read_file',
          message: 'Reading src/main.ts',
        },
      ],
      liveToolResults: [
        {
          tool_call_id: 'tool-1',
          tool_name: 'read_file',
          is_error: false,
          output: 'contents',
        },
      ],
      pendingPermission: {
        request_id: 'approval-1',
        tool_name: 'shell',
        tool_use_id: 'tool-2',
        title: 'Approve shell command',
        description: 'Run git status',
        input: {},
        blocked_path: 'C:\\repo',
        permission_suggestions: [{ action: 'allow', toolPattern: 'shell' }],
      },
      tasks,
    });

    expect(bundle.session?.title).toBe('Workspace Chat');
    expect(bundle.timeline.some((item) => item.kind === 'tool')).toBe(true);
    expect(bundle.approvals[0].title).toBe('Approve shell command');
    expect(bundle.approvals[0].metadata.permission_suggestions_count).toBe('1');
    expect(bundle.taskTree[0].description).toBe('Investigate issue');
    expect(bundle.connection.state).toBe('local');
  });
});
