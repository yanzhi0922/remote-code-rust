import { describe, expect, it } from 'vitest';
import { parseAgentFileName, formatFileSize, isAgentFile } from './agentFileUtils';

describe('agentFileUtils', () => {
  it('parses agent file name', () => {
    expect(parseAgentFileName('/agents/my-agent.md')).toBe('my-agent');
    expect(parseAgentFileName('test.yaml')).toBe('test');
  });

  it('formats file sizes', () => {
    expect(formatFileSize(500)).toBe('500B');
    expect(formatFileSize(1500)).toBe('1.5KB');
    expect(formatFileSize(1500000)).toBe('1.4MB');
  });

  it('detects agent files', () => {
    expect(isAgentFile('agent.md')).toBe(true);
    expect(isAgentFile('agent.yaml')).toBe(true);
    expect(isAgentFile('agent.json')).toBe(true);
    expect(isAgentFile('agent.txt')).toBe(false);
  });
});
