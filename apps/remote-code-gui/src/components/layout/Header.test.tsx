import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { Header } from './Header';
import { useAppStore } from '../../stores/useAppStore';

const resetStore = () => {
  useAppStore.setState({
    provider: null,
    runtimeStatus: null,
    settings: null,
    sessions: [],
    archivedSessions: [],
    activeSessionId: null,
    projects: [],
    activeProjectPath: null,
    lastPromptResult: null,
    contextUsageBySession: {},
    contextOverflowBySession: {},
    contextCompactionBySession: {},
  });
};

describe('Header', () => {
  beforeEach(() => {
    resetStore();
  });

  afterEach(() => {
    cleanup();
    resetStore();
  });

  it('renders runtime status badges for fallback model and setting sources', () => {
    useAppStore.setState({
      runtimeStatus: {
        session_name: 'Parity',
        provider: {
          name: 'glm-coding',
          model: 'glm-5.1',
          protocol: 'anthropic',
          base_url: 'https://open.bigmodel.cn/api/anthropic',
          auth_source: 'settings:runtime.toml',
          effort: 'medium',
          fallback_model: 'glm-5-turbo',
        },
        permission_mode: 'acceptEdits',
        setting_sources: ['settings:runtime.toml', 'cli:model', 'env:GLM_API_KEY'],
        allowed_setting_sources: ['user', 'project'],
        allowed_tools: ['read_file'],
        disallowed_tools: ['bash_command'],
      },
      sessions: [
        {
          id: 'session-1',
          title: 'GUI parity',
          cwd: 'C:\\repo',
          provider_name: 'glm-coding',
          model: 'glm-5.1',
          created_at: '2026-04-13T00:00:00Z',
          updated_at: '2026-04-13T00:00:00Z',
          archived: false,
        },
      ],
      activeSessionId: 'session-1',
    });

    render(<Header />);

    expect(screen.getByText('fallback glm-5-turbo')).toBeInTheDocument();
    expect(screen.getByText('settings 3')).toBeInTheDocument();
    expect(screen.getByText('scope user/project')).toBeInTheDocument();
    expect(screen.getByText('tools +1 / -1')).toBeInTheDocument();
    expect(screen.getByText('acceptEdits')).toBeInTheDocument();
  });
});
